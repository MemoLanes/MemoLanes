use chrono::{DateTime, NaiveDate, Utc};

use crate::gps_processor::RawData;

/// One explicitly marked MemoLanes journey in a GPX/KML interchange file.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedJourney {
    pub source_journey_id: Option<String>,
    pub source_revision: Option<String>,
    pub journey_date: Option<NaiveDate>,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub segments: Vec<Vec<RawData>>,
}

impl ImportedJourney {
    pub fn generic(segments: Vec<Vec<RawData>>) -> Self {
        Self {
            source_journey_id: None,
            source_revision: None,
            journey_date: None,
            start: None,
            end: None,
            segments,
        }
    }

    pub fn has_memolanes_metadata(&self) -> bool {
        self.source_journey_id.is_some() && self.journey_date.is_some()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParsedVectorData {
    /// Groups appear in the same order as the source file. A group without
    /// MemoLanes metadata represents ordinary third-party track data.
    pub groups: Vec<ImportedJourney>,
}

impl ParsedVectorData {
    pub fn flatten(&self) -> Vec<Vec<RawData>> {
        self.groups
            .iter()
            .flat_map(|group| group.segments.iter().cloned())
            .collect()
    }

    pub fn has_memolanes_metadata(&self) -> bool {
        self.groups
            .iter()
            .any(ImportedJourney::has_memolanes_metadata)
    }
}
