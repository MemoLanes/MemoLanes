use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};

use crate::gps::RawGPSPoint;
use crate::gps_processor::GpsPostprocessor;
use crate::import_data::{
    conversion, csv, gpx,
    journey_partition::{self, SegmentSlice},
    kml, ImportPreprocessor,
};
use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyHeader, JourneyInfo, JourneyKind, JourneyType};
use crate::main_db::NewJourney;
use crate::storage::Storage;

/// A parser group containing geometry and optional exported-journey metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedJourney {
    pub journey_date: Option<NaiveDate>,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub segments: Vec<Vec<RawGPSPoint>>,
}

impl ImportedJourney {
    pub fn generic(segments: Vec<Vec<RawGPSPoint>>) -> Self {
        Self {
            journey_date: None,
            start: None,
            end: None,
            segments,
        }
    }

    pub fn has_memolanes_metadata(&self) -> bool {
        self.journey_date.is_some()
    }
}

/// File-level policy: explicit CSV groups are still raw samples, while exported
/// GPX/KML journeys already contain processed geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImportMode {
    GenericByDate,
    GenericByJourney,
    ExportedJourney,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedVectorData {
    groups: Vec<ImportedJourney>,
    mode: ImportMode,
}

impl ParsedVectorData {
    pub(crate) fn new(mode: ImportMode, groups: Vec<ImportedJourney>) -> Result<Self> {
        for group in &groups {
            match mode {
                ImportMode::ExportedJourney if group.journey_date.is_none() => {
                    bail!("Exported journeys must all have a valid journey date");
                }
                ImportMode::GenericByDate | ImportMode::GenericByJourney
                    if group.journey_date.is_some()
                        || group.start.is_some()
                        || group.end.is_some() =>
                {
                    bail!("Generic import groups must not contain exported journey metadata");
                }
                _ => {}
            }
        }
        Ok(Self { groups, mode })
    }

    pub fn groups(&self) -> &[ImportedJourney] {
        &self.groups
    }

    pub(crate) fn mode(&self) -> ImportMode {
        self.mode
    }

    pub fn flatten(&self) -> Vec<Vec<RawGPSPoint>> {
        self.groups
            .iter()
            .flat_map(|group| group.segments.iter().cloned())
            .collect()
    }

    pub fn is_exported_journey(&self) -> bool {
        self.mode == ImportMode::ExportedJourney
    }

    pub(crate) fn build_parts(&self) -> Vec<ImportPart> {
        match self.mode {
            ImportMode::GenericByJourney | ImportMode::ExportedJourney => self
                .groups
                .iter()
                .enumerate()
                .map(|(index, group)| {
                    let (date, start, end) = if self.mode == ImportMode::ExportedJourney {
                        (
                            group.journey_date.expect("validated export date"),
                            group.start,
                            group.end,
                        )
                    } else {
                        let info = conversion::journey_info_from_raw_vector_data(&group.segments);
                        (info.journey_date, info.start_time, info.end_time)
                    };
                    ImportPart {
                        key: format!("journey:{index}"),
                        date,
                        start,
                        end,
                        point_count: group.segments.iter().map(Vec::len).sum::<usize>() as u64,
                        missing_timestamp_count: group
                            .segments
                            .iter()
                            .flatten()
                            .filter(|point| point.timestamp_ms.is_none())
                            .count() as u64,
                        source: ImportPartSource::Journey(index),
                    }
                })
                .collect(),
            ImportMode::GenericByDate => {
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
    }

    pub(crate) fn materialize_part(&self, part: &ImportPart) -> Vec<Vec<RawGPSPoint>> {
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
        Some("csv") => csv::load_csv(file_path)?,
        extension => bail!("Unknown extension: {extension:?}"),
    };

    let journey_info = match (parsed.mode, parsed.groups.first()) {
        (ImportMode::ExportedJourney, Some(group)) => JourneyInfo {
            journey_date: group.journey_date.expect("validated export date"),
            start_time: group.start,
            end_time: group.end,
            note: None,
            journey_kind: JourneyKind::DefaultKind,
        },
        _ => conversion::journey_info_from_raw_vector_data(&parsed.flatten()),
    };
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
            if parsed.mode == ImportMode::ExportedJourney
                && !existing_by_date.contains_key(&part.date)
            {
                existing_by_date.insert(
                    part.date,
                    txn.query_journeys(Some(part.date), Some(part.date), None)?,
                );
            }
        }

        let mut imported_count = 0;
        for (part, data) in prepared {
            if parsed.mode == ImportMode::ExportedJourney {
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
            txn.create_and_insert_journey(NewJourney {
                journey_date: part.date,
                start: part.start,
                end: part.end,
                created_at: None,
                journey_kind,
                note: note.clone(),
                journey_data: data,
                raw_data: None,
            })?;
            imported_count += 1;
        }
        Ok(imported_count)
    })
}
