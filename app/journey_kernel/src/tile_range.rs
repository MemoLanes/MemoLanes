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
//! Each mipmap blob is serialized by `serialize_mipmap`, so each present tile
//! contributes:
//! - level count (`u16`)
//! - repeated for each level:
//!   - bit count (`u32`)
//!   - bitmap bytes (`ceil(bit_count / 8)`)
//!
//! The tail may be compressed based on the header `compression` field.
use crate::bitmap2d::BitMap2D;
use crate::tile_archive::{
    compress_with_len_prefix, decompress_zstd_block, deserialize_mipmap, serialize_mipmap,
    split_len_prefixed_block, zstd_compress_block, FTA_COMPRESSION_LZ4, FTA_COMPRESSION_NONE,
    FTA_COMPRESSION_ZSTD,
};
use crate::utils::xy_to_index;
use bitvec::prelude::BitVec;
use lz4_flex::{compress_prepend_size, decompress_size_prepended};

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

/// Per-tile source data used by `encode_tile_range_response_from_tiles`.
pub struct TilePixelData {
    pub x: i32,
    pub y: i32,
    /// Pixel coordinates within the tile.
    ///
    /// Each `(px, py)` must be in `[0, 2^tile_bitmap_exp)`.
    pub pixels: Vec<(i64, i64)>,
}

/// Encodes a TileRangeResponse directly from externally provided per-tile pixel coordinates.
///
/// This path lets callers build the response without constructing a `GenericTile` tree.
#[allow(clippy::too_many_arguments)]
pub fn encode_tile_range_response_from_tiles(
    z: u8,
    x0: i32,
    y0: i32,
    w: u32,
    h: u32,
    tile_bitmap_exp: u8,
    compression: u8,
    tiles: &[TilePixelData],
) -> Result<Vec<u8>, String> {
    if w == 0 || h == 0 {
        return Err("Invalid tile range".to_string());
    }

    let x1 = (x0 as i64)
        .checked_add(w as i64 - 1)
        .ok_or_else(|| "Range width overflow".to_string())?;
    let y1 = (y0 as i64)
        .checked_add(h as i64 - 1)
        .ok_or_else(|| "Range height overflow".to_string())?;
    let range_w = w;
    let range_h = h;
    let tile_count = range_w
        .checked_mul(range_h)
        .ok_or_else(|| "Range tile_count overflow".to_string())?;

    if range_w > u16::MAX as u32 || range_h > u16::MAX as u32 || tile_count > u16::MAX as u32 {
        return Err("TileRangeResponse range_w/range_h/tile_count exceed u16".to_string());
    }

    let _ = bitmap_bytes_for_exp(tile_bitmap_exp)
        .map_err(|e| format!("Invalid tile_bitmap_exp: {e}"))?;
    let tile_side = 1u32 << tile_bitmap_exp;
    let tile_bit_count = 1usize << (2 * tile_bitmap_exp as usize);
    let presence_len = (tile_count as usize).div_ceil(8);
    let mut presence = vec![0u8; presence_len];
    let mut payload = Vec::new();
    let mut present_count = 0u16;
    let mut tile_blobs = vec![None; tile_count as usize];

    for tile in tiles {
        let tx = tile.x as i64;
        let ty = tile.y as i64;
        if tx < x0 as i64 || tx > x1 || ty < y0 as i64 || ty > y1 {
            return Err(format!(
                "Tile ({}, {}) is outside query bounds x=[{}..{}], y=[{}..{}]",
                tx, ty, x0, x1, y0, y1
            ));
        }

        let idx = ((ty - y0 as i64) as u32 * range_w + (tx - x0 as i64) as u32) as usize;
        if tile_blobs[idx].is_some() {
            return Err(format!(
                "Duplicate tile coordinates in input: ({}, {})",
                tx, ty
            ));
        }

        let mut level0 = BitVec::repeat(false, tile_bit_count);
        for (px, py) in &tile.pixels {
            let px = *px as u32;
            let py = *py as u32;
            if px >= tile_side || py >= tile_side {
                return Err(format!(
                    "Pixel ({}, {}) is outside tile bounds [0, {}) for tile ({}, {})",
                    px, py, tile_side, tx, ty
                ));
            }
            let bit_idx = xy_to_index(px as i64, py as i64, tile_bitmap_exp as i16);
            level0.set(bit_idx, true);
        }

        if level0.not_any() {
            continue;
        }
        let mut bm = BitMap2D::from_bitvec(tile_bitmap_exp, level0);
        bm.build_lods();
        let levels = bm.into_all_levels();
        tile_blobs[idx] = Some(serialize_mipmap(&levels));
    }

    for (idx, tile_blob) in tile_blobs.into_iter().enumerate() {
        if let Some(blob) = tile_blob {
            set_lsb_bit(&mut presence, idx, true);
            payload.extend_from_slice(&blob);
            present_count = present_count
                .checked_add(1)
                .ok_or_else(|| "present_count overflow".to_string())?;
        }
    }

    let mut raw_tail = Vec::with_capacity(presence.len() + payload.len());
    raw_tail.extend_from_slice(&presence);
    raw_tail.extend_from_slice(&payload);
    let encoded_tail = compress_tile_range_tail(&raw_tail, compression)
        .map_err(|e| format!("Failed to encode TileRangeResponse tail: {e}"))?;

    let mut out = Vec::with_capacity(TILE_RANGE_HEADER_SIZE + encoded_tail.len());
    out.push(tile_bitmap_exp);
    out.push(z);
    out.push(compression);
    out.push(0);
    out.extend_from_slice(&x0.to_le_bytes());
    out.extend_from_slice(&y0.to_le_bytes());
    out.extend_from_slice(&(range_w as u16).to_le_bytes());
    out.extend_from_slice(&(range_h as u16).to_le_bytes());
    out.extend_from_slice(&(tile_count as u16).to_le_bytes());
    out.extend_from_slice(&present_count.to_le_bytes());
    out.extend_from_slice(&encoded_tail);
    Ok(out)
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
    let header = parse_tile_range_header(data)?;
    if header.compression == FTA_COMPRESSION_NONE {
        return Ok(data.to_vec());
    }
    let encoded_tail = data
        .get(TILE_RANGE_HEADER_SIZE..)
        .ok_or_else(|| "Missing TileRangeResponse body".to_string())?;
    let raw_tail = decompress_tile_range_tail(encoded_tail, header.compression)?;
    let mut normalized = Vec::with_capacity(TILE_RANGE_HEADER_SIZE + raw_tail.len());
    normalized.extend_from_slice(&data[..TILE_RANGE_HEADER_SIZE]);
    normalized[2] = FTA_COMPRESSION_NONE;
    normalized.extend_from_slice(&raw_tail);
    Ok(normalized)
}

