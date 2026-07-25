//! Compact storage for one border tile: palette of distinct values +
//! packed bit-indices into it, with a per-tile zstd envelope.
//!
//! Single source of truth shared by the offline rasterizer (writer) and
//! the runtime `BorderTileStore` (reader).

use crate::{GeoEntityId, CELLS_PER_TILE};

pub const MAX_PALETTE_ENTRIES: usize = 16;

/// One border tile: palette of distinct values + packed bit-indices into it.
#[derive(Debug, Clone)]
pub struct PackedTile {
    palette: Vec<Option<GeoEntityId>>, // up to MAX_PALETTE_ENTRIES
    bits_per_cell: u8,                 // 1, 2, or 4
    indices: Box<[u8]>,                // ceil(CELLS_PER_TILE * bits_per_cell / 8) bytes
}

impl PackedTile {
    /// Fallible constructor: validates the dense cell buffer and returns an
    /// error instead of panicking on a wrong cell count or palette overflow.
    /// This is the path callers handling untrusted/derived data (e.g.
    /// `write_geo_data`) should use.
    pub fn try_from_dense(cells: &[Option<GeoEntityId>]) -> anyhow::Result<Self> {
        anyhow::ensure!(
            cells.len() == CELLS_PER_TILE,
            "tile must have {CELLS_PER_TILE} cells, got {}",
            cells.len()
        );

        // Build palette in first-seen order; map each value to its palette index.
        let mut palette: Vec<Option<GeoEntityId>> = Vec::with_capacity(4);
        let mut cell_idx: Vec<u8> = Vec::with_capacity(CELLS_PER_TILE);
        for c in cells {
            let i = match palette.iter().position(|p| p == c) {
                Some(i) => i,
                None => {
                    anyhow::ensure!(
                        palette.len() < MAX_PALETTE_ENTRIES,
                        "palette overflow (more than {MAX_PALETTE_ENTRIES} distinct values)"
                    );
                    palette.push(*c);
                    palette.len() - 1
                }
            };
            cell_idx.push(i as u8);
        }

        let bits_per_cell = bits_for_palette(palette.len());
        let indices = pack(&cell_idx, bits_per_cell);

        Ok(Self {
            palette,
            bits_per_cell,
            indices,
        })
    }

    /// Panicking convenience wrapper around [`Self::try_from_dense`] for
    /// callers that construct tiles from known-valid in-memory data (tests,
    /// fixtures).
    pub fn from_dense(cells: &[Option<GeoEntityId>]) -> Self {
        Self::try_from_dense(cells).expect("PackedTile::from_dense: invalid dense cells")
    }

    pub fn lookup(&self, cell_idx: usize) -> Option<GeoEntityId> {
        debug_assert!(cell_idx < CELLS_PER_TILE);
        let pal_idx = unpack_one(&self.indices, self.bits_per_cell, cell_idx) as usize;
        self.palette[pal_idx]
    }

    /// Expand back to the dense `CELLS_PER_TILE` form. Symmetric with
    /// [`Self::try_from_dense`]. Used by the runtime's eager (OSS) store
    /// and by tests.
    pub fn to_dense(&self) -> Vec<Option<GeoEntityId>> {
        (0..CELLS_PER_TILE).map(|i| self.lookup(i)).collect()
    }

    /// Bit width of one palette index: 1, 2, or 4.
    pub fn bits_per_cell(&self) -> u8 {
        self.bits_per_cell
    }

    pub fn to_compressed_bytes(&self) -> Vec<u8> {
        let mut raw = Vec::with_capacity(2 + self.palette.len() * 5 + self.indices.len());
        raw.push(self.palette.len() as u8);
        raw.push(self.bits_per_cell);
        for entry in &self.palette {
            match entry {
                None => raw.extend_from_slice(&[0, 0, 0, 0, 0]),
                Some(id) => {
                    raw.push(1);
                    raw.extend_from_slice(&id.0.to_le_bytes());
                }
            }
        }
        raw.extend_from_slice(&self.indices);
        zstd::encode_all(raw.as_slice(), PER_TILE_ZSTD_LEVEL)
            .expect("zstd encode of a small in-memory buffer cannot fail")
    }

    pub fn from_compressed_bytes(bytes: &[u8]) -> Self {
        let raw = zstd::decode_all(bytes).expect("decompress per-tile blob");
        let mut p = 0usize;
        let palette_len = raw[p] as usize;
        p += 1;
        let bits_per_cell = raw[p];
        p += 1;
        assert!(palette_len <= MAX_PALETTE_ENTRIES);
        assert!(matches!(bits_per_cell, 1 | 2 | 4));
        let mut palette = Vec::with_capacity(palette_len);
        for _ in 0..palette_len {
            let tag = raw[p];
            p += 1;
            let id = u32::from_le_bytes(raw[p..p + 4].try_into().unwrap());
            p += 4;
            palette.push(match tag {
                0 => None,
                1 => Some(GeoEntityId(id)),
                _ => panic!("bad palette tag {tag}"),
            });
        }
        let indices_len = (CELLS_PER_TILE * bits_per_cell as usize).div_ceil(8);
        assert_eq!(raw.len() - p, indices_len);
        let indices = raw[p..p + indices_len].to_vec().into_boxed_slice();
        Self {
            palette,
            bits_per_cell,
            indices,
        }
    }
}

pub const PER_TILE_ZSTD_LEVEL: i32 = 6;

fn bits_for_palette(n: usize) -> u8 {
    // 1 entry would be a Single tile (not Border) but the math still works as 1 bit.
    match n {
        0..=2 => 1,
        3..=4 => 2,
        _ => 4,
    }
}

fn pack(cell_idx: &[u8], bits_per_cell: u8) -> Box<[u8]> {
    let total_bits = cell_idx.len() * bits_per_cell as usize;
    let bytes = total_bits.div_ceil(8);
    let mut out = vec![0u8; bytes];
    let mask = (1u8 << bits_per_cell) - 1;
    let cells_per_byte = 8 / bits_per_cell as usize;
    for (i, &v) in cell_idx.iter().enumerate() {
        let byte = i / cells_per_byte;
        let shift = (i % cells_per_byte) * bits_per_cell as usize;
        out[byte] |= (v & mask) << shift;
    }
    out.into_boxed_slice()
}

fn unpack_one(indices: &[u8], bits_per_cell: u8, i: usize) -> u8 {
    let mask = (1u8 << bits_per_cell) - 1;
    let cells_per_byte = 8 / bits_per_cell as usize;
    let byte = i / cells_per_byte;
    let shift = (i % cells_per_byte) * bits_per_cell as usize;
    (indices[byte] >> shift) & mask
}
