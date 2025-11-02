use crate::utils;
use bitvec::prelude::*;
use std::{
    clone::Clone,
    collections::HashMap,
    mem::take,
};

pub const TILE_WIDTH_OFFSET: i16 = 7;
pub const MAP_WIDTH_OFFSET: i16 = 9;
pub const MAP_WIDTH: i64 = 1 << MAP_WIDTH_OFFSET;
pub const TILE_WIDTH: i64 = 1 << TILE_WIDTH_OFFSET;
pub const BITMAP_WIDTH_OFFSET: i16 = 6;
pub const BITMAP_WIDTH: i64 = 1 << BITMAP_WIDTH_OFFSET;
pub const BITMAP_SIZE: usize = (BITMAP_WIDTH * BITMAP_WIDTH / 8) as usize;
const MIPMAP_LEVELS: [usize; 6] = [32, 16, 8, 4, 2, 1];

pub const MIPMAP_BIT_SIZE: usize = {
    let mut sum = 0;
    let mut i = 0;
    while i < MIPMAP_LEVELS.len() {
        let level = MIPMAP_LEVELS[i];
        sum += (level * level) as usize;
        i += 1;
    }
    sum
};

const ALL_OFFSET: i16 = TILE_WIDTH_OFFSET + BITMAP_WIDTH_OFFSET;

// we have 512*512 tiles, 128*128 blocks and a single block contains
// a 64*64 bitmap.
#[derive(PartialEq, Eq, Debug, Clone, Default)]
pub struct JourneyBitmap {
    pub tiles: HashMap<(u16, u16), Tile>,
}

impl JourneyBitmap {
    pub fn new() -> Self {
        Self::default()
    }

    // NOTE: `add_line` is cherry picked from: https://github.com/tavimori/fogcore/blob/d0888508e25652164742db8e7d879e651b6607d7/src/fogmaps.rs
    // TODO: clean up the code:
    //       - make sure we are using the consistent and correct one of `u64`/`i64`/`i32`.
    //       - better variable naming.
    //       - reduce code duplications.

    pub fn add_line(&mut self, start_lng: f64, start_lat: f64, end_lng: f64, end_lat: f64) {
        self.add_line_with_change_callback(start_lng, start_lat, end_lng, end_lat, |_| {});
    }

