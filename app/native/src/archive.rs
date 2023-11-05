use std::{
    cmp::Ordering,
    collections::HashMap,
    io::{Seek, Write},
};

use anyhow::Result;
use chrono::{Datelike, Utc};
use hex::ToHex;
use protobuf::{EnumOrUnknown, Message};
use protos::archive::Metadata;
use sha1::{Digest, Sha1};

use crate::{
    main_db::{JourneyType, MainDb},
    protos::{
        self,
        archive::{metadata, section_header::journey_info, SectionDataEntry, SectionHeader},
    },
};

// TODO: maybe we want a higher one for archive
// 3 is the zstd default
const ZSTD_COMPRESS_LEVEL: i32 = 3;

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
        assert_eq!(ym(2000, 10) > ym(2000, 9), true);
        assert_eq!(ym(1999, 12) < ym(2000, 9), true);
    }
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
            .or_insert_with(|| Vec::new())
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

    let metadata_bytes = zstd::encode_all(
        metadata_proto.write_to_bytes()?.as_slice(),
        ZSTD_COMPRESS_LEVEL,
    )?;

    // TODO: pick a file extension
    zip.start_file("metadata.xxm", default_options)?;
    // TODO: pick a magic header
    zip.write(&['X' as u8, 'X' as u8, 'M' as u8])?;
    // version num
    zip.write(&[1])?;
    // metadata size
    zip.write(&(metadata_bytes.len() as u32).to_be_bytes())?;
    // content
    zip.write(&metadata_bytes)?;

    // writing section data
    for (_, section_id, journeys) in &to_process {
        let mut section_header = SectionHeader::new();
        section_header.section_id = section_id.clone();
        for j in journeys {
            let mut journey_info = protos::archive::section_header::JourneyInfo::new();
            journey_info.type_ = EnumOrUnknown::new(match j.journey_type {
                JourneyType::Bitmap => journey_info::Type::BITMAP,
                JourneyType::Track => journey_info::Type::TRACK,
            });
            // TODO: we could avoid this `clone`
            journey_info.header.0 = Some(Box::new(j.clone().to_proto()));
            section_header.journey_info.push(journey_info);
        }
        let section_header_bytes = zstd::encode_all(
            section_header.write_to_bytes()?.as_slice(),
            ZSTD_COMPRESS_LEVEL,
        )?;

        zip.start_file(section_id.clone(), default_options)?;
        // TODO: pick a magic header
        zip.write(&['X' as u8, 'X' as u8, 'S' as u8])?;
        // version num
        zip.write(&[1])?;
        // write header
        zip.write(&(section_header_bytes.len() as u32).to_be_bytes())?;
        zip.write(&section_header_bytes)?;

        // write data entries
        for j in journeys {
            let journey_data = main_db.get_journey(&j.id)?;
            let mut data_entry = SectionDataEntry::new();
            data_entry.data.0 = Some(Box::new(journey_data));

            let data_entry_bytes =
                zstd::encode_all(data_entry.write_to_bytes()?.as_slice(), ZSTD_COMPRESS_LEVEL)?;

            zip.write(&(data_entry_bytes.len() as u32).to_be_bytes())?;
            zip.write(&data_entry_bytes)?;
        }
    }

    zip.finish()?;
    Ok(())
}
