use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};

use crate::api::import::{ImportPreprocessor, JourneyInfo};
use crate::gps_processor::RawData;
use crate::import_data::{
    conversion, csv, gpx,
    journey_partition::{self, SegmentSlice},
    kml,
};
use crate::journey_data::JourneyData;
use crate::journey_header::JourneyKind;
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
}

pub(crate) fn part_for_key<'a>(parts: &'a [ImportPart], key: &str) -> Result<&'a ImportPart> {
    parts
        .iter()
        .find(|part| part.key == key)
        .with_context(|| format!("No vector data for part {key}"))
}

pub(crate) fn select_parts<'a>(
    parts: &'a [ImportPart],
    keys: Vec<String>,
) -> Result<Vec<&'a ImportPart>> {
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
            prepared.push((part.date, part.start, part.end, journey_data));
        }
    }

    let imported_count = prepared.len() as u64;
    storage.with_db_txn(|txn| {
        for (date, start, end, data) in prepared {
            // GPX/KML are lossy interchange formats. Import as new journeys so
            // a source ID/revision conflict never overwrites the local journey.
            txn.create_and_insert_journey(
                date,
                start,
                end,
                None,
                journey_kind,
                note.clone(),
                data,
            )?;
        }
        Ok(())
    })?;
    Ok(imported_count)
}
