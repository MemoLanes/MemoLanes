use std::io::{Read, Write};

pub use crate::journey_bitmap::{deserialize_journey_bitmap, serialize_journey_bitmap};

use crate::{
    journey_bitmap::{BitmapEditor, JourneyBitmap},
    journey_header::JourneyType,
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    utils,
};
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use flutter_rust_bridge::frb;
use integer_encoding::*;

#[derive(Debug, PartialEq, Clone)]
#[frb(ignore)]
pub enum JourneyData {
    Vector(JourneyVector),
    Bitmap(JourneyBitmap),
}

// TODO: maybe we want a higher one for archive
// 3 is the zstd default
pub const ZSTD_COMPRESS_LEVEL: i32 = 3;

const JOURNEY_VECTOR_MAGIC_HEADER: [u8; 2] = *b"V0";
// TODO: I don't have a strong reason on putting all serializations here
#[auto_context]
pub fn serialize_journey_vector<T: Write>(
    journey_vector: &JourneyVector,
    mut writer: T,
) -> Result<()> {
    // magic header
    writer.write_all(&JOURNEY_VECTOR_MAGIC_HEADER)?;

    // data is compressed as a whole
    let mut encoder = zstd::Encoder::new(writer, ZSTD_COMPRESS_LEVEL)?.auto_finish();
    encoder.write_all(&(journey_vector.track_segments.len() as u64).encode_var_vec())?;
    for track_segmant in &journey_vector.track_segments {
        encoder.write_all(&(track_segmant.track_points.len() as u64).encode_var_vec())?;
        for track_point in &track_segmant.track_points {
            encoder.write_all(&track_point.latitude.to_be_bytes())?;
            encoder.write_all(&track_point.longitude.to_be_bytes())?;
        }
    }
    Ok(())
}

#[auto_context]
pub fn deserialize_journey_vector<T: Read>(mut reader: T) -> Result<JourneyVector> {
    utils::validate_magic_header(&mut reader, &JOURNEY_VECTOR_MAGIC_HEADER)?;

    // data is compressed as a whole
    let mut decoder = zstd::Decoder::new(reader)?;
    let segments_count: u64 = decoder.read_varint()?;
    let mut track_segments = Vec::with_capacity(segments_count as usize);
    for _ in 0..segments_count {
        let points_count: u64 = decoder.read_varint()?;
        let mut track_points = Vec::with_capacity(points_count as usize);
        for _ in 0..points_count {
            let mut buf: [u8; 8] = [0; 8];
            decoder.read_exact(&mut buf)?;
            let latitude = f64::from_be_bytes(buf);
            decoder.read_exact(&mut buf)?;
            let longitude = f64::from_be_bytes(buf);
            track_points.push(TrackPoint {
                latitude,
                longitude,
            })
        }
        track_segments.push(TrackSegment { track_points });
    }
    Ok(JourneyVector { track_segments })
}

impl JourneyData {
    pub fn merge_into(self, editor: &mut BitmapEditor<'_>) {
        match self {
            JourneyData::Bitmap(b) => editor.merge(b),
            JourneyData::Vector(v) => editor.merge_vector(&v),
        }
    }

    pub fn merge_into_with_partial_clone(&self, editor: &mut BitmapEditor<'_>) {
        match self {
            JourneyData::Bitmap(b) => editor.merge_with_partial_clone(b),
            JourneyData::Vector(v) => editor.merge_vector(v),
        }
    }

    pub fn type_(&self) -> JourneyType {
        match self {
            JourneyData::Vector(_) => JourneyType::Vector,
            JourneyData::Bitmap(_) => JourneyType::Bitmap,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            JourneyData::Vector(vector) => vector
                .track_segments
                .iter()
                .all(|segment| segment.track_points.is_empty()),
            JourneyData::Bitmap(bitmap) => bitmap.is_empty(),
        }
    }

    pub fn serialize<T: Write>(&mut self, writer: T) -> Result<()> {
        match self {
            JourneyData::Vector(vector) => serialize_journey_vector(vector, writer)?,
            JourneyData::Bitmap(bitmap) => serialize_journey_bitmap(bitmap, writer)?,
        };
        Ok(())
    }

    /// Fully validate imported bitmap contents when `full_validation` is true.
    /// External input requires validation regardless of the storage policy.
    pub fn deserialize<T: Read>(
        reader: T,
        journey_type: JourneyType,
        full_validation: bool,
    ) -> Result<JourneyData> {
        match journey_type {
            JourneyType::Vector => Ok(JourneyData::Vector(deserialize_journey_vector(reader)?)),
            JourneyType::Bitmap => Ok(JourneyData::Bitmap(deserialize_journey_bitmap(
                reader,
                full_validation,
            )?)),
        }
    }
}
