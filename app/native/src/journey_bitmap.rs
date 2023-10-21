// TODO: remove this
#![allow(dead_code)]

use itertools::Itertools;
use std::collections::HashMap;

use crate::protos;

pub const TILE_WIDTH_OFFSET: i16 = 7;
pub const BITMAP_WIDTH_OFFSET: i16 = 6;
pub const BITMAP_WIDTH: u64 = 1 << BITMAP_WIDTH_OFFSET;
pub const BITMAP_SIZE: usize = (BITMAP_WIDTH * BITMAP_WIDTH / 8) as usize;

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
}

pub struct Block {
    x: u8,
    y: u8,
    data: [u8; BITMAP_SIZE],
}

impl Block {
    pub fn new_with_data(x: u8, y: u8, data: [u8; BITMAP_SIZE]) -> Self {
        Self { x, y, data }
    }

    pub fn is_visited(&self, x: u8, y: u8) -> bool {
        let bit_offset = 7 - (x % 8);
        let i = (x / 8) as usize;
        let j = (y) as usize;
        (self.data[i + j * 8] & (1 << bit_offset)) != 0
    }
}
