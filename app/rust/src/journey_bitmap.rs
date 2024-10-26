use crate::journey_data;
use crate::utils;
use anyhow::{Ok, Result};
use std::io::{Read, Write};
use std::{
    clone::Clone,
    collections::HashMap,
    ops::{BitAnd, BitOr, Not},
};

pub const TILE_WIDTH_OFFSET: i16 = 7;
pub const MAP_WIDTH_OFFSET: i16 = 9;
pub const MAP_WIDTH: i64 = 1 << MAP_WIDTH_OFFSET;
pub const TILE_WIDTH: i64 = 1 << TILE_WIDTH_OFFSET;
pub const TILE_BLOCK_COUNT: usize = (TILE_WIDTH * TILE_WIDTH) as usize;
pub const BITMAP_WIDTH_OFFSET: i16 = 6;
pub const BITMAP_WIDTH: i64 = 1 << BITMAP_WIDTH_OFFSET;
pub const BITMAP_SIZE: usize = (BITMAP_WIDTH * BITMAP_WIDTH / 8) as usize;
const ALL_OFFSET: i16 = TILE_WIDTH_OFFSET + BITMAP_WIDTH_OFFSET;

// we have 512*512 tiles, 128*128 blocks and a single block contains
// a 64*64 bitmap.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct JourneyBitmap {
    pub tiles: HashMap<(u16, u16), Tile>,
}

impl JourneyBitmap {
    pub fn new() -> Self {
        Self {
            tiles: HashMap::new(),
        }
    }

    // NOTE: `add_line` is cherry picked from: https://github.com/tavimori/fogcore/blob/d0888508e25652164742db8e7d879e651b6607d7/src/fogmaps.rs
    // TODO: clean up the code:
    //       - make sure we are using the consistent and correct one of `u64`/`i64`/`i32`.
    //       - better variable naming.
    //       - reduce code duplications.
    pub fn add_line(&mut self, start_lng: f64, start_lat: f64, end_lng: f64, end_lat: f64) {
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
        // The line is X-axis dominant
        if dy0 <= dx0 {
            let (mut x, mut y, xe) = if dx >= 0 {
                // Line is drawn left to right
                (x0 as i64, y0 as i64, x1 as i64)
            } else {
                // Line is drawn right to left (swap ends)
                (x1 as i64, y1 as i64, x0 as i64)
            };
            while x < xe {
                // tile_x is not rounded, it may exceed the antimeridian
                let (tile_x, tile_y) = (x >> ALL_OFFSET, y >> ALL_OFFSET);
                let tile = self
                    .tiles
                    .entry(((tile_x % MAP_WIDTH) as u16, tile_y as u16))
                    .or_insert(Tile::new());
                (x, y, px) = tile.add_line(
                    x - (tile_x << ALL_OFFSET),
                    y - (tile_y << ALL_OFFSET),
                    xe - (tile_x << ALL_OFFSET),
                    px,
                    dx0,
                    dy0,
                    true,
                    (dx < 0 && dy < 0) || (dx > 0 && dy > 0),
                );
                x += tile_x << ALL_OFFSET;
                y += tile_y << ALL_OFFSET;
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
            while y < ye {
                // tile_x is not rounded, it may exceed the antimeridian
                let (tile_x, tile_y) = (x >> ALL_OFFSET, y >> ALL_OFFSET);
                let tile = self
                    .tiles
                    .entry(((tile_x % MAP_WIDTH) as u16, tile_y as u16))
                    .or_insert(Tile::new());
                (x, y, py) = tile.add_line(
                    x - (tile_x << ALL_OFFSET),
                    y - (tile_y << ALL_OFFSET),
                    ye - (tile_y << ALL_OFFSET),
                    py,
                    dx0,
                    dy0,
                    false,
                    (dx < 0 && dy < 0) || (dx > 0 && dy > 0),
                );
                x += tile_x << ALL_OFFSET;
                y += tile_y << ALL_OFFSET;
            }
        }
    }

    pub fn merge(&mut self, other_journey_bitmap: JourneyBitmap) {
        for (key, other_tile) in other_journey_bitmap.tiles {
            match self.tiles.get_mut(&key) {
                None => {
                    self.tiles.insert(key, other_tile);
                }
                Some(self_tile) => {
                    self_tile.merge(other_tile);
                }
            }
        }
    }

    pub fn difference(&mut self, other_journey_bitmap: JourneyBitmap) {
        for (tile_key, other_tile) in other_journey_bitmap.tiles {
            if let Some(tile) = self.tiles.get_mut(&tile_key) {
                tile.difference(other_tile);

                if tile.is_empty() {
                    self.tiles.remove(&tile_key);
                }
            }
        }
    }

    pub fn intersection(&mut self, other_journey_bitmap: JourneyBitmap) {
        self.tiles.retain(
            |tile_key, tile| match other_journey_bitmap.tiles.get(tile_key) {
                None => false,
                Some(other_tile) => {
                    tile.intersection(other_tile);
                    !tile.is_empty()
                }
            },
        );
    }
}

#[derive(Eq, Debug, Clone)]
pub struct Tile {
    blocks_count: usize,
    block_keys: Vec<i16>,
    blocks_buffer: Vec<Option<Block>>,
}

impl Tile {
    pub fn new() -> Self {
        Self {
            blocks_count: 0,
            block_keys: vec![-1; TILE_BLOCK_COUNT],
            blocks_buffer: Vec::new(),
        }
    }