/// Parses tiles from an already decompressed TileRangeResponse body.
pub fn parse_tiles_from_body(
    tile_bitmap_exp: u8,
    x_origin: i32,
    y_origin: i32,
    range_w: usize,
    tile_count: usize,
    present_count: usize,
    body: &[u8],
) -> Result<Vec<(i32, i32, BitMap2D)>, String> {
    let presence_len = tile_count.div_ceil(8);
    if body.len() < presence_len {
        return Err("TileRangeResponse body too small for presence bitmap".to_string());
    }
    let presence = &body[..presence_len];
    let payload = &body[presence_len..];
    let mut offset = 0usize;
    let mut out = Vec::with_capacity(present_count);
    let mut seen_present = 0usize;

    for idx in 0..tile_count {
        if test_lsb_bit(presence, idx) {
            let mipmap_blob = payload
                .get(offset..)
                .ok_or_else(|| "Truncated tile mipmap payload".to_string())?;
            let blob_len = parse_mipmap_blob_len(mipmap_blob)?;
            let blob = &mipmap_blob[..blob_len];
            let levels = deserialize_mipmap(blob)?;
            validate_leaf_mipmap_levels(tile_bitmap_exp, &levels)?;
            offset += blob_len;
            let base = levels[0].clone();
            let lods = levels[1..].to_vec();
            let bitmap = BitMap2D::from_precomputed(tile_bitmap_exp, base, lods);
            let x = x_origin + (idx % range_w) as i32;
            let y = y_origin + (idx / range_w) as i32;
            out.push((x, y, bitmap));
            seen_present += 1;
        }
    }

    if seen_present != present_count {
        return Err("present_count does not match presence bitmap".to_string());
    }
    if offset != payload.len() {
        return Err("Unexpected trailing bytes in TileRangeResponse".to_string());
    }
    Ok(out)
}

pub fn decode_tile_range_response(data: &[u8]) -> Result<Vec<(i32, i32, BitMap2D)>, String> {
    let decompressed = decompress_tile_range_response(data)?;
    let header = parse_tile_range_header(&decompressed)?;
    let body = decompressed
        .get(TILE_RANGE_HEADER_SIZE..)
        .ok_or_else(|| "Missing TileRangeResponse body".to_string())?;
    parse_tiles_from_body(
        header.tile_bitmap_exp,
        header.x0,
        header.y0,
        header.range_w as usize,
        header.tile_count as usize,
        header.present_count as usize,
        body,
    )
}

/// Compresses the response tail (presence bitmap + mipmap payload).
pub fn compress_tile_range_tail(raw_tail: &[u8], compression: u8) -> Result<Vec<u8>, String> {
    match compression {
        FTA_COMPRESSION_NONE => Ok(raw_tail.to_vec()),
        FTA_COMPRESSION_LZ4 => Ok(compress_prepend_size(raw_tail)),
        FTA_COMPRESSION_ZSTD => Ok(compress_with_len_prefix(
            raw_tail.len(),
            zstd_compress_block(raw_tail, 3)?,
        )),
        other => Err(format!(
            "Unsupported TileRangeResponse compression: {}",
            other
        )),
    }
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

fn parse_mipmap_blob_len(bytes: &[u8]) -> Result<usize, String> {
    if bytes.len() < 2 {
        return Err("Invalid tile mipmap payload: missing level count".to_string());
    }
    let level_count = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
    let mut offset = 2usize;
    for _ in 0..level_count {
        if bytes.len() < offset + 4 {
            return Err("Invalid tile mipmap payload: truncated level header".to_string());
        }
        let bit_count = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        offset += 4;
        let byte_count = bit_count.div_ceil(8);
        if bytes.len() < offset + byte_count {
            return Err("Invalid tile mipmap payload: truncated level data".to_string());
        }
        offset += byte_count;
    }
    Ok(offset)
}

fn validate_leaf_mipmap_levels(
    tile_bitmap_exp: u8,
    levels: &[bitvec::prelude::BitVec],
) -> Result<(), String> {
    let expected_levels = tile_bitmap_exp as usize + 1;
    if levels.len() != expected_levels {
        return Err(format!(
            "Invalid tile mipmap level count: expected {}, got {}",
            expected_levels,
            levels.len()
        ));
    }
    let mut expected_bit_count = 1usize << (2 * tile_bitmap_exp as usize);
    for (idx, level) in levels.iter().enumerate() {
        if level.len() != expected_bit_count {
            return Err(format!(
                "Invalid tile mipmap level {} bit count: expected {}, got {}",
                idx,
                expected_bit_count,
                level.len()
            ));
        }
        if expected_bit_count > 1 {
            expected_bit_count /= 4;
        }
    }
    Ok(())
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
