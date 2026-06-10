mod fow;
mod gpx;
mod kml;

use anyhow::Result;

use crate::{journey_data::JourneyData, journey_header::JourneyHeader, main_db};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportError {
    NoJourneys,
    JourneyNotFound,
    EmptyJourneyData,
    DataTypeMismatch,
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            ExportError::NoJourneys => "no journeys to export",
            ExportError::JourneyNotFound => "journey not found",
            ExportError::EmptyJourneyData => "journey data is empty",
            ExportError::DataTypeMismatch => "journey data type does not support export format",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ExportError {}

pub(crate) fn load_journey_for_export(
    journey_id: &str,
    txn: &main_db::Txn,
) -> Result<(JourneyHeader, JourneyData)> {
    let Some(header) = txn.get_journey_header(journey_id)? else {
        return Err(anyhow!(ExportError::JourneyNotFound));
    };
    let journey_data = txn.get_journey_data(journey_id)?;
    if journey_data.is_empty() {
        return Err(anyhow!(ExportError::EmptyJourneyData));
    }
    Ok((header, journey_data))
}

pub use fow::{journey_bitmap_to_fwss_file, journey_vector_to_fwss_file};
pub use gpx::{
    journey_vector_to_gpx_file, raw_data_csv_to_gpx_file, JOURNEY_TYPE_NAME, RAWDATA_TYPE_NAME,
};
pub use kml::journey_vector_to_kml_file;
