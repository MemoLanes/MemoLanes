use std::fs::File;
use std::io::BufWriter;

use anyhow::{anyhow, Result};

use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyHeader, JourneyType};
use crate::raw_data::JourneyRawData;
use crate::storage::Storage;

use super::{
    gpx::GpxJourneyWriter, kml::KmlJourneyWriter, raw_csv::write_raw_data_csv, write_export_file,
    JourneyExport, RawDataExportType, RAW_DATA_LABEL,
};

pub(crate) enum VectorExportFormat {
    Gpx,
    Kml,
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
        |file, journeys| match format {
            VectorExportFormat::Gpx => {
                let mut writer = GpxJourneyWriter::new(file)?;
                for journey in journeys {
                    writer.write_journey(&journey?)?;
                }
                writer.finish()
            }
            VectorExportFormat::Kml => {
                let mut writer = KmlJourneyWriter::new(file)?;
                for journey in journeys {
                    writer.write_journey(&journey?)?;
                }
                writer.finish()
            }
        },
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
        |file, journeys| match format {
            RawDataExportType::CSV => {
                let mut writer = csv::Writer::from_writer(file);
                for journey in journeys {
                    let (_, data) = journey?;
                    write_raw_data_csv(&mut writer, &data)?;
                }
                writer.flush()?;
                Ok(())
            }
            RawDataExportType::GPX => {
                let mut writer = GpxJourneyWriter::new_raw_data(file)?;
                for (index, journey) in journeys.enumerate() {
                    let (header, data) = journey?;
                    writer.write_raw_data(
                        &data,
                        &format!("{RAW_DATA_LABEL} {} #{}", header.journey_date, index + 1),
                    )?;
                }
                writer.finish()
            }
            RawDataExportType::KML => {
                let mut writer = KmlJourneyWriter::new_raw_data(file)?;
                for (index, journey) in journeys.enumerate() {
                    let (header, data) = journey?;
                    writer.write_raw_data(
                        &data,
                        &format!("{RAW_DATA_LABEL} {} #{}", header.journey_date, index + 1),
                    )?;
                }
                writer.finish()
            }
        },
    )
}

/// Shares traversal, empty-export handling and buffering across export formats.
/// Only one journey's decoded points are retained at a time.
fn export_journeys<J>(
    storage: &Storage,
    target_filepath: &str,
    mut load_journey: impl FnMut(JourneyHeader) -> Result<Option<J>>,
    write: impl FnOnce(&mut BufWriter<File>, &mut dyn Iterator<Item = Result<J>>) -> Result<()>,
) -> Result<bool> {
    // Queries return newest first. Export oldest first so raw-data reimports
    // can partition dates and filter GPS points without time going backwards.
    let headers = storage
        .with_db_txn(|txn| txn.query_journeys(None, None, None))?
        .into_iter()
        .rev();
    let mut journeys = headers.filter_map(|header| load_journey(header).transpose());
    let Some(first) = journeys.next().transpose()? else {
        return Ok(false);
    };

    write_export_file(target_filepath, |file| {
        write(file, &mut std::iter::once(Ok(first)).chain(journeys))
    })?;
    Ok(true)
}
