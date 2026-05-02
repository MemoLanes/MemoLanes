use std::collections::HashSet;
use std::fs::File;
use std::sync::Mutex;
use std::{ffi::OsStr, path::Path};

use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::{DateTime, Local, NaiveDate, Utc};
use flutter_rust_bridge::frb;

use super::api;
use crate::api::api::{get, OpaqueJourneyData};
use crate::archive::MldxReader;
use crate::gps_processor::SegmentGapRule;
use crate::journey_header::JourneyHeader;
use crate::journey_vector::JourneyVector;
use crate::{
    flight_track_processor,
    gps_processor::RawData,
    import_data::{self, journey_info_from_raw_vector_data},
    journey_data::JourneyData,
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
}

#[auto_context]
pub fn load_fow_data(file_path: String) -> Result<(JourneyInfo, OpaqueJourneyData)> {
    let extension = Path::new(&file_path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    let (journey_bitmap, _warnings) = match extension.as_deref() {
        Some("zip") => import_data::load_fow_sync_data(&file_path)?,
        Some("fwss") => import_data::load_fow_snapshot_data(&file_path)?,
        _ => bail!("Unknown extension {extension:?}"),
    };

    let journey_info = JourneyInfo {
        journey_date: Local::now().date_naive(),
        start_time: None,
        end_time: None,
        note: None,
        journey_kind: JourneyKind::DefaultKind,
    };
    Ok((
        journey_info,
        OpaqueJourneyData::new(JourneyData::Bitmap(journey_bitmap)),
    ))
}

#[auto_context]
pub fn load_gpx_or_kml(
    file_path: String,
) -> Result<(JourneyInfo, RawVectorData, ImportPreprocessor)> {
    let (raw_vector_data, import_preprocessor) = match Path::new(&file_path)
        .extension()
        .and_then(OsStr::to_str)
        .map(|x| x.to_lowercase())
        .as_deref()
    {
        Some("gpx") => import_data::load_gpx(&file_path)?,
        Some("kml") => import_data::load_kml(&file_path)?,
        extension => return Err(anyhow!("Unknown extension: {extension:?}")),
    };

    Ok((
        journey_info_from_raw_vector_data(&raw_vector_data),
        RawVectorData {
            data: raw_vector_data,
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

pub enum ImportPreprocessor {
    None,
    Generic,
    FlightTrack,
    Spare,
}

#[auto_context]
pub fn process_vector_data(
    vector_data: &RawVectorData,
    import_processor: ImportPreprocessor,
) -> Result<OpaqueJourneyData> {
    let journey_vector_opt = match import_processor {
        ImportPreprocessor::None => {
            import_data::journey_vector_from_raw_data_with_gps_preprocessor(&vector_data.data, None)
        }
        ImportPreprocessor::Generic => {
            import_data::journey_vector_from_raw_data_with_gps_preprocessor(
                &vector_data.data,
                Some(SegmentGapRule::Default),
            )
        }
        ImportPreprocessor::FlightTrack => flight_track_processor::process(&vector_data.data),
        ImportPreprocessor::Spare => {
            import_data::journey_vector_from_raw_data_with_gps_preprocessor(
                &vector_data.data,
                Some(SegmentGapRule::Spare),
            )
        }
    };

    let journey_vector = journey_vector_opt.unwrap_or_else(|| JourneyVector {
        track_segments: vec![],
    });
    Ok(OpaqueJourneyData::new(JourneyData::Vector(journey_vector)))
}

#[auto_context]
pub fn is_journey_data_empty(journey_data: &OpaqueJourneyData) -> bool {
    let journey_data = journey_data.borrow_inner();
    match *journey_data {
        JourneyData::Vector(ref vector_data) => vector_data.track_segments.is_empty(),
        JourneyData::Bitmap(ref bitmap_data) => bitmap_data.is_empty(),
    }
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
