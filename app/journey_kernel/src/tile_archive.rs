use bitvec::prelude::BitVec;
use lz4_flex::block::decompress_size_prepended;
use miniz_oxide::inflate::decompress_to_vec_with_limit as deflate_decompress_to_vec_with_limit;
use std::convert::TryInto;

#[cfg(not(target_arch = "wasm32"))]
use zstd::bulk::{compress as zstd_compress_to_vec, decompress as zstd_decompress_to_vec};

#[cfg(target_arch = "wasm32")]
use ruzstd::decoding::StreamingDecoder;
#[cfg(target_arch = "wasm32")]
use ruzstd::encoding::{compress_to_vec as ruzstd_compress_to_vec, CompressionLevel};
#[cfg(target_arch = "wasm32")]
use std::io::Read;

use crate::bitmap2d::{bitvec_from_bytes_lsb, bitvec_to_bytes_lsb};

pub const FTA_COMPRESSION_NONE: u8 = 0;
pub const FTA_COMPRESSION_LZ4: u8 = 1;
pub const FTA_COMPRESSION_DEFLATE: u8 = 2;
pub const FTA_COMPRESSION_ZSTD: u8 = 3;

pub fn serialize_mipmap(levels: &[BitVec]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(levels.len() as u16).to_le_bytes());
    for level in levels {
        let bit_count = level.len() as u32;
        out.extend_from_slice(&bit_count.to_le_bytes());
        out.extend_from_slice(&bitvec_to_bytes_lsb(level));
    }
    out
}

pub fn deserialize_mipmap(bytes: &[u8]) -> Result<Vec<BitVec>, String> {
    if bytes.len() < 2 {
        return Err("Mipmap section too small".to_string());
    }

    let mut offset = 0usize;
    let level_count = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
    offset += 2;

    let mut levels = Vec::with_capacity(level_count);
    for _ in 0..level_count {
        if bytes.len() < offset + 4 {
            return Err("Invalid mipmap metadata".to_string());
        }
        let bit_count = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        let byte_count = bit_count.div_ceil(8);
        if bytes.len() < offset + byte_count {
            return Err("Invalid mipmap bytes".to_string());
        }

        let level = bitvec_from_bytes_lsb(&bytes[offset..offset + byte_count], bit_count);

        levels.push(level);
        offset += byte_count;
    }

    if offset != bytes.len() {
        return Err("Unexpected trailing bytes in mipmap blob".to_string());
    }

    Ok(levels)
}

pub fn decompress_tile_block(compression: u8, block: &[u8]) -> Result<Vec<u8>, String> {
    match compression {
        FTA_COMPRESSION_NONE => Ok(block.to_vec()),
        FTA_COMPRESSION_LZ4 => decompress_size_prepended(block)
            .map_err(|e| format!("Failed to decompress LZ4 tile block: {}", e)),
        FTA_COMPRESSION_DEFLATE => {
            let (expected_len, payload) = split_len_prefixed_block(block)?;
            deflate_decompress_to_vec_with_limit(payload, expected_len)
                .map_err(|e| format!("Failed to decompress DEFLATE tile block: {:?}", e))
        }
        FTA_COMPRESSION_ZSTD => {
            let (expected_len, payload) = split_len_prefixed_block(block)?;
            decompress_zstd_block(payload, expected_len)
        }
        other => Err(format!("Unsupported block compression: {}", other)),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn zstd_compress_block(block: &[u8], level: u8) -> Result<Vec<u8>, String> {
    let level = i32::from(level.clamp(1, 22));
    zstd_compress_to_vec(block, level)
        .map_err(|e| format!("Failed to compress ZSTD tile block: {e}"))
}

#[cfg(target_arch = "wasm32")]
pub fn zstd_compress_block(block: &[u8], _level: u8) -> Result<Vec<u8>, String> {
    Ok(ruzstd_compress_to_vec(block, CompressionLevel::Fastest))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn decompress_zstd_block(
    payload: &[u8],
    expected_len: usize,
) -> Result<Vec<u8>, String> {
    zstd_decompress_to_vec(payload, expected_len)
        .map_err(|e| format!("Failed to decompress ZSTD tile block: {e}"))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn decompress_zstd_block(
    payload: &[u8],
    expected_len: usize,
) -> Result<Vec<u8>, String> {
    let mut decoder = StreamingDecoder::new(payload)
        .map_err(|e| format!("Failed to create ZSTD decoder: {:?}", e))?;
    let mut out = Vec::with_capacity(expected_len);
    decoder
        .read_to_end(&mut out)
        .map_err(|e| format!("Failed to decompress ZSTD tile block: {}", e))?;
    if out.len() != expected_len {
        return Err(format!(
            "ZSTD tile block length mismatch: expected {}, got {}",
            expected_len,
            out.len()
        ));
    }
    Ok(out)
}

pub fn compress_with_len_prefix(uncompressed_len: usize, compressed_payload: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + compressed_payload.len());
    out.extend_from_slice(&(uncompressed_len as u32).to_le_bytes());
    out.extend_from_slice(&compressed_payload);
    out
}

pub(crate) fn split_len_prefixed_block(block: &[u8]) -> Result<(usize, &[u8]), String> {
    if block.len() < 4 {
        return Err("Compressed block too small for length prefix".to_string());
    }
    let expected_len = u32::from_le_bytes(block[0..4].try_into().unwrap()) as usize;
    Ok((expected_len, &block[4..]))
}