    pub fn add_line_with_change_callback<F>(
        &mut self,
        start_lng: f64,
        start_lat: f64,
        end_lng: f64,
        end_lat: f64,
        mut tile_changed: F,
    ) where
        F: FnMut((u16, u16)),
    {
        use std::f64::consts::PI;

        let (mut x0, y0) = utils::lng_lat_to_tile_x_y(
            start_lng,
            start_lat,
            (ALL_OFFSET + MAP_WIDTH_OFFSET) as i32,
        );
        let (mut x1, y1) =
            utils::lng_lat_to_tile_x_y(end_lng, end_lat, (ALL_OFFSET + MAP_WIDTH_OFFSET) as i32);

        let (x_half, _) =
            utils::lng_lat_to_tile_x_y(0.0, 0.0, (ALL_OFFSET + MAP_WIDTH_OFFSET) as i32);

        if x1 - x0 > x_half {
            x0 += 2 * x_half;
        } else if x0 - x1 > x_half {
            x1 += 2 * x_half;
        }

        // Iterators, counters required by algorithm
        // Calculate line deltas
        let dx = x1 as i64 - x0 as i64;
        let dy = y1 as i64 - y0 as i64;
        // Create a positive copy of deltas (makes iterating easier)
        let dx0 = dx.abs();
        let dy0 = dy.abs();
        // Calculate error intervals for both axis
        let mut px = 2 * dy0 - dx0;
        let mut py = 2 * dx0 - dy0;

        // The line is X-axis dominant or only in one tile
        if dy0 <= dx0 {
            let (mut x, mut y, xe) = if dx >= 0 {
                // Line is drawn left to right
                (x0 as i64, y0 as i64, x1 as i64)
            } else {
                // Line is drawn right to left (swap ends)
                (x1 as i64, y1 as i64, x0 as i64)
            };
            loop {
                // tile_x is not rounded, it may exceed the antimeridian
                let (tile_x, tile_y) = (x >> ALL_OFFSET, y >> ALL_OFFSET);
                let (_, tile_lat) = utils::tile_x_y_to_lng_lat(
                    x as i32,
                    y as i32,
                    (ALL_OFFSET + MAP_WIDTH_OFFSET) as i32,
                );
                let latirad = tile_lat * PI / 180.0;
                let width: u8 = (1.0 / latirad.cos()).round() as u8;

                let tile_pos = ((tile_x % MAP_WIDTH) as u16, tile_y as u16);
                let tile = self.tiles.entry(tile_pos).or_default();
                (x, y, px) = tile.add_line(
                    x - (tile_x << ALL_OFFSET),
                    y - (tile_y << ALL_OFFSET),
                    xe - (tile_x << ALL_OFFSET),
                    px,
                    dx0,
                    dy0,
                    true,
                    (dx < 0 && dy < 0) || (dx > 0 && dy > 0),
                    width,
                );
                // TODO: We might want to check if the tile is actually changed
                tile_changed(tile_pos);

                x += tile_x << ALL_OFFSET;
                y += tile_y << ALL_OFFSET;

                if x >= xe {
                    break;
                }
            }
        } else {
            // The line is Y-axis dominant
            let (mut x, mut y, ye) = if dy >= 0 {
                // Line is drawn bottom to top
                (x0 as i64, y0 as i64, y1 as i64)
            } else {
                // Line is drawn top to bottom
                (x1 as i64, y1 as i64, y0 as i64)
            };
            loop {
                // tile_x is not rounded, it may exceed the antimeridian
                let (tile_x, tile_y) = (x >> ALL_OFFSET, y >> ALL_OFFSET);
                let (_, tile_lat) = utils::tile_x_y_to_lng_lat(
                    x as i32,
                    y as i32,
                    (ALL_OFFSET + MAP_WIDTH_OFFSET) as i32,
                );
                let latirad = tile_lat * PI / 180.0;
                let width: u8 = (1.0 / latirad.cos()).round() as u8;
                let tile_pos = ((tile_x % MAP_WIDTH) as u16, tile_y as u16);
                let tile = self.tiles.entry(tile_pos).or_default();
                (x, y, py) = tile.add_line(
                    x - (tile_x << ALL_OFFSET),
                    y - (tile_y << ALL_OFFSET),
                    ye - (tile_y << ALL_OFFSET),
                    py,
                    dx0,
                    dy0,
                    false,
                    (dx < 0 && dy < 0) || (dx > 0 && dy > 0),
                    width,
                );
                // TODO: We might want to check if the tile is actually changed
                tile_changed(tile_pos);

                x += tile_x << ALL_OFFSET;
                y += tile_y << ALL_OFFSET;

                if y >= ye {
                    break;
                }
            }
        }
    }

    pub fn merge(&mut self, other_journey_bitmap: JourneyBitmap) {
        for (key, mut other_tile) in other_journey_bitmap.tiles {
            match self.tiles.get_mut(&key) {
                None => {
                    self.tiles.insert(key, other_tile);
                }
                Some(self_tile) => {
                    for i in 0..other_tile.blocks.len() {
                        match take(&mut other_tile.blocks[i]) {
                            None => (),
                            Some(other_block) => match &mut self_tile.blocks[i] {
                                None => {
                                    self_tile.blocks[i] = Some(other_block);
                                }
                                Some(self_block) => {
                                    // merge other_block into self_block
                                    self_block.merge_with(other_block.as_ref());
                                }
                            },
                        }
                    }
                }
            }
        }
    }

    pub fn difference(&mut self, other_journey_bitmap: &JourneyBitmap) {
        for (tile_key, other_tile) in &other_journey_bitmap.tiles {
            if let Some(tile) = self.tiles.get_mut(tile_key) {
                for i in 0..other_tile.blocks.len() {
                    match &other_tile.blocks[i] {
                        None => (),
                        Some(other_block) => {
                            if let Some(block) = &mut tile.blocks[i] {
                                // subtract other_block from block
                                block.difference_with(other_block.as_ref());
                                if block.is_empty() {
                                    tile.blocks[i] = None;
                                }
                            }
                        }
                    }
                }

                if tile.is_empty() {
                    self.tiles.remove(tile_key);
                }
            }
        }
    }

