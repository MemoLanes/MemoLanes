use super::geometry::{
    block_pixel_bounds, polar_pixel_to_lng_lat, polar_rect_radius_bounds_squared, BitmapPlane,
    MAP_WIDTH, MAP_WIDTH_OFFSET, POLAR_DISK_RADIUS, TILE_WIDTH, TILE_WIDTH_OFFSET,
};
use super::{BlockKey, JourneyBitmap, TileKey};

use crate::utils::{tile_x_y_to_lng_lat, MapBounds};

/// Returns geographic bounds containing every occupied block's valid coverage.
/// Longitude is circular: `east` may be greater than 180 degrees when that is
/// the narrow tile-level representation of a journey crossing the antimeridian.
pub fn get_bounds_from_journey_bitmap(journey_bitmap: &mut JourneyBitmap) -> Option<MapBounds> {
    let mercator_bounds = get_mercator_bounds(journey_bitmap);
    if !journey_bitmap.has_polar_data() {
        return mercator_bounds;
    }
    let mut bounds = BoundsAccumulator::new();
    if let Some(mercator) = mercator_bounds {
        bounds.include(mercator);
    }
    for plane in [BitmapPlane::NorthPolar, BitmapPlane::SouthPolar] {
        let tile_keys = journey_bitmap
            .all_plane_tile_keys(plane)
            .copied()
            .collect::<Vec<_>>();
        for tile_key in tile_keys {
            // A serialized tile's block-key bitset is enough; there is no need
            // to decompress its bitmap or populate the deserialized-tile cache.
            let summary = journey_bitmap.get_tile_summary(&tile_key)?;
            for block_key in summary.iter_blocks() {
                if let Some(block_bounds) = polar_block_bounds(tile_key, block_key) {
                    bounds.include(block_bounds);
                }
            }
        }
    }
    bounds.finish()
}

