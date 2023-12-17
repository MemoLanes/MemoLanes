use anyhow::Result;
use chrono::{Datelike, Utc};
use hex::ToHex;
use integer_encoding::*;
use protobuf::EnumOrUnknown;
use sha1::{Digest, Sha1};
use std::{
    cmp::Ordering,
    collections::HashMap,
    io::{Seek, Write},
};

use crate::{
    journey_data,
    main_db::MainDb,
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

// TODO: support incremetnal archiving by loading the previous metadata, we need
// this for syncing.

// TODO: support archive/export a seleted set of journeys instead of everything.

// TODO: support recover from archive file. i.e. replace all journeys in the
// main db with journeys in the archive file.

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

pub fn archive_all_as_zip<T: Write + Seek>(main_db: &mut MainDb, writer: &mut T) -> Result<()> {
    let all_journeys = main_db.list_all_journeys()?;

    // group journeys into sections and sort them(by end time and tie
    // break by id, the deterministic ordering is important).
    let mut group_by_year_month = HashMap::new();
    for journey in all_journeys {
        let year_month = YearMonth {
            year: journey.end.year() as i16,
            month: journey.end.month() as u8,
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
        section_info.first_journey_timestamp_sec = journeys.first().unwrap().end.timestamp();
        section_info.last_journey_timestamp_sec = journeys.last().unwrap().end.timestamp();
        section_info.section_id = section_id.clone();
        section_info.num_of_journeys = journeys.len() as u32;
        metadata_proto.section_infos.push(section_info)
    }

    // TODO: pick a file extension
    zip.start_file("metadata.xxm", default_options)?;
    // TODO: pick a magic header
    zip.write_all(&[b'X', b'X', b'M'])?;
    // version num
    zip.write_all(&[1])?;

    // metadata
    write_proto_as_compressed_block(&mut zip, metadata_proto)?;

    // writing section data
    for (_, section_id, journeys) in &to_process {
        let mut section_header = SectionHeader::new();
        section_header.section_id = section_id.clone();
        for j in journeys {
            section_header.journey_headers.push(j.clone().to_proto());
        }

        zip.start_file(section_id.clone(), default_options)?;
        // TODO: pick a magic header
        zip.write_all(&[b'X', b'X', b'S'])?;
        // version num
        zip.write_all(&[1])?;
        // write header
        write_proto_as_compressed_block(&mut zip, section_header)?;

        // write data entries
        for j in journeys {
            // TODO: maybe we want to just take the bytes from db without doing
            // a roundtrip.
            let journey_data = main_db.get_journey(&j.id)?;
            let mut buf = Vec::new();
            journey_data.serialize(&mut buf)?;
            write_bytes_with_size_header(&mut zip, &buf)?;
        }
    }

    zip.finish()?;
    Ok(())
}
