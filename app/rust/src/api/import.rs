use std::collections::HashSet;
use std::fs::File;
use std::sync::{Mutex, OnceLock};
use std::{ffi::OsStr, path::Path};

use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::{DateTime, FixedOffset, Local, NaiveDate, Utc};
use flutter_rust_bridge::frb;

use super::api;
use crate::api::api::{get, OpaqueJourneyData};
use crate::archive::MldxReader;
use crate::gps_processor::SegmentGapRule;
use crate::journey_header::JourneyHeader;
use crate::journey_vector::JourneyVector;
use crate::{
    flight_track_processor, gps_processor::RawData, import_data, journey_data::JourneyData,
    journey_header::JourneyKind,
};

#[derive(Debug)]
#[frb(non_opaque)]
pub struct JourneyInfo {
    pub journey_date: NaiveDate,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub journey_kind: JourneyKind,
    pub note: Option<String>,
}

#[frb(opaque)]
pub struct RawVectorData {
    data: Vec<Vec<RawData>>,
    partition: OnceLock<import_data::journey_partition::PartitionByDate>,
}

#[derive(Debug)]
#[frb(non_opaque)]
pub struct VectorImportPartSummary {
    pub journey_date: String,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub point_count: u64,
    pub missing_timestamp_count: u64,
}

fn parse_fwss_snapshot_time_from_filename(file_path: &str) -> Option<DateTime<FixedOffset>> {
    let stem = Path::new(file_path).file_stem()?.to_str()?;
    let timestamp = match stem.split_once('-') {
        None => stem,
        Some((prefix, timestamp)) => {
            if !prefix.eq_ignore_ascii_case("snapshot") {
                return None;
            };
            timestamp
        }
    };
    DateTime::parse_from_str(timestamp, "%Y%m%dT%H%M%S%z").ok()
}

#[auto_context]
pub fn load_fow_data(file_path: String) -> Result<(JourneyInfo, OpaqueJourneyData)> {
    let extension = Path::new(&file_path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    let (journey_bitmap, _warnings) = match extension.as_deref() {
        Some("zip") => import_data::fow::load_fow_sync_data(&file_path)?,
        Some("fwss") => import_data::fow::load_fow_snapshot_data(&file_path)?,
        _ => bail!("Unknown extension {extension:?}"),
    };

    let snapshot_time = match extension.as_deref() {
        Some("fwss") => parse_fwss_snapshot_time_from_filename(&file_path),
        _ => None,
    };
    let journey_date = snapshot_time
        .as_ref()
        .map(|time| time.date_naive())
        .unwrap_or_else(|| Local::now().date_naive());
    let end_time = snapshot_time.map(|time| time.with_timezone(&Utc));

    let journey_info = JourneyInfo {
        journey_date,
        start_time: None,
        end_time,
        note: None,
        journey_kind: JourneyKind::DefaultKind,
    };

    Ok((
        journey_info,
        OpaqueJourneyData::new(JourneyData::Bitmap(journey_bitmap)),
    ))
}

#[auto_context]
pub fn load_vector_data(
    file_path: String,
) -> Result<(JourneyInfo, RawVectorData, ImportPreprocessor)> {
    let (raw_vector_data, import_preprocessor) = match Path::new(&file_path)
        .extension()
        .and_then(OsStr::to_str)
        .map(|x| x.to_lowercase())
        .as_deref()
    {
        Some("gpx") => import_data::gpx::load_gpx(&file_path)?,
        Some("kml") => import_data::kml::load_kml(&file_path)?,
        Some("csv") => import_data::csv::load_csv(&file_path)?,
        extension => return Err(anyhow!("Unknown extension: {extension:?}")),
    };

    Ok((
        import_data::conversion::journey_info_from_raw_vector_data(&raw_vector_data),
        RawVectorData {
            data: raw_vector_data,
            partition: OnceLock::new(),
        },
        import_preprocessor,
    ))
}

#[auto_context]
pub fn import_journey_data(
    journey_info: JourneyInfo,
    journey_data: OpaqueJourneyData,
) -> Result<()> {
    let _id = api::get().storage.with_db_txn(|txn| {
        txn.create_and_insert_journey(
            journey_info.journey_date,
            journey_info.start_time,
            journey_info.end_time,
            None,
            journey_info.journey_kind,
            journey_info.note,
            journey_data.into_inner(),
        )
    })?;
    Ok(())
}

#[derive(Clone, Copy)]
pub enum ImportPreprocessor {
    None,
    Generic,
    FlightTrack,
    Spare,
}

impl RawVectorData {
    fn partition(&self) -> &import_data::journey_partition::PartitionByDate {
        self.partition
            .get_or_init(|| import_data::journey_partition::partition_by_date(&self.data))
    }
}

fn data_for_date(vector_data: &RawVectorData, journey_date: &str) -> Result<Vec<Vec<RawData>>> {
    let journey_date = NaiveDate::parse_from_str(journey_date, "%Y-%m-%d")?;
    let slices = vector_data
        .partition()
        .index
        .get(&journey_date)
        .with_context(|| format!("No vector data for date {journey_date}"))?;
    Ok(import_data::journey_partition::materialize_partition(
        &vector_data.data,
        slices,
    ))
}

pub fn analyze_vector_data_by_date(vector_data: &RawVectorData) -> Vec<VectorImportPartSummary> {
    vector_data
        .partition()
        .summaries
        .iter()
        .map(|(journey_date, summary)| VectorImportPartSummary {
            journey_date: journey_date.format("%Y-%m-%d").to_string(),
            start_time: summary.start_time,
            end_time: summary.end_time,
            point_count: summary.point_count,
            missing_timestamp_count: summary.missing_timestamp_count,
        })
        .collect()
}

#[auto_context]
pub fn process_vector_data_for_date(
    vector_data: &RawVectorData,
    journey_date: String,
    import_processor: ImportPreprocessor,
) -> Result<OpaqueJourneyData> {
    let data = data_for_date(vector_data, &journey_date)?;
    Ok(process_raw_vector_data(&data, import_processor))
}

#[auto_context]
pub fn import_vector_data_by_date(
    vector_data: &RawVectorData,
    journey_dates: Vec<String>,
    import_processor: ImportPreprocessor,
    journey_kind: JourneyKind,
    note: Option<String>,
) -> Result<u64> {
    let selected_dates = journey_dates
        .into_iter()
        .map(|date| NaiveDate::parse_from_str(&date, "%Y-%m-%d"))
        .collect::<Result<HashSet<_>, _>>()?;
    let partition = vector_data.partition();
    let mut missing_dates = selected_dates
        .iter()
        .filter(|date| !partition.index.contains_key(date))
        .copied()
        .collect::<Vec<_>>();
    if !missing_dates.is_empty() {
        missing_dates.sort_unstable();
        bail!("No vector data for dates: {missing_dates:?}");
    }
    let mut parts = Vec::new();

    for (journey_date, slices) in &partition.index {
        if !selected_dates.contains(journey_date) {
            continue;
        }
        let summary = partition
            .summaries
            .get(journey_date)
            .expect("partition index and summaries are built together");
        let raw_data =
            import_data::journey_partition::materialize_partition(&vector_data.data, slices);
        let journey_data = process_raw_vector_data(&raw_data, import_processor).into_inner();
        if !journey_data.is_empty() {
            parts.push((
                *journey_date,
                summary.start_time,
                summary.end_time,
                journey_data,
            ));
        }
    }

    let imported_count = parts.len() as u64;
    api::get().storage.with_db_txn(|txn| {
        for (journey_date, start_time, end_time, journey_data) in parts {
            txn.create_and_insert_journey(
                journey_date,
                start_time,
                end_time,
                None,
                journey_kind,
                note.clone(),
                journey_data,
            )?;
        }
        Ok(())
    })?;
    Ok(imported_count)
}

fn process_raw_vector_data(
    raw_data: &[Vec<RawData>],
    import_processor: ImportPreprocessor,
) -> OpaqueJourneyData {
    let journey_vector_opt = match import_processor {
        ImportPreprocessor::None => {
            import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
                raw_data, None,
            )
        }
        ImportPreprocessor::Generic => {
            import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
                raw_data,
                Some(SegmentGapRule::Default),
            )
        }
        ImportPreprocessor::FlightTrack => flight_track_processor::process(raw_data),
        ImportPreprocessor::Spare => {
            import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
                raw_data,
                Some(SegmentGapRule::Spare),
            )
        }
    };

    let journey_vector = journey_vector_opt.unwrap_or_else(|| JourneyVector {
        track_segments: vec![],
    });
    OpaqueJourneyData::new(JourneyData::Vector(journey_vector))
}

