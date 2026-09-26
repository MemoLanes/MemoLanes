use std::fs::File;
use std::io::{BufWriter, Write};

use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, Utc};

use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyHeader, JourneyType};
use crate::journey_vector::JourneyVector;
use crate::main_db::Txn;
use crate::storage::Storage;

use super::{gpx::GpxJourneyWriter, kml::KmlJourneyWriter};

pub(crate) enum VectorExportFormat {
    Gpx,
    Kml,
}

enum VectorWriter<W: Write> {
    Gpx(GpxJourneyWriter<W>),
    Kml(KmlJourneyWriter<W>),
}

impl<W: Write> VectorWriter<W> {
    fn new(writer: W, format: VectorExportFormat) -> Result<Self> {
        match format {
            VectorExportFormat::Gpx => Ok(Self::Gpx(GpxJourneyWriter::new(writer)?)),
            VectorExportFormat::Kml => Ok(Self::Kml(KmlJourneyWriter::new(writer)?)),
        }
    }

    fn write_journey(&mut self, journey: &JourneyExport) -> Result<()> {
        match self {
            Self::Gpx(writer) => writer.write_journey(journey),
            Self::Kml(writer) => writer.write_journey(journey),
        }
    }

    fn finish(self) -> Result<()> {
        match self {
            Self::Gpx(writer) => writer.finish(),
            Self::Kml(writer) => writer.finish(),
        }
    }
}

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

    pub(crate) fn is_empty(&self) -> bool {
        self.vector
            .track_segments
            .iter()
            .all(|segment| segment.track_points.is_empty())
    }
}

/// Collects vector journey headers without retaining any tracks in memory.
fn collect_vector_journey_headers(txn: &mut Txn<'_>) -> Result<Vec<JourneyHeader>> {
    let mut result = Vec::new();
    for header in txn.query_journeys(None, None, None)? {
        if header.journey_type != JourneyType::Vector {
            continue;
        }
        result.push(header);
    }
    Ok(result)
}

fn load_vector_journey(txn: &mut Txn<'_>, header: JourneyHeader) -> Result<JourneyExport> {
    let JourneyData::Vector(vector) = txn.get_journey_data(&header.id)? else {
        return Err(anyhow!("Journey {} is not vector data", header.id));
    };
    Ok(JourneyExport::from_header(header, vector))
}

/// Exports one vector journey at a time. An empty export leaves no file behind.
pub(crate) fn export_all_vector_journeys(
    storage: &Storage,
    target_filepath: &str,
    format: VectorExportFormat,
) -> Result<bool> {
    let mut headers = storage
        .with_db_txn(collect_vector_journey_headers)?
        .into_iter();
    let first = loop {
        let Some(header) = headers.next() else {
            return Ok(false);
        };
        let journey = storage.with_db_txn(|txn| load_vector_journey(txn, header))?;
        if !journey.is_empty() {
            break journey;
        }
    };

    let mut file = BufWriter::new(File::create(target_filepath)?);
    let mut writer = VectorWriter::new(&mut file, format)?;
    writer.write_journey(&first)?;
    for header in headers {
        let journey = storage.with_db_txn(|txn| load_vector_journey(txn, header))?;
        if !journey.is_empty() {
            writer.write_journey(&journey)?;
        }
    }
    writer.finish()?;
    file.flush()?;
    Ok(true)
}
