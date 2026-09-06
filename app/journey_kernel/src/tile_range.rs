//! TileRangeResponse binary wire format used by `/tile-range`.
//!
//! The message contains a fixed-width 20-byte header followed by an encoded tail.
//! All multi-byte integer fields use little-endian byte order.
//!
//! Header layout:
//! - byte 0: `tile_bitmap_exp` (`u8`)
//! - byte 1: `z` (`u8`)
//! - byte 2: `compression` (`u8`, see `FTA_COMPRESSION_*`)
//! - byte 3: reserved (`0`)
//! - bytes 4..8: `x0` (`i32`)
//! - bytes 8..12: `y0` (`i32`)
//! - bytes 12..14: `range_w` (`u16`)
//! - bytes 14..16: `range_h` (`u16`)
//! - bytes 16..18: `tile_count` (`u16`, equals `range_w * range_h`)
//! - bytes 18..20: `present_count` (`u16`)
//!
//! Tail layout:
//! 1. Presence bitmap (`ceil(tile_count / 8)` bytes), LSB-first bit order.
//! 2. Concatenated mipmap blobs for present tiles in row-major tile order.
//!
//! Each mipmap blob is serialized by `serialize_mipmap_into`, so each present tile
//! contributes:
//! - level count (`u16`)
//! - repeated for each level:
//!   - bit count (`u32`)
//!   - bitmap bytes (`ceil(bit_count / 8)`)
//!
//! The tail may be compressed based on the header `compression` field.
use crate::bitmap2d::{bitvec_from_bytes_lsb, BitMap2D};
use crate::tile_archive::{
    decompress_zstd_block, serialize_mipmap_into, split_len_prefixed_block, zstd_compress_block,
    FTA_COMPRESSION_LZ4, FTA_COMPRESSION_NONE, FTA_COMPRESSION_ZSTD,
};
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use std::borrow::Cow;

pub const TILE_RANGE_HEADER_SIZE: usize = 20;

#[derive(Clone, Copy, Debug)]
pub struct TileRangeHeader {
    pub tile_bitmap_exp: u8,
    pub z: u8,
    pub compression: u8,
    pub x0: i32,
    pub y0: i32,
    pub range_w: u16,
    pub range_h: u16,
    pub tile_count: u16,
    pub present_count: u16,
}

/// Encodes a tile range one borrowed bitmap at a time, in row-major order.
/// Each `push` represents one position; `None` represents an empty tile.
/// Non-empty bitmaps must already contain their complete LOD pyramid.
pub struct TileRangeEncoder {
    header: TileRangeHeader,
    raw_tail: Vec<u8>,
    next_tile: usize,
}

impl TileRangeEncoder {
    pub fn new(
        z: u8,
        x0: i32,
        y0: i32,
        w: u32,
        h: u32,
        tile_bitmap_exp: u8,
        compression: u8,
    ) -> Result<Self, String> {
        if w == 0 || h == 0 {
            return Err("Invalid tile range".to_string());
        }
        let tile_count = w
            .checked_mul(h)
            .ok_or_else(|| "Range tile_count overflow".to_string())?;
        if w > u16::MAX as u32 || h > u16::MAX as u32 || tile_count > u16::MAX as u32 {
            return Err("TileRangeResponse range_w/range_h/tile_count exceed u16".to_string());
        }
        bitmap_bytes_for_exp(tile_bitmap_exp)
            .map_err(|e| format!("Invalid tile_bitmap_exp: {e}"))?;
        Ok(Self {
            header: TileRangeHeader {
                tile_bitmap_exp,
                z,
                compression,
                x0,
                y0,
                range_w: w as u16,
                range_h: h as u16,
                tile_count: tile_count as u16,
                present_count: 0,
            },
            raw_tail: vec![0; (tile_count as usize).div_ceil(8)],
            next_tile: 0,
        })
    }

    /// Serializes before returning; no reference to the bitmap is retained.
    pub fn push(&mut self, bitmap: Option<&BitMap2D>) -> Result<(), String> {
        if self.next_tile >= self.header.tile_count as usize {
            return Err("Too many tiles for tile range".to_string());
        }
        if let Some(bitmap) = bitmap {
            if bitmap.width_exp() != self.header.tile_bitmap_exp {
                return Err("Tile bitmap exponent mismatch".to_string());
            }
            if !bitmap.is_empty() {
                if bitmap.num_levels() != self.header.tile_bitmap_exp as usize + 1 {
                    return Err("Tile bitmap LOD pyramid is incomplete".to_string());
                }
                set_lsb_bit(&mut self.raw_tail, self.next_tile, true);
                serialize_mipmap_into(
                    (0..bitmap.num_levels()).map(|level| bitmap.level_at_offset(level).unwrap()),
                    &mut self.raw_tail,
                );
                self.header.present_count += 1;
            }
        }
        self.next_tile += 1;
        Ok(())
    }

