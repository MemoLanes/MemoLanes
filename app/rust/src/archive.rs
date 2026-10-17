// MemoLanes archive files (`.mldx`) are ZIP containers with one metadata entry
// and one entry per journey section.
//
//      archive.mldx
//      |
//      +-- metadata.xxm or metadata.mldm
//      |     "MLM"
//      |     version: u8
//      |     length of next block: varint
//      |     Metadata protobuf, zstd-compressed
//      |
//      +-- <section_id>
//      |     "MLS"
//      |     version: u8
//      |     length of next block: varint
//      |     SectionHeader protobuf, zstd-compressed
//      |     ...
//
// Section v1 stores one length-prefixed JourneyData blob per journey:
//
//      |     journey data length: varint
//      |     JourneyData bytes
//      |     ...
//
// Section v2 stores one field-count record per journey:
//
//      |     field count: varint
//      |     field 1 length: varint
//      |     JourneyData bytes
//      |     field 2 length: varint
//      |     optional SerializedJourneyRawData bytes
//      |     field 3 length: varint
//      |     future field bytes
//      |     ...
//
// Metadata lists all section ids. Each SectionHeader lists that section's
// journey headers; the following JourneyData entries appear in the same order.
// Sections currently group journeys by `journey_date` year/month.
// Exports always use metadata.mldm and section v2; section v1 is read-only.

use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use chrono::{Datelike, Utc};
use flutter_rust_bridge::frb;
use hex::ToHex;
use integer_encoding::*;
use protobuf::{EnumOrUnknown, Message};
use sha1::{Digest, Sha1};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    io::{Read, Seek, Write},
};

use crate::{
    journey_data::{self, JourneyData},
    journey_header::JourneyHeader,
    main_db,
    protos::archive::{metadata, Metadata, SectionHeader},
    raw_data::SerializedJourneyRawData,
};

const METADATA_MAGIC_HEADER: [u8; 3] = *b"MLM";
const SECTION_MAGIC_HEADER: [u8; 3] = *b"MLS";
const METADATA_VERSION: u8 = 1;
const METADATA_FILE_NAME_OLD: &str = "metadata.xxm";
const METADATA_FILE_NAME_NEW: &str = "metadata.mldm";

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
enum SectionVersion {
    V1 = 1,
    V2 = 2,
}

impl SectionVersion {
    fn of_u8(version: u8) -> Result<Self> {
        match version {
            1 => Ok(SectionVersion::V1),
            2 => Ok(SectionVersion::V2),
            _ => bail!("Unsupported section version: {version}"),
        }
    }
}

// TODO: support incremental archiving by loading the previous metadata, we need
// this for syncing.

// TODO: support archive/export for a selected set of journeys instead of everything.

#[derive(Clone, Debug, PartialEq)]
#[frb]
pub struct MldxImportResult {
    pub imported_count: u32,
    pub skipped_count: u32,
    pub overwritten_count: u32,
    pub ignored_by_filter_count: u32,
}

pub struct MldxReader<R: Read + Seek> {
    zip: zip::ZipArchive<R>,
    metadata: Metadata,
    // we could load the following two lazily, doesn't matter for now though (because we always need them).
    journey_headers: Vec<JourneyHeader>,
    journey_id_to_section_id: HashMap<String, String>,
}