#[auto_context]
pub fn process_vector_data(
    vector_data: &RawVectorData,
    import_processor: ImportPreprocessor,
) -> Result<OpaqueJourneyData> {
    Ok(process_raw_vector_data(&vector_data.data, import_processor))
}

#[auto_context]
pub fn is_journey_data_empty(journey_data: &OpaqueJourneyData) -> bool {
    journey_data.borrow_inner().is_empty()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MldxJourneyImportAnalyzeResult {
    New = 0,
    Conflict = 1,
    Unchanged = 2,
}

#[frb(opaque)]
pub struct OpaqueMldxReader {
    reader: Mutex<MldxReader<File>>,
}

impl OpaqueMldxReader {
    pub fn open(mldx_file_path: String) -> Result<Self> {
        let file = File::open(mldx_file_path)?;
        Ok(Self {
            reader: Mutex::new(MldxReader::open(file)?),
        })
    }

    pub fn analyze(&self) -> Result<Vec<(JourneyHeader, MldxJourneyImportAnalyzeResult)>> {
        let mldx_reader = self.reader.lock().unwrap();

        get().storage.with_db_txn(|txn| {
            let mut result = Vec::new();
            for journey_header in mldx_reader.iter_journey_headers() {
                let import_type = match txn.get_journey_header(&journey_header.id)? {
                    Some(existing) => {
                        if existing.revision == journey_header.revision {
                            MldxJourneyImportAnalyzeResult::Unchanged
                        } else {
                            MldxJourneyImportAnalyzeResult::Conflict
                        }
                    }
                    None => MldxJourneyImportAnalyzeResult::New,
                };
                result.push((journey_header.clone(), import_type));
            }
            Ok(result)
        })
    }

    pub fn load_single_journey(
        &self,
        journey_id: String,
    ) -> Result<Option<(JourneyHeader, OpaqueJourneyData)>> {
        let mut mldx_reader = self.reader.lock().unwrap();
        Ok(mldx_reader
            .load_single_journey(&journey_id)?
            .map(|(header, data)| (header, OpaqueJourneyData::new(data))))
    }

    /// `journey_ids = None` means import all journeys.
    /// `journey_ids = Some(set)` means import only journeys whose id is in `set`.
    pub fn import_journeys(&self, journey_ids: Option<HashSet<String>>) -> Result<()> {
        let mut mldx_reader = self.reader.lock().unwrap();
        get()
            .storage
            .with_db_txn(|txn| mldx_reader.import(txn, journey_ids.as_ref()))?;
        Ok(())
    }
}