    pub fn finish(self) -> Result<Vec<u8>, String> {
        if self.next_tile != self.header.tile_count as usize {
            return Err("Incomplete tile range".to_string());
        }
        let header = self.header;
        let tail_capacity = if header.compression == FTA_COMPRESSION_NONE {
            self.raw_tail.len()
        } else {
            0
        };
        let mut out = Vec::with_capacity(TILE_RANGE_HEADER_SIZE + tail_capacity);
        out.push(header.tile_bitmap_exp);
        out.push(header.z);
        out.push(header.compression);
        out.push(0);
        out.extend_from_slice(&header.x0.to_le_bytes());
        out.extend_from_slice(&header.y0.to_le_bytes());
        out.extend_from_slice(&header.range_w.to_le_bytes());
        out.extend_from_slice(&header.range_h.to_le_bytes());
        out.extend_from_slice(&header.tile_count.to_le_bytes());
        out.extend_from_slice(&header.present_count.to_le_bytes());
        append_compressed_tile_range_tail(&mut out, &self.raw_tail, header.compression)
            .map_err(|e| format!("Failed to encode TileRangeResponse tail: {e}"))?;
        Ok(out)
    }
}

pub fn parse_tile_range_header(data: &[u8]) -> Result<TileRangeHeader, String> {
    if data.len() < TILE_RANGE_HEADER_SIZE {
        return Err("TileRangeResponse too small".to_string());
    }

    let header = TileRangeHeader {
        tile_bitmap_exp: data[0],
        z: data[1],
        compression: data[2],
        x0: i32::from_le_bytes([data[4], data[5], data[6], data[7]]),
        y0: i32::from_le_bytes([data[8], data[9], data[10], data[11]]),
        range_w: u16::from_le_bytes([data[12], data[13]]),
        range_h: u16::from_le_bytes([data[14], data[15]]),
        tile_count: u16::from_le_bytes([data[16], data[17]]),
        present_count: u16::from_le_bytes([data[18], data[19]]),
    };

    if header.range_w as usize * header.range_h as usize != header.tile_count as usize {
        return Err("Tile count mismatch".to_string());
    }
    let _ = bitmap_bytes_for_exp(header.tile_bitmap_exp)?;
    Ok(header)
}

/// Normalizes a TileRangeResponse into an uncompressed form.
///
/// The returned bytes preserve the exact header and payload semantics,
/// except `compression` is rewritten to `FTA_COMPRESSION_NONE` and the
/// tail is always decompressed.
pub fn decompress_tile_range_response(data: &[u8]) -> Result<Vec<u8>, String> {
    let (_, body) = decode_header_and_body(data)?;
    let mut out = Vec::with_capacity(TILE_RANGE_HEADER_SIZE + body.len());
    out.extend_from_slice(&data[..TILE_RANGE_HEADER_SIZE]);
    out[2] = FTA_COMPRESSION_NONE;
    out.extend_from_slice(&body);
    Ok(out)
}

/// Borrow uncompressed payloads; compressed payloads own exactly one decode buffer.
fn decode_header_and_body(data: &[u8]) -> Result<(TileRangeHeader, Cow<'_, [u8]>), String> {
    let header = parse_tile_range_header(data)?;
    let encoded = &data[TILE_RANGE_HEADER_SIZE..];
    let body = if header.compression == FTA_COMPRESSION_NONE {
        Cow::Borrowed(encoded)
    } else {
        Cow::Owned(decompress_tile_range_tail(encoded, header.compression)?)
    };
    Ok((header, body))
}

fn parse_present_tiles_from_body(
    header: &TileRangeHeader,
    body: &[u8],
    mut consume: impl FnMut(usize, BitMap2D),
) -> Result<(), String> {
    let presence_len = (header.tile_count as usize).div_ceil(8);
    if body.len() < presence_len {
        return Err("TileRangeResponse body too small for presence bitmap".to_string());
    }
    let (presence, mut payload) = body.split_at(presence_len);
    let mut seen_present = 0usize;
    for idx in 0..header.tile_count as usize {
        if test_lsb_bit(presence, idx) {
            let bitmap = read_tile_mipmap(&mut payload, header.tile_bitmap_exp)?;
            consume(idx, bitmap);
            seen_present += 1;
        }
    }
    if seen_present != header.present_count as usize {
        return Err("present_count does not match presence bitmap".to_string());
    }
    if !payload.is_empty() {
        return Err("Unexpected trailing bytes in TileRangeResponse".to_string());
    }
    Ok(())
}