    pub fn add_block(&mut self, x: i64, y: i64, block: Block) {
        // TODO: current implementation will replace the tile if exist, change it to additive editing.
        // TODO: rethink the data type and whether should use into()
        let index = Self::block_key_to_index(x, y);
        self.add_block_by_index(index, block);
    }

    fn add_block_by_index(&mut self, index: usize, block: Block) {
        if self.block_keys[index] == -1 {
            self.block_keys[index] = self.blocks_buffer.len() as i16;
            self.blocks_buffer.push(Some(block));
            self.blocks_count += 1;
        } else {
            self.blocks_buffer[self.block_keys[index] as usize] = Some(block);
        }
    }

    fn get_or_insert_block(&mut self, x: i64, y: i64) -> &mut Block {
        let index = Self::block_key_to_index(x, y);
        if self.block_keys[index] == -1 {
            self.block_keys[index] = self.blocks_buffer.len() as i16;
            self.blocks_buffer.push(Some(Block::new()));
            self.blocks_count += 1;
        }
        self.blocks_buffer[self.block_keys[index] as usize]
            .as_mut()
            .unwrap()
    }

    pub fn get_block(&self, x: i64, y: i64) -> Option<&Block> {
        let index = Self::block_key_to_index(x, y);
        self.get_block_by_index(index)
    }

    fn get_block_by_index(&self, index: usize) -> Option<&Block> {
        if self.block_keys[index] == -1 {
            None
        } else {
            self.blocks_buffer[self.block_keys[index] as usize].as_ref()
        }
    }

    fn get_block_mut_by_index(&mut self, index: usize) -> Option<&mut Block> {
        if self.block_keys[index] == -1 {
            None
        } else {
            self.blocks_buffer[self.block_keys[index] as usize].as_mut()
        }
    }

    fn remove_block_by_index(&mut self, index: usize) {
        debug_assert!(self.block_keys[index] != -1);
        self.blocks_buffer[self.block_keys[index] as usize] = None;
        self.block_keys[index] = -1;
        self.blocks_count -= 1;
    }

    fn is_empty(&self) -> bool {
        self.blocks_count == 0
    }

    pub fn merge(&mut self, other_tile: Tile) {
        let mut other_tile = other_tile;
        for i in 0..TILE_BLOCK_COUNT {
            if other_tile.block_keys[i] != -1 {
                let other_block = other_tile.blocks_buffer[other_tile.block_keys[i] as usize]
                    .take()
                    .unwrap();

                if let Some(self_block) = self.get_block_mut_by_index(i) {
                    self_block.merge(&other_block);
                } else {
                    self.add_block_by_index(i, other_block);
                }
            }
        }
    }

