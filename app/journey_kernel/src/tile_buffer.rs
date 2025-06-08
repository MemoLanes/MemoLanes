use crate::journey_bitmap::JourneyBitmap;
use crate::tile_shader2::TileShader2;
use serde::{Deserialize, Serialize};

/// Represents a buffer of map tiles with sparse pixel data
/// Stores tiles from (x,y) to (x+width-1, y+height-1) at zoom level z
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TileBuffer {
    /// Starting x coordinate of the tile range
    pub x: i64,
    /// Starting y coordinate of the tile range
    pub y: i64,
    /// Zoom level of the tiles
    pub z: i16,
    /// Number of tiles in the x direction
    pub width: i64,
    /// Number of tiles in the y direction
    pub height: i64,
    pub buffer_size_power: i16,
    /// Vector of tiles with sparse pixel data
    /// Index: (tile_y - y) * width + (tile_x - x)
    /// Each entry is a Vec of (a,b) pixel coordinates that have data
    pub tile_data: Vec<Vec<(u16, u16)>>,
}

impl TileBuffer {
    /// Create a new empty TileBuffer
    pub fn new(x: i64, y: i64, z: i16, width: i64, height: i64, buffer_size_power: i16) -> Self {
        Self {
            x,
            y,
            z,
            width,
            height,
            buffer_size_power,
            tile_data: vec![Vec::new(); (width * height) as usize],
        }
    }

    /// Create a new TileBuffer from a JourneyBitmap for a range of tiles
    pub fn from_journey_bitmap(
        journey_bitmap: &JourneyBitmap,
        x: i64,
        y: i64,
        z: i16,
        width: i64,
        height: i64,
        buffer_size_power: i16,
    ) -> Self {
        let mut buffer = Self::new(x, y, z, width, height, buffer_size_power);

        // Calculate mercator coordinate cycle length for zoom level z
        let zoom_coefficient = 1 << z;

        // For each tile in the range
        for tile_y in y..(y + height) {
            for tile_x in x..(x + width) {
                // Round off tile_x to ensure it's within mercator coordinate range (0 to 2^z-1)
                let tile_x_rounded =
                    ((tile_x % zoom_coefficient) + zoom_coefficient) % zoom_coefficient;

                // Get the pixels using TileShader2
                // start_x and start_y are 0, buffer_size_power is 8
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
                let tile_pixels = &mut buffer.tile_data[idx];

                // Convert from i64 coordinates to u16 coordinates for the TileBuffer
                for (px, py) in pixels {
                    // TODO: modify this / simplify this?
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

        buffer
    }

    /// Check if the given tile coordinates are within this buffer
    pub fn contains_tile(&self, tile_x: i64, tile_y: i64) -> bool {
        let zoom_coefficient = 1 << self.z;
        let x_offset = ((tile_x - self.x) % zoom_coefficient + zoom_coefficient) % zoom_coefficient;
        let y_offset = tile_y - self.y;

        x_offset >= 0 && x_offset < self.width && y_offset >= 0 && y_offset < self.height
    }

    /// Calculate the internal index for a tile
    pub fn calculate_tile_index(&self, tile_x: i64, tile_y: i64) -> usize {
        let zoom_coefficient = 1 << self.z;
        let x_offset = ((tile_x - self.x) % zoom_coefficient + zoom_coefficient) % zoom_coefficient;
        let y_offset = tile_y - self.y;

        (y_offset * self.width + x_offset) as usize
    }

    /// Get the pixels for a specific tile at the given coordinates and zoom level
    pub fn get_tile_pixels(
        &self,
        tile_x: i64,
        tile_y: i64,
        tile_z: i16,
        buffer_size_power: i16,
    ) -> Option<&Vec<(u16, u16)>> {
        println!(
            "get_tile_pixels: tile_x: {}, tile_y: {}, tile_z: {}, buffer_size_power: {}",
            tile_x, tile_y, tile_z, buffer_size_power
        );
        println!(
            "self.z: {}, self.buffer_size_power: {}",
            self.z, self.buffer_size_power
        );
        // Check if the tile is within this buffer and at the correct zoom level
        if tile_z != self.z
            || buffer_size_power != self.buffer_size_power
            || !self.contains_tile(tile_x, tile_y)
        {
            return None;
        }

        // Calculate the index and return the pixel data
        let idx = self.calculate_tile_index(tile_x, tile_y);
        Some(&self.tile_data[idx])
    }

    /// Serialize the TileBuffer to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|e| format!("Failed to serialize TileBuffer: {}", e))
    }
}