/// Decode directly into the row-major grid used by the WASM TileBuffer.
#[cfg(any(feature = "wasm", test))]
pub(crate) fn decode_tile_range_response_to_grid(
    data: &[u8],
) -> Result<(TileRangeHeader, Vec<Option<BitMap2D>>), String> {
    let (header, body) = decode_header_and_body(data)?;
    let mut tiles = (0..header.tile_count).map(|_| None).collect::<Vec<_>>();
    parse_present_tiles_from_body(&header, &body, |idx, bitmap| tiles[idx] = Some(bitmap))?;
    Ok((header, tiles))
}

pub fn decode_tile_range_response(data: &[u8]) -> Result<Vec<(i32, i32, BitMap2D)>, String> {
    let (header, body) = decode_header_and_body(data)?;
    let mut tiles = Vec::with_capacity(header.present_count as usize);
    parse_present_tiles_from_body(&header, &body, |idx, bitmap| {
        let x = header.x0 + (idx % header.range_w as usize) as i32;
        let y = header.y0 + (idx / header.range_w as usize) as i32;
        tiles.push((x, y, bitmap));
    })?;
    Ok(tiles)
}

fn append_compressed_tile_range_tail(
    out: &mut Vec<u8>,
    raw_tail: &[u8],
    compression: u8,
) -> Result<(), String> {
    match compression {
        FTA_COMPRESSION_NONE => out.extend_from_slice(raw_tail),
        FTA_COMPRESSION_LZ4 => out.extend_from_slice(&compress_prepend_size(raw_tail)),
        FTA_COMPRESSION_ZSTD => {
            let compressed = zstd_compress_block(raw_tail, 3)?;
            out.extend_from_slice(&(raw_tail.len() as u32).to_le_bytes());
            out.extend_from_slice(&compressed);
        }
        other => Err(format!(
            "Unsupported TileRangeResponse compression: {}",
            other
        ))?,
    }
    Ok(())
}

/// Decompresses the response tail into raw presence bitmap + payload bytes.
pub(crate) fn decompress_tile_range_tail(
    encoded_tail: &[u8],
    compression: u8,
) -> Result<Vec<u8>, String> {
    match compression {
        FTA_COMPRESSION_NONE => Ok(encoded_tail.to_vec()),
        FTA_COMPRESSION_LZ4 => decompress_size_prepended(encoded_tail)
            .map_err(|e| format!("failed to decompress LZ4 TileRangeResponse tail: {}", e)),
        FTA_COMPRESSION_ZSTD => {
            let (expected_len, payload) = split_len_prefixed_block(encoded_tail)?;
            decompress_zstd_block(payload, expected_len)
        }
        other => Err(format!(
            "Unsupported TileRangeResponse compression: {}",
            other
        )),
    }
}

/// Consume one complete pyramid, validating dimensions before allocating each level.
fn read_tile_mipmap(input: &mut &[u8], bitmap_exp: u8) -> Result<BitMap2D, String> {
    let count_bytes = input
        .get(..2)
        .ok_or_else(|| "Invalid tile mipmap payload: missing level count".to_string())?;
    let level_count = u16::from_le_bytes(count_bytes.try_into().unwrap()) as usize;
    let expected_levels = bitmap_exp as usize + 1;
    if level_count != expected_levels {
        return Err(format!(
            "Invalid tile mipmap level count: expected {expected_levels}, got {level_count}"
        ));
    }
    *input = &input[2..];
    let mut expected_bits = 1usize << (2 * bitmap_exp as usize);
    let base = read_mipmap_level(input, expected_bits, 0)?;
    let mut lods = Vec::with_capacity(bitmap_exp as usize);
    for level in 1..expected_levels {
        expected_bits /= 4;
        lods.push(read_mipmap_level(input, expected_bits, level)?);
    }
    Ok(BitMap2D::from_precomputed(bitmap_exp, base, lods))
}

fn read_mipmap_level(
    input: &mut &[u8],
    expected_bits: usize,
    level: usize,
) -> Result<bitvec::prelude::BitVec, String> {
    let header = input
        .get(..4)
        .ok_or_else(|| "Invalid tile mipmap payload: truncated level header".to_string())?;
    let bit_count = u32::from_le_bytes(header.try_into().unwrap()) as usize;
    if bit_count != expected_bits {
        return Err(format!("Invalid tile mipmap level {level} bit count: expected {expected_bits}, got {bit_count}"));
    }
    let end = 4 + bit_count.div_ceil(8);
    let bytes = input
        .get(4..end)
        .ok_or_else(|| "Invalid tile mipmap payload: truncated level data".to_string())?;
    let bits = bitvec_from_bytes_lsb(bytes, bit_count);
    *input = &input[end..];
    Ok(bits)
}