    pub fn intersection(&mut self, other_journey_bitmap: &JourneyBitmap) {
        self.tiles.retain(
            |tile_key, tile| match other_journey_bitmap.tiles.get(tile_key) {
                None => false,
                Some(other_tile) => {
                    for i in 0..other_tile.blocks.len() {
                        match &other_tile.blocks[i] {
                            None => tile.blocks[i] = None,
                            Some(other_block) => {
                                if let Some(block) = &mut tile.blocks[i] {
                                    // intersect block with other_block
                                    block.intersect_with(other_block.as_ref());
                                    if block.is_empty() {
                                        tile.blocks[i] = None;
                                    }
                                }
                            }
                        }
                    }
                    !tile.is_empty()
                }
            },
        );
    }
}

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

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Tile {
    blocks: [Option<Box<Block>>; (TILE_WIDTH * TILE_WIDTH) as usize],
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

    pub fn set(&mut self, block_key: BlockKey, block: Block) {
        self.blocks[block_key.index()] = Some(Box::new(block));
    }

    pub fn get(&self, block_key: BlockKey) -> Option<&Block> {
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

    #[allow(clippy::too_many_arguments)]
    fn add_line(
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
    ) -> (i64, i64, i64) {
        let mut p = p;
        let mut x = x;
        let mut y = y;

        if xaxis {
            // Rasterize the line
            loop {
                if x >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH
                    || y >> BITMAP_WIDTH_OFFSET < 0
                    || y >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH
                {
                    break;
                }
                let block_x = x >> BITMAP_WIDTH_OFFSET;
                let block_y = y >> BITMAP_WIDTH_OFFSET;

                let block_key = BlockKey::from_x_y(block_x as u8, block_y as u8);

                let block = &mut self.blocks[block_key.index()]
                    .get_or_insert_with(|| Box::new(Block::new()));

                (x, y, p) = block.add_line(
                    x - (block_x << BITMAP_WIDTH_OFFSET),
                    y - (block_y << BITMAP_WIDTH_OFFSET),
                    e - (block_x << BITMAP_WIDTH_OFFSET),
                    p,
                    dx0,
                    dy0,
                    xaxis,
                    quadrants13,
                    width,
                );

                x += block_x << BITMAP_WIDTH_OFFSET;
                y += block_y << BITMAP_WIDTH_OFFSET;

                if x >= e {
                    break;
                }
            }
        } else {
            // Rasterize the line
            loop {
                if y >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH
                    || x >> BITMAP_WIDTH_OFFSET < 0
                    || x >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH
                {
                    break;
                }
                let block_x = x >> BITMAP_WIDTH_OFFSET;
                let block_y = y >> BITMAP_WIDTH_OFFSET;

                let block_key = BlockKey::from_x_y(block_x as u8, block_y as u8);

                let block = &mut self.blocks[block_key.index()]
                    .get_or_insert_with(|| Box::new(Block::new()));

                (x, y, p) = block.add_line(
                    x - (block_x << BITMAP_WIDTH_OFFSET),
                    y - (block_y << BITMAP_WIDTH_OFFSET),
                    e - (block_y << BITMAP_WIDTH_OFFSET),
                    p,
                    dx0,
                    dy0,
                    xaxis,
                    quadrants13,
                    width,
                );

                x += block_x << BITMAP_WIDTH_OFFSET;
                y += block_y << BITMAP_WIDTH_OFFSET;
                if y >= e {
                    break;
                }
            }
        }
        (x, y, p)
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub data: [u8; BITMAP_SIZE],
    pub mipmap: Option<BitArr!(for MIPMAP_BIT_SIZE, in u8, Msb0)>,
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
            mipmap: None,
        }
    }
}

impl Block {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_data(data: [u8; BITMAP_SIZE]) -> Self {
        Self { data, mipmap: None }
    }

    /// Merge `other` into `self` (bitwise OR). Clears mipmap cache.
    pub fn merge_with(&mut self, other: &Block) {
        for i in 0..self.data.len() {
            self.data[i] |= other.data[i];
        }
        // TODO: maybe we should consider merging mipmap directly or only reset
        // it when there is a real change.
        self.mipmap = None;
    }

