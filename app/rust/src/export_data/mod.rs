pub mod csv;
mod file;
pub mod fow;
pub mod gpx;
mod journeys;
pub mod kml;

pub(crate) use file::write_export_file;
pub(crate) use journeys::{
    export_all_raw_journeys, export_all_vector_journeys, load_journey_raw_data, VectorExportFormat,
};

use crate::journey_header::JourneyHeader;
use crate::journey_vector::JourneyVector;
use anyhow::Result;
use chrono::NaiveDate;
use quick_xml::{
    events::{BytesEnd, BytesStart, BytesText, Event},
    Writer,
};
use std::io::Write;

// Export names also provide the default import preprocessor hint.
pub(crate) const JOURNEY_LABEL: &str = "MemoLanes Journey";
pub(crate) const RAW_DATA_LABEL: &str = "MemoLanes Raw Data";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawDataExportType {
    CSV = 0,
    GPX = 1,
    KML = 2,
}

/// The geometry and header metadata that can be represented by GPX/KML.
/// MLDX remains the complete backup format.
#[derive(Clone, Debug)]
pub struct JourneyExport {
    pub journey_date: NaiveDate,
    pub vector: JourneyVector,
}

impl JourneyExport {
    pub(crate) fn from_header(header: JourneyHeader, vector: JourneyVector) -> Self {
        Self {
            journey_date: header.journey_date,
            vector,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.vector
            .track_segments
            .iter()
            .all(|segment| segment.track_points.is_empty())
    }
}

pub(super) fn write_text<W: Write>(xml: &mut Writer<W>, tag: &str, value: &str) -> Result<()> {
    xml.write_event(Event::Start(BytesStart::new(tag)))?;
    xml.write_event(Event::Text(BytesText::new(value)))?;
    xml.write_event(Event::End(BytesEnd::new(tag)))?;
    Ok(())
}
