use super::ImportPreprocessor;
use crate::export_data::{JOURNEY_LABEL, RAW_DATA_LABEL};

#[derive(Clone, Copy)]
pub(crate) enum TrackFileFormat {
    Gpx,
    Kml,
}

/// Select a default processor from the file header. This is only a hint:
/// parsing, partitioning and importing use the same paths for every source.
pub(crate) fn detect_preprocessor(xml: &str, format: TrackFileFormat) -> ImportPreprocessor {
    const PROBE_LIMIT: usize = 8 * 1024;
    let head = xml
        .chars()
        .take(PROBE_LIMIT)
        .collect::<String>()
        .to_ascii_lowercase();

    // Preserve the existing GPX heuristics for 一生足迹 and 灵感足迹.
    if matches!(format, TrackFileFormat::Gpx)
        && (head.contains("stepofmyworld") || head.contains("yourapp"))
    {
        return ImportPreprocessor::Spare;
    }

    // Both writers name processed tracks "MemoLanes Journey". The creator
    // alone is insufficient: raw exports still need normal preprocessing.
    let has_raw_data = head.contains(&RAW_DATA_LABEL.to_ascii_lowercase());
    if head.contains(&JOURNEY_LABEL.to_ascii_lowercase()) && !has_raw_data {
        ImportPreprocessor::None
    } else {
        ImportPreprocessor::Generic
    }
}