    /// Subtract `other` from `self` (self = self & !other). Clears mipmap cache.
    pub fn difference_with(&mut self, other: &Block) {
        for i in 0..self.data.len() {
            self.data[i] &= !other.data[i];
        }
        self.mipmap = None;
    }

    /// Intersect `self` with `other` (bitwise AND). Clears mipmap cache.
    pub fn intersect_with(&mut self, other: &Block) {
        for i in 0..self.data.len() {
            self.data[i] &= other.data[i];
        }
        self.mipmap = None;
    }

    fn is_empty(&self) -> bool {
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
    pub fn regenerate_mipmaps(&mut self) {
        // Create a new bitarray for mipmaps (only mipmap levels, not including original data)
        let mut mipmap = bitarr![u8, Msb0; 0; MIPMAP_BIT_SIZE];

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

        self.mipmap = Some(mipmap);
    }

    /// Get a bit from mipmap level z at position (x, y)
    /// z=0: 1x1 level (x and y must be 0)
    /// z=1: 2x2 level (x, y ∈ [0, 1])
    /// z=2: 4x4 level (x, y ∈ [0, 3])
    /// ...
    /// z=5: 32x32 level (x, y ∈ [0, 31])
    /// z=6: 64x64 level (x, y ∈ [0, 63]) - the original bitmap in data field
    pub fn get_at_level(&self, x: usize, y: usize, z: usize) -> Option<bool> {
        let size = 1 << z; // 2^z: z=0 -> 1, z=1 -> 2, z=2 -> 4, ..., z=6 -> 64

        // Check bounds
        if x >= size || y >= size {
            return None;
        }

        // Special case: z=6 reads from original data
        if z == 6 {
            return Some(self.is_visited(x as u8, y as u8));
        }


        // TODO: This could be implemented with const expressions with `MIPMAP_LEVELS`,
        // so we don't have to hardcode the offsets here. But I am too lazy to do it now.

        // For z=0 to z=5, read from mipmap if it exists
        // Note: Block2's mipmap does NOT include the original 64x64 data
        let offset = match z {
            0 => 1024 + 256 + 64 + 16 + 4, // 1x1 at bit 1364
            1 => 1024 + 256 + 64 + 16,     // 2x2 at bit 1360
            2 => 1024 + 256 + 64,          // 4x4 at bit 1344
            3 => 1024 + 256,               // 8x8 at bit 1280
            4 => 1024,                     // 16x16 at bit 1024
            5 => 0,                        // 32x32 at bit 0
            _ => return None,
        };

        let mipmap = self.mipmap.as_ref()?;
        let index = offset + y * size + x;
        Some(mipmap[index])
    }

    fn draw_width_x(&mut self, x: u8, y: u8, w: u8, even_draw_flag: bool) {
        let mut delta_st: u8 = w / 2;
        let mut delta_ed: u8 = w / 2;
        if w.is_multiple_of(2) {
            if even_draw_flag {
                delta_st -= 1;
            } else {
                delta_ed -= 1;
            }
        }
        let y_st: u8 = y.saturating_sub(delta_st);
        let y_ed: u8 = y + delta_ed;

        for y_index in y_st..=y_ed {
            self.set_point(x, y_index, true);
        }
    }

    fn draw_width_y(&mut self, x: u8, y: u8, w: u8, even_draw_flag: bool) {
        let mut delta_st: u8 = w / 2;
        let mut delta_ed: u8 = w / 2;
        if w.is_multiple_of(2) {
            if even_draw_flag {
                delta_st -= 1;
            } else {
                delta_ed -= 1;
            }
        }
        let x_st: u8 = x.saturating_sub(delta_st);
        let x_ed: u8 = x + delta_ed;
        for x_index in x_st..=x_ed {
            self.set_point(x_index, y, true);
        }
    }

    fn draw_width_point(&mut self, x: u8, y: u8, w: u8) {
        let delta_st: u8 = w / 2;
        let mut delta_ed: u8 = w / 2;
        if w.is_multiple_of(2) {
            delta_ed -= 1;
        }
        let x_st: u8 = x.saturating_sub(delta_st);
        let x_ed: u8 = x + delta_ed;
        let y_st: u8 = y.saturating_sub(delta_st);
        let y_ed: u8 = y + delta_ed;
        for x_index in x_st..=x_ed {
            for y_index in y_st..=y_ed {
                self.set_point(x_index, y_index, true);
            }
        }
    }

    // x ∈ [0,63], y ∈ [0，63]
    fn set_point(&mut self, x: u8, y: u8, val: bool) {
        if x > 63 || y > 63 {
            return;
        }
        let bit_offset = 7 - (x % 8);
        let i = (x / 8) as usize;
        let j = (y) as usize;
        let val_number = if val { 1 } else { 0 };
        let current = self.data[i + j * 8];
        self.data[i + j * 8] = (current & !(1 << bit_offset)) | (val_number << bit_offset);
        if self.data[i + j * 8] != current {
            self.mipmap = None;
        }
    }

    // a modified Bresenham algorithm with initialized error from upper layer
    #[allow(clippy::too_many_arguments)]
    fn add_line(
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
    ) -> (i64, i64, i64) {
        // Draw the first pixel
        let mut p = p;
        let mut x = x;
        let mut y = y;
        self.draw_width_point(x as u8, y as u8, width);
        if xaxis {
            // Rasterize the line
            while x < e {
                x += 1;
                // Deal with octants...
                if p < 0 {
                    p += 2 * dy0;
                } else {
                    if quadrants13 {
                        y += 1;
                    } else {
                        y -= 1;
                    }
                    p += 2 * (dy0 - dx0);
                }

                if x >= BITMAP_WIDTH || !(0..BITMAP_WIDTH).contains(&y) {
                    break;
                }

                let mut draw_flag: bool = p < (-dx0);
                if quadrants13 {
                    draw_flag = !draw_flag;
                }

                // Draw pixel from line span at
                // currently rasterized position
                self.draw_width_x(x as u8, y as u8, width, draw_flag);
            }
        } else {
            // The line is Y-axis dominant
            // Rasterize the line
            while y < e {
                y += 1;
                // Deal with octants...
                if p <= 0 {
                    p += 2 * dx0;
                } else {
                    if quadrants13 {
                        x += 1;
                    } else {
                        x -= 1;
                    }
                    p += 2 * (dx0 - dy0);
                }

                if y >= BITMAP_WIDTH || !(0..BITMAP_WIDTH).contains(&x) {
                    break;
                }

                let mut draw_flag: bool = p < (-dy0);
                if quadrants13 {
                    draw_flag = !draw_flag;
                }
                // Draw pixel from line span at
                // currently rasterized position
                self.draw_width_y(x as u8, y as u8, width, draw_flag);
            }
        }
        (x, y, p)
    }
}

#[cfg(test)]
mod tests {
    use crate::journey_bitmap::{Block, BlockKey};

