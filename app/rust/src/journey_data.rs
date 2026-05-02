use std::io::{Read, Write};

use crate::{
    journey_bitmap::{JourneyBitmap, TileKey},
    journey_header::JourneyType,
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
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

const JOURNEY_VECTOR_MAGIC_HEADER: [u8; 2] = [b'V', b'0'];
const JOURNEY_BITMAP_MAGIC_HEADER: [u8; 2] = [b'B', b'0'];

pub fn validate_magic_header<T: Read>(reader: &mut T, expected_header: &[u8; 2]) -> Result<()> {
    // magic header
    let mut magic_header: [u8; 2] = [0; 2];
    reader.read_exact(&mut magic_header)?;
    if &magic_header != expected_header {
        bail!(
            "Invalid magic header, expect: {:?}, got: {:?}",
            expected_header,
            &magic_header
        );
    };
    Ok(())
}

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
    validate_magic_header(&mut reader, &JOURNEY_VECTOR_MAGIC_HEADER)?;

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

#[auto_context]
pub fn serialize_journey_bitmap<T: Write>(
    journey_bitmap: &mut JourneyBitmap,
    mut writer: T,
) -> Result<()> {
    // magic header
    writer.write_all(&JOURNEY_BITMAP_MAGIC_HEADER)?;

    let mut keys = journey_bitmap.all_tile_keys().cloned().collect::<Vec<_>>();
    keys.sort();

    writer.write_all(&(keys.len() as u64).encode_var_vec())?;
    for key in &keys {
        writer.write_all(&key.x.to_be_bytes())?;
        writer.write_all(&key.y.to_be_bytes())?;
        // Also write down the size of the tile so we could load the bitmap
        // without eagerly deserialize all tiles.
        let tile_bytes = journey_bitmap.get_tile_bytes(key).unwrap(); // must exists
        writer.write_all(&(tile_bytes.len() as u64).encode_var_vec())?;
        writer.write_all(&tile_bytes)?;
    }
    Ok(())
}

#[auto_context]
/// `full_validation = true`  will fully deserialize (instead of a lazy version)
/// the bitmap to make sure it is valid. It can be slow but is required for
/// loading data from the outside.
pub fn deserialize_journey_bitmap<T: Read>(
    mut reader: T,
    full_validation: bool,
) -> Result<JourneyBitmap> {
    validate_magic_header(&mut reader, &JOURNEY_BITMAP_MAGIC_HEADER)?;

    let tiles_count: usize = reader.read_varint()?;
    let mut tiles_and_bytes = Vec::with_capacity(tiles_count);
    for _ in 0..tiles_count {
        let mut buf: [u8; 2] = [0; 2];
        reader.read_exact(&mut buf)?;
        let tile_x = u16::from_be_bytes(buf);

        reader.read_exact(&mut buf)?;
        let tile_y = u16::from_be_bytes(buf);

        let tile_data_len: u64 = reader.read_varint()?;
        let mut tile_bytes = vec![0u8; tile_data_len as usize];
        reader.read_exact(&mut tile_bytes)?;
        tiles_and_bytes.push((TileKey::new(tile_x, tile_y), tile_bytes));
    }

    let mut journey_bitmap = JourneyBitmap::of_tile_bytes_without_validation(tiles_and_bytes)?;
    if full_validation {
        journey_bitmap.validate()?;
    }
    Ok(journey_bitmap)
}

impl JourneyData {
    pub fn merge_into(self, bitmap: &mut JourneyBitmap) {
        match self {
            JourneyData::Bitmap(b) => bitmap.merge(b),
            JourneyData::Vector(v) => bitmap.merge_vector(&v),
        }
    }

    pub fn merge_into_with_partial_clone(&self, bitmap: &mut JourneyBitmap) {
        match self {
            JourneyData::Bitmap(b) => bitmap.merge_with_partial_clone(b),
            JourneyData::Vector(v) => bitmap.merge_vector(v),
        }
    }

    pub fn type_(&self) -> JourneyType {
        match self {
            JourneyData::Vector(_) => JourneyType::Vector,
            JourneyData::Bitmap(_) => JourneyType::Bitmap,
        }
    }

    pub fn serialize<T: Write>(&mut self, writer: T) -> Result<()> {
        match self {
            JourneyData::Vector(vector) => serialize_journey_vector(vector, writer)?,
            JourneyData::Bitmap(bitmap) => serialize_journey_bitmap(bitmap, writer)?,
        };
        Ok(())
    }

    /// `full_validation = true`  will fully deserialize (instead of a lazy version)
    /// the bitmap to make sure it is valid. It can be slow but is required for
    /// loading data from the outside.
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
