use crate::utils;
use anyhow::Result;
use bitvec::prelude::*;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::{clone::Clone, collections::HashMap, mem::take};

// NOTE: Interface here can be weird in some cases, these are for journey bitmap v2.

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
        sum += level * level;
        i += 1;
    }
    sum
};

const ALL_OFFSET: i16 = TILE_WIDTH_OFFSET + BITMAP_WIDTH_OFFSET;
const TILE_ZSTD_COMPRESS_LEVEL: i32 = 3;

#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash, PartialOrd, Ord)]
pub struct TileKey {
    pub x: u16,
    pub y: u16,
}

impl TileKey {
    pub fn new(x: u16, y: u16) -> Self {
        TileKey { x, y }
    }
}

// we have 512*512 tiles, 128*128 blocks and a single block contains
// a 64*64 bitmap.
#[derive(Debug, Clone, Default)]
pub struct JourneyBitmap {
    tiles: HashMap<TileKey, Tile>,
}

impl PartialEq for JourneyBitmap {
    fn eq(&self, other: &Self) -> bool {
        self.tiles == other.tiles
    }
}

impl Eq for JourneyBitmap {}

impl JourneyBitmap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    // `validate` should be called immediately after when using this on imported data.
    pub fn of_tile_bytes_without_validation(data: Vec<(TileKey, Vec<u8>)>) -> Result<Self> {
        let mut journey_bitmap = Self::new();
        for (key, serialized_bytes) in data {
            journey_bitmap
                .tiles
                .insert(key, Tile::deserialize(&serialized_bytes)?);
        }
        Ok(journey_bitmap)
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn contains_tile(&self, key: &TileKey) -> bool {
        self.tiles.contains_key(key)
    }

    pub fn get_tile(&mut self, key: &TileKey) -> Option<&Tile> {
        self.tiles.get(key)
    }

    pub fn get_tile_mut(&mut self, key: &TileKey) -> Option<&mut Tile> {
        self.tiles.get_mut(key)
    }

    pub fn get_tile_mut_or_insert_empty(&mut self, key: &TileKey) -> &mut Tile {
        self.tiles.entry(*key).or_default()
    }

    pub fn peek_tile_without_updating_cache<F, R>(&self, key: &TileKey, mut f: F) -> R
    where
        F: FnMut(Option<&Tile>) -> R,
    {
        match self.tiles.get(key) {
            None => f(None),
            Some(tile) => f(Some(tile)),
        }
    }

    pub fn get_tile_summary(&mut self, key: &TileKey) -> Option<TileSummary<'_>> {
        self.tiles.get(key).map(TileSummary::Tile)
    }

    pub fn get_tile_bytes(&mut self, key: &TileKey) -> Option<Vec<u8>> {
        self.tiles.get(key).map(|tile| tile.serialize())
    }

    pub fn all_tile_keys(&self) -> impl Iterator<Item = &TileKey> {
        self.tiles.keys()
    }

    pub fn insert_tile(&mut self, key: &TileKey, tile: Tile) {
        self.tiles.insert(*key, tile);
    }

    /// Validate that all serialized tiles can be deserialized successfully.
    /// This is necessary when importing data. Other parts of the code assume
    /// deserialization will always succeed.
    /// We also try to remove empty tiles and blocks here. This is not strictly
    /// necessary, but we are aware that some users have "invalid" data like this
    /// which are created by old versions of this app.
    pub fn validate(&mut self) -> anyhow::Result<()> {
        // Actually nothing needs to be done here.
        self.tiles.retain(|_, tile| {
            for block in &mut tile.blocks {
                if block.as_ref().is_some_and(|b| b.is_empty()) {
                    *block = None;
                }
            }
            !tile.is_empty()
        });

        Ok(())
    }

    // NOTE: `add_line` is cherry picked from: https://github.com/tavimori/fogcore/blob/d0888508e25652164742db8e7d879e651b6607d7/src/fogmaps.rs
    // TODO: clean up the code:
    //       - make sure we are using the consistent and correct one of `u64`/`i64`/`i32`.
    //       - better variable naming.

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
        F: FnMut(TileKey),
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

        // Calculate line deltas
        let dx = x1 as i64 - x0 as i64;
        let dy = y1 as i64 - y0 as i64;
        // Create a positive copy of deltas (makes iterating easier)
        let dx0 = dx.abs();
        let dy0 = dy.abs();
        // Calculate error intervals for both axis
        let px = 2 * dy0 - dx0;
        let py = 2 * dx0 - dy0;

        let xaxis = dy0 <= dx0;
        let quadrants13 = (dx < 0 && dy < 0) || (dx > 0 && dy > 0);

        // Ensure we always draw in the positive primary direction
        let primary_delta = if xaxis { dx } else { dy };
        let ((mut x, mut y), (end_x, end_y)) = if primary_delta >= 0 {
            ((x0 as i64, y0 as i64), (x1 as i64, y1 as i64))
        } else {
            ((x1 as i64, y1 as i64), (x0 as i64, y0 as i64))
        };
        let end = if xaxis { end_x } else { end_y };
        let mut p = if xaxis { px } else { py };

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

            let tile_key = TileKey::new((tile_x % MAP_WIDTH) as u16, tile_y as u16);
            let (tile_x_offset, tile_y_offset) = (tile_x << ALL_OFFSET, tile_y << ALL_OFFSET);
            {
                let tile = self.get_tile_mut_or_insert_empty(&tile_key);
                (x, y, p) = tile.add_line(
                    x - tile_x_offset,
                    y - tile_y_offset,
                    end - if xaxis { tile_x_offset } else { tile_y_offset },
                    p,
                    dx0,
                    dy0,
                    xaxis,
                    quadrants13,
                    width,
                );
            }
            // TODO: We might want to check if the tile is actually changed
            tile_changed(tile_key);

            x += tile_x_offset;
            y += tile_y_offset;

            if (if xaxis { x } else { y }) >= end {
                break;
            }
        }
    }

    pub fn merge_vector(&mut self, journey_vector: &crate::journey_vector::JourneyVector) {
        for track_segment in &journey_vector.track_segments {
            for (i, point) in track_segment.track_points.iter().enumerate() {
                let prev_idx = i.saturating_sub(1);
                let prev = &track_segment.track_points[prev_idx];
                self.add_line(
                    prev.longitude,
                    prev.latitude,
                    point.longitude,
                    point.latitude,
                );
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

    pub fn merge_with_partial_clone(&mut self, other_journey_bitmap: &JourneyBitmap) {
        for (key, other_tile) in &other_journey_bitmap.tiles {
            match self.tiles.get_mut(key) {
                None => {
                    self.tiles.insert(*key, other_tile.clone());
                }
                Some(self_tile) => {
                    self_tile.merge_with_partial_clone(other_tile);
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

    pub fn check_invariant_and_debug_log(&mut self) {
        let total_tiles = self.tiles.len();
        info!("total tiles: {}", total_tiles);
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

const BLOCK_KEYS_SIZE: usize = (TILE_WIDTH * TILE_WIDTH / 8) as usize;

#[derive(Debug, Clone)]
pub struct BlockKeyBitset([u8; BLOCK_KEYS_SIZE]);

impl BlockKeyBitset {
    fn new() -> Self {
        BlockKeyBitset([0; BLOCK_KEYS_SIZE])
    }

    fn set(&mut self, block_key: BlockKey) {
        let i = block_key.index();
        self.0[i / 8] |= 1 << (i % 8);
    }

    fn iter(&self) -> impl Iterator<Item = BlockKey> + '_ {
        self.0.iter().enumerate().flat_map(|(byte_index, &byte)| {
            (0..8_usize).filter_map(move |offset| {
                if byte & (1 << offset) != 0 {
                    Some(BlockKey::from_index(byte_index * 8 + offset))
                } else {
                    None
                }
            })
        })
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> anyhow::Result<()> {
        writer.write_all(&self.0)?;
        Ok(())
    }

    fn read_from<R: Read>(reader: &mut R) -> anyhow::Result<Self> {
        let mut bitset = BlockKeyBitset::new();
        reader.read_exact(&mut bitset.0)?;
        Ok(bitset)
    }
}

/// A lightweight tile that only provides information about which blocks are not empty.
pub enum TileSummary<'a> {
    Tile(&'a Tile),
}

impl<'a> TileSummary<'a> {
    pub fn contains_block(&self, block_key: &BlockKey) -> bool {
        match self {
            TileSummary::Tile(tile) => tile.blocks[block_key.index()].is_some(),
        }
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

    fn merge(&mut self, mut other_tile: Tile) {
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

    fn merge_with_partial_clone(&mut self, other_tile: &Tile) {
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
            (x, y, p) = block.add_line(
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

            x += block_x_offset;
            y += block_y_offset;

            if (if xaxis { x } else { y }) >= e {
                break;
            }
        }
        (x, y, p)
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
    mipmap: RefCell<Option<Mipmap>>,
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
            _ => panic!("Invalid mipmap level: z={}", z),
        };

        let mut mipmap_cell = self.mipmap.borrow_mut();
        let mipmap = mipmap_cell.get_or_insert_with(|| self.regenerate_mipmaps());
        let index = offset + y * size + x;
        mipmap[index]
    }

    /// Draw a line segment of width `w` perpendicular to the dominant axis.
    /// `xaxis`: true if the line is x-axis dominant (spread along y), false for y-axis dominant (spread along x).
    fn draw_width(&mut self, x: u8, y: u8, w: u8, even_draw_flag: bool, xaxis: bool) {
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
        for s in sec_st..=sec_ed {
            if xaxis {
                self.set_point(x, s, true);
            } else {
                self.set_point(s, y, true);
            }
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
            self.update_mipmap_for_point(x as usize, y as usize, val);
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
        let mut xy = [x, y];
        self.draw_width_point(xy[0] as u8, xy[1] as u8, width);

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
            self.draw_width(xy[0] as u8, xy[1] as u8, width, draw_flag, xaxis);
        }
        (xy[0], xy[1], p)
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
        // Verify mipmap correctness at every level for point (10, 10).
        // Each level downsamples by 2x from the original 64×64:
        //   z=6 (64×64): direct data read
        //   z=5 (32×32): (10,10) → (5,5)
        //   z=4 (16×16): → (2,2), z=3 (8×8): → (1,1)
        //   z=2 (4×4):   → (0,0), z=1 (2×2): → (0,0), z=0 (1×1): → (0,0)
        let mut block = Block::new();
        block.set_point(10, 10, true);
        assert!(block.get_at_level(10, 10, 6));
        assert!(!block.get_at_level(11, 10, 6));
        assert!(block.get_at_level(5, 5, 5) && !block.get_at_level(4, 5, 5));
        assert!(block.get_at_level(2, 2, 4) && !block.get_at_level(3, 2, 4));
        assert!(block.get_at_level(1, 1, 3) && !block.get_at_level(0, 1, 3));
        assert!(block.get_at_level(0, 0, 2) && !block.get_at_level(1, 0, 2));
        assert!(block.get_at_level(0, 0, 1) && !block.get_at_level(1, 0, 1));
        assert!(block.get_at_level(0, 0, 0));

        // Results reflect updates (set_point, merge_with, difference_with, intersect_with)
        block.set_point(10, 10, false);
        assert!(!block.get_at_level(0, 0, 0));

        let mut other = Block::new();
        other.set_point(20, 20, true);
        block.merge_with(&other);
        assert!(block.get_at_level(0, 0, 0));

        block.difference_with(&other);
        assert!(!block.get_at_level(0, 0, 0));

        block.merge_with(&other);
        block.intersect_with(&Block::new());
        assert!(!block.get_at_level(0, 0, 0));
    }

    fn assert_mipmap_matches_regenerated(block: &Block) {
        let cached = block.mipmap.borrow();
        let cached = cached.as_ref().expect("mipmap should be cached");
        let fresh = block.regenerate_mipmaps();
        assert_eq!(
            *cached, fresh,
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
