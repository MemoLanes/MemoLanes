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
//      |     future field bytes
//      |     ...
//
// Metadata lists all section ids. Each SectionHeader lists that section's
// journey headers; the following JourneyData entries appear in the same order.
// Sections currently group journeys by `journey_date` year/month.

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
};

const METADATA_MAGIC_HEADER: [u8; 3] = *b"MLM";
const SECTION_MAGIC_HEADER: [u8; 3] = *b"MLS";
const METADATA_VERSION: u8 = 1;
const METADATA_FILE_NAME_OLD: &str = "metadata.xxm";
const METADATA_FILE_NAME_NEW: &str = "metadata.mldm";
const SECTION_V2_JOURNEY_DATA_FIELD_COUNT: u64 = 1;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SectionVersion {
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

    fn to_u8(self) -> u8 {
        self as u8
    }

    fn metadata_file_name(self) -> &'static str {
        match self {
            SectionVersion::V1 => METADATA_FILE_NAME_OLD,
            SectionVersion::V2 => METADATA_FILE_NAME_NEW,
        }
    }
}

// TODO: support incremetnal archiving by loading the previous metadata, we need
// this for syncing.

// TODO: support archive/export a seleted set of journeys instead of everything.

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
    // we could load the following two lazily, doesn't matter for now tho (because we always need them).
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

fn read_v2_journey_data_bytes<T: Read>(reader: &mut T) -> Result<Vec<u8>> {
    let field_count: u64 = reader.read_varint()?;
    if field_count == 0 {
        bail!("Missing JourneyData field in section v2 journey record");
    }

    let journey_data = read_bytes_with_size_header(reader)?;
    for _ in 1..field_count {
        skip_bytes_with_size_header(reader)?;
    }
    Ok(journey_data)
}

fn skip_v2_journey_record<T: Read>(reader: &mut T) -> Result<()> {
    let field_count: u64 = reader.read_varint()?;
    for _ in 0..field_count {
        skip_bytes_with_size_header(reader)?;
    }
    Ok(())
}

fn read_journey_data_bytes<T: Read>(
    reader: &mut T,
    section_version: SectionVersion,
) -> Result<Vec<u8>> {
    match section_version {
        SectionVersion::V1 => read_bytes_with_size_header(reader),
        SectionVersion::V2 => read_v2_journey_data_bytes(reader),
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

    #[auto_context]
    pub fn load_single_journey(
        &mut self,
        journey_id: &str,
    ) -> Result<Option<(JourneyHeader, JourneyData)>> {
        let section_id = match self.journey_id_to_section_id.get(journey_id) {
            Some(id) => id.clone(),
            None => return Ok(None),
        };
        let mut file = self.zip.by_name(&section_id)?;
        let (section_version, section_header) = Self::read_section_header(&mut file)?;
        for header in section_header.journey_headers {
            if header.id == journey_id {
                let journey_header = JourneyHeader::of_proto(header)?;
                let buf = read_journey_data_bytes(&mut file, section_version)?;
                let journey_data =
                    JourneyData::deserialize(buf.as_slice(), journey_header.journey_type, true)?;
                return Ok(Some((journey_header, journey_data)));
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
                let journey_header = JourneyHeader::of_proto(header)?;

                let ignore = match selected_journey_ids {
                    None => false,
                    Some(set) => !set.contains(&journey_header.id),
                };

                let need_to_import = if ignore {
                    result.ignored_by_filter_count += 1;
                    false
                } else {
                    match txn.get_journey_header(&journey_header.id)? {
                        Some(existing) => {
                            if existing.revision == journey_header.revision {
                                result.skipped_count += 1;
                                false
                            } else {
                                txn.delete_journey(&journey_header.id)?;
                                result.overwritten_count += 1;
                                true
                            }
                        }
                        None => true,
                    }
                };

                if need_to_import {
                    let buf = read_journey_data_bytes(&mut file, section_version)?;
                    let journey_data = JourneyData::deserialize(
                        buf.as_slice(),
                        journey_header.journey_type,
                        true,
                    )?;
                    txn.insert_journey(journey_header, journey_data)?;
                    result.imported_count += 1;
                } else {
                    skip_journey_record(&mut file, section_version)?;
                }
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

fn write_v2_journey_record<T: Write>(writer: &mut T, journey_data: &[u8]) -> Result<()> {
    writer.write_all(&SECTION_V2_JOURNEY_DATA_FIELD_COUNT.encode_var_vec())?;
    write_bytes_with_size_header(writer, journey_data)
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
    section_version: SectionVersion,
) -> Result<()> {
    let journey_headers = txn.query_journeys(None, None)?;
    write_mldx(
        journey_headers,
        |journey_id| txn.get_journey_data(journey_id),
        writer,
        section_version,
    )
}

#[auto_context]
pub fn export_single_journey_as_mldx<T: Write + Seek>(
    journey_header: JourneyHeader,
    journey_data: JourneyData,
    writer: &mut T,
    section_version: SectionVersion,
) -> Result<()> {
    let expected_journey_id = journey_header.id.clone();
    let mut journey_data = Some(journey_data);
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
        writer,
        section_version,
    )
}

fn write_mldx<T, F>(
    journey_headers: Vec<JourneyHeader>,
    mut load_journey_data: F,
    writer: &mut T,
    section_version: SectionVersion,
) -> Result<()>
where
    T: Write + Seek,
    F: FnMut(&str) -> Result<JourneyData>,
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

    // generate section id, which is roughly the hash of the list of
    // journey id + revision
    let mut to_process = Vec::new();
    for (year_month, journeys) in group_by_year_month {
        let section_id: String = {
            let mut hasher = Sha1::new();
            for j in &journeys {
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

    zip.start_file(section_version.metadata_file_name(), default_options)?;
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
        zip.write_all(&[section_version.to_u8()])?;
        // write header
        write_proto_as_compressed_block(&mut zip, section_header)?;

        // write data entries
        for j in journeys {
            // TODO: maybe we want to just take the bytes from db without doing
            // a roundtrip.
            let mut journey_data = load_journey_data(&j.id)?;
            let mut buf = Vec::new();
            journey_data.serialize(&mut buf)?;
            match section_version {
                SectionVersion::V1 => write_bytes_with_size_header(&mut zip, &buf)?,
                SectionVersion::V2 => write_v2_journey_record(&mut zip, &buf)?,
            }
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
    ) -> Result<Option<SectionVersion>> {
        let section_id = match reader.journey_id_to_section_id.get(journey_id) {
            Some(id) => id.clone(),
            None => return Ok(None),
        };
        let mut file = reader.zip.by_name(&section_id)?;
        let (section_version, _) = MldxReader::<R>::read_section_header(&mut file)?;
        Ok(Some(section_version))
    }
}

#[cfg(test)]
mod tests {
    use crate::archive::YearMonth;

    #[test]
    fn order() {
        let ym = |year, month| YearMonth { year, month };
        assert_eq!(ym(2000, 10), ym(2000, 10));
        assert!(ym(2000, 10) > ym(2000, 9));
        assert!(ym(1999, 12) < ym(2000, 9));
    }
}