fn get_mercator_bounds(journey_bitmap: &mut JourneyBitmap) -> Option<MapBounds> {
    let block_zoom = (TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32;
    let mut occupied_tile_columns = [false; MAP_WIDTH as usize];
    let mut tile_y_bounds: Option<(u16, u16)> = None;
    let tile_coordinates: Vec<_> = journey_bitmap
        .all_tile_keys()
        .filter_map(|key| match *key {
            TileKey::Mercator { x, y } if x < MAP_WIDTH as u16 && y < MAP_WIDTH as u16 => {
                Some((x, y))
            }
            TileKey::Mercator { .. } | TileKey::NorthPolar { .. } | TileKey::SouthPolar { .. } => {
                None
            }
        })
        .collect();
    for &(x, y) in &tile_coordinates {
        occupied_tile_columns[x as usize] = true;
        tile_y_bounds = Some(match tile_y_bounds {
            Some((min_y, max_y)) => (min_y.min(y), max_y.max(y)),
            None => (y, y),
        });
    }
    let (north_tile_y, south_tile_y) = tile_y_bounds?;
    let occupied_columns: Vec<usize> = occupied_tile_columns
        .iter()
        .enumerate()
        .filter_map(|(x, occupied)| occupied.then_some(x))
        .collect();

    // Remove the largest empty gap on the circular x axis. The remaining arc is
    // the narrowest interval containing every occupied tile column.
    let mut largest_gap = 0;
    let mut west_tile_x = occupied_columns[0];
    let mut east_tile_x = occupied_columns[0];
    for (index, &current) in occupied_columns.iter().enumerate() {
        let next = if index + 1 < occupied_columns.len() {
            occupied_columns[index + 1]
        } else {
            occupied_columns[0] + MAP_WIDTH as usize
        };
        let gap = next - current - 1;
        if index == 0 || gap > largest_gap {
            largest_gap = gap;
            west_tile_x = next % MAP_WIDTH as usize;
            east_tile_x = current;
        }
    }

    let edge_tile_coordinates = tile_coordinates.into_iter().filter(|&(x, y)| {
        x as usize == west_tile_x
            || x as usize == east_tile_x
            || y == north_tile_y
            || y == south_tile_y
    });

    let mut west_block_x: Option<u8> = None;
    let mut east_block_x: Option<u8> = None;
    let mut north_block_y: Option<u8> = None;
    let mut south_block_y: Option<u8> = None;

    // Loading a tile may deserialize it, so inspect only tiles on the four
    // coarse edges instead of every tile in the journey.
    for (tile_x, tile_y) in edge_tile_coordinates {
        let tile = journey_bitmap.get_tile(&TileKey::Mercator {
            x: tile_x,
            y: tile_y,
        })?;
        for (block_key, _) in tile.iter() {
            if tile_x as usize == west_tile_x {
                west_block_x =
                    Some(west_block_x.map_or(block_key.x(), |current| current.min(block_key.x())));
            }
            if tile_x as usize == east_tile_x {
                east_block_x =
                    Some(east_block_x.map_or(block_key.x(), |current| current.max(block_key.x())));
            }
            if tile_y == north_tile_y {
                north_block_y =
                    Some(north_block_y.map_or(block_key.y(), |current| current.min(block_key.y())));
            }
            if tile_y == south_tile_y {
                south_block_y =
                    Some(south_block_y.map_or(block_key.y(), |current| current.max(block_key.y())));
            }
        }
    }

    let west_x = west_tile_x * TILE_WIDTH as usize + west_block_x? as usize;
    let mut east_x = east_tile_x * TILE_WIDTH as usize + east_block_x? as usize + 1;
    if east_x <= west_x {
        east_x += (MAP_WIDTH * TILE_WIDTH) as usize;
    }
    let north_y = north_tile_y as i32 * TILE_WIDTH as i32 + north_block_y? as i32;
    let south_y = south_tile_y as i32 * TILE_WIDTH as i32 + south_block_y? as i32 + 1;

    let (west, north) = tile_x_y_to_lng_lat(west_x as i32, north_y, block_zoom);
    let (east, south) = tile_x_y_to_lng_lat(east_x as i32, south_y, block_zoom);

    Some(MapBounds {
        west,
        south,
        east,
        north,
    })
}

fn polar_block_bounds(tile: TileKey, block: BlockKey) -> Option<MapBounds> {
    let (plane, tile_x, tile_y) = match tile {
        TileKey::NorthPolar { x, y } => (BitmapPlane::NorthPolar, x, y),
        TileKey::SouthPolar { x, y } => (BitmapPlane::SouthPolar, x, y),
        TileKey::Mercator { .. } => return None,
    };
    let radius = POLAR_DISK_RADIUS as f64;
    let (x, y) = block_pixel_bounds((tile_x, tile_y), (block.x(), block.y()));
    let (near_squared, far_squared) = polar_rect_radius_bounds_squared(x.clone(), y.clone());
    if near_squared >= POLAR_DISK_RADIUS * POLAR_DISK_RADIUS {
        return None;
    }
    let near = (near_squared as f64).sqrt();
    let far = (far_squared as f64).sqrt();
    let near_lat = polar_pixel_to_lng_lat(plane, radius + near, radius).1;
    let far_lat = polar_pixel_to_lng_lat(plane, radius + far.min(radius), radius).1;

    // A block cannot straddle the pole: the pole lies on block boundaries.
    // Its nonzero corners therefore delimit its complete angular footprint.
    let mut longitude_ranges = [
        (x.start, y.start),
        (x.start, y.end),
        (x.end, y.start),
        (x.end, y.end),
    ]
    .into_iter()
    .filter(|&(x, y)| x != POLAR_DISK_RADIUS || y != POLAR_DISK_RADIUS)
    .map(|(x, y)| {
        let lng = polar_pixel_to_lng_lat(plane, x as f64, y as f64).0;
        let angle = (lng + 180.0).rem_euclid(360.0);
        (angle, angle)
    })
    .collect::<Vec<_>>();
    longitude_ranges.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (west, east) = bounds_around_longitude_ranges(&longitude_ranges);
    Some(MapBounds {
        west,
        east,
        south: near_lat.min(far_lat),
        north: near_lat.max(far_lat),
    })
}

/// Sorted, disjoint longitude ranges in [0, 360]. Preserve interval interiors:
/// merging just their endpoints could discard a Mercator arc wider than 180°.
fn bounds_around_longitude_ranges(ranges: &[(f64, f64)]) -> (f64, f64) {
    let mut largest_gap = -1.0;
    let (mut west, mut span) = (0.0, 360.0);
    for (i, &(_, end)) in ranges.iter().enumerate() {
        let next_start =
            ranges[(i + 1) % ranges.len()].0 + if i + 1 == ranges.len() { 360.0 } else { 0.0 };
        if next_start - end > largest_gap {
            largest_gap = next_start - end;
            west = next_start.rem_euclid(360.0) - 180.0;
            span = 360.0 - largest_gap;
        }
    }
    if span >= 360.0 {
        (-180.0, 180.0)
    } else {
        (west, west + span)
    }
}

struct BoundsAccumulator {
    longitude_ranges: Vec<(f64, f64)>,
    south: f64,
    north: f64,
}

impl BoundsAccumulator {
    fn new() -> Self {
        Self {
            longitude_ranges: Vec::new(),
            south: f64::INFINITY,
            north: f64::NEG_INFINITY,
        }
    }

    fn include(&mut self, bounds: MapBounds) {
        self.south = self.south.min(bounds.south);
        self.north = self.north.max(bounds.north);
        let west = (bounds.west + 180.0).rem_euclid(360.0);
        let east = west + bounds.east - bounds.west;
        self.longitude_ranges.push((west, east.min(360.0)));
        if east > 360.0 {
            self.longitude_ranges.push((0.0, east - 360.0));
        }
    }

    fn finish(mut self) -> Option<MapBounds> {
        if self.longitude_ranges.is_empty() {
            return None;
        }
        self.longitude_ranges.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for range in self.longitude_ranges {
            if let Some(last) = merged.last_mut().filter(|last| last.1 >= range.0) {
                last.1 = last.1.max(range.1);
            } else {
                merged.push(range);
            }
        }
        let (west, east) = bounds_around_longitude_ranges(&merged);
        Some(MapBounds {
            west,
            east,
            south: self.south,
            north: self.north,
        })
    }
}
