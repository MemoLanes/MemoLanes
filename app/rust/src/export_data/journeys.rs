use std::fs::File;
use std::io::BufWriter;

use anyhow::{anyhow, Result};

use crate::journey_data::JourneyData;
use crate::journey_header::JourneyType;
use crate::main_db::Txn;
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
        |txn, journey_id| {
            let Some(header) = txn.get_journey_header(journey_id)? else {
                return Ok(None);
            };
            if header.journey_type != JourneyType::Vector {
                return Ok(None);
            }
            let JourneyData::Vector(vector) = txn.get_journey_data(journey_id)? else {
                return Err(anyhow!("Journey {journey_id} is not vector data"));
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
        |txn, journey_id| {
            let Some(header) = txn.get_journey_header(journey_id)? else {
                return Ok(None);
            };
            let Some(serialized) = txn.get_journey_raw_data(journey_id)? else {
                return Ok(None);
            };
            let data = serialized.deserialize()?;
            Ok((!data.is_empty()).then_some((header, data)))
        },
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

/// Streams a consistent snapshot, retaining only one journey's decoded points.
fn export_journeys<J>(
    storage: &Storage,
    target_filepath: &str,
    mut load_journey: impl FnMut(&Txn, &str) -> Result<Option<J>>,
    write: impl FnOnce(&mut BufWriter<File>, &mut dyn Iterator<Item = Result<J>>) -> Result<()>,
) -> Result<bool> {
    // TODO: Use a dedicated read-only connection and a WAL read transaction to
    // keep a consistent snapshot without holding Storage's mutex during export.
    storage.with_db_txn(|txn| {
        let ids = txn.query_journey_ids_for_export()?.into_iter();
        let mut journeys = ids.filter_map(|id| load_journey(txn, &id).transpose());
        let Some(first) = journeys.next().transpose()? else {
            return Ok(false);
        };

        write_export_file(target_filepath, |file| {
            write(file, &mut std::iter::once(Ok(first)).chain(journeys))
        })?;
        Ok(true)
    })
}
