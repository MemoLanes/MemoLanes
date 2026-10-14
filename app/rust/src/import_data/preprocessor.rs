use super::ImportPreprocessor;

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
    let has_raw_data = head.contains("memolanes rawdata") || head.contains("memolanes raw data");
    if head.contains("memolanes journey") && !has_raw_data {
        ImportPreprocessor::None
    } else {
        ImportPreprocessor::Generic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_formats_distinguish_processed_journeys_from_raw_data() {
        for format in [TrackFileFormat::Gpx, TrackFileFormat::Kml] {
            assert!(matches!(
                detect_preprocessor("<name>MemoLanes Journey 2026-10-14</name>", format),
                ImportPreprocessor::None
            ));
            for header in [
                "creator=\"MemoLanes\"",
                "<name>Other App</name>",
                "<type>MemoLanes RawData</type>",
                "<name>MemoLanes Raw Data</name>",
                "<name>MemoLanes Journey</name><type>MemoLanes RawData</type>",
                "<name>MemoLanes Journey</name><name>MemoLanes Raw Data</name>",
            ] {
                assert!(
                    matches!(
                        detect_preprocessor(header, format),
                        ImportPreprocessor::Generic
                    ),
                    "{header}"
                );
            }
        }
    }

    #[test]
    fn existing_app_heuristics_remain_gpx_only_and_case_insensitive() {
        for source in ["StepOfMyWorld", "YourApp"] {
            assert!(matches!(
                detect_preprocessor(source, TrackFileFormat::Gpx),
                ImportPreprocessor::Spare
            ));
            assert!(matches!(
                detect_preprocessor(source, TrackFileFormat::Kml),
                ImportPreprocessor::Generic
            ));
        }
    }
}
