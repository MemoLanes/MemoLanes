use crate::journey_bitmap::JourneyBitmap;
use crate::renderer::tile_shader::TileShader;
use crate::renderer::utils::TileSize;
use image::Rgba;
use image::RgbaImage;

pub trait TileRendererTrait {
    fn get_tile_size(&self) -> TileSize;

    fn render_image(
        &self,
        journey_bitmap: &JourneyBitmap,
        view_x: i64,
        view_y: i64,
        zoom: i16,
        bg_color: Rgba<u8>,
        fg_color: Rgba<u8>,
    ) -> RgbaImage {
        let mut image = RgbaImage::new(
            self.get_tile_size().size() as u32,
            self.get_tile_size().size() as u32,
        );
        self.render_on_image(
            &mut image,
            0,
            0,
            journey_bitmap,
            view_x,
            view_y,
            zoom,
            bg_color,
            fg_color,
        );
        image
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
    );
}

pub struct TileRendererBasic {
    tile_size: TileSize,
}

impl TileRendererBasic {
    pub fn new(tile_size: TileSize) -> Self {
        Self { tile_size }
    }
}

impl TileRendererTrait for TileRendererBasic {
    fn get_tile_size(&self) -> TileSize {
        self.tile_size
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
        debug_assert!(image.width() >= start_x + self.tile_size.size() as u32);
        debug_assert!(image.height() >= start_y + self.tile_size.size() as u32);

        TileShader::render_on_image(
            image,
            start_x,
            start_y,
            journey_bitmap,
            view_x,
            view_y,
            zoom,
            self.tile_size.power(),
            bg_color,
            fg_color,
        );
    }
}
