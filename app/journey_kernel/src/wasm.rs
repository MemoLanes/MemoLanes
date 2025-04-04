use crate::journey_bitmap::JourneyBitmap as JourneyBitmapNative;
use crate::tile_shader::TileShader;
use crate::utils::DEFAULT_TILE_SIZE;
use crate::utils::DEFAULT_TILE_SIZE_POWER;
use crate::utils::{DEFAULT_BG_COLOR, DEFAULT_FG_COLOR};
use wasm_bindgen::prelude::*;
extern crate console_error_panic_hook;
use image::RgbaImage;
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
    pub fn get_tile_image(&self, view_x: i64, view_y: i64, zoom: i16) -> Vec<u8> {
        let mut image = RgbaImage::new(DEFAULT_TILE_SIZE, DEFAULT_TILE_SIZE);
        TileShader::render_on_image(
            &mut image,
            0,
            0,
            &self.journey_bitmap,
            view_x,
            view_y,
            zoom,
            DEFAULT_TILE_SIZE_POWER,
            DEFAULT_BG_COLOR,
            DEFAULT_FG_COLOR,
        );
        image.into_vec()
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
