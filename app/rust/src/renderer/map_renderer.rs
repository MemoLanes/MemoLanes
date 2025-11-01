use journey_kernel::TileBuffer;

use crate::journey_area_utils;
use crate::journey_bitmap::{JourneyBitmap, Tile};
use crate::renderer::tile_shader2::TileShader2;
use std::collections::HashMap;

pub struct MapRenderer {
    journey_bitmap: JourneyBitmap,
    /* for each tile of 512*512 tiles in a JourneyBitmap, use buffered area to record any update */
    tile_area_cache: HashMap<(u16, u16), f64>,
    version: u64,
    current_area: Option<u64>,
}

impl MapRenderer {
    pub fn new(journey_bitmap: JourneyBitmap) -> Self {
        let mut journey_bitmap = journey_bitmap;
        Self::prepare_journey_bitmap_for_rendering(&mut journey_bitmap);
        Self {
            journey_bitmap,
            tile_area_cache: HashMap::new(),
            version: 0,
            current_area: None,
        }
    }

    fn prepare_journey_bitmap_for_rendering(journey_bitmap: &mut JourneyBitmap) {
        for tile in journey_bitmap.tiles.values_mut() {
            Self::prepare_tiles_for_rendering(tile);
        }
    }

    fn prepare_tiles_for_rendering(tile: &mut Tile) {
        // just a place holder for future implementation
    }

    pub fn update<F>(&mut self, f: F)
    where
        F: Fn(&mut JourneyBitmap, &mut dyn FnMut((u16, u16))),
    {
        // Collect changed tile positions first
        let mut changed_tiles = Vec::new();
        let mut tile_changed = |tile_pos: (u16, u16)| {
            changed_tiles.push(tile_pos);
        };
        
        // Apply the update function
        f(&mut self.journey_bitmap, &mut tile_changed);
        
        // Now prepare tiles for rendering for all changed tiles
        for tile_pos in changed_tiles {
            if let Some(tile) = self.journey_bitmap.tiles.get_mut(&tile_pos) {
                Self::prepare_tiles_for_rendering(tile);
            }
            // Invalidate cache for this tile
            self.tile_area_cache.remove(&tile_pos);
        }
        
        // TODO: we should improve the cache invalidation rule
        self.reset();
    }

    pub fn replace(&mut self, journey_bitmap: JourneyBitmap) {
        let mut journey_bitmap = journey_bitmap;
        Self::prepare_journey_bitmap_for_rendering(&mut journey_bitmap);
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

    pub fn get_current_area(&mut self) -> u64 {
        *self.current_area.get_or_insert_with(|| {
            journey_area_utils::compute_journey_bitmap_area(
                &self.journey_bitmap,
                Some(&mut self.tile_area_cache),
            )
        })
    }

    pub fn get_tile_buffer(
        &self,
        x: i64,
        y: i64,
        z: i16,
        width: i64,
        height: i64,
        buffer_size_power: i16,
    ) -> Result<TileBuffer, String> {
        tile_buffer_from_journey_bitmap(&self.journey_bitmap, x, y, z, width, height, buffer_size_power)
    }
}

/// Create a new TileBuffer from a JourneyBitmap for a range of tiles
fn tile_buffer_from_journey_bitmap(
    journey_bitmap: &JourneyBitmap,
    x: i64,
    y: i64,
    z: i16,
    width: i64,
    height: i64,
    buffer_size_power: i16,
) -> Result<TileBuffer, String> {
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

    if !(0..=25).contains(&z) {
        return Err(format!("Invalid zoom level: {z} (must be 0-25)"));
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

    // Create buffer with validated parameters
    let mut buffer = TileBuffer {
        x,
        y,
        z,
        width,
        height,
        buffer_size_power,
        tile_data: vec![Vec::new(); (width * height) as usize],
    };

    // For each tile in the range
    for tile_y in y..(y + height) {
        for tile_x in x..(x + width) {
            // Round off tile_x to ensure it's within mercator coordinate range (0 to 2^z-1)
            let tile_x_rounded =
                ((tile_x % zoom_coefficient) + zoom_coefficient) % zoom_coefficient;

            // Get the pixels using TileShader2
            let pixels = TileShader2::get_pixels_coordinates(
                0,
                0,
                journey_bitmap,
                tile_x_rounded,
                tile_y,
                z,
                buffer_size_power,
            );

            // Convert to tile-relative coordinates and add to buffer
            let idx = buffer.calculate_tile_index(tile_x, tile_y);

            // Bounds check for safety (should never fail with our validation above)
            if idx >= buffer.tile_data.len() {
                return Err(format!(
                    "Index out of bounds: {} >= {}",
                    idx,
                    buffer.tile_data.len()
                ));
            }

            let tile_pixels = &mut buffer.tile_data[idx];

            // Convert from i64 coordinates to u16 coordinates for the TileBuffer
            for (px, py) in pixels {
                if px >= 0
                    && px < (1 << buffer_size_power)
                    && py >= 0
                    && py < (1 << buffer_size_power)
                {
                    // Only add if not already present
                    let pixel = (px as u16, py as u16);
                    tile_pixels.push(pixel);
                }
            }
        }
    }

    Ok(buffer)
}
