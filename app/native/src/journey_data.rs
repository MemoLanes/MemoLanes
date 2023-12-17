use std::io::{Read, Write};

use anyhow::{Ok, Result};
use integer_encoding::*;
use itertools::Itertools;

use crate::{
    journey_bitmap::{JourneyBitmap, Tile},
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
const JOURNEY_BITMAP_MAGIC_HEADER: [u8; 2] = [b'B', b'0'];

// TODO: I don't have a strong reason on putting all serializations here
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

pub fn serialize_journey_bitmap<T: Write>(
    journey_bitmap: &JourneyBitmap,
    mut writer: T,
) -> Result<()> {
    let serialize_tile = |tile: &Tile| -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut encoder = zstd::Encoder::new(&mut buf, ZSTD_COMPRESS_LEVEL)?.auto_finish();
        encoder.write_all(&(tile.blocks.len() as u64).encode_var_vec())?;
        for (x, y) in tile.blocks.keys().sorted() {
            let block = tile.blocks.get(&(*x, *y)).unwrap();
            encoder.write_all(&x.to_be_bytes())?;
            encoder.write_all(&y.to_be_bytes())?;
            encoder.write_all(&block.data)?;
        }
        drop(encoder);
        Ok(buf)
    };

    // magic header
    writer.write_all(&JOURNEY_BITMAP_MAGIC_HEADER)?;

    writer.write_all(&(journey_bitmap.tiles.len() as u64).encode_var_vec())?;
    for (x, y) in journey_bitmap.tiles.keys().sorted() {
        let tile = journey_bitmap.tiles.get(&(*x, *y)).unwrap();
        let serialized_tile = serialize_tile(tile)?;
        writer.write_all(&x.to_be_bytes())?;
        writer.write_all(&y.to_be_bytes())?;
        // Also write down the size of the tile so we could load the bitmap
        // without eagerly deserialize all tiles.
        writer.write_all(&(serialized_tile.len() as u64).encode_var_vec())?;
        writer.write_all(&serialized_tile)?;
    }

    Ok(())
}

impl JourneyData {
    pub fn serialize<T: Write>(&self, writer: T) -> Result<()> {
        match self {
            JourneyData::Vector(vector) => serialize_journey_vector(vector, writer)?,
            JourneyData::Bitmap(bitmap) => serialize_journey_bitmap(bitmap, writer)?,
        };
        Ok(())
    }

    pub fn deserialize<T: Read>(reader: T, journey_type: JourneyType) -> Result<JourneyData> {
        match journey_type {
            JourneyType::Vector => Ok(JourneyData::Vector(deserialize_journey_vector(reader)?)),
            JourneyType::Bitmap => {
                panic!("TODO")
            }
        }
    }
}
