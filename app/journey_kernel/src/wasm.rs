use crate::tile_buffer::TileBuffer as TileBufferNative;
use wasm_bindgen::prelude::*;
extern crate console_error_panic_hook;
use wasm_bindgen::JsError;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
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