fn read_bytes_with_size_header<T: Read>(reader: &mut T) -> Result<Vec<u8>> {
    let len: u64 = reader.read_varint()?;
    let len = usize::try_from(len)?;
    let mut buf = vec![0_u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

fn skip_bytes_with_size_header<T: Read>(reader: &mut T) -> Result<()> {
    let len: u64 = reader.read_varint()?;
    let copied = std::io::copy(&mut reader.by_ref().take(len), &mut std::io::sink())?;
    if copied != len {
        bail!("Unexpected EOF while skipping {len} bytes, skipped {copied}");
    }
    Ok(())
}

fn read_journey_field_count<T: Read>(
    reader: &mut T,
    section_version: SectionVersion,
) -> Result<u64> {
    let field_count: u64 = match section_version {
        SectionVersion::V1 => 1,
        SectionVersion::V2 => reader.read_varint()?,
    };
    if field_count == 0 {
        bail!("Missing JourneyData field in section v2 journey record");
    }
    Ok(field_count)
}

/// Read the processed data field and return the number of attachment/extension
/// fields that follow it. Preview callers can stop here without reading them.
fn read_journey_data_field<T: Read>(
    reader: &mut T,
    section_version: SectionVersion,
) -> Result<(Vec<u8>, u64)> {
    let field_count = read_journey_field_count(reader, section_version)?;
    Ok((read_bytes_with_size_header(reader)?, field_count - 1))
}

fn read_journey_attachment_fields<T: Read>(
    reader: &mut T,
    remaining_fields: u64,
) -> Result<Option<SerializedJourneyRawData>> {
    let raw_data = if remaining_fields >= 1 {
        let raw_data = SerializedJourneyRawData::from_bytes(read_bytes_with_size_header(reader)?);
        // Archive input is untrusted. Validate before it can reach the database.
        raw_data.deserialize()?;
        Some(raw_data)
    } else {
        None
    };
    for _ in 1..remaining_fields {
        skip_bytes_with_size_header(reader)?;
    }
    Ok(raw_data)
}

fn skip_v2_journey_record<T: Read>(reader: &mut T) -> Result<()> {
    let field_count: u64 = reader.read_varint()?;
    for _ in 0..field_count {
        skip_bytes_with_size_header(reader)?;
    }
    Ok(())
}

/// A section stream positioned at one journey record. Reading consumes the
/// handle so callers cannot try to read the same record twice.
struct JourneyRecordReader<'a, R: Read> {
    file: zip::read::ZipFile<'a, R>,
    section_version: SectionVersion,
    header: JourneyHeader,
}

impl<R: Read> JourneyRecordReader<'_, R> {
    fn read_track(&mut self) -> Result<(JourneyData, u64)> {
        let (bytes, remaining_fields) =
            read_journey_data_field(&mut self.file, self.section_version)?;
        self.header
            .correct_has_raw_data(remaining_fields > 0, "load_mldx_journey");
        let data = JourneyData::deserialize(bytes.as_slice(), self.header.journey_type, true)?;
        Ok((data, remaining_fields))
    }

    fn read_preview(mut self) -> Result<(JourneyHeader, JourneyData)> {
        let (data, _) = self.read_track()?;
        Ok((self.header, data))
    }

    fn read_with_raw_data(
        mut self,
    ) -> Result<(JourneyHeader, JourneyData, Option<SerializedJourneyRawData>)> {
        let (data, remaining_fields) = self.read_track()?;
        let raw_data = read_journey_attachment_fields(&mut self.file, remaining_fields)?;
        Ok((self.header, data, raw_data))
    }
}

fn skip_journey_record<T: Read>(reader: &mut T, section_version: SectionVersion) -> Result<()> {
    match section_version {
        SectionVersion::V1 => skip_bytes_with_size_header(reader),
        SectionVersion::V2 => skip_v2_journey_record(reader),
    }
}

impl<R: Read + Seek> MldxReader<R> {
    #[auto_context]
    fn read_metadata(zip: &mut zip::ZipArchive<R>) -> Result<Metadata> {
        let metadata_file_index = zip
            .index_for_name(METADATA_FILE_NAME_NEW)
            .or_else(|| zip.index_for_name(METADATA_FILE_NAME_OLD))
            .ok_or_else(|| {
                anyhow!(
                    "Missing metadata file ({} or {})",
                    METADATA_FILE_NAME_NEW,
                    METADATA_FILE_NAME_OLD
                )
            })?;
        let mut file = zip.by_index(metadata_file_index)?;
        let mut magic_header: [u8; 3] = [0; 3];
        file.read_exact(&mut magic_header)?;
        if magic_header != METADATA_MAGIC_HEADER {
            bail!(
                "Invalid magic header, expect: {:?}, got: {:?}",
                METADATA_MAGIC_HEADER,
                magic_header
            );
        };
        let mut version_number: [u8; 1] = [0; 1];
        file.read_exact(&mut version_number)?;
        if version_number[0] != METADATA_VERSION {
            bail!(
                "Unsupported metadata version: {}, expected: {}",
                version_number[0],
                METADATA_VERSION
            );
        }

        let len: u64 = file.read_varint()?;
        let mut decoder = zstd::Decoder::new(file.take(len))?;
        let metadata: Metadata = Message::parse_from_reader(&mut decoder)?;
        Ok(metadata)
    }

