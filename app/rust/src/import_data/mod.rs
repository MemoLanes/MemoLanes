pub mod conversion;
pub mod csv;
pub mod fow;
pub mod gpx;
pub mod journey_partition;
mod journeys;
pub mod kml;

pub(crate) use journeys::{
    import_selected_parts, load_vector_file, process_part, select_parts, ImportMode, ImportPart,
};
pub use journeys::{ImportedJourney, ParsedVectorData};

#[derive(Clone, Copy, Debug)]
pub enum ImportPreprocessor {
    None,
    Generic,
    FlightTrack,
    Spare,
}