    pub fn difference(&mut self, other_tile: Tile) {
        let mut other_tile = other_tile;
        for i in 0..TILE_BLOCK_COUNT {
            if other_tile.block_keys[i] != -1 {
                let other_block = other_tile.blocks_buffer[other_tile.block_keys[i] as usize]
                    .take()
                    .unwrap();
                if let Some(self_block) = self.get_block_mut_by_index(i) {
                    self_block.difference(&other_block);
                    if self_block.is_empty() {
                        self.remove_block_by_index(i);
                    }
                }
            }
        }
    }

    pub fn intersection(&mut self, other_tile: &Tile) {
        for i in 0..TILE_BLOCK_COUNT {
            if self.block_keys[i] != -1 {
                if let Some(other_block) = other_tile.get_block_by_index(i) {
                    let self_block = self.get_block_mut_by_index(i).unwrap();
                    self_block.intersection(other_block);
                    if self_block.is_empty() {
                        self.remove_block_by_index(i);
                    }
                } else {
                    self.remove_block_by_index(i);
                }
            }
        }
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
    ) -> (i64, i64, i64) {
        let mut p = p;
        let mut x = x;
        let mut y = y;
        if xaxis {
            // Rasterize the line
            while x < e {
                if x >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH
                    || y >> BITMAP_WIDTH_OFFSET < 0
                    || y >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH
                {
                    break;
                }
                let block_x = x >> BITMAP_WIDTH_OFFSET;
                let block_y = y >> BITMAP_WIDTH_OFFSET;

                let block = self.get_or_insert_block(block_x, block_y);

                (x, y, p) = block.add_line(
                    x - (block_x << BITMAP_WIDTH_OFFSET),
                    y - (block_y << BITMAP_WIDTH_OFFSET),
                    e - (block_x << BITMAP_WIDTH_OFFSET),
                    p,
                    dx0,
                    dy0,
                    xaxis,
                    quadrants13,
                );

                x += block_x << BITMAP_WIDTH_OFFSET;
                y += block_y << BITMAP_WIDTH_OFFSET;
            }
        } else {
            // Rasterize the line
            while y < e {
                if y >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH
                    || x >> BITMAP_WIDTH_OFFSET < 0
                    || x >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH
                {
                    break;
                }
                let block_x = x >> BITMAP_WIDTH_OFFSET;
                let block_y = y >> BITMAP_WIDTH_OFFSET;

                let block = self.get_or_insert_block(block_x, block_y);

                (x, y, p) = block.add_line(
                    x - (block_x << BITMAP_WIDTH_OFFSET),
                    y - (block_y << BITMAP_WIDTH_OFFSET),
                    e - (block_y << BITMAP_WIDTH_OFFSET),
                    p,
                    dx0,
                    dy0,
                    xaxis,
                    quadrants13,
                );

                x += block_x << BITMAP_WIDTH_OFFSET;
                y += block_y << BITMAP_WIDTH_OFFSET;
            }
        }
        (x, y, p)
    }

    fn block_key_to_index(x: i64, y: i64) -> usize {
        assert!(x < TILE_WIDTH);
        assert!(y < TILE_WIDTH);
        ((x << TILE_WIDTH_OFFSET) + y) as usize
    }

    fn block_index_to_key(i: usize) -> (i64, i64) {
        let x = i as i64 / TILE_WIDTH;
        let y = i as i64 % TILE_WIDTH;
        (x, y)
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut encoder =
            zstd::Encoder::new(&mut buf, journey_data::ZSTD_COMPRESS_LEVEL)?.auto_finish();
        // We put all block ids in front so we could get all blocks without
        // deserializing the whole block.
        // A bitmap seems more efficient for this case.
        let mut compact_block_keys = [0_u8; TILE_BLOCK_COUNT / 8];

        for i in 0..self.block_keys.len() {
            if self.block_keys[i] != -1 {
                let byte_index = i / 8;
                compact_block_keys[byte_index] |= 1 << (i % 8);
            }
        }

        encoder.write_all(&compact_block_keys)?;

        for (byte_index, _val) in compact_block_keys.iter().enumerate() {
            for offset in 0..8 {
                if compact_block_keys[byte_index] & (1 << offset) != 0 {
                    let block = self.get_block_by_index(byte_index * 8 + offset).unwrap();
                    encoder.write_all(&block.data)?;
                }
            }
        }

        drop(encoder);
        Ok(buf)
    }

