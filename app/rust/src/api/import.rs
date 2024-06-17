use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use flutter_rust_bridge::frb;

use crate::{
    gps_processor::RawData, journey_bitmap::JourneyBitmap, journey_data::JourneyData,
    journey_header::JourneyKind,
};

use super::api;

#[derive(Debug)]
#[frb(non_opaque)]
pub struct JourneyInfo {
    pub journey_date: NaiveDate,
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
    panic!("TODO")
}

pub fn load_gpx(file_path: String) -> Result<(JourneyInfo, RawVectorData)> {
    panic!("TODO")
}

pub fn load_kml(file_path: String) -> Result<(JourneyInfo, RawVectorData)> {
    panic!("TODO")
}

fn import(journey_info: JourneyInfo, journey_data : JourneyData) -> Result<()> {
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
    let journey_data: JourneyData = JourneyData::Vector(vector_data.data);
    import(journey_info, journey_data)
}
