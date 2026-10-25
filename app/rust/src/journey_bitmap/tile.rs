use super::{BITMAP_WIDTH, BITMAP_WIDTH_OFFSET, TILE_WIDTH};
use bitvec::prelude::*;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::mem::take;
use std::ops::Range;

pub const BITMAP_SIZE: usize = (BITMAP_WIDTH * BITMAP_WIDTH / 8) as usize;
const MIPMAP_LEVELS: [usize; 6] = [32, 16, 8, 4, 2, 1];

// Indexed by z (level width = 2^z), while the packed levels run largest first.
const MIPMAP_OFFSETS: [usize; MIPMAP_LEVELS.len()] = {
    let mut offsets = [0; MIPMAP_LEVELS.len()];
    let mut offset = 0;
    let mut i = 0;
    while i < MIPMAP_LEVELS.len() {
        let dim = MIPMAP_LEVELS[i];
        offsets[dim.ilog2() as usize] = offset;
        offset += dim * dim;
        i += 1;
    }
    offsets
};

pub const MIPMAP_BIT_SIZE: usize = {
    let mut sum = 0;
    let mut i = 0;
    while i < MIPMAP_LEVELS.len() {
        let level = MIPMAP_LEVELS[i];
        sum += level * level;
        i += 1;
    }
    sum
};

const TILE_ZSTD_COMPRESS_LEVEL: i32 = 3;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct BlockKey(usize);

impl BlockKey {
    pub fn from_index(index: usize) -> Self {
        debug_assert!(index < (TILE_WIDTH * TILE_WIDTH) as usize);
        BlockKey(index)
    }

    pub fn from_x_y(x: u8, y: u8) -> Self {
        debug_assert!(x < TILE_WIDTH as u8);
        debug_assert!(y < TILE_WIDTH as u8);
        BlockKey((x as usize) * TILE_WIDTH as usize + (y as usize))
    }

    pub fn x(&self) -> u8 {
        (self.0 as i64 / TILE_WIDTH) as u8
    }

    pub fn y(&self) -> u8 {
        (self.0 as i64 % TILE_WIDTH) as u8
    }

    pub fn index(&self) -> usize {
        self.0
    }
}

const BLOCK_KEYS_SIZE: usize = (TILE_WIDTH * TILE_WIDTH / 8) as usize;

#[derive(Debug, Clone)]
pub(super) struct BlockKeyBitset(pub(super) [u8; BLOCK_KEYS_SIZE]);

impl BlockKeyBitset {
    fn new() -> Self {
        BlockKeyBitset([0; BLOCK_KEYS_SIZE])
    }

