use crate::journey_bitmap::JourneyBitmap as JourneyBitmapNative;
use crate::tile_buffer::TileBuffer as TileBufferNative;
use crate::tile_shader::TileShader;
use crate::tile_shader2::TileShader2;
use wasm_bindgen::prelude::*;
extern crate console_error_panic_hook;
use wasm_bindgen::JsError;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub struct JourneyBitmap {
    journey_bitmap: JourneyBitmapNative,
}

// TileBuffer wrapper for wasm
#[wasm_bindgen]
pub struct TileBuffer {
    tile_buffer: TileBufferNative,
}

#[wasm_bindgen]
impl TileBuffer {
    /// Create a TileBuffer from serialized bytes
    #[wasm_bindgen]
    pub fn from_bytes(bytes: &[u8]) -> Result<TileBuffer, JsError> {
        console_error_panic_hook::set_once();
        let tile_buffer = bincode::deserialize(bytes)
            .map_err(|e| JsError::new(&format!("Failed to deserialize TileBuffer: {}", e)))?;

        Ok(TileBuffer { tile_buffer })
    }

    /// Get pixels for a specific tile at the given coordinates
    /// Returns a flat array of u16 where every two elements represent an (x,y) coordinate
    /// Returns an empty array if the tile is not found in the buffer
    #[wasm_bindgen]
    pub fn get_tile_pixels(
        &self,
        tile_x: i64,
        tile_y: i64,
        tile_z: i16,
        buffer_size_power: i16,
    ) -> Vec<u16> {
        // Get the pixels from the native implementation
        if let Some(pixels) =
            self.tile_buffer
                .get_tile_pixels(tile_x, tile_y, tile_z, buffer_size_power)
        {
            // Convert the Vec<(u16, u16)> to a flat Vec<u16> for easier WASM interop
            let mut flat_pixels = Vec::with_capacity(pixels.len() * 2);
            for &(x, y) in pixels {
                flat_pixels.push(x);
                flat_pixels.push(y);
            }
            flat_pixels
        } else {
            // Return empty array if tile not found
            Vec::new()
        }
    }
}

#[wasm_bindgen]
impl JourneyBitmap {
    /// Get pixels for a specific tile at the given coordinates
    /// Returns a flat array of u16 where every two elements represent an (x,y) coordinate
    /// Returns an empty array if the coordinates are invalid
    #[wasm_bindgen]
    pub fn get_tile_pixels(
        &self,
        tile_x: i64,
        tile_y: i64,
        tile_z: i16,
        buffer_size_power: i16,
    ) -> Vec<u16> {
        // Convert parameters to the format expected by TileShader2
        let coordinates = TileShader2::get_pixels_coordinates(
            0,
            0,
            &self.journey_bitmap,
            tile_x,
            tile_y,
            tile_z,
            buffer_size_power,
        );

        // Convert the i64 coordinates to u16 for WASM compatibility
        // Skip any coordinates that are out of u16 range
        let mut flat_pixels = Vec::with_capacity(coordinates.len() * 2);
        for &(x, y) in &coordinates {
            if x >= 0 && x <= u16::MAX as i64 && y >= 0 && y <= u16::MAX as i64 {
                flat_pixels.push(x as u16);
                flat_pixels.push(y as u16);
            }
        }

        flat_pixels
    }

    #[wasm_bindgen]
    pub fn from_bytes(bytes: &[u8]) -> Result<JourneyBitmap, JsError> {
        console_error_panic_hook::set_once();
        let journey_bitmap =
            JourneyBitmapNative::from_bytes(bytes).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(JourneyBitmap { journey_bitmap })
    }

    #[wasm_bindgen]
    pub fn to_bytes(&self) -> Result<Vec<u8>, JsError> {
        self.journey_bitmap
            .to_bytes()
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

