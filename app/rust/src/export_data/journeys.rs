use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, Utc};

use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyHeader, JourneyType};
use crate::journey_vector::JourneyVector;
use crate::main_db::Txn;

/// The geometry and header metadata that can be represented by GPX/KML.
/// MLDX remains the complete backup format.
#[derive(Clone, Debug)]
pub(crate) struct JourneyExport {
    pub source_journey_id: String,
    pub source_revision: String,
    pub journey_date: NaiveDate,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub vector: JourneyVector,
}

impl JourneyExport {
    pub(crate) fn from_header(header: JourneyHeader, vector: JourneyVector) -> Self {
        Self {
            source_journey_id: header.id,
            source_revision: header.revision,
            journey_date: header.journey_date,
            start: header.start,
            end: header.end,
            vector,
        }
    }
}

/// Collects headers for non-empty vector journeys without retaining every track
/// in memory. Export reads each journey again just before writing it.
///
/// This belongs to the export layer because it combines database records with
/// the lossy export model. The API layer only coordinates the transaction and
/// file output, while `main_db` remains unaware of GPX/KML concerns.
pub(crate) fn collect_vector_journey_headers(txn: &mut Txn<'_>) -> Result<Vec<JourneyHeader>> {
    let mut result = Vec::new();
    for header in txn.query_journeys(None, None, None)? {
        if header.journey_type != JourneyType::Vector {
            continue;
        }
        let JourneyData::Vector(vector) = txn.get_journey_data(&header.id)? else {
            return Err(anyhow!("Journey {} is not vector data", header.id));
        };
        if !vector
            .track_segments
            .iter()
            .any(|segment| !segment.track_points.is_empty())
        {
            continue;
        }
        result.push(header);
    }
    Ok(result)
}

pub(crate) fn load_vector_journey(
    txn: &mut Txn<'_>,
    header: JourneyHeader,
) -> Result<JourneyExport> {
    let JourneyData::Vector(vector) = txn.get_journey_data(&header.id)? else {
        return Err(anyhow!("Journey {} is not vector data", header.id));
    };
    Ok(JourneyExport::from_header(header, vector))
}
