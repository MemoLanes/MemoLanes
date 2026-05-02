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

/* The persistent exchange data format for finalized journeys.
   The high level design is: a metadata file + a set of files each contains a
   data section. Each data section contains a set of journeys for a specific
   year + month of the journey end time in UTC. The idea is that we have
   multiple files so the archive could be incrementally updated/synced. Instead
   of having a journey per file, we do a bit grouping so we don't end up with
   a lot of small files and by using end time as the key, most changes only need
   to update the latest file.
*/

const METADATA_MAGIC_HEADER: [u8; 3] = [b'M', b'L', b'M'];
const SECTION_MAGIC_HEADER: [u8; 3] = [b'M', b'L', b'S'];

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

impl<R: Read + Seek> MldxReader<R> {
    #[auto_context]
    fn read_metadata(zip: &mut zip::ZipArchive<R>) -> Result<Metadata> {
        let mut file = zip.by_name("metadata.xxm")?;
        let mut magic_header: [u8; 3] = [0; 3];
        file.read_exact(&mut magic_header)?;
        if magic_header != METADATA_MAGIC_HEADER {
            bail!(
                "Invalid magic header, expect: {:?}, got: {:?}",
                METADATA_MAGIC_HEADER,
                &magic_header
            );
        };
        let mut version_number: [u8; 1] = [0; 1];
        file.read_exact(&mut version_number)?;

        let len: u64 = file.read_varint()?;
        let mut decoder = zstd::Decoder::new(file.take(len))?;
        let metadata: Metadata = Message::parse_from_reader(&mut decoder)?;
        Ok(metadata)
    }

    #[auto_context]
    fn read_section_header(file: &mut impl Read) -> Result<SectionHeader> {
        let mut magic_header: [u8; 3] = [0; 3];
        file.read_exact(&mut magic_header)?;
        if magic_header != SECTION_MAGIC_HEADER {
            bail!(
                "Invalid magic header, expect: {:?}, got: {:?}",
                SECTION_MAGIC_HEADER,
                &magic_header
            );
        };
        let mut version_number: [u8; 1] = [0; 1];
        file.read_exact(&mut version_number)?;
        let len: u64 = file.read_varint()?;
        let mut decoder = zstd::Decoder::new(file.by_ref().take(len))?;
        let section_header: SectionHeader = Message::parse_from_reader(&mut decoder)?;
        Ok(section_header)
    }

    #[auto_context]
    pub fn open(reader: R) -> Result<Self> {
        let mut zip = zip::ZipArchive::new(reader)?;

        let metadata = Self::read_metadata(&mut zip)?;

        let mut journey_headers = Vec::new();
        let mut journey_id_to_section_id = HashMap::new();
        for section_info in &metadata.section_infos {
            let mut file = zip.by_name(&section_info.section_id)?;
            let section_header = Self::read_section_header(&mut file)?;
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
        let section_header = Self::read_section_header(&mut file)?;
        for header in section_header.journey_headers {
            let data_len: u64 = file.read_varint()?;
            if header.id == journey_id {
                let journey_header = JourneyHeader::of_proto(header)?;
                let mut buf = vec![0_u8; data_len as usize];
                file.read_exact(&mut buf)?;
                let journey_data =
                    JourneyData::deserialize(buf.as_slice(), journey_header.journey_type, true)?;
                return Ok(Some((journey_header, journey_data)));
            } else {
                std::io::copy(&mut file.by_ref().take(data_len), &mut std::io::sink())?;
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
            let section_header = Self::read_section_header(&mut file)?;
            for header in section_header.journey_headers {
                let data_len: u64 = file.read_varint()?;
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
                    let mut buf = vec![0_u8; data_len as usize];
                    file.read_exact(&mut buf)?;
                    let journey_data = JourneyData::deserialize(
                        buf.as_slice(),
                        journey_header.journey_type,
                        true,
                    )?;
                    txn.insert_journey(journey_header, journey_data)?;
                    result.imported_count += 1;
                } else {
                    std::io::copy(&mut file.by_ref().take(data_len), &mut std::io::sink())?;
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

fn write_proto_as_compressed_block<W: Write, M: protobuf::Message>(
    writer: &mut W,
    message: M,
) -> Result<()> {
    // TODO: use streaming to avoid one extra allocation
    let buf = message.write_to_bytes()?;
    let buf = zstd::encode_all(buf.as_slice(), journey_data::ZSTD_COMPRESS_LEVEL)?;
    write_bytes_with_size_header(writer, &buf)
}

pub enum WhatToExport {
    All,
    Just(String),
}

#[auto_context]
pub fn export_as_mldx<T: Write + Seek>(
    what_to_export: &WhatToExport,
    txn: &main_db::Txn,
    writer: &mut T,
) -> Result<()> {
    let journey_to_export = match what_to_export {
        WhatToExport::All => txn.query_journeys(None, None)?,
        WhatToExport::Just(journey_id) => {
            let journey_header = txn
                .get_journey_header(journey_id)?
                .ok_or_else(|| anyhow!("Failed to find journy, journey_id = {journey_id}"))?;
            vec![journey_header]
        }
    };

    // group journeys into sections and sort them(by end time and tie
    // break by id, the deterministic ordering is important).
    let mut group_by_year_month = HashMap::new();
    for journey in journey_to_export {
        let year_month = YearMonth {
            year: journey.journey_date.year() as i16,
            month: journey.journey_date.month() as u8,
        };
        group_by_year_month
            .entry(year_month)
            .or_insert_with(Vec::new)
            .push(journey);
    }
    for (_, journeys) in group_by_year_month.iter_mut() {
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

    zip.start_file("metadata.xxm", default_options)?;
    zip.write_all(&METADATA_MAGIC_HEADER)?;
    // version num
    zip.write_all(&[1])?;

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
        zip.write_all(&[1])?;
        // write header
        write_proto_as_compressed_block(&mut zip, section_header)?;

        // write data entries
        for j in journeys {
            // TODO: maybe we want to just take the bytes from db without doing
            // a roundtrip.
            let mut journey_data = txn.get_journey_data(&j.id)?;
            let mut buf = Vec::new();
            journey_data.serialize(&mut buf)?;
            write_bytes_with_size_header(&mut zip, &buf)?;
        }
    }

    zip.finish()?;
    Ok(())
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