    #[auto_context]
    fn read_section_header(file: &mut impl Read) -> Result<(SectionVersion, SectionHeader)> {
        let mut magic_header: [u8; 3] = [0; 3];
        file.read_exact(&mut magic_header)?;
        if magic_header != SECTION_MAGIC_HEADER {
            bail!(
                "Invalid magic header, expect: {:?}, got: {:?}",
                SECTION_MAGIC_HEADER,
                magic_header
            );
        };
        let mut version_number: [u8; 1] = [0; 1];
        file.read_exact(&mut version_number)?;
        let section_version = SectionVersion::of_u8(version_number[0])?;

        let len: u64 = file.read_varint()?;
        let mut decoder = zstd::Decoder::new(file.by_ref().take(len))?;
        let section_header: SectionHeader = Message::parse_from_reader(&mut decoder)?;
        Ok((section_version, section_header))
    }

    #[auto_context]
    pub fn open(reader: R) -> Result<Self> {
        let mut zip = zip::ZipArchive::new(reader)?;

        let metadata = Self::read_metadata(&mut zip)?;

        let mut journey_headers = Vec::new();
        let mut journey_id_to_section_id = HashMap::new();
        for section_info in &metadata.section_infos {
            let mut file = zip.by_name(&section_info.section_id)?;
            let (_, section_header) = Self::read_section_header(&mut file)?;
            drop(file);
            for header in section_header.journey_headers {
                let journey_header = JourneyHeader::of_proto(header)?;
                journey_id_to_section_id
                    .insert(journey_header.id.clone(), section_info.section_id.clone());
                journey_headers.push(journey_header);
            }
        }

        Ok(Self {
            zip,
            metadata,
            journey_headers,
            journey_id_to_section_id,
        })
    }

    #[auto_context]
    pub fn iter_journey_headers(&self) -> &[JourneyHeader] {
        &self.journey_headers
    }

    /// Read the processed track and attachment presence for a preview. Raw data
    /// and later extension fields are neither read nor validated. Full reads
    /// and imports validate attachments before returning or storing them.
    #[auto_context]
    pub fn load_single_journey(
        &mut self,
        journey_id: &str,
    ) -> Result<Option<(JourneyHeader, JourneyData)>> {
        self.open_journey_record(journey_id)?
            .map(JourneyRecordReader::read_preview)
            .transpose()
    }

    #[auto_context]
    pub fn load_single_journey_with_raw_data(
        &mut self,
        journey_id: &str,
    ) -> Result<Option<(JourneyHeader, JourneyData, Option<SerializedJourneyRawData>)>> {
        self.open_journey_record(journey_id)?
            .map(JourneyRecordReader::read_with_raw_data)
            .transpose()
    }

    fn open_journey_record(
        &mut self,
        journey_id: &str,
    ) -> Result<Option<JourneyRecordReader<'_, R>>> {
        let section_id = match self.journey_id_to_section_id.get(journey_id) {
            Some(id) => id.clone(),
            None => return Ok(None),
        };
        let mut file = self.zip.by_name(&section_id)?;
        let (section_version, section_header) = Self::read_section_header(&mut file)?;
        for header in section_header.journey_headers {
            if header.id == journey_id {
                return Ok(Some(JourneyRecordReader {
                    file,
                    section_version,
                    header: JourneyHeader::of_proto(header)?,
                }));
            } else {
                skip_journey_record(&mut file, section_version)?;
            }
        }
        Ok(None)
    }

    #[auto_context]
    pub fn import(
        &mut self,
        txn: &mut main_db::Txn,
        selected_journey_ids: Option<&HashSet<String>>,
    ) -> Result<MldxImportResult> {
        let mut result = MldxImportResult {
            imported_count: 0,
            skipped_count: 0,
            overwritten_count: 0,
            ignored_by_filter_count: 0,
        };
        for section_id in self.metadata.section_infos.iter().map(|s| &s.section_id) {
            let mut file = self.zip.by_name(section_id)?;
            let (section_version, section_header) = Self::read_section_header(&mut file)?;
            for header in section_header.journey_headers {
                let mut journey_header = JourneyHeader::of_proto(header)?;

                let ignore = match selected_journey_ids {
                    None => false,
                    Some(set) => !set.contains(&journey_header.id),
                };
                if ignore {
                    result.ignored_by_filter_count += 1;
                    skip_journey_record(&mut file, section_version)?;
                    continue;
                }

                // The archive header may disagree with its record. Normalize
                // before comparing revisions, including on repeated imports.
                let field_count = read_journey_field_count(&mut file, section_version)?;
                journey_header.correct_has_raw_data(field_count > 1, "import_mldx");
                let existing = txn.get_journey_header(&journey_header.id)?;
                if existing
                    .as_ref()
                    .is_some_and(|existing| existing.revision == journey_header.revision)
                {
                    for _ in 0..field_count {
                        skip_bytes_with_size_header(&mut file)?;
                    }
                    result.skipped_count += 1;
                    continue;
                }

                let journey_data = read_bytes_with_size_header(&mut file)?;
                let raw_data = read_journey_attachment_fields(&mut file, field_count - 1)?;

                if existing.is_some() {
                    txn.delete_journey(&journey_header.id)?;
                    result.overwritten_count += 1;
                }
                let journey_data = JourneyData::deserialize(
                    journey_data.as_slice(),
                    journey_header.journey_type,
                    true,
                )?;
                txn.insert_journey_with_raw_data(journey_header, journey_data, raw_data)?;
                result.imported_count += 1;
            }
        }

        Ok(result)
    }
}

