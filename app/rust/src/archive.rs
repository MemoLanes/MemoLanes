use anyhow::{Ok, Result};
use chrono::{Datelike, Utc};
use hex::ToHex;
use integer_encoding::*;
use protobuf::{EnumOrUnknown, Message};
use sha1::{Digest, Sha1};
use std::{
    cmp::Ordering,
    collections::HashMap,
    fs::File,
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

pub fn recover_archive_file(txn: &mut main_db::Txn, zip_file_path: &str) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(zip_file_path)?)?;
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
    let metadata_proto: Metadata = Message::parse_from_reader(&mut decoder)?;
    drop(decoder);

    txn.clear_journeys()?;
    for section_info in metadata_proto.section_infos {
        let mut file = zip.by_name(&section_info.section_id)?;
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
        drop(decoder);

        for header in section_header.journey_headers {
            let len: u64 = file.read_varint()?;
            let mut buf = vec![0_u8; len as usize];
            file.read_exact(&mut buf)?;

            let journey_header = JourneyHeader::of_proto(header)?;
            let journey_data =
                JourneyData::deserialize(buf.as_slice(), journey_header.journey_type)?;
            txn.insert_journey(journey_header, journey_data)?;
        }
    }
    Ok(())
}

// TODO: support import from archive file. i.e. inserting new journeys from a
// give archive file. If there are journeys with conflicting id, skip them if
// the revision is the same and error if otherwise.

// TODO: support conflict resolvation by asking user what to do.

// TODO: think about whether or not we should have a compact data format for
// exporting a single journey.

// `YearMonth` is the key we used to group jounry into different sections but
// but we don't expose this internal design in things like the data format, so
// we still have the chance to change this in the future.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct YearMonth {
    year: i16,
    month: u8,
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

pub fn archive_all_as_zip<T: Write + Seek>(txn: &main_db::Txn, writer: &mut T) -> Result<()> {
    let all_journeys = txn.list_all_journeys()?;

    // group journeys into sections and sort them(by end time and tie
    // break by id, the deterministic ordering is important).
    let mut group_by_year_month = HashMap::new();
    for journey in all_journeys {
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
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

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

    // TODO: pick a file extension
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
            let journey_data = txn.get_journey(&j.id)?;
            let mut buf = Vec::new();
            journey_data.serialize(&mut buf)?;
            write_bytes_with_size_header(&mut zip, &buf)?;
        }
    }

    zip.finish()?;
    Ok(())
}
