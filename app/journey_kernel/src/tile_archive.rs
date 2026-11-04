use bitvec::prelude::BitVec;
use std::convert::TryInto;

use zstd::bulk::{compress as zstd_compress_to_vec, decompress as zstd_decompress_to_vec};

use crate::bitmap2d::append_bitvec_bytes_lsb;

pub const FTA_COMPRESSION_NONE: u8 = 0;
pub const FTA_COMPRESSION_LZ4: u8 = 1;
pub const FTA_COMPRESSION_DEFLATE: u8 = 2;
pub const FTA_COMPRESSION_ZSTD: u8 = 3;

pub(crate) fn serialize_mipmap_into<'a>(
    levels: impl ExactSizeIterator<Item = &'a BitVec>,
    out: &mut Vec<u8>,
) {
    out.extend_from_slice(&(levels.len() as u16).to_le_bytes());
    for level in levels {
        let bit_count = level.len() as u32;
        out.extend_from_slice(&bit_count.to_le_bytes());
        append_bitvec_bytes_lsb(level, out);
    }
}

pub fn zstd_compress_block(block: &[u8], level: u8) -> Result<Vec<u8>, String> {
    let level = i32::from(level.clamp(1, 22));
    zstd_compress_to_vec(block, level)
        .map_err(|e| format!("Failed to compress ZSTD tile block: {e}"))
}

pub(crate) fn decompress_zstd_block(
    payload: &[u8],
    expected_len: usize,
) -> Result<Vec<u8>, String> {
    // The bulk API treats expected_len as a capacity limit, not an exact size.
    let out = zstd_decompress_to_vec(payload, expected_len)
        .map_err(|e| format!("Failed to decompress ZSTD tile block: {e}"))?;
    if out.len() != expected_len {
        return Err(format!(
            "ZSTD tile block length mismatch: expected {}, got {}",
            expected_len,
            out.len()
        ));
    }
    Ok(out)
}

pub(crate) fn split_len_prefixed_block(block: &[u8]) -> Result<(usize, &[u8]), String> {
    if block.len() < 4 {
        return Err("Compressed block too small for length prefix".to_string());
    }
    let expected_len = u32::from_le_bytes(block[0..4].try_into().unwrap()) as usize;
    Ok((expected_len, &block[4..]))
}
