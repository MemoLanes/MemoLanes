pub mod map_renderer;
pub use map_renderer::{MapRenderer, RenderResult};

pub mod tile_renderer_basic;
pub mod tile_shader;
pub use tile_renderer_basic::TileRendererBasic;
pub use tile_renderer_basic::TileRendererTrait;

pub mod utils;

pub mod map_server;
pub use map_server::MapServer;

// TODO: currently this is just temp code for reexport the api function without influencing the frb code generation
use crate::api::api::CameraOption;
use crate::journey_kernel::JourneyBitmap;
pub fn get_default_camera_option_from_journey_bitmap(
    journey_bitmap: &JourneyBitmap,
) -> Option<CameraOption> {
    crate::api::api::get_default_camera_option_from_journey_bitmap(journey_bitmap)
}