    fn set(&mut self, block_key: BlockKey) {
        let i = block_key.index();
        self.0[i / 8] |= 1 << (i % 8);
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = BlockKey> + '_ {
        self.0.iter().enumerate().flat_map(|(byte_index, &byte)| {
            let mut remaining = byte;
            std::iter::from_fn(move || {
                if remaining == 0 {
                    return None;
                }
                let offset = remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                Some(BlockKey::from_index(byte_index * 8 + offset))
            })
        })
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> anyhow::Result<()> {
        writer.write_all(&self.0)?;
        Ok(())
    }

    pub(super) fn read_from<R: Read>(reader: &mut R) -> anyhow::Result<Self> {
        let mut bitset = BlockKeyBitset::new();
        reader.read_exact(&mut bitset.0)?;
        Ok(bitset)
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Tile {
    pub(super) blocks: [Option<Box<Block>>; (TILE_WIDTH * TILE_WIDTH) as usize],
}

impl Default for Tile {
    fn default() -> Self {
        Self::new()
    }
}

impl Tile {
    pub fn new() -> Self {
        Self {
            blocks: [const { None }; (TILE_WIDTH * TILE_WIDTH) as usize],
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (BlockKey, &Block)> {
        self.blocks.iter().enumerate().filter_map(|(i, block)| {
            block
                .as_ref()
                .map(|block| (BlockKey::from_index(i), block.as_ref()))
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (BlockKey, &mut Block)> {
        self.blocks.iter_mut().enumerate().filter_map(|(i, block)| {
            block
                .as_mut()
                .map(|block| (BlockKey::from_index(i), block.as_mut()))
        })
    }

    pub fn set(&mut self, block_key: &BlockKey, block: Block) {
        self.blocks[block_key.index()] = Some(Box::new(block));
    }

    pub fn get(&self, block_key: &BlockKey) -> Option<&Block> {
        self.blocks[block_key.index()].as_deref()
    }

    pub fn is_empty(&self) -> bool {
        for b in &self.blocks {
            if b.is_some() {
                return false;
            }
        }
        true
    }

    pub(super) fn remove_empty_blocks(&mut self) -> bool {
        let mut changed = false;
        for block in &mut self.blocks {
            if block.as_ref().is_some_and(|block| block.is_empty()) {
                *block = None;
                changed = true;
            }
        }
        changed
    }

    pub(super) fn merge(&mut self, mut other_tile: Tile) {
        for i in 0..other_tile.blocks.len() {
            match take(&mut other_tile.blocks[i]) {
                None => (),
                Some(other_block) => match &mut self.blocks[i] {
                    None => {
                        self.blocks[i] = Some(other_block);
                    }
                    Some(self_block) => {
                        self_block.merge_with(other_block.as_ref());
                    }
                },
            }
        }
    }

    pub(super) fn merge_with_partial_clone(&mut self, other_tile: &Tile) {
        for i in 0..other_tile.blocks.len() {
            match &other_tile.blocks[i] {
                None => (),
                Some(other_block) => match &mut self.blocks[i] {
                    None => {
                        self.blocks[i] = Some(Box::new((**other_block).clone()));
                    }
                    Some(self_block) => {
                        self_block.merge_with(other_block.as_ref());
                    }
                },
            }
        }
    }

    pub(super) fn fill_rect(&mut self, x: Range<i64>, y: Range<i64>) -> bool {
        let mut changed = false;
        for block_x in x.start / BITMAP_WIDTH..=(x.end - 1) / BITMAP_WIDTH {
            for block_y in y.start / BITMAP_WIDTH..=(y.end - 1) / BITMAP_WIDTH {
                let key = BlockKey::from_x_y(block_x as u8, block_y as u8);
                let block = self.blocks[key.index()].get_or_insert_with(|| Box::new(Block::new()));
                let (origin_x, origin_y) = (block_x * BITMAP_WIDTH, block_y * BITMAP_WIDTH);
                changed |= block.fill_rect(
                    (x.start - origin_x).max(0)..(x.end - origin_x).min(BITMAP_WIDTH),
                    (y.start - origin_y).max(0)..(y.end - origin_y).min(BITMAP_WIDTH),
                );
            }
        }
        changed
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn add_line(
        &mut self,
        x: i64,
        y: i64,
        e: i64,
        p: i64,
        dx0: i64,
        dy0: i64,
        xaxis: bool,
        quadrants13: bool,
        width: u8,
    ) -> ((i64, i64, i64), bool) {
        let mut p = p;
        let mut x = x;
        let mut y = y;
        let mut changed = false;

        loop {
            let (primary, secondary) = if xaxis { (x, y) } else { (y, x) };
            if primary >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH
                || secondary >> BITMAP_WIDTH_OFFSET < 0
                || secondary >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH
            {
                break;
            }
            let block_x = x >> BITMAP_WIDTH_OFFSET;
            let block_y = y >> BITMAP_WIDTH_OFFSET;

            let block_key = BlockKey::from_x_y(block_x as u8, block_y as u8);

            let block =
                &mut self.blocks[block_key.index()].get_or_insert_with(|| Box::new(Block::new()));

            let (block_x_offset, block_y_offset) = (
                block_x << BITMAP_WIDTH_OFFSET,
                block_y << BITMAP_WIDTH_OFFSET,
            );
            let (next, block_changed) = block.add_line(
                x - block_x_offset,
                y - block_y_offset,
                e - if xaxis {
                    block_x_offset
                } else {
                    block_y_offset
                },
                p,
                dx0,
                dy0,
                xaxis,
                quadrants13,
                width,
            );
            (x, y, p) = next;
            changed |= block_changed;

            x += block_x_offset;
            y += block_y_offset;

            // At a block boundary the walker returns the next, undrawn pixel.
            // Reaching the endpoint only finishes the stroke if this block
            // actually drew it; otherwise let the next block stamp it.
            if (if xaxis { x } else { y }) >= e
                && x >> BITMAP_WIDTH_OFFSET == block_x
                && y >> BITMAP_WIDTH_OFFSET == block_y
            {
                break;
            }
        }
        ((x, y, p), changed)
    }

    fn serialize_internal(&self) -> anyhow::Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut encoder = zstd::Encoder::new(&mut buf, TILE_ZSTD_COMPRESS_LEVEL)?.auto_finish();
        let mut block_keys = BlockKeyBitset::new();
        for (block_key, _) in self.iter() {
            block_keys.set(block_key);
        }
        block_keys.write_to(&mut encoder)?;
        for block_key in block_keys.iter() {
            let block = self.get(&block_key).unwrap();
            encoder.write_all(block.raw_data())?;
        }
        drop(encoder);
        Ok(buf)
    }

    pub fn serialize(&self) -> Vec<u8> {
        self.serialize_internal().expect("failed to serialize tile")
    }

    pub fn deserialize(data: &[u8]) -> anyhow::Result<Tile> {
        let mut decoder = zstd::Decoder::new(data)?;
        let mut tile = Tile::new();
        let block_keys = BlockKeyBitset::read_from(&mut decoder)?;
        for block_key in block_keys.iter() {
            let mut block_data = [0_u8; BITMAP_SIZE];
            decoder.read_exact(&mut block_data)?;
            let block = Block::new_with_data(block_data);
            tile.set(&block_key, block);
        }
        Ok(tile)
    }

    pub fn deserialize_exn(data: &[u8]) -> Tile {
        Self::deserialize(data).expect("failed to deserialize tile")
    }
}

type Mipmap = BitArr!(for MIPMAP_BIT_SIZE, in u8, Msb0);

#[derive(Debug, Clone)]
pub struct Block {
    data: [u8; BITMAP_SIZE],
    mipmap: RefCell<Option<Box<Mipmap>>>,
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        let Block {
            data: data_a,
            mipmap: _,
        } = self;
        let Block {
            data: data_b,
            mipmap: _,
        } = other;
        data_a == data_b
    }
}

impl Eq for Block {}

impl Default for Block {
    fn default() -> Self {
        Self {
            data: [0; BITMAP_SIZE],
            mipmap: RefCell::new(None),
        }
    }
}

impl Block {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_data(data: [u8; BITMAP_SIZE]) -> Self {
        Self {
            data,
            mipmap: RefCell::new(None),
        }
    }

    pub fn raw_data(&self) -> &[u8; BITMAP_SIZE] {
        &self.data
    }

    pub(super) fn fill_rect(&mut self, x: Range<i64>, y: Range<i64>) -> bool {
        debug_assert!(0 <= x.start && x.start < x.end && x.end <= BITMAP_WIDTH);
        debug_assert!(0 <= y.start && y.start < y.end && y.end <= BITMAP_WIDTH);
        // Each 64-pixel row is an MSB-first word, matching the bitmap layout.
        let mask = (u64::MAX >> x.start) & (u64::MAX << (BITMAP_WIDTH - x.end));
        let mut changed = false;
        for row in self.data[y.start as usize * 8..y.end as usize * 8]
            .as_chunks_mut::<8>()
            .0
        {
            let old = u64::from_be_bytes(*row);
            let filled = old | mask;
            changed |= old != filled;
            *row = filled.to_be_bytes();
        }
        if changed {
            if let Some(mipmap) = self.mipmap.get_mut() {
                // OR-downsampling maps a filled rectangle to a filled rectangle
                // at each level. Keep cached mipmaps current without point writes.
                let mut x = x.start as usize..x.end as usize;
                let mut y = y.start as usize..y.end as usize;
                let mut offset = 0;
                for dim in MIPMAP_LEVELS {
                    x = x.start / 2..x.end.div_ceil(2);
                    y = y.start / 2..y.end.div_ceil(2);
                    for row in y.clone() {
                        let row_offset = offset + row * dim;
                        mipmap[row_offset + x.start..row_offset + x.end].fill(true);
                    }
                    offset += dim * dim;
                }
            }
        }
        changed
    }

    /// Merge `other` into `self` (bitwise OR). Incrementally updates mipmap if cached.
    pub fn merge_with(&mut self, other: &Block) {
        for i in 0..self.data.len() {
            self.data[i] |= other.data[i];
        }

        if let Some(self_mipmap) = self.mipmap.get_mut() {
            match other.mipmap.borrow().as_ref() {
                Some(other_mipmap) => {
                    let self_raw = self_mipmap.as_raw_mut_slice();
                    let other_raw = other_mipmap.as_raw_slice();
                    for i in 0..self_raw.len() {
                        self_raw[i] |= other_raw[i];
                    }
                }
                None => *self.mipmap.get_mut() = None,
            }
        }
    }

    /// Subtract `other` from `self` (self = self & !other). Clears mipmap cache.
    pub fn difference_with(&mut self, other: &Block) {
        for i in 0..self.data.len() {
            self.data[i] &= !other.data[i];
        }
        *self.mipmap.borrow_mut() = None;
    }

    /// Intersect `self` with `other` (bitwise AND). Clears mipmap cache.
    pub fn intersect_with(&mut self, other: &Block) {
        for i in 0..self.data.len() {
            self.data[i] &= other.data[i];
        }
        *self.mipmap.borrow_mut() = None;
    }

    pub fn is_empty(&self) -> bool {
        for i in self.data {
            if i != 0 {
                return false;
            }
        }
        true
    }

    pub fn count(&self) -> u32 {
        self.data.iter().map(|x| x.count_ones()).sum()
    }

    pub fn is_visited(&self, x: u8, y: u8) -> bool {
        let bit_offset = 7 - (x % 8);
        let i = (x / 8) as usize;
        let j = (y) as usize;
        (self.data[i + j * 8] & (1 << bit_offset)) != 0
    }

    /// Regenerates all mipmap levels from the original 64x64 bitmap
    pub fn regenerate_mipmaps(&self) -> Mipmap {
        // Create a new bitarray for mipmaps (only mipmap levels, not including original data)
        let mut mipmap = bitarr![u8, Msb0; 0; MIPMAP_BIT_SIZE];

        // Below 1/32 (3.125%) source occupancy, scatter each occupied pixel
        // directly into all six ancestor levels. Counting once and retaining
        // the dense path avoids repeated writes becoming costly on fuller blocks.
        if self.count() < (BITMAP_WIDTH * BITMAP_WIDTH / 32) as u32 {
            for index in self.data.view_bits::<Msb0>().iter_ones() {
                let mut x = index % BITMAP_WIDTH as usize;
                let mut y = index / BITMAP_WIDTH as usize;
                let mut offset = 0;
                for dim in MIPMAP_LEVELS {
                    x /= 2;
                    y /= 2;
                    mipmap.set(offset + y * dim + x, true);
                    offset += dim * dim;
                }
            }
            return mipmap;
        }

        let mut mipmap_offset = 0;

        let mut src_offset: Option<usize> = None; // None means read from original data
        let mut src_size = 64;

        for dst_dim in MIPMAP_LEVELS {
            // Downsample from src_size x src_size to dst_dim x dst_dim
            for dst_y in 0..dst_dim {
                for dst_x in 0..dst_dim {
                    // Check 2x2 block in source
                    let src_x = dst_x * 2;
                    let src_y = dst_y * 2;

                    // OR the 4 pixels together (if any is set, result is set)
                    let bit = if let Some(offset) = src_offset {
                        // Read from previous mipmap level
                        let index1 = offset + src_y * src_size + src_x;
                        let index2 = offset + src_y * src_size + (src_x + 1);
                        let index3 = offset + (src_y + 1) * src_size + src_x;
                        let index4 = offset + (src_y + 1) * src_size + (src_x + 1);
                        mipmap[index1] || mipmap[index2] || mipmap[index3] || mipmap[index4]
                    } else {
                        // Read from original data (first level, downsample from 64x64)
                        self.is_visited(src_x as u8, src_y as u8)
                            || self.is_visited((src_x + 1) as u8, src_y as u8)
                            || self.is_visited(src_x as u8, (src_y + 1) as u8)
                            || self.is_visited((src_x + 1) as u8, (src_y + 1) as u8)
                    };

                    // Set the bit in the mipmap
                    let dst_index = mipmap_offset + dst_y * dst_dim + dst_x;
                    mipmap.set(dst_index, bit);
                }
            }

            // Move to next level
            src_offset = Some(mipmap_offset);
            src_size = dst_dim;
            mipmap_offset += dst_dim * dst_dim;
        }

        mipmap
    }

    /// Get a bit from mipmap level z at position (x, y)
    /// z=0: 1x1 level (x and y must be 0)
    /// z=1: 2x2 level (x, y ∈ [0, 1])
    /// z=2: 4x4 level (x, y ∈ [0, 3])
    /// ...
    /// z=5: 32x32 level (x, y ∈ [0, 31])
    /// z=6: 64x64 level (x, y ∈ [0, 63]) - the original bitmap in data field
    pub fn get_at_level(&self, x: usize, y: usize, z: usize) -> bool {
        let size = 1 << z; // 2^z: z=0 -> 1, z=1 -> 2, z=2 -> 4, ..., z=6 -> 64

        debug_assert!(
            x < size && y < size,
            "Invalid coordinates for mipmap level: (x={}, y={}, z={})",
            x,
            y,
            z
        );

        // Special case: z=6 reads from original data
        if z == 6 {
            return self.is_visited(x as u8, y as u8);
        }

        let offset = MIPMAP_OFFSETS[z];

        let mut mipmap_cell = self.mipmap.borrow_mut();
        let mipmap = mipmap_cell.get_or_insert_with(|| Box::new(self.regenerate_mipmaps()));
        let index = offset + y * size + x;
        mipmap[index]
    }

    /// Draw a line segment of width `w` perpendicular to the dominant axis.
    /// `xaxis`: true if the line is x-axis dominant (spread along y), false for y-axis dominant (spread along x).
    fn draw_width(&mut self, x: u8, y: u8, w: u8, even_draw_flag: bool, xaxis: bool) -> bool {
        let mut delta_st: u8 = w / 2;
        let mut delta_ed: u8 = w / 2;
        if w.is_multiple_of(2) {
            if even_draw_flag {
                delta_st -= 1;
            } else {
                delta_ed -= 1;
            }
        }
        // Spread perpendicular to the dominant axis
        let secondary = if xaxis { y } else { x };
        let sec_st = secondary.saturating_sub(delta_st);
        let sec_ed = secondary + delta_ed;
        let mut changed = false;
        for s in sec_st..=sec_ed {
            if xaxis {
                changed |= self.set_point_if_changed(x, s, true);
            } else {
                changed |= self.set_point_if_changed(s, y, true);
            }
        }
        changed
    }

    fn draw_width_point(&mut self, x: u8, y: u8, w: u8) -> bool {
        let delta_st: u8 = w / 2;
        let mut delta_ed: u8 = w / 2;
        if w.is_multiple_of(2) {
            delta_ed -= 1;
        }
        let x_st: u8 = x.saturating_sub(delta_st);
        let x_ed: u8 = x + delta_ed;
        let y_st: u8 = y.saturating_sub(delta_st);
        let y_ed: u8 = y + delta_ed;
        let mut changed = false;
        for x_index in x_st..=x_ed {
            for y_index in y_st..=y_ed {
                changed |= self.set_point_if_changed(x_index, y_index, true);
            }
        }
        changed
    }

    fn update_mipmap_for_point(&self, x: usize, y: usize, val: bool) {
        if !val {
            // setting a point to false is not a common operation.
            // So we just invalid the cache to keep it simple.
            *self.mipmap.borrow_mut() = None;
            return;
        }
        let mut mipmap_cell = self.mipmap.borrow_mut();
        let mipmap = match mipmap_cell.as_mut() {
            Some(m) => m,
            None => return,
        };

        let mut cur_x = x;
        let mut cur_y = y;
        let mut dst_offset: usize = 0;

        for &dim in &MIPMAP_LEVELS {
            let dst_x = cur_x / 2;
            let dst_y = cur_y / 2;
            let dst_idx = dst_offset + dst_y * dim + dst_x;

            mipmap.set(dst_idx, true);

            cur_x = dst_x;
            cur_y = dst_y;
            dst_offset += dim * dim;
        }
    }

    // x ∈ [0,63], y ∈ [0，63]
    pub fn set_point(&mut self, x: u8, y: u8, val: bool) {
        self.set_point_if_changed(x, y, val);
    }

    fn set_point_if_changed(&mut self, x: u8, y: u8, val: bool) -> bool {
        if x > 63 || y > 63 {
            return false;
        }
        let bit_offset = 7 - (x % 8);
        let i = (x / 8) as usize;
        let j = (y) as usize;
        let val_number = if val { 1 } else { 0 };
        let current = self.data[i + j * 8];
        self.data[i + j * 8] = (current & !(1 << bit_offset)) | (val_number << bit_offset);
        if self.data[i + j * 8] != current {
            self.update_mipmap_for_point(x as usize, y as usize, val);
            true
        } else {
            false
        }
    }

    // a modified Bresenham algorithm with initialized error from upper layer
    #[allow(clippy::too_many_arguments)]
    pub(super) fn add_line(
        &mut self,
        x: i64,
        y: i64,
        e: i64,
        p: i64,
        dx0: i64,
        dy0: i64,
        xaxis: bool,
        quadrants13: bool,
        width: u8,
    ) -> ((i64, i64, i64), bool) {
        // Draw the first pixel
        let mut p = p;
        let mut xy = [x, y];
        let mut changed = self.draw_width_point(xy[0] as u8, xy[1] as u8, width);

        // xaxis: primary is x (index 0), yaxis: primary is y (index 1)
        let pri = if xaxis { 0usize } else { 1 };
        let sec = 1 - pri;
        let (pri_delta, sec_delta) = if xaxis { (dx0, dy0) } else { (dy0, dx0) };

        while xy[pri] < e {
            xy[pri] += 1;
            // Deal with octants...
            // Note: xaxis uses strict `p < 0`, yaxis uses `p <= 0`
            if if xaxis { p < 0 } else { p <= 0 } {
                p += 2 * sec_delta;
            } else {
                if quadrants13 {
                    xy[sec] += 1;
                } else {
                    xy[sec] -= 1;
                }
                p += 2 * (sec_delta - pri_delta);
            }

            if xy[pri] >= BITMAP_WIDTH || !(0..BITMAP_WIDTH).contains(&xy[sec]) {
                break;
            }

            let mut draw_flag: bool = p < (-pri_delta);
            if quadrants13 {
                draw_flag = !draw_flag;
            }

            // Draw pixel from line span at currently rasterized position
            changed |= self.draw_width(xy[0] as u8, xy[1] as u8, width, draw_flag, xaxis);
        }
        ((xy[0], xy[1], p), changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_fill_matches_point_writes_with_cached_mipmaps() {
        let mut block = Block::new();
        let mut expected = Block::new();
        for (x, y) in [
            (7..10, 8..12),
            (8..12, 9..13),
            (0..64, 1..2),
            (62..64, 62..64),
            (0..1, 0..64),
            (0..64, 0..64),
        ] {
            block.get_at_level(0, 0, 0); // Cache mipmaps before each mutation.
            let mut changed = false;
            for y in y.clone() {
                for x in x.clone() {
                    changed |= !expected.is_visited(x as u8, y as u8);
                    expected.set_point(x as u8, y as u8, true);
                }
            }
            assert_eq!(block.fill_rect(x.clone(), y.clone()), changed);
            assert_eq!(block, expected);
            assert_mipmap_matches_regenerated(&block);
            assert!(!block.fill_rect(x, y));
            assert_mipmap_matches_regenerated(&block);
        }
    }

    #[test]
    fn block_key_bitset_iterates_only_set_bits_in_order() {
        let expected = [0, 1, 7, 8, 127, 128, 8_209, 16_383];
        let mut bitset = super::BlockKeyBitset::new();
        for &index in &expected {
            bitset.set(BlockKey::from_index(index));
        }

        let actual = bitset.iter().map(|key| key.index()).collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    fn assert_mipmap_matches_regenerated(block: &Block) {
        let cached = block.mipmap.borrow();
        let cached = cached.as_ref().expect("mipmap should be cached");
        let fresh = block.regenerate_mipmaps();
        assert_eq!(
            **cached, fresh,
            "incremental mipmap does not match regenerated mipmap"
        );
    }

    fn has_mipmap(block: &Block) -> bool {
        block.mipmap.borrow().is_some()
    }

    #[test]
    fn incremental_mipmap_set_true() {
        let mut block = Block::new();
        // Trigger initial mipmap generation
        block.get_at_level(0, 0, 0);
        assert!(has_mipmap(&block));

        for (x, y) in [(10, 10), (50, 30), (0, 0), (63, 63), (32, 32), (1, 62)] {
            block.set_point(x, y, true);
            assert!(has_mipmap(&block));
            assert_mipmap_matches_regenerated(&block);
        }
    }

    #[test]
    fn incremental_mipmap_set_false() {
        let mut block = Block::new();
        let points: &[(u8, u8)] = &[(10, 10), (10, 11), (20, 20), (0, 0), (63, 63)];
        for &(x, y) in points {
            block.set_point(x, y, true);
        }
        // Generate mipmap
        block.get_at_level(0, 0, 0);
        assert!(has_mipmap(&block));

        for &(x, y) in points {
            block.set_point(x, y, false);
            assert!(!has_mipmap(&block));
        }
    }

    #[test]
    fn incremental_mipmap_merge() {
        let mut block_a = Block::new();
        block_a.set_point(5, 5, true);
        block_a.set_point(10, 10, true);
        block_a.get_at_level(0, 0, 0);
        assert!(has_mipmap(&block_a));

        // Merge with block that has no mipmap
        let mut block_b = Block::new();
        block_b.set_point(30, 30, true);
        block_b.set_point(50, 50, true);
        block_a.merge_with(&block_b);
        assert!(!has_mipmap(&block_a));

        // Merge with block that has a mipmap
        block_a.get_at_level(0, 0, 0);
        assert!(has_mipmap(&block_a));
        let mut block_c = Block::new();
        block_c.set_point(0, 0, true);
        block_c.set_point(63, 63, true);
        block_c.get_at_level(0, 0, 0);
        block_a.merge_with(&block_c);
        assert!(has_mipmap(&block_a));
        assert_mipmap_matches_regenerated(&block_a);
    }
}