pub fn bitmap_bytes_for_exp(exp: u8) -> Result<usize, String> {
    if !(2..=15).contains(&exp) {
        return Err("bitmap exponent out of supported range [2, 15]".to_string());
    }
    Ok(1usize << (2 * exp as usize - 3))
}

pub fn set_lsb_bit(bytes: &mut [u8], idx: usize, value: bool) {
    let byte = idx / 8;
    let bit = idx % 8;
    let mask = 1u8 << bit;
    if value {
        bytes[byte] |= mask;
    } else {
        bytes[byte] &= !mask;
    }
}

pub(crate) fn test_lsb_bit(bytes: &[u8], idx: usize) -> bool {
    let byte = idx / 8;
    let bit = idx % 8;
    (bytes[byte] & (1u8 << bit)) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_grid_decode_roundtrips_sparse_tiles_for_all_compressions() {
        for compression in [
            FTA_COMPRESSION_NONE,
            FTA_COMPRESSION_LZ4,
            FTA_COMPRESSION_ZSTD,
        ] {
            let mut first = BitMap2D::new(4);
            first.set(0, 0, true);
            let mut last = BitMap2D::new(4);
            last.set(15, 15, true);
            first.build_lods();
            last.build_lods();
            let mut encoder = TileRangeEncoder::new(9, 10, 20, 2, 2, 4, compression).unwrap();
            encoder.push(Some(&first)).unwrap();
            encoder.push(None).unwrap();
            encoder.push(None).unwrap();
            encoder.push(Some(&last)).unwrap();
            let encoded = encoder.finish().unwrap();

            let (header, grid) = decode_tile_range_response_to_grid(&encoded).unwrap();
            let coordinates = decode_tile_range_response(&encoded).unwrap();
            assert_eq!(coordinates.len(), 2);
            assert_eq!((coordinates[0].0, coordinates[0].1), (10, 20));
            assert_eq!((coordinates[1].0, coordinates[1].1), (11, 21));
            assert_eq!(header.compression, compression);
            assert_eq!(grid.len(), 4);
            assert!(grid[1].is_none());
            assert!(grid[2].is_none());
            for (index, expected) in [(0, &first), (3, &last)] {
                let decoded = grid[index].as_ref().unwrap();
                assert_eq!(decoded.as_bitvec(), expected.as_bitvec());
                assert_eq!(decoded.lod_levels(), expected.lod_levels());
            }
        }
    }

    #[test]
    fn rejects_truncated_and_malformed_pyramids() {
        let mut bitmap = BitMap2D::new(4);
        bitmap.set(0, 0, true);
        bitmap.build_lods();
        let mut encoder = TileRangeEncoder::new(9, 10, 20, 1, 1, 4, FTA_COMPRESSION_NONE).unwrap();
        encoder.push(Some(&bitmap)).unwrap();
        let valid = encoder.finish().unwrap();
        for len in 0..valid.len() {
            assert!(
                decode_tile_range_response(&valid[..len]).is_err(),
                "length {len}"
            );
            assert!(
                decode_tile_range_response_to_grid(&valid[..len]).is_err(),
                "length {len}"
            );
        }

        let mut invalid_levels = valid.clone();
        invalid_levels[21..23].copy_from_slice(&0u16.to_le_bytes());
        let mut oversized_level = valid.clone();
        oversized_level[23..27].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut invalid_lod = valid.clone();
        // Level 0: four bytes of metadata followed by 32 bytes of bitmap.
        invalid_lod[59..63].copy_from_slice(&1u32.to_le_bytes());
        let mut invalid_presence = valid.clone();
        invalid_presence[18..20].copy_from_slice(&0u16.to_le_bytes());
        let mut trailing = valid;
        trailing.push(0);
        for invalid in [
            invalid_levels,
            oversized_level,
            invalid_lod,
            invalid_presence,
            trailing,
        ] {
            assert!(decode_tile_range_response(&invalid).is_err());
            assert!(decode_tile_range_response_to_grid(&invalid).is_err());
        }
    }

    #[test]
    fn uncompressed_decode_borrows_the_input_body() {
        let mut encoder = TileRangeEncoder::new(9, 0, 0, 1, 1, 4, FTA_COMPRESSION_NONE).unwrap();
        encoder.push(None).unwrap();
        let encoded = encoder.finish().unwrap();
        let (_, body) = decode_header_and_body(&encoded).unwrap();
        assert!(matches!(body, Cow::Borrowed(_)));
        assert_eq!(body.as_ptr(), encoded[TILE_RANGE_HEADER_SIZE..].as_ptr());
    }
}
