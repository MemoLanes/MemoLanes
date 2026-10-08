use crate::bitmap2d::BitMap2D;
use crate::tile_iter::PixelQuery;
use crate::tile_range::{
    decode_tile_range_response_to_grid,
    decompress_tile_range_response as core_decompress_tile_range_response,
};
use crate::utils::set_panic_hook;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
/// Decoded tile container built from TileRangeResponse wire-format bytes.
/// TileBuffer stores a set of tiles, and proxy the queries the requests to the tiles.
///   TileBuffer allows two groups of queries:
///   - get_tile_pixels: get pixel coordinates within a single tile(subtile or tile).
///   - query_range_pixels: query pixels within a range of tiles.
///
/// The wire format itself is defined in `crate::tile_range`.
pub struct TileBuffer {
    pub(super) grid_origin_x: i32,
    pub(super) grid_origin_y: i32,
    pub(super) grid_w: u16,
    pub(super) grid_h: u16,
    /// Row-major grid: index = (y - grid_origin_y) * grid_w + (x - grid_origin_x).
    /// Absent tiles are `None`.
    pub(super) tiles: Vec<Option<BitMap2D>>,
    pub(super) tile_grid_exp: u8,
    pub(super) tile_bitmap_exp: u8,
}

impl TileBuffer {
    /// Visit the packed pixel coordinates for one queried tile without first
    /// materializing an intermediate `Vec<u16>`.
    pub(super) fn for_each_tile_pixel<F>(
        &self,
        tile_x: i32,
        tile_y: i32,
        tile_z: u8,
        render_exp: u8,
        mut visit: F,
    ) where
        F: FnMut(u16, u16),
    {
        let Some(tiles_per_axis) = 1i64.checked_shl(tile_z as u32) else {
            return;
        };
        // y is always non-negative in web mercator; x can be negative for world wrapping
        if tile_y < 0 || tile_y as i64 >= tiles_per_axis {
            return;
        }

        let render_exp = self.clamped_query_render_exp(tile_z, render_exp);

        if tile_z >= self.tile_grid_exp {
            // Case 1: The queried tiles are smaller than the TileBuffer's internal tile grid.
            let dz = tile_z - self.tile_grid_exp;
            let parent_x = tile_x >> dz;
            let parent_y = tile_y >> dz;

            let Some(tile) = self.find_tile(parent_x, parent_y) else {
                return;
            };

            let child_mask = if dz == 0 { 0 } else { (1i32 << dz) - 1 };
            let child_x = (tile_x & child_mask) as i64;
            let child_y = (tile_y & child_mask) as i64;
            let child_z = dz as i16;
            for (px, py) in tile.iter_pixels(PixelQuery {
                origin: (0, 0),
                subtile: (child_x, child_y),
                zoom: child_z,
                resolution_exp: render_exp as i16,
            }) {
                if (0..=u16::MAX as i64).contains(&px) && (0..=u16::MAX as i64).contains(&py) {
                    visit(px as u16, py as u16);
                }
            }
            return;
        }

        let span = self.tile_grid_exp - tile_z;
        let subtiles_per_axis = 1u32 << span;
        let base_x = (tile_x as i64) << span;
        let base_y = (tile_y as i64) << span;

        let sub_render_exp = render_exp.checked_sub(span);
        for dy in 0..subtiles_per_axis {
            for dx in 0..subtiles_per_axis {
                let gx = base_x + i64::from(dx);
                let gy = base_y + i64::from(dy);
                let (Ok(gx), Ok(gy)) = (i32::try_from(gx), i32::try_from(gy)) else {
                    continue;
                };
                let Some(tile) = self.find_tile(gx, gy) else {
                    continue;
                };
                if let Some(exp) = sub_render_exp {
                    // Each source tile contributes multiple output pixels.
                    for (px, py) in tile.iter_pixels(PixelQuery::full_tile(exp as i16)) {
                        let out_x = (dx << exp) + px as u32;
                        let out_y = (dy << exp) + py as u32;
                        if out_x <= u16::MAX as u32 && out_y <= u16::MAX as u32 {
                            visit(out_x as u16, out_y as u16);
                        }
                    }
                } else if !tile.is_empty() {
                    // Multiple source tiles contribute occupancy to one output pixel.
                    let coarse_shift = span - render_exp;
                    let out_x = dx >> coarse_shift;
                    let out_y = dy >> coarse_shift;
                    if out_x <= u16::MAX as u32 && out_y <= u16::MAX as u32 {
                        visit(out_x as u16, out_y as u16);
                    }
                }
            }
        }
    }
}

#[wasm_bindgen]
impl TileBuffer {
    pub(super) fn find_tile(&self, grid_x: i32, grid_y: i32) -> Option<&BitMap2D> {
        // X-wrap normalization: the query x may be offset by multiples of the world
        // size (1 << tile_grid_exp) due to multi-world-copy rendering or antimeridian
        // crossing during drag. Use Euclidean modulo to map any x that has a
        // modular-equivalent copy inside the buffer range back into that range.
        let world_size = 1i64 << self.tile_grid_exp;
        let dx = (i64::from(grid_x) - i64::from(self.grid_origin_x)).rem_euclid(world_size);
        let dy = i64::from(grid_y) - i64::from(self.grid_origin_y);
        if dx >= i64::from(self.grid_w) || dy < 0 || dy >= i64::from(self.grid_h) {
            return None;
        }
        self.tiles[dy as usize * self.grid_w as usize + dx as usize].as_ref()
    }

