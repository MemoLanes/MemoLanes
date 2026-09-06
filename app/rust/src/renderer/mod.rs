pub mod map_renderer;
pub use map_renderer::MapRenderer;

pub mod internal_server;

mod render_tile_cache;
mod tile_rasterizer;

// JourneyBitmap stores 512 × 512 source tiles.
const SOURCE_TILE_ZOOM: i16 = 9;
