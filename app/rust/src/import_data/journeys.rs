use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};

use crate::api::import::{ImportPreprocessor, JourneyInfo};
use crate::gps_processor::GpsPostprocessor;
use crate::gps_processor::RawData;
use crate::import_data::{
    conversion, csv, gpx,
    journey_partition::{self, SegmentSlice},
    kml,
};
use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyHeader, JourneyKind, JourneyType};
use crate::storage::Storage;

/// One explicitly marked MemoLanes journey in a GPX/KML interchange file.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedJourney {
    pub source_journey_id: Option<String>,
    pub source_revision: Option<String>,
    pub journey_date: Option<NaiveDate>,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub segments: Vec<Vec<RawData>>,
}

impl ImportedJourney {
    pub fn generic(segments: Vec<Vec<RawData>>) -> Self {
        Self {
            source_journey_id: None,
            source_revision: None,
            journey_date: None,
            start: None,
            end: None,
            segments,
        }
    }

    pub fn has_memolanes_metadata(&self) -> bool {
        self.source_journey_id.is_some() && self.journey_date.is_some()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParsedVectorData {
    /// Groups appear in the same order as the source file. A group without
    /// MemoLanes metadata represents ordinary third-party track data.
    pub groups: Vec<ImportedJourney>,
}

impl ParsedVectorData {
    pub fn flatten(&self) -> Vec<Vec<RawData>> {
        self.groups
            .iter()
            .flat_map(|group| group.segments.iter().cloned())
            .collect()
    }

    pub fn has_memolanes_metadata(&self) -> bool {
        self.groups
            .iter()
            .any(ImportedJourney::has_memolanes_metadata)
    }

    pub(crate) fn build_parts(&self) -> Vec<ImportPart> {
        if self.has_memolanes_metadata() {
            self.groups
                .iter()
                .enumerate()
                .filter_map(|(index, group)| {
                    let date = group.journey_date?;
                    Some(ImportPart {
                        key: format!("journey:{index}"),
                        date,
                        start: group.start,
                        end: group.end,
                        point_count: group.segments.iter().map(Vec::len).sum::<usize>() as u64,
                        missing_timestamp_count: group
                            .segments
                            .iter()
                            .flatten()
                            .filter(|point| point.timestamp_ms.is_none())
                            .count() as u64,
                        source: ImportPartSource::Journey(index),
                    })
                })
                .collect()
        } else {
            let partition = journey_partition::partition_generic_groups(&self.groups);
            partition
                .index
                .into_iter()
                .map(|(date, slices)| {
                    let summary = partition.summaries.get(&date).expect("summary exists");
                    ImportPart {
                        key: date.to_string(),
                        date,
                        start: summary.start_time,
                        end: summary.end_time,
                        point_count: summary.point_count,
                        missing_timestamp_count: summary.missing_timestamp_count,
                        source: ImportPartSource::Date(slices),
                    }
                })
                .collect()
        }
    }

    pub(crate) fn materialize_part(&self, part: &ImportPart) -> Vec<Vec<RawData>> {
        match &part.source {
            ImportPartSource::Journey(index) => self.groups[*index].segments.clone(),
            ImportPartSource::Date(slices) => {
                journey_partition::materialize_group_partition(&self.groups, slices)
            }
        }
    }
}

pub(crate) fn load_vector_file(
    file_path: &str,
) -> Result<(JourneyInfo, ParsedVectorData, ImportPreprocessor)> {
    let (parsed, preprocessor) = match Path::new(file_path)
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("gpx") => gpx::load_gpx(file_path)?,
        Some("kml") => kml::load_kml(file_path)?,
        Some("csv") => {
            let (data, preprocessor) = csv::load_csv(file_path)?;
            (
                ParsedVectorData {
                    groups: vec![ImportedJourney::generic(data)],
                },
                preprocessor,
            )
        }
        extension => bail!("Unknown extension: {extension:?}"),
    };

    let journey_info = parsed
        .groups
        .iter()
        .find(|group| group.has_memolanes_metadata())
        .and_then(|group| {
            group.journey_date.map(|journey_date| JourneyInfo {
                journey_date,
                start_time: group.start,
                end_time: group.end,
                note: None,
                journey_kind: JourneyKind::DefaultKind,
            })
        })
        .unwrap_or_else(|| conversion::journey_info_from_raw_vector_data(&parsed.flatten()));
    Ok((journey_info, parsed, preprocessor))
}

pub(crate) struct ImportPart {
    pub key: String,
    pub date: NaiveDate,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub point_count: u64,
    pub missing_timestamp_count: u64,
    source: ImportPartSource,
}

enum ImportPartSource {
    Journey(usize),
    Date(Vec<SegmentSlice>),
}

impl ImportPart {
    pub(crate) fn is_memolanes_journey(&self) -> bool {
        matches!(self.source, ImportPartSource::Journey(_))
    }

