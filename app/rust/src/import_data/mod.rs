pub mod conversion;
pub mod csv;
pub mod fow;
pub mod gpx;
pub mod journey_partition;
mod journeys;
pub mod kml;
pub(crate) mod preprocessor;

use crate::gps::RawGPSPoint;
pub(crate) use journeys::{
    import_selected_parts, load_vector_file, process_part, select_parts, ImportPart,
};

#[derive(Clone, Copy, Debug)]
pub enum ImportPreprocessor {
    None,
    Generic,
    FlightTrack,
    Spare,
}

/// A group of source segments, optionally grouped by CSV journey ID.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedJourney {
    pub segments: Vec<Vec<RawGPSPoint>>,
}

impl ImportedJourney {
    pub fn generic(segments: Vec<Vec<RawGPSPoint>>) -> Self {
        Self { segments }
    }
}

/// GPX/KML and ordinary CSV use date-based grouping; CSV with journey IDs
/// retains its explicit groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImportGrouping {
    ByDate,
    ByJourney,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedVectorData {
    groups: Vec<ImportedJourney>,
    grouping: ImportGrouping,
}

impl ParsedVectorData {
    pub(crate) fn new(grouping: ImportGrouping, groups: Vec<ImportedJourney>) -> Self {
        Self { groups, grouping }
    }
}
