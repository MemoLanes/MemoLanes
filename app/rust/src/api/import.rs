use std::{ffi::OsStr, path::Path};

use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::{DateTime, Local, NaiveDate, Utc};
use flutter_rust_bridge::frb;

use super::api;
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
pub fn load_fow_data(file_path: String) -> Result<(JourneyInfo, JourneyData)> {
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
    Ok((journey_info, JourneyData::Bitmap(journey_bitmap)))
}

#[auto_context]
pub fn load_gpx_or_kml(file_path: String) -> Result<(JourneyInfo, RawVectorData)> {
    let raw_vector_data = match Path::new(&file_path)
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
    ))
}

#[auto_context]
pub fn import_journey_data(journey_info: JourneyInfo, journey_data: JourneyData) -> Result<()> {
    let _id = api::get().storage.with_db_txn(|txn| {
        txn.create_and_insert_journey(
            journey_info.journey_date,
            journey_info.start_time,
            journey_info.end_time,
            None,
            journey_info.journey_kind,
            journey_info.note,
            journey_data,
        )
    })?;
    Ok(())
}

pub enum ImportPreprocessor {
    None,
    Generic,
    FlightTrack,
}

#[auto_context]
pub fn process_vector_data(
    vector_data: &RawVectorData,
    import_processor: ImportPreprocessor,
) -> Result<JourneyData> {
    let journey_vector_opt = match import_processor {
        ImportPreprocessor::None => {
            import_data::journey_vector_from_raw_data_with_gps_preprocessor(
                &vector_data.data,
                false,
            )
        }
        ImportPreprocessor::Generic => {
            import_data::journey_vector_from_raw_data_with_gps_preprocessor(&vector_data.data, true)
        }
        ImportPreprocessor::FlightTrack => flight_track_processor::process(&vector_data.data),
    };

    let journey_vector = journey_vector_opt.unwrap_or_else(|| JourneyVector {
        track_segments: vec![],
    });
    Ok(JourneyData::Vector(journey_vector))
}

#[auto_context]
pub fn is_journey_data_empty(journey_data: &JourneyData) -> bool {
    match journey_data {
        JourneyData::Vector(vector_data) => vector_data.track_segments.is_empty(),
        JourneyData::Bitmap(bitmap_data) => bitmap_data.tiles.is_empty(),
    }
}
