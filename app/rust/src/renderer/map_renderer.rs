use flutter_rust_bridge::frb;
use journey_kernel::TileRangeEncoder;
use journey_kernel::FTA_COMPRESSION_ZSTD;

use super::render_tile_cache::{CachedRenderTile, RenderTileCache, RenderTileCacheKey};
use super::tile_rasterizer;
use crate::journey_area_utils;
use crate::journey_bitmap::{JourneyBitmap, TileKey};
use crate::utils;
use crate::utils::MapBounds;
use std::collections::HashMap;

#[frb(ignore)]
pub struct MapRenderer {
    journey_bitmap: JourneyBitmap,
    // Cached area per source tile, invalidated when that tile changes.
    tile_area_cache: HashMap<TileKey, i64>,
    render_tile_cache: RenderTileCache,
    version: u64,
    current_area: Option<u64>,
}

impl MapRenderer {
    pub fn new(journey_bitmap: JourneyBitmap) -> Self {
        Self {
            journey_bitmap,
            tile_area_cache: HashMap::new(),
            render_tile_cache: RenderTileCache::default(),
            version: 0,
            current_area: None,
        }
    }

    pub fn update<F>(&mut self, f: F)
    where
        F: FnOnce(&mut JourneyBitmap, &mut dyn FnMut(TileKey)),
    {
        let mut changed_tiles = Vec::new();
        f(&mut self.journey_bitmap, &mut |tile| {
            changed_tiles.push(tile)
        });
        if changed_tiles.is_empty() {
            return;
        }

        changed_tiles.sort_unstable();
        changed_tiles.dedup();
        for tile in &changed_tiles {
            self.tile_area_cache.remove(tile);
        }
        self.render_tile_cache
            .invalidate_changed_sources(&changed_tiles);
        self.mark_changed();
    }

    pub fn replace(&mut self, journey_bitmap: JourneyBitmap) {
        self.journey_bitmap = journey_bitmap;
        self.tile_area_cache.clear();
        self.render_tile_cache.clear();
        self.mark_changed();
    }

    fn mark_changed(&mut self) {
        self.version = self.version.wrapping_add(1);
        self.current_area = None;
    }

    pub fn get_current_version(&self) -> u64 {
        self.version
    }

    pub fn get_version_string(&self) -> String {
        format!("{:x}", self.version)
    }

    pub fn parse_version_string(version_str: &str) -> Option<u64> {
        // Remove quotes if present
        let cleaned = version_str.trim_matches('"');
        u64::from_str_radix(cleaned, 16).ok()
    }

    pub fn matches_version(&self, client_version: Option<&str>) -> bool {
        client_version.and_then(Self::parse_version_string) == Some(self.version)
    }

    pub fn peek_latest_bitmap(&self) -> &JourneyBitmap {
        &self.journey_bitmap
    }

    pub fn check_bitmap_invariant_and_debug_log(&mut self) {
        self.journey_bitmap.check_invariant_and_debug_log();
    }

    pub fn get_map_bounds(&mut self) -> Option<MapBounds> {
        utils::get_bounds_from_journey_bitmap(&mut self.journey_bitmap)
    }

    pub fn get_current_area(&mut self) -> u64 {
        *self.current_area.get_or_insert_with(|| {
            journey_area_utils::journey_bitmap_area_m2_rounded(
                &self.journey_bitmap,
                Some(&mut self.tile_area_cache),
            )
        })
    }

    pub fn get_tile_range_response(
        &mut self,
        x: i64,
        y: i64,
        z: i16,
        width: i64,
        height: i64,
        buffer_size_power: i16,
    ) -> Result<Vec<u8>, String> {
        validate_tile_range_request(y, z, width, height, buffer_size_power)?;

        let zoom_coefficient = 1i64 << z;
        let mut encoder = TileRangeEncoder::new(
            z as u8,
            x as i32,
            y as i32,
            width as u32,
            height as u32,
            buffer_size_power as u8,
            FTA_COMPRESSION_ZSTD,
        )?;

        for tile_y in y..(y + height) {
            for tile_x in x..(x + width) {
                let tile_x_rounded = tile_x.rem_euclid(zoom_coefficient);
                let cache_key = RenderTileCacheKey {
                    x: tile_x_rounded,
                    y: tile_y,
                    z,
                    buffer_size_power,
                };

                if let Some(tile) = self.render_tile_cache.get(&cache_key) {
                    encoder.push(tile.as_bitmap())?;
                } else {
                    let bitmap = tile_rasterizer::render_tile_bitmap(
                        &mut self.journey_bitmap,
                        tile_x_rounded,
                        tile_y,
                        z,
                        buffer_size_power,
                    );
                    let tile = CachedRenderTile::from_bitmap(bitmap);
                    encoder.push(tile.as_bitmap())?;
                    self.render_tile_cache.insert(cache_key, tile);
                }
            }
        }

        encoder.finish()
    }
}

fn validate_tile_range_request(
    y: i64,
    z: i16,
    width: i64,
    height: i64,
    buffer_size_power: i16,
) -> Result<(), String> {
    // Validate parameters to prevent overflow and invalid operations
    if width <= 0 || height <= 0 {
        return Err(format!(
            "Invalid dimensions: width={width}, height={height}"
        ));
    }

    if width > 20 || height > 20 {
        return Err(format!(
            "Dimensions too large: width={width}, height={height} (max: 20x20)"
        ));
    }

    if !(0..=16).contains(&z) {
        return Err(format!("Invalid zoom level: {z} (must be 0-16)"));
    }

    if !(6..=11).contains(&buffer_size_power) {
        return Err(format!(
            "Invalid buffer_size_power: {buffer_size_power} (must be 6-11, corresponding to 64-2048 pixel tiles)"
        ));
    }

    // Calculate mercator coordinate cycle length for zoom level z (used for validation and processing)
    let zoom_coefficient = 1i64 << z;

    // Validate coordinate bounds for the given zoom level
    if y < 0 || y >= zoom_coefficient {
        return Err(format!(
            "Invalid y coordinate: {} (must be 0-{})",
            y,
            zoom_coefficient - 1
        ));
    }

    Ok(())
}