    fn source_version<'a>(&self, parsed: &'a ParsedVectorData) -> Option<(&'a str, &'a str)> {
        let ImportPartSource::Journey(index) = self.source else {
            return None;
        };
        let group = &parsed.groups[index];
        Some((
            group.source_journey_id.as_deref()?,
            group.source_revision.as_deref()?,
        ))
    }
}

pub(crate) fn part_for_key<'a>(parts: &'a [ImportPart], key: &str) -> Result<&'a ImportPart> {
    parts
        .iter()
        .find(|part| part.key == key)
        .with_context(|| format!("No vector data for part {key}"))
}

pub(crate) fn select_parts(parts: &[ImportPart], keys: Vec<String>) -> Result<Vec<&ImportPart>> {
    let selected_keys = keys.into_iter().collect::<HashSet<_>>();
    let mut missing_keys = selected_keys
        .iter()
        .filter(|key| !parts.iter().any(|part| &part.key == *key))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_keys.is_empty() {
        missing_keys.sort_unstable();
        bail!("No vector data for parts: {missing_keys:?}");
    }
    Ok(parts
        .iter()
        .filter(|part| selected_keys.contains(&part.key))
        .collect())
}

pub(crate) fn process_part(
    parsed: &ParsedVectorData,
    parts: &[ImportPart],
    key: &str,
    preprocessor: ImportPreprocessor,
) -> Result<JourneyData> {
    let part = part_for_key(parts, key)?;
    let data = parsed.materialize_part(part);
    Ok(conversion::process_vector_data(parsed, &data, preprocessor))
}

pub(crate) fn import_selected_parts(
    storage: &Storage,
    parsed: &ParsedVectorData,
    selected: Vec<&ImportPart>,
    preprocessor: ImportPreprocessor,
    journey_kind: JourneyKind,
    note: Option<String>,
) -> Result<u64> {
    let mut prepared = Vec::new();
    for part in selected {
        let data = parsed.materialize_part(part);
        let journey_data = conversion::process_vector_data(parsed, &data, preprocessor);
        if !journey_data.is_empty() {
            prepared.push((part, journey_data));
        }
    }

    storage.with_db_txn(|txn| {
        // Compare against the database before inserting any part so identical
        // journeys within one export remain separate on their first import.
        let mut existing_by_date: HashMap<NaiveDate, Vec<JourneyHeader>> = HashMap::new();
        for (part, _) in &prepared {
            if part.is_memolanes_journey() && !existing_by_date.contains_key(&part.date) {
                existing_by_date.insert(
                    part.date,
                    txn.query_journeys(Some(part.date), Some(part.date), None)?,
                );
            }
        }

        let mut imported_count = 0;
        for (part, data) in prepared {
            if part.is_memolanes_journey() {
                if let Some((source_id, source_revision)) = part.source_version(parsed) {
                    if txn
                        .get_journey_header(source_id)?
                        .is_some_and(|header| header.revision == source_revision)
                    {
                        continue;
                    }
                }
                let JourneyData::Vector(vector) = &data else {
                    unreachable!("MemoLanes GPX/KML parts are vector data")
                };
                let normalized = GpsPostprocessor::process(vector.clone());
                let mut duplicate = false;
                for header in &existing_by_date[&part.date] {
                    if header.journey_type != JourneyType::Vector
                        || header.start != part.start
                        || header.end != part.end
                    {
                        continue;
                    }
                    if let JourneyData::Vector(existing) = txn.get_journey_data(&header.id)? {
                        if existing == normalized {
                            duplicate = true;
                            break;
                        }
                    }
                }
                if duplicate {
                    continue;
                }
            }

            // GPX/KML are lossy interchange formats, so conflicts import as
            // copies rather than replacing an existing source journey.
            txn.create_and_insert_journey(
                part.date,
                part.start,
                part.end,
                None,
                journey_kind,
                note.clone(),
                data,
            )?;
            imported_count += 1;
        }
        Ok(imported_count)
    })
}
