use native::{journey_bitmap::JourneyBitmap, map_renderer::MapRenderer};
mod test_utils;

#[test]
fn add_line_cross_antimeridian() {
    let mut journey_bitmap = JourneyBitmap::new();

    // Melbourne to Hawaii
    let (start_lng, start_lat, end_lng, end_lat) =
        (144.847737, 37.6721702, -160.3644029, 21.3186185);
    journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);

    // Hawaii to Guan
    let (start_lng, start_lat, end_lng, end_lat) =
        (-160.3644029, 21.3186185, 121.4708788, 9.4963078);
    journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result = map_renderer
        .maybe_render_map_overlay(0.0, -170.0, 80.0, 170.0, -80.0)
        .unwrap();
    test_utils::assert_image(
        &render_result.data.0,
        "journey_bitmap_add_line_cross_antimeridian",
        "3eb61d8bae656e73894b54c1cd009046caf6f75f",
    );
}
//use std::collections::HashMap;

use native::journey_bitmap::{Block, Tile};
use rand::prelude::*;

#[test]
fn test_merge() {
    println!("-----test merge------");

    let mut j1: JourneyBitmap = JourneyBitmap::new();
    for i in 0..2 {
        let t = gen_tile(i + 1, i + 1, 2, 5);
        let k = ((i + 1) as u16, (i + 1) as u16);
        j1.tiles.insert(k, t);
    }
    println!("{:?}", j1);

    let mut j2 = JourneyBitmap::new();
    for i in 0..1 {
        let t = gen_tile(i + 1, i + 1, 1, 6);
        let k = ((i + 1) as u16, (i + 1) as u16);
        j2.tiles.insert(k, t);
    }
    println!("{:?}", j2);

    j1.merge(j2);
    println!("{:?}", j1);

    println!("------end-----");
}

fn gen_tile(x: u8, y: u8, count_block: u8, val_block: u8) -> Tile {
    let mut tile = Tile::new(x as u16, y as u16);

    for i in 0..count_block {
        let b_x = x + val_block + i;
        let b_y = y + val_block + i;
        let block = gen_block(b_x, b_y);
        tile.blocks.insert((b_x, b_y), block);
    }

    tile
}

fn gen_block(x: u8, y: u8) -> Block {
    let data = gen_data_rand_512();
    let block = Block::new_with_data(x, y, data);
    block
}

fn gen_data_rand_512() -> [u8; 512] {
    let mut data: [u8; 512] = [0; 512];
    let mut rng = rand::thread_rng();
    for i in 0..512 {
        let r: u8 = rng.gen_range(0..=1);
        data[i] = r;
    }
    data
}

#[warn(dead_code)]
fn gen_data(len: u8) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut data = vec![];
    for _ in 0..len {
        let r: u8 = rng.gen_range(0..=1);
        data.push(r);
    }
    data
}
