pub mod tile_buffer;
pub use tile_buffer::{decompress_tile_range_response, TileBuffer};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PixelType {
    Pixel32,
    Pixel64,
    Triangle64,
}

pub fn push_mercator_pixel(
    pixels: &mut Vec<f32>,
    pixel_type: PixelType,
    merc_x: f64,
    merc_y: f64,
    pixel_mercator_size: f64,
) {
    match pixel_type {
        PixelType::Pixel32 => add_f32_mercator_coordinates(pixels, merc_x, merc_y),
        PixelType::Pixel64 => add_f64_mercator_coordinates(pixels, merc_x, merc_y),
        PixelType::Triangle64 => {
            let delta = pixel_mercator_size;
            add_f64_mercator_coordinates(pixels, merc_x, merc_y);
            add_f64_mercator_coordinates(pixels, merc_x + delta, merc_y);
            add_f64_mercator_coordinates(pixels, merc_x, merc_y + delta);
            add_f64_mercator_coordinates(pixels, merc_x + delta, merc_y);
            add_f64_mercator_coordinates(pixels, merc_x + delta, merc_y + delta);
            add_f64_mercator_coordinates(pixels, merc_x, merc_y + delta);
        }
    }
}

pub(crate) fn add_f32_mercator_coordinates(pixels: &mut Vec<f32>, x: f64, y: f64) {
    pixels.push(x as f32);
    pixels.push(y as f32);
}

pub(crate) fn add_f64_mercator_coordinates(pixels: &mut Vec<f32>, x: f64, y: f64) {
    fn f32_residue(val: f64) -> f32 {
        (val - (val as f32 as f64)) as f32
    }
    pixels.push(x as f32);
    pixels.push(y as f32);
    pixels.push(f32_residue(x));
    pixels.push(f32_residue(y));
}