    pub(super) fn clamped_query_render_exp(&self, tile_z: u8, requested_render_exp: u8) -> u8 {
        let world_detail_exp = self.tile_grid_exp as i16 + self.tile_bitmap_exp as i16;
        let max_render_exp = (world_detail_exp - tile_z as i16).max(0) as u8;
        requested_render_exp.min(max_render_exp)
    }

    #[wasm_bindgen]
    /// Query tile buffer for pixels within a single tile(subtile or tile).
    pub fn get_tile_pixels(
        &self,
        tile_x: i32,
        tile_y: i32,
        tile_z: u8,
        render_exp: u8,
    ) -> Vec<u16> {
        let mut packed = Vec::new();
        self.for_each_tile_pixel(tile_x, tile_y, tile_z, render_exp, |px, py| {
            packed.push(px);
            packed.push(py);
        });
        packed
    }

    #[wasm_bindgen]
    /// Parses raw TileRangeResponse bytes returned by the `/tile-range` endpoint.
    ///
    /// `data` must match the binary format documented in `crate::tile_range`.
    pub fn new_from_tile_range_response(data: &[u8]) -> Result<TileBuffer, JsValue> {
        set_panic_hook();
        let (header, tiles) = decode_tile_range_response_to_grid(data)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse TileRangeResponse: {}", e)))?;
        let grid_w = header.range_w;
        let grid_h = header.range_h;
        Ok(TileBuffer {
            grid_origin_x: header.x0,
            grid_origin_y: header.y0,
            grid_w,
            grid_h,
            tiles,
            tile_grid_exp: header.z,
            tile_bitmap_exp: header.tile_bitmap_exp,
        })
    }

    #[wasm_bindgen]
    pub fn tile_count(&self) -> u32 {
        self.tiles.iter().filter(|t| t.is_some()).count() as u32
    }

    #[wasm_bindgen]
    pub fn total_pixel_count(&self) -> u32 {
        let mut count = 0u32;
        for bm in self.tiles.iter().filter_map(|t| t.as_ref()) {
            count += bm
                .iter_pixels(PixelQuery::full_tile(self.tile_bitmap_exp as i16))
                .count() as u32;
        }
        count
    }

    /// Split range query into tile queries and merge the results.
    #[wasm_bindgen]
    pub fn query_range_pixels(
        &self,
        x: i32,
        y: i32,
        z: u8,
        w: u32,
        h: u32,
        render_exp: u8,
    ) -> Vec<u16> {
        let mut out = Vec::new();
        for (tile_x, tile_y) in tile_coordinates(x, y, w, h) {
            self.for_each_tile_pixel(tile_x, tile_y, z, render_exp, |px, py| {
                out.push(px);
                out.push(py);
            });
        }
        out
    }
}

#[wasm_bindgen]
pub fn decompress_tile_range_response(data: &[u8]) -> Result<Vec<u8>, JsValue> {
    core_decompress_tile_range_response(data)
        .map_err(|e| JsValue::from_str(&format!("Failed to decompress TileRangeResponse: {e}")))
}

/// Row-major coordinates, clipped to the public i32 coordinate representation.
/// An i32 origin plus a u32 extent always fits in i64.
pub(super) fn tile_coordinates(x: i32, y: i32, w: u32, h: u32) -> impl Iterator<Item = (i32, i32)> {
    let x_end = (i64::from(x) + i64::from(w)).min(i64::from(i32::MAX) + 1);
    let h = if w == 0 { 0 } else { h };
    let y_end = (i64::from(y) + i64::from(h)).min(i64::from(i32::MAX) + 1);
    (i64::from(y)..y_end)
        .flat_map(move |ty| (i64::from(x)..x_end).map(move |tx| (tx as i32, ty as i32)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TileRangeEncoder, FTA_COMPRESSION_NONE};

    #[test]
    fn range_coordinates_keep_row_order_and_clip_without_overflow() {
        assert_eq!(
            tile_coordinates(-1, 2, 2, 2).collect::<Vec<_>>(),
            [(-1, 2), (0, 2), (-1, 3), (0, 3)]
        );
        assert_eq!(
            tile_coordinates(i32::MAX, i32::MAX, u32::MAX, u32::MAX).collect::<Vec<_>>(),
            [(i32::MAX, i32::MAX)]
        );
        assert_eq!(tile_coordinates(0, 0, 0, u32::MAX).next(), None);
        assert_eq!(tile_coordinates(0, 0, u32::MAX, 0).next(), None);
    }

    #[test]
    fn wrapped_and_coarse_queries_preserve_pixels() {
        let mut bitmap = BitMap2D::new(3);
        bitmap.set(0, 0, true);
        bitmap.set(7, 7, true);
        bitmap.build_lods();
        let mut encoder = TileRangeEncoder::new(2, -1, 1, 2, 1, 3, FTA_COMPRESSION_NONE).unwrap();
        encoder.push(Some(&bitmap)).unwrap();
        encoder.push(None).unwrap();
        let buffer = TileBuffer::new_from_tile_range_response(&encoder.finish().unwrap()).unwrap();
        assert_eq!(buffer.get_tile_pixels(-1, 1, 2, 3), [0, 0, 7, 7]);
        assert_eq!(buffer.get_tile_pixels(3, 1, 2, 3), [0, 0, 7, 7]);
        assert_eq!(buffer.get_tile_pixels(-2, 2, 3, 3), [0, 0]);
        assert_eq!(buffer.get_tile_pixels(-1, 0, 1, 3), [4, 4, 7, 7]);
        assert_eq!(buffer.get_tile_pixels(-1, 0, 0, 0), [0, 0]);
        assert!(buffer.get_tile_pixels(0, 1, 2, 3).is_empty());
    }
}
