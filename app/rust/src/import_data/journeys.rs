use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};

use crate::gps::RawGPSPoint;
use crate::import_data::{
    conversion, csv, gpx,
    journey_partition::{self, SegmentSlice},
    kml, ImportGrouping, ImportPreprocessor, ImportedJourney, ParsedVectorData,
};
use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyInfo, JourneyKind};
use crate::main_db::NewJourney;
use crate::storage::Storage;

impl ParsedVectorData {
    pub fn groups(&self) -> &[ImportedJourney] {
        &self.groups
    }

    pub(crate) fn grouping(&self) -> ImportGrouping {
        self.grouping
    }

    pub fn flatten(&self) -> Vec<Vec<RawGPSPoint>> {
        self.groups
            .iter()
            .flat_map(|group| group.segments.iter().cloned())
            .collect()
    }

    pub(crate) fn build_parts(&self) -> Vec<ImportPart> {
        match self.grouping {
            ImportGrouping::ByJourney => self
                .groups
                .iter()
                .enumerate()
                .map(|(index, group)| {
                    let info = conversion::journey_info_from_raw_vector_data(&group.segments);
                    ImportPart {
                        key: format!("journey:{index}"),
                        date: info.journey_date,
                        start: info.start_time,
                        end: info.end_time,
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
            ImportGrouping::ByDate => {
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

    let journey_info = conversion::journey_info_from_raw_vector_data(&parsed.flatten());
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
    Ok(conversion::process_vector_data(&data, preprocessor))
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
        let journey_data = conversion::process_vector_data(&data, preprocessor);
        if !journey_data.is_empty() {
            prepared.push((part, journey_data));
        }
    }

    storage.with_db_txn(|txn| {
        let mut imported_count = 0;
        for (part, data) in prepared {
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
