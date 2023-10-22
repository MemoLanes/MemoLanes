// TODO: remove this
#![allow(dead_code)]

use itertools::Itertools;
use std::collections::HashMap;

use crate::{protos, utils};

pub const TILE_WIDTH_OFFSET: i16 = 7;
const MAP_WIDTH_OFFSET: i16 = 9;
const TILE_WIDTH: u64 = 1 << TILE_WIDTH_OFFSET;
pub const BITMAP_WIDTH_OFFSET: i16 = 6;
pub const BITMAP_WIDTH: u64 = 1 << BITMAP_WIDTH_OFFSET;
pub const BITMAP_SIZE: usize = (BITMAP_WIDTH * BITMAP_WIDTH / 8) as usize;
const ALL_OFFSET: i16 = TILE_WIDTH_OFFSET + BITMAP_WIDTH_OFFSET;

// we have 512*512 tiles, 128*128 blocks and a single block contains
// a 64*64 bitmap.
pub struct JourneyBitmap {
    pub tiles: HashMap<(u16, u16), Tile>,
}

impl JourneyBitmap {
    pub fn new() -> Self {
        Self {
            tiles: HashMap::new(),
        }
    }

    pub fn to_proto(&self) -> protos::journey::data::Bitmap {
        let mut proto = protos::journey::data::Bitmap::new();
        for (tile_coord, tile) in self.tiles.iter().sorted_by_key(|x| x.0) {
            let mut tile_proto = protos::journey::data::Tile::new();
            tile_proto.x = tile_coord.0 as u32;
            tile_proto.y = tile_coord.1 as u32;
            for (block_coord, block) in tile.blocks.iter().sorted_by_key(|x| x.0) {
                let mut block_proto = protos::journey::data::Block::new();
                block_proto.x = block_coord.0 as u32;
                block_proto.y = block_coord.1 as u32;
                block_proto.data = block.data.to_vec();
                tile_proto.blocks.push(block_proto);
            }
            proto.tiles.push(tile_proto);
        }
        proto
    }

    pub fn of_proto(proto: protos::journey::data::Bitmap) -> Self {
        // TODO: reduce the amount of copy and allocation?
        let mut t = JourneyBitmap::new();
        for tile_proto in proto.tiles {
            let mut tile = Tile::new(tile_proto.x as u16, tile_proto.y as u16);
            for block_proto in tile_proto.blocks {
                let block = Block::new_with_data(
                    block_proto.x as u8,
                    block_proto.y as u8,
                    block_proto.data.try_into().unwrap(),
                );
                tile.blocks.insert((block.x, block.y), block);
            }
            t.tiles.insert((tile.x, tile.y), tile);
        }
        t
    }

    // NOTE: `add_line` is charry picked from: https://github.com/tavimori/fogcore/blob/57adea9f3eb704198c02417a52a5298ef14489de/src/fogmaps.rs
    // TODO: clean up the code:
    //       - make sure we are using the consistent and correct one of `u64`/`i64`/`i32`.
    //       - get rid of `println`.
    //       - better variable naming.
    //       - reduce code duplications.
    // TODO: handle antimeridian.
    // TODO: add test.
    pub fn add_line(&mut self, start_lng: f64, start_lat: f64, end_lng: f64, end_lat: f64) {
        println!("[{},{}] to [{},{}]", start_lng, start_lat, end_lng, end_lat);

        let (x0, y0) =
            utils::lng_lat_to_tile_xy(start_lng, start_lat, (ALL_OFFSET + MAP_WIDTH_OFFSET) as i32);
        let (x1, y1) =
            utils::lng_lat_to_tile_xy(end_lng, end_lat, (ALL_OFFSET + MAP_WIDTH_OFFSET) as i32);

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
                (x0 as u64, y0 as u64, x1 as u64)
            } else {
                // Line is drawn right to left (swap ends)
                (x1 as u64, y1 as u64, x0 as u64)
            };
            while x < xe {
                let (tile_x, tile_y) = (x >> ALL_OFFSET, y >> ALL_OFFSET);
                println!("accessing tile {}-{}", tile_x, tile_y);
                let tile = self
                    .tiles
                    .entry((tile_x as u16, tile_y as u16))
                    .or_insert(Tile::new(tile_x as u16, tile_y as u16));
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
                println!("tile draw: tile_x: {}, tile_y: {}", tile_x, tile_y);
            }
        } else {
            // The line is Y-axis dominant
            let (mut x, mut y, ye) = if dy >= 0 {
                // Line is drawn bottom to top
                (x0 as u64, y0 as u64, y1 as u64)
            } else {
                // Line is drawn top to bottom
                (x1 as u64, y1 as u64, y0 as u64)
            };
            println!("y {} ye {}", y, ye);
            while y < ye {
                let (tile_x, tile_y) = (x >> ALL_OFFSET, y >> ALL_OFFSET);
                println!("accessing tile {}-{}", tile_x, tile_y);
                let tile = self
                    .tiles
                    .entry((tile_x as u16, tile_y as u16))
                    .or_insert(Tile::new(tile_x as u16, tile_y as u16));
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
                println!("tile draw: tile_x: {}, tile_y: {}", tile_x, tile_y);
            }
        }
    }
}