// TODO: think about whether or not we should have a compact data format for
// exporting a single journey.

// `YearMonth` is the key we used to group journey into different sections but
// but we don't expose this internal design in things like the data format, so
// we still have the chance to change this in the future.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct YearMonth {
    year: i16,
    month: u8,
}

fn write_bytes_with_size_header<T: Write>(writer: &mut T, buf: &[u8]) -> Result<()> {
    writer.write_all(&(buf.len() as u64).encode_var_vec())?;
    writer.write_all(buf)?;
    Ok(())
}

fn write_v2_journey_record<T: Write>(
    writer: &mut T,
    journey_data: &[u8],
    raw_data: Option<&SerializedJourneyRawData>,
) -> Result<()> {
    let field_count = if raw_data.is_some() { 2_u64 } else { 1_u64 };
    writer.write_all(&field_count.encode_var_vec())?;
    write_bytes_with_size_header(writer, journey_data)?;
    if let Some(raw_data) = raw_data {
        write_bytes_with_size_header(writer, raw_data.as_bytes())?;
    }
    Ok(())
}

fn write_proto_as_compressed_block<W: Write, M: protobuf::Message>(
    writer: &mut W,
    message: M,
) -> Result<()> {
    // TODO: use streaming to avoid one extra allocation
    let buf = message.write_to_bytes()?;
    let buf = zstd::encode_all(buf.as_slice(), journey_data::ZSTD_COMPRESS_LEVEL)?;
    write_bytes_with_size_header(writer, &buf)
}

#[auto_context]
pub fn export_all_journeys_as_mldx<T: Write + Seek>(
    txn: &main_db::Txn,
    writer: &mut T,
    include_raw_data: bool,
) -> Result<()> {
    let journey_headers = txn.query_journeys(None, None, None)?;
    write_mldx(
        journey_headers,
        |journey_id| txn.get_journey_data(journey_id),
        |journey_id| txn.get_journey_raw_data(journey_id),
        writer,
        include_raw_data,
    )
}

#[auto_context]
pub fn export_single_journey_as_mldx<T: Write + Seek>(
    journey_header: JourneyHeader,
    journey_data: JourneyData,
    raw_data: Option<SerializedJourneyRawData>,
    writer: &mut T,
    include_raw_data: bool,
) -> Result<()> {
    let expected_journey_id = journey_header.id.clone();
    let expected_raw_data_journey_id = expected_journey_id.clone();
    let mut journey_data = Some(journey_data);
    let mut raw_data = raw_data;
    write_mldx(
        vec![journey_header],
        |journey_id| {
            if journey_id != expected_journey_id {
                bail!(
                    "Unexpected journey id, expected: {}, got: {}",
                    expected_journey_id,
                    journey_id
                );
            }
            journey_data
                .take()
                .ok_or_else(|| anyhow!("Journey data has already been written"))
        },
        |journey_id| {
            if journey_id != expected_raw_data_journey_id {
                bail!(
                    "Unexpected journey id, expected: {}, got: {}",
                    expected_raw_data_journey_id,
                    journey_id
                );
            }
            Ok(raw_data.take())
        },
        writer,
        include_raw_data,
    )
}

