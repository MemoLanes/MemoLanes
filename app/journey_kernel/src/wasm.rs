use crate::journey_bitmap::JourneyBitmap as JourneyBitmapNative;
use crate::tile_shader::TileShader;
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

#[wasm_bindgen]
impl JourneyBitmap {
    #[wasm_bindgen]
    pub fn get_tile_data(&self, x: i64, y: i64, z: i16, extension: &str) -> Vec<u8> {
        match extension {
            "imagedata" => TileShader::get_tile_image(&self.journey_bitmap, x, y, z),
            _ => panic!("Unsupported extension: {}", extension),
        }
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
