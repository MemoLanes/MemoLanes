use std::io::{Read, Write};

use anyhow::Result;
use integer_encoding::*;

use crate::{
    journey_bitmap::JourneyBitmap,
    journey_header::JourneyType,
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
};

pub enum JourneyData {
    Vector(JourneyVector),
    Bitmap(JourneyBitmap),
}

// TODO: maybe we want a higher one for archive
// 3 is the zstd default
pub const ZSTD_COMPRESS_LEVEL: i32 = 3;
const JOURNEY_VECTOR_MAGIC_HEADER: [u8; 2] = [b'V', b'0'];

// TODO: I don't have a strong reason on putting all serializations here
pub fn serialize_journey_vector<T: Write>(
    journey_vector: &JourneyVector,
    mut writer: T,
) -> Result<()> {
    // magic header
    writer.write_all(&JOURNEY_VECTOR_MAGIC_HEADER)?;

    // data is compressed as a whole
    let mut encoder = zstd::stream::write::Encoder::new(writer, ZSTD_COMPRESS_LEVEL)?.auto_finish();
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

pub fn deserialize_journey_vector<T: Read>(mut reader: T) -> Result<JourneyVector> {
    // magic header
    let mut magic_header: [u8; 2] = [0; 2];
    reader.read_exact(&mut magic_header)?;
    if magic_header != JOURNEY_VECTOR_MAGIC_HEADER {
        bail!(
            "Invalid magic header, expect: {:?}, got: {:?}",
            &JOURNEY_VECTOR_MAGIC_HEADER,
            &magic_header
        );
    }

    // data is compressed as a whole
    let mut decoder = zstd::stream::read::Decoder::new(reader)?;
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
    pub fn serialize<T: Write>(&self, writer: T) -> Result<()> {
        match self {
            JourneyData::Vector(vector) => {
                serialize_journey_vector(vector, writer)?;
            }
            JourneyData::Bitmap(_bitmap) => {
                panic!("TODO")
            }
        };
        Ok(())
    }

    pub fn deserialize<T: Read>(reader: T, journey_type: JourneyType) -> Result<JourneyData> {
        match journey_type {
            JourneyType::Vector => {
                let vector = deserialize_journey_vector(reader)?;
                Ok(JourneyData::Vector(vector))
            }
            JourneyType::Bitmap => {
                panic!("TODO")
            }
        }
    }
}
