pub mod bitmap2d;
pub mod tile_archive;
pub mod tile_iter;
pub mod tile_range;
pub mod utils;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use tile_range::TileRangeEncoder;

pub use tile_archive::FTA_COMPRESSION_DEFLATE;
pub use tile_archive::FTA_COMPRESSION_LZ4;
pub use tile_archive::FTA_COMPRESSION_NONE;
pub use tile_archive::FTA_COMPRESSION_ZSTD;
