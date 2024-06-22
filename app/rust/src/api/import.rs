use anyhow::Result;
use chrono::{DateTime, Local, Utc};
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
    pub journey_date: chrono::NaiveDate,
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
        journey_date: Local::now().date_naive(),
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

// TODO: Consider just detect file type here so we can merge the two functions below.
pub fn load_gpx(file_path: String) -> Result<(JourneyInfo, RawVectorData)> {
    let raw_vecotr_data = import_data::load_gpx(&file_path)?;
    Ok((
        journey_info_from_raw_vector_data(&raw_vecotr_data),
        RawVectorData {
            data: raw_vecotr_data,
        },
    ))
}

pub fn load_kml(file_path: String) -> Result<(JourneyInfo, RawVectorData)> {
    let raw_vecotr_data = import_data::load_kml(&file_path)?;
    Ok((
        journey_info_from_raw_vector_data(&raw_vecotr_data),
        RawVectorData {
            data: raw_vecotr_data,
        },
    ))
}

fn import(journey_info: JourneyInfo, journey_data: JourneyData) -> Result<()> {
    api::get().storage.with_db_txn(|txn| {
        txn.create_and_insert_journey(
            journey_info.journey_date,
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