// Preserve source headers when including raw data. Readers reconcile attachment
// presence from the record itself, so incorrect flags cannot omit actual data.
fn write_mldx<T, F, G>(
    journey_headers: Vec<JourneyHeader>,
    mut load_journey_data: F,
    mut load_raw_data: G,
    writer: &mut T,
    include_raw_data: bool,
) -> Result<()>
where
    T: Write + Seek,
    F: FnMut(&str) -> Result<JourneyData>,
    G: FnMut(&str) -> Result<Option<SerializedJourneyRawData>>,
{
    // group journeys into sections and sort them(by end time and tie
    // break by id, the deterministic ordering is important).
    let mut group_by_year_month = HashMap::new();
    for journey in journey_headers {
        let year_month = YearMonth {
            year: journey.journey_date.year() as i16,
            month: journey.journey_date.month() as u8,
        };
        group_by_year_month
            .entry(year_month)
            .or_insert_with(Vec::new)
            .push(journey);
    }
    for journeys in group_by_year_month.values_mut() {
        journeys.sort_by(|a, b| {
            let result = a.end.cmp(&b.end);
            if result != Ordering::Equal {
                result
            } else {
                a.id.cmp(&b.id)
            }
        })
    }

    // Use the exported id + revision pairs, matching journey equality on import.
    let mut to_process = Vec::new();
    for (year_month, mut journeys) in group_by_year_month {
        let section_id: String = {
            let mut hasher = Sha1::new();
            for j in &mut journeys {
                if !include_raw_data {
                    // Only the exported header matters when attachments are omitted.
                    j.remove_raw_data();
                }
                hasher.update(format!("[{}|{}]", j.id, j.revision));
            }
            let result = hasher.finalize();
            result.encode_hex::<String>()
        };
        to_process.push((year_month, section_id, journeys));
    }
    to_process.sort_by_key(|x| x.0);

    // start writing files
    let mut zip = zip::ZipWriter::new(writer);
    // we already compress data inside the file, do no need to do it in zip.
    let default_options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // writing metadata
    let mut metadata_proto = Metadata::new();
    metadata_proto.created_at_timestamp_sec = Utc::now().timestamp();
    metadata_proto.kind = Some(EnumOrUnknown::new(metadata::Kind::FULL_ARCHIVE));
    metadata_proto.note = None;
    for (_, section_id, journeys) in &to_process {
        let mut section_info = metadata::SectionInfo::new();
        section_info.section_id.clone_from(section_id);
        section_info.num_of_journeys = journeys.len() as u32;
        metadata_proto.section_infos.push(section_info)
    }

    zip.start_file(METADATA_FILE_NAME_NEW, default_options)?;
    zip.write_all(&METADATA_MAGIC_HEADER)?;
    // version num
    zip.write_all(&[METADATA_VERSION])?;

    // metadata
    write_proto_as_compressed_block(&mut zip, metadata_proto)?;

    // writing section data
    for (_, section_id, journeys) in &to_process {
        let mut section_header = SectionHeader::new();
        section_header.section_id.clone_from(section_id);
        for j in journeys {
            section_header.journey_headers.push(j.clone().to_proto());
        }

        zip.start_file(section_id.clone(), default_options)?;
        zip.write_all(&SECTION_MAGIC_HEADER)?;
        // version num
        zip.write_all(&[SectionVersion::V2 as u8])?;
        // write header
        write_proto_as_compressed_block(&mut zip, section_header)?;

        // write data entries
        for j in journeys {
            // TODO: maybe we want to just take the bytes from db without doing
            // a roundtrip.
            let mut journey_data = load_journey_data(&j.id)?;
            let mut buf = Vec::new();
            journey_data.serialize(&mut buf)?;
            let raw_data = if include_raw_data {
                load_raw_data(&j.id)?
            } else {
                None
            };
            write_v2_journey_record(&mut zip, &buf, raw_data.as_ref())?;
        }
    }

    zip.finish()?;
    Ok(())
}

#[doc(hidden)]
pub mod for_testing {
    use super::*;

    #[auto_context]
    pub fn section_version_for_journey<R: Read + Seek>(
        reader: &mut MldxReader<R>,
        journey_id: &str,
    ) -> Result<Option<u8>> {
        let section_id = match reader.journey_id_to_section_id.get(journey_id) {
            Some(id) => id.clone(),
            None => return Ok(None),
        };
        let mut file = reader.zip.by_name(&section_id)?;
        let (section_version, _) = MldxReader::<R>::read_section_header(&mut file)?;
        Ok(Some(section_version as u8))
    }
}
