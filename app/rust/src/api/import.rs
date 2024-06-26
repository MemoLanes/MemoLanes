use std::{ffi::OsStr, path::Path};

use anyhow::Result;
use chrono::{DateTime, Local, NaiveDate, Utc};
use flutter_rust_bridge::frb;

use crate::{
    gps_processor::RawData,
    import_data::{self, journey_info_from_raw_vector_data},
    journey_bitmap::JourneyBitmap,
    journey_data::JourneyData,
    journey_header::JourneyKind,
};

use super::api;

#[derive(Debug)]
#[frb(non_opaque)]
pub struct JourneyInfo {
    pub journey_date: String,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub note: Option<String>,
}

#[frb(opaque)]
pub struct RawBitmapData {
    data: JourneyBitmap,
}

#[frb(opaque)]
pub struct RawVectorData {
    data: Vec<Vec<RawData>>,
}

pub fn load_fow_sync_data(file_path: String) -> Result<(JourneyInfo, RawBitmapData)> {
    let (journey_bitmap, _warnings) = import_data::load_fow_sync_data(&file_path)?;
    let journey_info = JourneyInfo {
        journey_date: Local::now().date_naive().format("%Y-%m-%d").to_string(),
        start_time: None,
        end_time: None,
        note: None,
    };
    Ok((
        journey_info,
        RawBitmapData {
            data: journey_bitmap,
        },
    ))
}

pub fn load_gpx_or_kml(file_path: String) -> Result<(JourneyInfo, RawVectorData)> {
    let raw_vecotr_data = match Path::new(&file_path)
        .extension()
        .and_then(OsStr::to_str)
        .map(|x| x.to_lowercase())
        .as_deref()
    {
        Some("gpx") => import_data::load_gpx(&file_path)?,
        Some("kml") => import_data::load_kml(&file_path)?,
        extension => return Err(anyhow!("Unknown extension: {:?}", extension)),
    };

    Ok((
        journey_info_from_raw_vector_data(&raw_vecotr_data),
        RawVectorData {
            data: raw_vecotr_data,
        },
    ))
}

fn import(journey_info: JourneyInfo, journey_data: JourneyData) -> Result<()> {
    let journey_date = NaiveDate::parse_from_str(&journey_info.journey_date, "%Y-%m-%d")
        .unwrap_or_else(|_| Local::now().naive_local().date());
    api::get().storage.with_db_txn(|txn| {
        txn.create_and_insert_journey(
            journey_date,
            journey_info.start_time,
            journey_info.end_time,
            None,
            JourneyKind::DefaultKind,
            journey_info.note,
            journey_data,
        )
    })
}

pub fn import_bitmap(journey_info: JourneyInfo, bitmap_data: RawBitmapData) -> Result<()> {
    let journey_data: JourneyData = JourneyData::Bitmap(bitmap_data.data);
    import(journey_info, journey_data)
}

pub fn import_vector(
    journey_info: JourneyInfo,
    vector_data: RawVectorData,
    run_preprocessor: bool,
) -> Result<()> {
    let journey_vector =
        import_data::journey_vector_from_raw_data(vector_data.data, run_preprocessor);
    match journey_vector {
        None => {
            // TODO: return a strucutred error to outside for better error handling.
            Err(anyhow!("The imported file produced empty result"))
        }
        Some(journey_vector) => {
            let journey_data: JourneyData = JourneyData::Vector(journey_vector);
            import(journey_info, journey_data)
        }
    }
}
