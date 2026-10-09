use std::fs::File;
use std::io::{BufWriter, Write};

use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, Utc};

use crate::api::api::RawDataExportType;
use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyHeader, JourneyType};
use crate::journey_vector::JourneyVector;
use crate::raw_data::JourneyRawData;
use crate::storage::Storage;

use super::{csv::write_raw_data_csv, gpx::GpxJourneyWriter, kml::KmlJourneyWriter};

pub(crate) enum VectorExportFormat {
    Gpx,
    Kml,
}

enum VectorWriter<W: Write> {
    Gpx(GpxJourneyWriter<W>),
    Kml(KmlJourneyWriter<W>),
}

trait JourneyFileWriter {
    type Journey;

    fn write_journey(&mut self, index: usize, journey: &Self::Journey) -> Result<()>;
    fn finish(self) -> Result<()>;
}

impl<W: Write> VectorWriter<W> {
    fn new(writer: W, format: VectorExportFormat) -> Result<Self> {
        match format {
            VectorExportFormat::Gpx => Ok(Self::Gpx(GpxJourneyWriter::new(writer)?)),
            VectorExportFormat::Kml => Ok(Self::Kml(KmlJourneyWriter::new(writer)?)),
        }
    }
}

impl<W: Write> JourneyFileWriter for VectorWriter<W> {
    type Journey = JourneyExport;

    fn write_journey(&mut self, _index: usize, journey: &JourneyExport) -> Result<()> {
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

enum RawDataWriter<W: Write> {
    Csv(Box<csv::Writer<W>>),
    Gpx(GpxJourneyWriter<W>),
    Kml(KmlJourneyWriter<W>),
}

impl<W: Write> RawDataWriter<W> {
    fn new(writer: W, format: RawDataExportType) -> Result<Self> {
        match format {
            RawDataExportType::CSV => Ok(Self::Csv(Box::new(csv::Writer::from_writer(writer)))),
            RawDataExportType::GPX => Ok(Self::Gpx(GpxJourneyWriter::new_raw_data(writer)?)),
            RawDataExportType::KML => Ok(Self::Kml(KmlJourneyWriter::new_raw_data(writer)?)),
        }
    }
}

impl<W: Write> JourneyFileWriter for RawDataWriter<W> {
    type Journey = (JourneyHeader, JourneyRawData);

    fn write_journey(&mut self, index: usize, (header, data): &Self::Journey) -> Result<()> {
        let name = format!("MemoLanes Raw Data {} #{index}", header.journey_date);
        match self {
            Self::Csv(writer) => write_raw_data_csv(writer, data, Some(&header.id)),
            Self::Gpx(writer) => writer.write_raw_data(data, &name),
            Self::Kml(writer) => writer.write_raw_data(data, &name),
        }
    }

    fn finish(self) -> Result<()> {
        match self {
            Self::Csv(mut writer) => {
                writer.flush()?;
                Ok(())
            }
            Self::Gpx(writer) => writer.finish(),
            Self::Kml(writer) => writer.finish(),
        }
    }
}

/// The geometry and header metadata that can be represented by GPX/KML.
/// MLDX remains the complete backup format.
#[derive(Clone, Debug)]
pub(crate) struct JourneyExport {
    pub journey_date: NaiveDate,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub vector: JourneyVector,
}

impl JourneyExport {
    pub(crate) fn from_header(header: JourneyHeader, vector: JourneyVector) -> Self {
        Self {
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

/// Exports one vector journey at a time. An empty export leaves no file behind.
pub(crate) fn export_all_vector_journeys(
    storage: &Storage,
    target_filepath: &str,
    format: VectorExportFormat,
) -> Result<bool> {
    export_journeys(
        storage,
        target_filepath,
        |header| {
            if header.journey_type != JourneyType::Vector {
                return Ok(None);
            }
            let JourneyData::Vector(vector) =
                storage.with_db_txn(|txn| txn.get_journey_data(&header.id))?
            else {
                return Err(anyhow!("Journey {} is not vector data", header.id));
            };
            let journey = JourneyExport::from_header(header, vector);
            Ok((!journey.is_empty()).then_some(journey))
        },
        |writer| VectorWriter::new(writer, format),
    )
}

pub(crate) fn load_journey_raw_data(
    storage: &Storage,
    journey_id: &str,
) -> Result<Option<JourneyRawData>> {
    let serialized = storage.with_db_txn(|txn| txn.get_journey_raw_data(journey_id))?;
    // Release the database lock before decompressing and decoding points.
    Ok(serialized
        .map(|data| data.deserialize())
        .transpose()?
        .filter(|data| !data.is_empty()))
}

pub(crate) fn export_all_raw_journeys(
    storage: &Storage,
    target_filepath: &str,
    format: RawDataExportType,
) -> Result<bool> {
    export_journeys(
        storage,
        target_filepath,
        |header| Ok(load_journey_raw_data(storage, &header.id)?.map(|data| (header, data))),
        |writer| RawDataWriter::new(writer, format),
    )
}

/// Shares traversal, empty-export handling and buffering across export formats.
/// Only one journey's decoded points are retained at a time.
fn export_journeys<W: JourneyFileWriter>(
    storage: &Storage,
    target_filepath: &str,
    mut load_journey: impl FnMut(JourneyHeader) -> Result<Option<W::Journey>>,
    new_writer: impl FnOnce(BufWriter<File>) -> Result<W>,
) -> Result<bool> {
    let mut headers = storage
        .with_db_txn(|txn| txn.query_journeys(None, None, None))?
        .into_iter();
    let first = loop {
        let Some(header) = headers.next() else {
            return Ok(false);
        };
        if let Some(journey) = load_journey(header)? {
            break journey;
        }
    };

    let file = BufWriter::new(File::create(target_filepath)?);
    let result = (|| {
        let mut writer = new_writer(file)?;
        writer.write_journey(1, &first)?;
        drop(first);
        let mut index = 1;
        for header in headers {
            if let Some(journey) = load_journey(header)? {
                index += 1;
                writer.write_journey(index, &journey)?;
            }
        }
        writer.finish()
    })();
    if result.is_err() {
        // A partial document must not be offered as a successful export.
        let _ = std::fs::remove_file(target_filepath);
    }
    result?;
    Ok(true)
}
