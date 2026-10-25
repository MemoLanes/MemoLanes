use super::{BitmapPlane, JourneyBitmap, TileKey};
use crate::utils;
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use integer_encoding::*;
use std::io::{Read, Write};

const JOURNEY_BITMAP_MAGIC_HEADER: [u8; 2] = *b"B0";
// Bound the initial allocation from a record-count hint. Larger bitmaps still
// grow normally as records arrive, and a truncated input cannot reserve a huge map.
const MAX_INITIAL_BITMAP_TILES: usize = 1024;
// B0 keeps its original flat tile list and two big-endian u16 coordinates.
// x bits 0..8 contain the local coordinate; bits 9..10 identify the plane:
// 00 = Mercator, 01 = NorthPolar, 10 = SouthPolar. Other bits are reserved.
// y is unchanged. Mercator-only records remain byte-compatible with old readers.
// Polar bitmaps are normally derived caches. Old readers
// discard their tagged coordinates as out of bounds; cross-version exchange of
// persistent polar bitmaps would require revisiting this compatibility policy.
const BITMAP_NAMESPACE_SHIFT: u32 = 9;
const BITMAP_X_MASK: u16 = (1 << BITMAP_NAMESPACE_SHIFT) - 1;

#[auto_context]
pub fn serialize_journey_bitmap<T: Write>(
    journey_bitmap: &mut JourneyBitmap,
    mut writer: T,
) -> Result<()> {
    writer.write_all(&JOURNEY_BITMAP_MAGIC_HEADER)?;
    let mut keys = journey_bitmap.all_tile_keys().copied().collect::<Vec<_>>();
    keys.sort();
    writer.write_all(&(keys.len() as u64).encode_var_vec())?;
    for key in &keys {
        let (plane, x, y) = match *key {
            TileKey::Mercator { x, y } => (BitmapPlane::Mercator, x, y),
            TileKey::NorthPolar { x, y } => (BitmapPlane::NorthPolar, x, y),
            TileKey::SouthPolar { x, y } => (BitmapPlane::SouthPolar, x, y),
        };
        let encoded_x = x | ((plane as u16) << BITMAP_NAMESPACE_SHIFT);
        writer.write_all(&encoded_x.to_be_bytes())?;
        writer.write_all(&y.to_be_bytes())?;
        // Each compressed tile is length-prefixed, independent of the storage policy.
        let tile_bytes = journey_bitmap.get_tile_bytes(key).unwrap();
        writer.write_all(&(tile_bytes.len() as u64).encode_var_vec())?;
        writer.write_all(tile_bytes.as_ref())?;
    }
    Ok(())
}

#[auto_context]
/// Validate and prune imported bitmap contents when `full_validation` is true.
/// Eager storage decodes tiles even when this flag is false. External input
/// requires full validation regardless of the storage policy.
pub fn deserialize_journey_bitmap<T: Read>(
    mut reader: T,
    full_validation: bool,
) -> Result<JourneyBitmap> {
    utils::validate_magic_header(&mut reader, &JOURNEY_BITMAP_MAGIC_HEADER)?;

    let tiles_count: usize = reader.read_varint()?;
    let mut journey_bitmap =
        JourneyBitmap::with_tile_capacity(tiles_count.min(MAX_INITIAL_BITMAP_TILES));
    let mut editor = journey_bitmap.edit();
    for _ in 0..tiles_count {
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf)?;
        let encoded_x = u16::from_be_bytes(buf);
        // Do not mask off reserved bits: unsupported encodings must not alias
        // a valid namespace or coordinate.
        let namespace = (encoded_x >> BITMAP_NAMESPACE_SHIFT) as u8;
        let plane = BitmapPlane::from_u8(namespace)
            .ok_or_else(|| anyhow::anyhow!("Invalid bitmap tile namespace: {namespace}"))?;
        let tile_x = encoded_x & BITMAP_X_MASK;
        reader.read_exact(&mut buf)?;
        let tile_y = u16::from_be_bytes(buf);
        let tile_data_len: u64 = reader.read_varint()?;
        let mut tile_bytes = vec![0u8; tile_data_len as usize];
        reader.read_exact(&mut tile_bytes)?;
        editor.insert_tile_bytes(TileKey::new(plane, tile_x, tile_y), tile_bytes)?;
    }
    if full_validation {
        editor.validate()?;
    }
    Ok(journey_bitmap)
}
