use std::collections::HashSet;
use std::fs::File;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::{DateTime, FixedOffset, Local, NaiveDate, Utc};
use flutter_rust_bridge::frb;

use super::api;
use crate::api::api::{get, OpaqueJourneyData};
use crate::archive::MldxReader;
use crate::journey_header::JourneyHeader;
use crate::{import_data, journey_data::JourneyData, journey_header::JourneyKind};

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
    parsed: import_data::ParsedVectorData,
    parts: OnceLock<Vec<import_data::ImportPart>>,
}

#[derive(Debug)]
#[frb(non_opaque)]
pub struct VectorImportPartSummary {
    pub part_key: String,
    pub is_memolanes_journey: bool,
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
    let (journey_info, parsed, import_preprocessor) = import_data::load_vector_file(&file_path)?;

    Ok((
        journey_info,
        RawVectorData {
            parsed,
            parts: OnceLock::new(),
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

#[derive(Clone, Copy, Debug)]
pub enum ImportPreprocessor {
    None,
    Generic,
    FlightTrack,
    Spare,
}

impl RawVectorData {
    fn parts(&self) -> &[import_data::ImportPart] {
        self.parts.get_or_init(|| self.parsed.build_parts())
    }
}

pub fn analyze_vector_data_parts(vector_data: &RawVectorData) -> Vec<VectorImportPartSummary> {
    vector_data
        .parts()
        .iter()
        .map(|part| VectorImportPartSummary {
            part_key: part.key.clone(),
            is_memolanes_journey: part.is_memolanes_journey(),
            journey_date: part.date.format("%Y-%m-%d").to_string(),
            start_time: part.start,
            end_time: part.end,
            point_count: part.point_count,
            missing_timestamp_count: part.missing_timestamp_count,
        })
        .collect()
}

#[auto_context]
pub fn process_vector_data_for_part(
    vector_data: &RawVectorData,
    part_key: String,
    import_processor: ImportPreprocessor,
) -> Result<OpaqueJourneyData> {
    Ok(OpaqueJourneyData::new(import_data::process_part(
        &vector_data.parsed,
        vector_data.parts(),
        &part_key,
        import_processor,
    )?))
}

#[auto_context]
pub fn import_vector_data_by_parts(
    vector_data: &RawVectorData,
    part_keys: Vec<String>,
    import_processor: ImportPreprocessor,
    journey_kind: JourneyKind,
    note: Option<String>,
) -> Result<u64> {
    let selected = import_data::select_parts(vector_data.parts(), part_keys)?;
    import_data::import_selected_parts(
        &api::get().storage,
        &vector_data.parsed,
        selected,
        import_processor,
        journey_kind,
        note,
    )
}

#[auto_context]
pub fn process_vector_data(
    vector_data: &RawVectorData,
    import_processor: ImportPreprocessor,
) -> Result<OpaqueJourneyData> {
    Ok(OpaqueJourneyData::new(
        import_data::conversion::process_vector_data(
            &vector_data.parsed,
            &vector_data.parsed.flatten(),
            import_processor,
        ),
    ))
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
