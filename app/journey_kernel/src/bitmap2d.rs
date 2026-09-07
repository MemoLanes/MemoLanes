use crate::tile_iter::{BitmapPixelIter, MipmapIter, OverscanIter, PixelQuery};
use bitvec::prelude::*;

pub(crate) fn append_bitvec_bytes_lsb(bits: &BitVec, out: &mut Vec<u8>) {
    let byte_count = bits.len().div_ceil(8);
    let output_end = out.len() + byte_count;
    let raw_words = bits.as_raw_slice();
    out.reserve(byte_count);
    for &word in raw_words {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out.truncate(output_end);
}

pub fn bitvec_from_bytes_lsb(bytes: &[u8], bit_count: usize) -> BitVec {
    let byte_count = bit_count.div_ceil(8);
    assert!(
        bytes.len() >= byte_count,
        "insufficient bytes for bit_count: need {}, got {}",
        byte_count,
        bytes.len()
    );
    // Decode complete machine words directly; only the final partial word
    // needs padding. Explicit little-endian conversion also handles big-endian hosts.
    const WORD_BYTES: usize = core::mem::size_of::<usize>();
    let mut words = vec![0usize; byte_count.div_ceil(WORD_BYTES)];
    let (chunks, remainder) = bytes[..byte_count].as_chunks::<WORD_BYTES>();
    for (word, chunk) in words.iter_mut().zip(chunks) {
        *word = usize::from_le_bytes(*chunk);
    }
    if !remainder.is_empty() {
        let mut padded = [0u8; WORD_BYTES];
        padded[..remainder.len()].copy_from_slice(remainder);
        *words.last_mut().unwrap() = usize::from_le_bytes(padded);
    }
    let mut bits = BitVec::from_vec(words);
    bits.truncate(bit_count);
    bits
}

/// A power-of-two square binary grid with optional LOD acceleration levels.
///
/// The grid is always `2^width_exp × 2^width_exp` pixels. LOD levels can be
/// generated via [`build_lods`](Self::build_lods), which produces progressively
/// half-resolution bitmaps down to 1×1 using 2×2 OR reduction — analogous to
/// `glGenerateMipmap` for textures.
#[derive(Clone, Debug)]
pub struct BitMap2D {
    bits: BitVec,
    width_exp: u8,
    lods: Vec<BitVec>,
}

impl BitMap2D {
    pub fn new(width_exp: u8) -> Self {
        let total = 1usize << (width_exp as u32 * 2);
        Self {
            bits: BitVec::repeat(false, total),
            width_exp,
            lods: Vec::new(),
        }
    }

    pub fn from_bitvec(width_exp: u8, bits: BitVec) -> Self {
        let expected = 1usize << (width_exp as u32 * 2);
        assert_eq!(
            bits.len(),
            expected,
            "BitVec length {} does not match expected {} for width_exp={}",
            bits.len(),
            expected,
            width_exp
        );
        Self {
            bits,
            width_exp,
            lods: Vec::new(),
        }
    }

    #[inline]
    pub fn width_exp(&self) -> u8 {
        self.width_exp
    }

    #[inline]
    pub fn side(&self) -> usize {
        1 << self.width_exp
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.bits[y * self.side() + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, val: bool) {
        let side = self.side();
        self.bits.set(y * side + x, val);
    }

    pub fn is_empty(&self) -> bool {
        self.bits.not_any()
    }

    #[inline]
    pub fn as_bitvec(&self) -> &BitVec {
        &self.bits
    }

    /// Produce a half-resolution bitmap via 2×2 OR reduction.
    ///
    /// Panics if `width_exp` is 0 (cannot downscale a 1×1 bitmap).
    #[cfg(test)]
    fn downscale(&self) -> BitMap2D {
        assert!(self.width_exp > 0, "cannot downscale a 1x1 bitmap");
        let new_exp = self.width_exp - 1;
        let new_side = 1usize << new_exp;
        let old_side = self.side();
        let mut out = BitVec::repeat(false, new_side * new_side);

        for y in 0..new_side {
            for x in 0..new_side {
                let ox = x * 2;
                let oy = y * 2;
                let val = self.bits[oy * old_side + ox]
                    || self.bits[oy * old_side + ox + 1]
                    || self.bits[(oy + 1) * old_side + ox]
                    || self.bits[(oy + 1) * old_side + ox + 1];
                out.set(y * new_side + x, val);
            }
        }

        BitMap2D {
            bits: out,
            width_exp: new_exp,
            lods: Vec::new(),
        }
    }

    /// Build all LOD levels down to 1×1 via repeated 2×2 OR reduction.
    ///
    /// `lods[0]` is half-resolution, `lods[1]` is quarter-resolution, etc.
    /// After calling this, `lod_level(k)` returns the level at index k.
    pub fn build_lods(&mut self) {
        self.lods.clear();
        if self.width_exp == 0 {
            return;
        }

        let mut current_side = self.side();
        let mut prev = &self.bits;
        let mut levels: Vec<BitVec> = Vec::with_capacity(self.width_exp as usize);

        loop {
            let new_side = current_side / 2;
            if new_side == 0 {
                break;
            }
            let mut out = BitVec::repeat(false, new_side * new_side);
            for y in 0..new_side {
                for x in 0..new_side {
                    let ox = x * 2;
                    let oy = y * 2;
                    let val = prev[oy * current_side + ox]
                        || prev[oy * current_side + ox + 1]
                        || prev[(oy + 1) * current_side + ox]
                        || prev[(oy + 1) * current_side + ox + 1];
                    out.set(y * new_side + x, val);
                }
            }
            levels.push(out);
            current_side = new_side;
            prev = levels.last().unwrap();
        }

        self.lods = levels;
    }

    /// Access LOD level k (0 = half-res, 1 = quarter-res, ...).
    /// Returns `None` if LODs are not built or k is out of range.
    pub fn lod_level(&self, k: usize) -> Option<&BitVec> {
        self.lods.get(k)
    }

    /// Borrow the full LOD stack.
    pub fn lod_levels(&self) -> &[BitVec] {
        &self.lods
    }

    /// Wrap pre-built base + LOD levels (for deserialization paths).
    pub fn from_precomputed(width_exp: u8, base: BitVec, lods: Vec<BitVec>) -> Self {
        let expected = 1usize << (width_exp as u32 * 2);
        assert_eq!(
            base.len(),
            expected,
            "BitVec length {} does not match expected {} for width_exp={}",
            base.len(),
            expected,
            width_exp
        );
        Self {
            bits: base,
            width_exp,
            lods,
        }
    }

    /// Access level by unified offset: 0 = base (full-res), k >= 1 = lods[k-1].
    /// This matches the indexing used by `GenericTile.mipmap[k]`.
    #[inline]
    pub fn level_at_offset(&self, offset: usize) -> Option<&BitVec> {
        if offset == 0 {
            Some(&self.bits)
        } else {
            self.lods.get(offset - 1)
        }
    }

    /// Total number of levels (base + LODs).
    #[inline]
    pub fn num_levels(&self) -> usize {
        1 + self.lods.len()
    }

    /// Owned bitmap allocation size, excluding small Vec metadata.
    pub fn storage_byte_len(&self) -> usize {
        self.bits.capacity().div_ceil(8)
            + self
                .lods
                .iter()
                .map(|level| level.capacity().div_ceil(8))
                .sum::<usize>()
    }

    /// Iterate pixels at a given (x, y, z, resolution_exp) viewport.
    ///
    /// This covers the two leaf-tile iteration cases:
    /// - Mipmap sampling when `z + resolution_exp <= width_exp`
    /// - Overscan expansion when the requested resolution exceeds the native data
    pub fn iter_pixels(&self, query: PixelQuery) -> BitmapPixelIter<'_> {
        let PixelQuery {
            zoom: z,
            resolution_exp,
            ..
        } = query;
        let width_exp = self.width_exp as i16;

        if z + resolution_exp <= width_exp {
            let buffer_exp = z + resolution_exp;
            let level_offset = (width_exp - resolution_exp - z) as usize;
            let Some(buffer) = self.level_at_offset(level_offset) else {
                return BitmapPixelIter::Empty;
            };

            BitmapPixelIter::MipmapIter(MipmapIter::new(buffer, query, buffer_exp))
        } else if z <= width_exp {
            BitmapPixelIter::OverscanIter(OverscanIter::new(&self.bits, query, width_exp))
        } else {
            BitmapPixelIter::Empty
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_bytes_decode_across_word_and_partial_byte_boundaries() {
        let bytes: Vec<u8> = (0..24).map(|i| (i * 73 + 19) as u8).collect();
        for bit_count in 0..=bytes.len() * 8 {
            let bits = bitvec_from_bytes_lsb(&bytes, bit_count);
            assert_eq!(bits.len(), bit_count);
            for index in 0..bit_count {
                assert_eq!(bits[index], bytes[index / 8] & (1 << (index % 8)) != 0);
            }
            let mut encoded = Vec::new();
            append_bitvec_bytes_lsb(&bits, &mut encoded);
            assert_eq!(encoded, bytes[..bit_count.div_ceil(8)]);
        }
    }

    #[test]
    fn overscan_expands_only_the_selected_subtile_at_the_output_origin() {
        let mut bitmap = BitMap2D::new(2);
        bitmap.set(3, 0, true);
        bitmap.set(0, 3, true);
        let pixels = bitmap
            .iter_pixels(PixelQuery {
                origin: (5, 7),
                subtile: (1, 0),
                zoom: 1,
                resolution_exp: 3,
            })
            .collect::<Vec<_>>();
        let expected = (7..11)
            .flat_map(|y| (9..13).map(move |x| (x, y)))
            .collect::<Vec<_>>();
        assert_eq!(pixels, expected);
    }

    #[test]
    fn new_is_empty() {
        let bm = BitMap2D::new(3);
        assert!(bm.is_empty());
        assert_eq!(bm.side(), 8);
        assert_eq!(bm.width_exp(), 3);
        for y in 0..8 {
            for x in 0..8 {
                assert!(!bm.get(x, y));
            }
        }
    }

    #[test]
    fn set_get_roundtrip() {
        let mut bm = BitMap2D::new(2);
        assert!(bm.is_empty());

        bm.set(1, 2, true);
        assert!(!bm.is_empty());
        assert!(bm.get(1, 2));
        assert!(!bm.get(0, 0));
        assert!(!bm.get(2, 1));

        bm.set(1, 2, false);
        assert!(bm.is_empty());
    }

    #[test]
    fn from_bitvec_wraps_correctly() {
        let mut bits = BitVec::repeat(false, 16);
        bits.set(5, true); // (1, 1) in a 4×4 grid
        let bm = BitMap2D::from_bitvec(2, bits);
        assert!(bm.get(1, 1));
        assert!(!bm.get(0, 0));
    }

    #[test]
    #[should_panic(expected = "does not match expected")]
    fn from_bitvec_wrong_size_panics() {
        let bits = BitVec::repeat(false, 10);
        BitMap2D::from_bitvec(2, bits);
    }

    #[test]
    fn downscale_4x4_to_2x2() {
        // 4×4 grid:
        // row0: 1 0 0 0
        // row1: 0 0 0 0
        // row2: 0 0 0 1
        // row3: 0 0 0 0
        let mut bm = BitMap2D::new(2);
        bm.set(0, 0, true); // top-left 2×2 block has a pixel
        bm.set(3, 2, true); // bottom-right 2×2 block has a pixel

        let ds = bm.downscale();
        assert_eq!(ds.width_exp(), 1);
        assert_eq!(ds.side(), 2);
        // (0,0) block: has (0,0) set -> true
        assert!(ds.get(0, 0));
        // (1,0) block: top-right 2×2 -> all zero
        assert!(!ds.get(1, 0));
        // (0,1) block: bottom-left 2×2 -> all zero
        assert!(!ds.get(0, 1));
        // (1,1) block: has (3,2) set -> true
        assert!(ds.get(1, 1));
    }

    #[test]
    fn downscale_empty_returns_empty() {
        let bm = BitMap2D::new(3);
        let ds = bm.downscale();
        assert!(ds.is_empty());
        assert_eq!(ds.width_exp(), 2);
    }

    #[test]
    fn build_lods_level_count() {
        let mut bm = BitMap2D::new(4); // 16×16
        bm.set(5, 5, true);
        bm.build_lods();
        // width_exp=4 -> lods: 8×8, 4×4, 2×2, 1×1 = 4 levels
        assert_eq!(bm.lod_levels().len(), 4);
        assert_eq!(bm.lod_level(0).unwrap().len(), 64); // 8×8
        assert_eq!(bm.lod_level(1).unwrap().len(), 16); // 4×4
        assert_eq!(bm.lod_level(2).unwrap().len(), 4); // 2×2
        assert_eq!(bm.lod_level(3).unwrap().len(), 1); // 1×1
    }

    #[test]
    fn build_lods_matches_manual_downscale() {
        let mut bm = BitMap2D::new(3); // 8×8
        bm.set(1, 1, true);
        bm.set(6, 7, true);

        // Build LODs via build_lods
        let mut bm_with_lods = bm.clone();
        bm_with_lods.build_lods();

        // Build manually via chained downscale
        let ds1 = bm.downscale();
        let ds2 = ds1.downscale();
        let ds3 = ds2.downscale();

        assert_eq!(bm_with_lods.lod_level(0).unwrap(), ds1.as_bitvec());
        assert_eq!(bm_with_lods.lod_level(1).unwrap(), ds2.as_bitvec());
        assert_eq!(bm_with_lods.lod_level(2).unwrap(), ds3.as_bitvec());
    }

    #[test]
    fn build_lods_final_level_is_1x1() {
        let mut bm = BitMap2D::new(5); // 32×32
        bm.set(0, 0, true);
        bm.build_lods();

        let last = bm.lod_levels().last().unwrap();
        assert_eq!(last.len(), 1);
        assert!(last[0]); // should be true since there's data
    }

    #[test]
    fn width_exp_0_no_lods() {
        let mut bm = BitMap2D::new(0);
        assert_eq!(bm.side(), 1);
        bm.set(0, 0, true);
        bm.build_lods();
        assert_eq!(bm.lod_levels().len(), 0);
    }

    #[test]
    fn sparse_pixel_iteration_preserves_row_major_subtile_coordinates() {
        let mut bm = BitMap2D::new(3); // 8x8
        for &(x, y) in &[(7, 0), (4, 1), (6, 3), (0, 7)] {
            bm.set(x, y, true);
        }
        bm.build_lods();

        let upper_right = bm
            .iter_pixels(PixelQuery {
                origin: (10, 20),
                subtile: (1, 0),
                zoom: 1,
                resolution_exp: 2,
            })
            .collect::<Vec<_>>();
        assert_eq!(upper_right, vec![(13, 20), (10, 21), (12, 23)]);

        let full = bm.iter_pixels(PixelQuery::full_tile(3)).collect::<Vec<_>>();
        assert_eq!(full, vec![(7, 0), (4, 1), (6, 3), (0, 7)]);
    }

    #[test]
    #[should_panic(expected = "cannot downscale")]
    fn downscale_1x1_panics() {
        let bm = BitMap2D::new(0);
        bm.downscale();
    }
}
