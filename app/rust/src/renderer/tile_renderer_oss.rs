use crate::journey_bitmap::JourneyBitmap;
use crate::renderer::utils::TileSize;
use crate::renderer::TileRendererBasic;
use crate::renderer::TileRendererTrait;
use image::GenericImage;
use image::Rgba;
use image::RgbaImage;
use imageproc::filter::gaussian_blur_f32;

pub struct TileRendererOss {
    renderer: TileRendererBasic,
}

impl TileRendererTrait for TileRendererOss {
    fn get_tile_size(&self) -> TileSize {
        self.renderer.get_tile_size()
    }

    fn render_on_image(
        &self,
        image: &mut RgbaImage,
        start_x: u32,
        start_y: u32,
        journey_bitmap: &JourneyBitmap,
        view_x: i64,
        view_y: i64,
        zoom: i16,
        bg_color: Rgba<u8>,
        fg_color: Rgba<u8>,
    ) {
        // check the image size
        debug_assert!(image.width() >= start_x + self.get_tile_size().size() as u32);
        debug_assert!(image.height() >= start_y + self.get_tile_size().size() as u32);

        // currently the gpu shading cannot be applied in-place
        let mut temp_image =
            self.renderer
                .render_image(journey_bitmap, view_x, view_y, zoom, bg_color, fg_color);

        // apply the original graphic enhancement
        // image dilation
        color_dilation2(&mut temp_image, fg_color);
        // gaussian blur
        let blurred_image = gaussian_blur_f32(&temp_image, 0.7);

        let _ = image.copy_from(&blurred_image, start_x, start_y);
    }
}

impl TileRendererOss {
    pub fn new(tile_size: TileSize) -> Self {
        let renderer = TileRendererBasic::new(tile_size);
        Self { renderer }
    }
}

// tracks dilation (morphology) according to l-2 norm and radius=1
pub fn color_dilation2(data2: &mut RgbaImage, color2: Rgba<u8>) {
    // Create a buffer to hold the indices of pixels that match the target color
    let mut matches = vec![];

    // Find all pixels that exactly match the target color
    for y in 0..data2.height() {
        for x in 0..data2.width() {
            if *data2.get_pixel(x, y) == color2 {
                matches.push((x, y));
            }
        }
    }

    // Apply dilation to all matched pixels
    for &(x, y) in &matches {
        data2.put_pixel(x, y, color2);
        // up
        if y < data2.height() - 1 {
            data2.put_pixel(x, y + 1, color2);
        }

        // down
        if y > 0 {
            data2.put_pixel(x, y - 1, color2);
        }

        // left
        if x > 0 {
            data2.put_pixel(x - 1, y, color2);
        }

        // right
        if x < data2.width() - 1 {
            data2.put_pixel(x + 1, y, color2);
        }
    }
}