// TODO: maybe we don't need store (x,y) inside a tile/block.
pub struct Tile {
    x: u16,
    y: u16,
    pub blocks: HashMap<(u8, u8), Block>,
}

impl Tile {
    pub fn new(x: u16, y: u16) -> Self {
        Self {
            x,
            y,
            blocks: HashMap::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_line(
        &mut self,
        x: u64,
        y: u64,
        e: u64,
        p: i64,
        dx0: i64,
        dy0: i64,
        xaxis: bool,
        quadrants13: bool,
    ) -> (u64, u64, i64) {
        let mut p = p;
        let mut x = x;
        let mut y = y;
        if xaxis {
            // Rasterize the line
            while x < e {
                if x >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH || y >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH
                {
                    break;
                }
                let block_x = x >> BITMAP_WIDTH_OFFSET;
                let block_y = y >> BITMAP_WIDTH_OFFSET;

                let block = self
                    .blocks
                    .entry((block_x as u8, block_y as u8))
                    .or_insert(Block::new(block_x as u8, block_y as u8));

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
                if y >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH || x >> BITMAP_WIDTH_OFFSET >= TILE_WIDTH
                {
                    break;
                }
                let block_x = x >> BITMAP_WIDTH_OFFSET;
                let block_y = y >> BITMAP_WIDTH_OFFSET;

                let block = self
                    .blocks
                    .entry((block_x as u8, block_y as u8))
                    .or_insert(Block::new(block_x as u8, block_y as u8));
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
}

pub struct Block {
    x: u8,
    y: u8,
    data: [u8; BITMAP_SIZE],
}

impl Block {
    pub fn new(x: u8, y: u8) -> Self {
        Self {
            x,
            y,
            data: [0; BITMAP_SIZE],
        }
    }

    pub fn new_with_data(x: u8, y: u8, data: [u8; BITMAP_SIZE]) -> Self {
        Self { x, y, data }
    }

    pub fn is_visited(&self, x: u8, y: u8) -> bool {
        let bit_offset = 7 - (x % 8);
        let i = (x / 8) as usize;
        let j = (y) as usize;
        (self.data[i + j * 8] & (1 << bit_offset)) != 0
    }

    fn set_point(&mut self, x: u64, y: u64, val: bool) {
        let bit_offset = 7 - (x % 8);
        let i = (x / 8) as usize;
        let j = (y) as usize;
        let val_number = if val { 1 } else { 0 };
        self.data[i + j * 8] =
            (self.data[i + j * 8] & !(1 << bit_offset)) | (val_number << bit_offset);
    }

    // a modified Bresenham algorithm with initialized error from upper layer
    #[allow(clippy::too_many_arguments)]
    fn add_line(
        &mut self,
        x: u64,
        y: u64,
        e: u64,
        p: i64,
        dx0: i64,
        dy0: i64,
        xaxis: bool,
        quadrants13: bool,
    ) -> (u64, u64, i64) {
        // console.log(`subblock draw: x:${x}, y:${y}, e:${e}`);
        // Draw the first pixel
        let mut p = p;
        let mut x = x;
        let mut y = y;
        self.set_point(x, y, true);
        if xaxis {
            // Rasterize the line
            while x < e {
                x += 1;
                // Deal with octants...
                if p < 0 {
                    p = p + 2 * dy0;
                } else {
                    if quadrants13 {
                        y = y + 1;
                    } else {
                        y = y - 1;
                    }
                    p = p + 2 * (dy0 - dx0);
                }

                if x >= BITMAP_WIDTH || y >= BITMAP_WIDTH {
                    break;
                }
                // Draw pixel from line span at
                // currently rasterized position
                self.set_point(x, y, true);
            }
        } else {
            // The line is Y-axis dominant
            // Rasterize the line
            while y < e {
                y = y + 1;
                // Deal with octants...
                if p <= 0 {
                    p = p + 2 * dx0;
                } else {
                    if quadrants13 {
                        x = x + 1;
                    } else {
                        x = x - 1;
                    }
                    p = p + 2 * (dx0 - dy0);
                }

                if y >= BITMAP_WIDTH || x >= BITMAP_WIDTH {
                    break;
                }
                // Draw pixel from line span at
                // currently rasterized position
                self.set_point(x, y, true);
            }
        }
        (x, y, p)
    }
}