    #[test]
    fn block_key_conversion() {
        let test = |(x, y), index| {
            assert_eq!(BlockKey::from_x_y(x, y), BlockKey::from_index(index));
            assert_eq!(BlockKey::from_x_y(x, y).index(), index);
        };
        test((0, 0), 0);
        test((127, 127), 16383);
        test((64, 17), 8209);
    }

    #[test]
    fn block_mipmap() {
        let mut block_with_mipmap = Block::new();
        block_with_mipmap.set_point(10, 10, true);
        let block_without_mipmap = block_with_mipmap.clone();
        block_with_mipmap.regenerate_mipmaps();
        assert_eq!(block_with_mipmap, block_without_mipmap);

        // mipmap will be cleared when update
        assert!(block_with_mipmap.mipmap.is_some());
        block_with_mipmap.set_point(10, 10, true);
        assert!(block_with_mipmap.mipmap.is_some());
        block_with_mipmap.set_point(10, 15, true);
        assert!(block_with_mipmap.mipmap.is_none());

        block_with_mipmap.regenerate_mipmaps();
        let mut other_block = Block::new();
        other_block.set_point(20,20, true);
        assert!(block_with_mipmap.mipmap.is_some());
        block_with_mipmap.merge_with(&other_block);
        assert!(block_with_mipmap.mipmap.is_none());
    }
}
