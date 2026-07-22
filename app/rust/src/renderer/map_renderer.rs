use flutter_rust_bridge::frb;
use journey_kernel::encode_tile_range_response_from_tiles;
use journey_kernel::TilePixelData;
use journey_kernel::FTA_COMPRESSION_ZSTD;

use crate::journey_area_utils;
use crate::journey_bitmap::{JourneyBitmap, TileKey};
use crate::renderer::tile_shader2::TileShader2;
use crate::utils;
use crate::utils::MapBounds;
use std::collections::HashMap;

#[frb(ignore)]
pub struct MapRenderer {
    journey_bitmap: JourneyBitmap,
    /* for each tile of 512*512 tiles in a JourneyBitmap, use buffered area to record any update */
    tile_area_cache: HashMap<TileKey, f64>,
    version: u64,
    current_area: Option<u64>,
}

impl MapRenderer {
    pub fn new(journey_bitmap: JourneyBitmap) -> Self {
        Self {
            journey_bitmap,
            tile_area_cache: HashMap::new(),
            version: 0,
            current_area: None,
        }
    }

    pub fn update<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut JourneyBitmap, &mut dyn FnMut(TileKey)),
    {
        // Collect changed tile positions first
        let mut changed_tiles = Vec::new();
        let mut tile_changed = |tile_pos: TileKey| {
            changed_tiles.push(tile_pos);
        };

        // Apply the update function
        f(&mut self.journey_bitmap, &mut tile_changed);

        for tile_pos in changed_tiles {
            self.tile_area_cache.remove(&tile_pos);
        }

        // TODO: we should improve the cache invalidation rule
        self.reset();
    }

    pub fn replace(&mut self, journey_bitmap: JourneyBitmap) {
        self.journey_bitmap = journey_bitmap;
        self.tile_area_cache.clear();
        self.reset();
    }

    fn reset(&mut self) {
        self.version = self.version.wrapping_add(1);
        self.current_area = None;
    }

    pub fn get_current_version(&self) -> u64 {
        self.version
    }

    pub fn get_version_string(&self) -> String {
        format!("{:x}", self.version)
    }

    pub fn parse_version_string(version_str: &str) -> Option<u64> {
        // Remove quotes if present
        let cleaned = version_str.trim_matches('"');
        u64::from_str_radix(cleaned, 16).ok()
    }

    // TODO: deprecate this method and merge it with `get_tile_buffer`.
    pub fn get_latest_bitmap_if_changed(
        &self,
        client_version: Option<&str>,
    ) -> Option<(&JourneyBitmap, String)> {
        match client_version {
            Some(v_str) if (Self::parse_version_string(v_str) == Some(self.version)) => None,
            _ => Some((&self.journey_bitmap, self.get_version_string())),
        }
    }

    pub fn peek_latest_bitmap(&self) -> &JourneyBitmap {
        &self.journey_bitmap
    }

    pub fn get_map_bounds(&mut self) -> Option<MapBounds> {
        utils::get_bounds_from_journey_bitmap(&mut self.journey_bitmap)
    }

    pub fn get_current_area(&mut self) -> u64 {
        *self.current_area.get_or_insert_with(|| {
            journey_area_utils::compute_journey_bitmap_area(
                &self.journey_bitmap,
                Some(&mut self.tile_area_cache),
            )
        })
    }

    pub fn get_tile_range_response(
        &mut self,
        x: i64,
        y: i64,
        z: i16,
        width: i64,
        height: i64,
        buffer_size_power: i16,
    ) -> Result<Vec<u8>, String> {
        tile_range_response_from_journey_bitmap(
            &mut self.journey_bitmap,
            x,
            y,
            z,
            width,
            height,
            buffer_size_power,
        )
    }
}

fn tile_range_response_from_journey_bitmap(
    journey_bitmap: &mut JourneyBitmap,
    x: i64,
    y: i64,
    z: i16,
    width: i64,
    height: i64,
    buffer_size_power: i16,
) -> Result<Vec<u8>, String> {
    // Validate parameters to prevent overflow and invalid operations
    if width <= 0 || height <= 0 {
        return Err(format!(
            "Invalid dimensions: width={width}, height={height}"
        ));
    }

    if width > 20 || height > 20 {
        return Err(format!(
            "Dimensions too large: width={width}, height={height} (max: 20x20)"
        ));
    }

    if !(0..=16).contains(&z) {
        return Err(format!("Invalid zoom level: {z} (must be 0-16)"));
    }

    if !(6..=11).contains(&buffer_size_power) {
        return Err(format!(
            "Invalid buffer_size_power: {buffer_size_power} (must be 6-11, corresponding to 64-2048 pixel tiles)"
        ));
    }

    // Calculate mercator coordinate cycle length for zoom level z (used for validation and processing)
    let zoom_coefficient = 1i64 << z;

    // Validate coordinate bounds for the given zoom level
    if y < 0 || y >= zoom_coefficient {
        return Err(format!(
            "Invalid y coordinate: {} (must be 0-{})",
            y,
            zoom_coefficient - 1
        ));
    }

    let mut tiles = Vec::new();

    // For each tile in the range
    for tile_y in y..(y + height) {
        for tile_x in x..(x + width) {
            // Round off tile_x to ensure it's within mercator coordinate range (0 to 2^z-1)
            let tile_x_rounded =
                ((tile_x % zoom_coefficient) + zoom_coefficient) % zoom_coefficient;

            let bitmap = TileShader2::render_tile_bitmap(
                journey_bitmap,
                tile_x_rounded,
                tile_y,
                z,
                buffer_size_power,
            );

            let tile_pixel_data = TilePixelData {
                x: tile_x as i32,
                y: tile_y as i32,
                bitmap,
            };

            tiles.push(tile_pixel_data);
        }
    }

    encode_tile_range_response_from_tiles(
        z as u8,
        x as i32,
        y as i32,
        width as u32,
        height as u32,
        buffer_size_power as u8,
        FTA_COMPRESSION_ZSTD,
        tiles,
    )
}