    pub fn deserialize<T: Read>(reader: T) -> Result<Tile> {
        let mut decoder = zstd::Decoder::new(reader)?;
        let mut tile = Tile::new();
        let mut compact_block_keys = [0_u8; TILE_BLOCK_COUNT / 8];
        decoder.read_exact(&mut compact_block_keys)?;

        for (byte_index, _val) in compact_block_keys.iter().enumerate() {
            for offset in 0..8 {
                if compact_block_keys[byte_index] & (1 << offset) != 0 {
                    let (x, y) = Self::block_index_to_key(byte_index * 8 + offset);
                    let mut block_data = [0_u8; BITMAP_SIZE];
                    decoder.read_exact(&mut block_data)?;
                    let block = Block::new_with_data(block_data);
                    tile.add_block(x, y, block);
                }
            }
        }

        Ok(tile)
    }
}

// since the blocks may store in different order in this data structure, we need to compare them one by one
impl PartialEq for Tile {
    fn eq(&self, other: &Self) -> bool {
        if self.blocks_count != other.blocks_count {
            return false;
        }
        for i in 0..TILE_BLOCK_COUNT {
            if self.get_block_by_index(i) != other.get_block_by_index(i) {
                return false;
            }
        }
        true
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Block {
    pub data: [u8; BITMAP_SIZE],
}

impl Block {
    pub fn new() -> Self {
        Self {
            data: [0; BITMAP_SIZE],
        }
    }

    pub fn new_with_data(data: [u8; BITMAP_SIZE]) -> Self {
        Self { data }
    }

    fn is_empty(&self) -> bool {
        for i in self.data {
            if i != 0 {
                return false;
            }
        }
        true
    }

    pub fn is_visited(&self, x: u8, y: u8) -> bool {
        let bit_offset = 7 - (x % 8);
        let i = (x / 8) as usize;
        let j = (y) as usize;
        (self.data[i + j * 8] & (1 << bit_offset)) != 0
    }

    fn set_point(&mut self, x: u8, y: u8, val: bool) {
        let bit_offset = 7 - (x % 8);
        let i = (x / 8) as usize;
        let j = (y) as usize;
        let val_number = if val { 1 } else { 0 };
        self.data[i + j * 8] =
            (self.data[i + j * 8] & !(1 << bit_offset)) | (val_number << bit_offset);
    }

    fn merge(&mut self, other_block: &Block) {
        for i in 0..self.data.len() {
            self.data[i] = self.data[i].bitor(other_block.data[i]);
        }
    }

    fn difference(&mut self, other_block: &Block) {
        for i in 0..self.data.len() {
            self.data[i] = self.data[i].bitand(other_block.data[i].not());
        }
    }

    fn intersection(&mut self, other_block: &Block) {
        for i in 0..self.data.len() {
            self.data[i] = self.data[i].bitand(other_block.data[i]);
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
    ) -> (i64, i64, i64) {
        // Draw the first pixel
        let mut p = p;
        let mut x = x;
        let mut y = y;
        self.set_point(x as u8, y as u8, true);
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
                // Draw pixel from line span at
                // currently rasterized position
                self.set_point(x as u8, y as u8, true);
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
                // Draw pixel from line span at
                // currently rasterized position
                self.set_point(x as u8, y as u8, true);
            }
        }
        (x, y, p)
    }
}

#[cfg(test)]
mod tests {
    use crate::journey_bitmap::*;

    #[test]
    fn block_key_conversion() {
        assert_eq!(Tile::block_key_to_index(0, 0), (0));
        assert_eq!(Tile::block_index_to_key(0), (0, 0));

        assert_eq!(Tile::block_key_to_index(127, 127), (16383));
        assert_eq!(Tile::block_index_to_key(16383), (127, 127));

        assert_eq!(Tile::block_key_to_index(64, 17), (8209));
        assert_eq!(Tile::block_index_to_key(8209), (64, 17));
    }
}
