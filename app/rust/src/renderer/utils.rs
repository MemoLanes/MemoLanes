use image::Rgba;
use image::RgbaImage;
use std::io::Cursor;

pub const DEFAULT_BG_COLOR: Rgba<u8> = Rgba([0, 0, 0, 127]);
pub const DEFAULT_FG_COLOR: Rgba<u8> = Rgba([0, 0, 0, 0]);
pub const DEFAULT_TILE_SIZE: TileSize = TileSize::TileSize512;

#[derive(Debug, Copy, Clone)]
pub enum TileSize {
    TileSize256,
    TileSize512,
    TileSize1024,
}

impl TileSize {
    pub fn size(&self) -> u32 {
        match self {
            TileSize::TileSize256 => 256,
            TileSize::TileSize512 => 512,
            TileSize::TileSize1024 => 1024,
        }
    }

    pub fn power(&self) -> i16 {
        match self {
            TileSize::TileSize256 => 8,
            TileSize::TileSize512 => 9,
            TileSize::TileSize1024 => 10,
        }
    }
}

pub fn image_to_png_data(image: &RgbaImage) -> Vec<u8> {
    let mut image_png: Vec<u8> = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut image_png), image::ImageFormat::Png)
        .unwrap();
    image_png
}
