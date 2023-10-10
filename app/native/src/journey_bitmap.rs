// TODO: remove this
#![allow(dead_code)]

use std::collections::HashMap;

// we have 512*512 tiles, 128*128 blocks and a single block contains
// a 64*64 bitmap.
pub struct JourneyBitmap {
    tiles: HashMap<(u16, u16), Tile>,
}

impl JourneyBitmap {
    pub fn new() -> Self {
        Self {
            tiles: HashMap::new(),
        }
    }
}

// TODO: maybe we don't need store (x,y) inside a tile/block.
pub struct Tile {
    x: u16,
    y: u16,
    blocks: HashMap<(u8, u8), Block>,
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
    data: [u8; 512],
}

impl Block {
    pub fn new_with_data(x: u8, y: u8, data: [u8; 512]) -> Self {
        Self { x, y, data }
    }

    pub fn is_visited(&self, x: u8, y: u8) -> bool {
        let bit_offset = 7 - (x % 8);
        let i = (x / 8) as usize;
        let j = (y) as usize;
        (self.data[i + j * 8] & (1 << bit_offset)) != 0
    }
}
