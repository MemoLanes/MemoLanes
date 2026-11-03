use super::geometry::{
    plane_for_latitude, polar_pixel_size_meters, polar_rect_radius_bounds_squared, polar_row_span,
    project_mercator, project_polar_laea, unwrap_mercator_x, EQUATORIAL_PIXEL_SIZE_METERS,
    MERCATOR_WORLD_PIXEL_OFFSET, MERCATOR_WORLD_PIXEL_WIDTH, POLAR_DISK_RADIUS,
    POLAR_WORLD_PIXEL_WIDTH,
};
use super::storage::TileStorage;
use super::{
    BitmapPlane, JourneyBitmap, Tile, TileKey, BITMAP_WIDTH_OFFSET, MAP_WIDTH,
    MERCATOR_MAX_LATITUDE, TILE_WIDTH_OFFSET,
};
use crate::{
    spherical_geometry::{GreatCircleArc, UnitPoint, EARTH_RADIUS_METERS},
    utils,
};
use anyhow::Result;
use std::collections::HashSet;
use std::ops::Range;

const ALL_OFFSET: i16 = TILE_WIDTH_OFFSET + BITMAP_WIDTH_OFFSET;
const SHORT_ARC_ANGLE_RADIANS: f64 = 1e-5;

#[derive(Clone, Copy, PartialEq, Eq)]
enum StrokeBrush {
    Square,
    DominantAxis,
}

/// Walk a stroke independently of storage, emitting complete brush rectangles.
fn rasterize_stroke(
    start: (i64, i64),
    end: (i64, i64),
    brush: StrokeBrush,
    mut width_at_y: impl FnMut(i64) -> u8,
    mut fill_rect: impl FnMut(Range<i64>, Range<i64>),
) {
    // A fixed direction also makes Bresenham's tie-breaking reversible.
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    let (mut x, mut y) = start;
    let dx = (end.0 - x).abs();
    let dy = -(end.1 - y).abs();
    let step_x = (end.0 - x).signum();
    let step_y = (end.1 - y).signum();
    let mut error = dx + dy;
    let x_dominant = dx >= -dy;
    let mut run_start = start;
    let mut run_width = width_at_y(y);
    // Consecutive brush spans on one row/column form a single rectangle.
    // Batch those runs before partitioning them into storage tiles.
    let mut fill_run = |start: (i64, i64), end: (i64, i64), width: u8| {
        let before = i64::from(width) / 2;
        let after = i64::from(width - 1) / 2;
        let (before_x, after_x) = if brush == StrokeBrush::Square || !x_dominant {
            (before, after)
        } else {
            (0, 0)
        };
        let (before_y, after_y) = if brush == StrokeBrush::Square || x_dominant {
            (before, after)
        } else {
            (0, 0)
        };
        fill_rect(
            start.0.min(end.0) - before_x..start.0.max(end.0) + after_x + 1,
            start.1.min(end.1) - before_y..start.1.max(end.1) + after_y + 1,
        );
    };

    while (x, y) != end {
        let previous = (x, y);
        let twice_error = 2 * error;
        if twice_error >= dy {
            error += dy;
            x += step_x;
        }
        if twice_error <= dx {
            error += dx;
            y += step_y;
        }
        let width = width_at_y(y);
        let run_ended = width != run_width
            || if x_dominant {
                y != previous.1
            } else {
                x != previous.0
            };
        if run_ended {
            fill_run(run_start, previous, run_width);
            run_start = (x, y);
            run_width = width;
        }
    }
    fill_run(run_start, end, run_width);
    if brush == StrokeBrush::DominantAxis {
        // Caps belong to the actual segment endpoints, never to storage
        // boundaries. Each cap is clipped/partitioned just like a stroke.
        for (x, y) in [start, end] {
            let width = i64::from(width_at_y(y));
            fill_rect(
                x - width / 2..x - width / 2 + width,
                y - width / 2..y - width / 2 + width,
            );
        }
    }
}

fn interpolate_x_at_y(x0: i32, y0: i32, x1: i32, y1: i32, y: i32) -> i32 {
    debug_assert_ne!(y0, y1);
    let t = (y as f64 - y0 as f64) / (y1 as f64 - y0 as f64);
    (x0 as f64 + (x1 as f64 - x0 as f64) * t).round() as i32
}

/// Test the shape rather than angular parameter spacing: meridians and the
/// equator project to straight lines even on long spherical segments.
fn mercator_arc_has_visible_curvature(arc: GreatCircleArc) -> bool {
    // Even at the Mercator edge, a <64-metre arc's curvature is far below
    // a quarter of a native cell. Avoid projection work for live GPS fixes.
    if arc.angle < SHORT_ARC_ANGLE_RADIANS {
        return false;
    }
    let project = |point: UnitPoint| {
        let (lng, lat) = point.to_lng_lat();
        project_mercator(lng, lat)
    };
    let a = project(arc.at(0.0));
    let mut b = project(arc.at(1.0));
    b.0 = unwrap_mercator_x(b.0, a.0);
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return false;
    }
    [0.25, 0.5, 0.75].into_iter().any(|t| {
        let mut p = project(arc.at(t));
        p.0 = unwrap_mercator_x(p.0, a.0);
        let along = ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length_squared;
        let along = along.clamp(0.0, 1.0);
        (p.0 - a.0 - along * dx).hypot(p.1 - a.1 - along * dy) > 0.25
    })
}

fn polar_brush_width() -> u8 {
    (EQUATORIAL_PIXEL_SIZE_METERS / polar_pixel_size_meters())
        .round()
        .clamp(1.0, u8::MAX as f64) as u8
}

/// Include every centre whose square brush can reach the adjacent chart.
/// The extra pixel accounts for centre rounding and partially covered cells.
fn seam_overlap_degrees() -> f64 {
    let polar_reach = (f64::from(polar_brush_width()) / 2.0 + 1.0)
        * polar_pixel_size_meters()
        * std::f64::consts::SQRT_2;
    let mercator_scale = MERCATOR_MAX_LATITUDE.to_radians().cos();
    let mercator_reach = ((1.0 / mercator_scale).round() / 2.0 + 1.0)
        * EQUATORIAL_PIXEL_SIZE_METERS
        * mercator_scale;
    (polar_reach.max(mercator_reach) / EARTH_RADIUS_METERS).to_degrees()
}

/// The scoped entry point for bitmap content changes.
///
/// Change tracking is optional and lasts only for this edit. Renderers handle
/// complete bitmap replacement separately.
pub struct BitmapEditor<'a> {
    bitmap: &'a mut JourneyBitmap,
    changed_tiles: Option<HashSet<TileKey>>,
}

impl<'a> BitmapEditor<'a> {
    pub(super) fn new(bitmap: &'a mut JourneyBitmap, track_changes: bool) -> Self {
        Self {
            bitmap,
            changed_tiles: track_changes.then(HashSet::new),
        }
    }

    /// End this edit and return its collected keys. Untracked edits return an
    /// empty set. Mutations take effect immediately; this only ends the borrow.
    pub fn finish(self) -> HashSet<TileKey> {
        self.changed_tiles.unwrap_or_default()
    }

    fn mark_changed(&mut self, key: TileKey) {
        if let Some(changes) = &mut self.changed_tiles {
            changes.insert(key);
        }
    }

    /// Mutable access conservatively reports a change even if the caller does
    /// not write. Rasterizers use the private mutation helper to detect no-ops.
    pub fn get_tile_mut(&mut self, key: &TileKey) -> Option<&mut Tile> {
        if !self.bitmap.contains_tile(key) {
            return None;
        }
        self.mark_changed(*key);
        self.bitmap.storage.get_tile_mut(key)
    }

    pub fn get_tile_mut_or_insert_empty(&mut self, key: &TileKey) -> &mut Tile {
        assert!(key.is_in_bounds(), "out-of-bounds bitmap tile: {key:?}");
        if !self.bitmap.contains_tile(key) {
            self.insert_tile(key, Tile::new());
        }
        self.get_tile_mut(key).unwrap()
    }

    fn mutate_tile_or_insert_empty<R, F>(&mut self, key: &TileKey, f: F) -> (R, bool)
    where
        F: FnOnce(&mut Tile) -> (R, bool),
    {
        assert!(key.is_in_bounds(), "out-of-bounds bitmap tile: {key:?}");
        let (result, changed) = self.bitmap.storage.mutate_or_insert_empty(key, f);
        if changed {
            self.mark_changed(*key);
        }
        (result, changed)
    }

    pub fn insert_tile(&mut self, key: &TileKey, tile: Tile) {
        if !key.is_in_bounds() {
            warn!(
                "Discarding out-of-bounds bitmap tile {key:?}; grid width is {}",
                key.plane().grid_width()
            );
            return;
        }
        self.bitmap.storage.insert_tile(*key, tile);
        self.mark_changed(*key);
    }

    /// Eager storage may reject malformed bytes immediately. With lazy storage,
    /// validate imported data before accessing its decoded contents.
    pub fn insert_tile_bytes(&mut self, key: TileKey, bytes: Vec<u8>) -> Result<()> {
        if !key.is_in_bounds() {
            warn!(
                "Discarding out-of-bounds bitmap tile {key:?}; grid width is {}",
                key.plane().grid_width()
            );
            return Ok(());
        }
        self.bitmap.storage.insert_bytes(key, bytes)?;
        self.mark_changed(key);
        Ok(())
    }

    /// Verify imported tile data and prune empty tiles and blocks.
    pub fn validate(&mut self) -> Result<()> {
        for key in self.bitmap.all_tile_keys().copied().collect::<Vec<_>>() {
            if self.bitmap.storage.validate_tile(&key)? {
                self.mark_changed(key);
            }
        }
        Ok(())
    }

    pub fn merge(&mut self, other: JourneyBitmap) {
        for (key, entry) in other.storage.into_entries() {
            match self.get_tile_mut(&key) {
                None => {
                    self.bitmap.storage.insert_entry(key, entry);
                    self.mark_changed(key);
                }
                Some(tile) => tile.merge(TileStorage::decode_entry(entry)),
            }
        }
    }

    pub fn merge_with_partial_clone(&mut self, other: &JourneyBitmap) {
        for key in other.all_tile_keys() {
            match self.get_tile_mut(key) {
                None => {
                    self.bitmap
                        .storage
                        .insert_entry(*key, other.storage.clone_entry(key));
                    self.mark_changed(*key);
                }
                Some(tile) => other.peek_tile_without_updating_cache(key, |other| {
                    tile.merge_with_partial_clone(other.unwrap());
                }),
            }
        }
    }

    pub fn difference(&mut self, other: &JourneyBitmap) {
        for key in other.all_tile_keys() {
            if let Some(tile) = self.get_tile_mut(key) {
                other.peek_tile_without_updating_cache(key, |other| {
                    let other = other.unwrap();
                    for index in 0..other.blocks.len() {
                        if let (Some(other), Some(block)) =
                            (&other.blocks[index], tile.blocks[index].as_mut())
                        {
                            block.difference_with(other);
                            if block.is_empty() {
                                tile.blocks[index] = None;
                            }
                        }
                    }
                });
                if tile.is_empty() {
                    self.bitmap.storage.remove(key);
                }
            }
        }
    }

    pub fn intersection(&mut self, other: &JourneyBitmap) {
        for key in self.bitmap.all_tile_keys().copied().collect::<Vec<_>>() {
            let keep = if other.contains_tile(&key) {
                let tile = self.get_tile_mut(&key).unwrap();
                other.peek_tile_without_updating_cache(&key, |other| {
                    let other = other.unwrap();
                    for index in 0..tile.blocks.len() {
                        match &other.blocks[index] {
                            None => tile.blocks[index] = None,
                            Some(other) => {
                                if let Some(block) = tile.blocks[index].as_mut() {
                                    block.intersect_with(other);
                                    if block.is_empty() {
                                        tile.blocks[index] = None;
                                    }
                                }
                            }
                        }
                    }
                    !tile.is_empty()
                })
            } else {
                false
            };
            if !keep {
                self.mark_changed(key);
                self.bitmap.storage.remove(&key);
            }
        }
    }

    fn fill_rect(&mut self, plane: BitmapPlane, x: Range<i64>, y: Range<i64>) {
        if plane == BitmapPlane::Mercator {
            self.fill_rect_in_grid(plane, x, y);
            return;
        }
        let radius = POLAR_DISK_RADIUS;
        let x = x.start.max(0)..x.end.min(POLAR_WORLD_PIXEL_WIDTH);
        let y = y.start.max(0)..y.end.min(POLAR_WORLD_PIXEL_WIDTH);
        if x.is_empty() || y.is_empty() {
            return;
        }
        let (near, far) = polar_rect_radius_bounds_squared(x.clone(), y.clone());
        if near >= radius * radius {
            return;
        }
        if far <= radius * radius {
            self.fill_rect_in_grid(plane, x, y);
            return;
        }
        // Retain boundary cells with any positive-area disk intersection. The
        // area calculation weights these cells by their exact clipped fraction.
        for row in y {
            let span = polar_row_span(row);
            self.fill_rect_in_grid(
                plane,
                x.start.max(span.start)..x.end.min(span.end),
                row..row + 1,
            );
        }
    }

    fn fill_rect_in_grid(&mut self, plane: BitmapPlane, x: Range<i64>, y: Range<i64>) {
        let tile_side = 1 << ALL_OFFSET;
        let plane_side = plane.grid_width() * tile_side;
        let x = x.start.max(0)..x.end.min(plane_side);
        let y = y.start.max(0)..y.end.min(plane_side);
        if x.is_empty() || y.is_empty() {
            return;
        }
        for tile_x in x.start / tile_side..=(x.end - 1) / tile_side {
            for tile_y in y.start / tile_side..=(y.end - 1) / tile_side {
                let key = TileKey::new(plane, tile_x as u16, tile_y as u16);
                let (origin_x, origin_y) = (tile_x * tile_side, tile_y * tile_side);
                self.mutate_tile_or_insert_empty(&key, |tile| {
                    let changed = tile.fill_rect(
                        (x.start - origin_x).max(0)..(x.end - origin_x).min(tile_side),
                        (y.start - origin_y).max(0)..(y.end - origin_y).min(tile_side),
                    );
                    ((), changed)
                });
            }
        }
    }

    fn draw_line(&mut self, plane: BitmapPlane, start: (i64, i64), end: (i64, i64), width: u8) {
        debug_assert!(width > 0);
        self.draw_line_with_width(plane, start, end, StrokeBrush::Square, |_| width);
    }

    fn draw_line_with_width(
        &mut self,
        plane: BitmapPlane,
        start: (i64, i64),
        end: (i64, i64),
        brush: StrokeBrush,
        width_at_y: impl FnMut(i64) -> u8,
    ) {
        rasterize_stroke(start, end, brush, width_at_y, |x, y| {
            self.fill_rect(plane, x, y);
        });
    }

    /// Rasterize the shortest spherical arc, clipping each projection's brush
    /// footprint to its grid. Short Mercator arcs keep the direct fast path.
    pub fn add_line(&mut self, start_lng: f64, start_lat: f64, end_lng: f64, end_lat: f64) {
        if !start_lng.is_finite()
            || !end_lng.is_finite()
            || !start_lat.is_finite()
            || !end_lat.is_finite()
            || !(-90.0..=90.0).contains(&start_lat)
            || !(-90.0..=90.0).contains(&end_lat)
        {
            return;
        }

        // A meridian/parallel path bounds the arc's angular distance. Safely
        // away from the seam, short fixes can use the existing small-arc policy
        // without constructing a spherical arc or evaluating trigonometry.
        let longitude_delta = ((end_lng - start_lng + 180.0).rem_euclid(360.0) - 180.0).abs();
        let angle_bound_degrees = (end_lat - start_lat).abs() + longitude_delta;
        if angle_bound_degrees.to_radians() < SHORT_ARC_ANGLE_RADIANS
            && start_lat.abs().max(end_lat.abs()) + angle_bound_degrees
                < MERCATOR_MAX_LATITUDE - seam_overlap_degrees()
        {
            self.add_mercator_line(start_lng, start_lat, end_lng, end_lat);
            return;
        }

        // Canonical direction gives identical subdivision and rasterization for
        // a reversed track, including roots numerically very close to the seam.
        let (start, end) = if (start_lng, start_lat) <= (end_lng, end_lat) {
            ((start_lng, start_lat), (end_lng, end_lat))
        } else {
            ((end_lng, end_lat), (start_lng, start_lat))
        };
        let arc = GreatCircleArc::new(
            UnitPoint::from_lng_lat(start.0, start.1),
            UnitPoint::from_lng_lat(end.0, end.1),
        );

        // RDP simplifies vectors against spherical arcs. A Mercator chord is
        // equivalent only while its deviation remains below native precision.
        // Keep the original rasterization (and original endpoint coordinates)
        // for ordinary short fixes and geometrically straight projected arcs.
        let overlap = seam_overlap_degrees();
        let inner_seam = MERCATOR_MAX_LATITUDE - overlap;
        let outer_seam = MERCATOR_MAX_LATITUDE + overlap;
        let enters_seam = arc.enters_latitude_cap(inner_seam);
        if !enters_seam && !mercator_arc_has_visible_curvature(arc) {
            self.add_mercator_line(start_lng, start_lat, end_lng, end_lat);
            return;
        }

        let mut cuts = vec![0.0, 1.0];
        cuts.extend(arc.latitude_intersections(MERCATOR_MAX_LATITUDE));
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        for interval in cuts.windows(2) {
            let latitude = arc.at((interval[0] + interval[1]) * 0.5).to_lng_lat().1;
            self.add_adaptive_arc(
                plane_for_latitude(latitude),
                arc,
                interval[0],
                interval[1],
                0,
            );
        }
        if !enters_seam {
            return;
        }

        // Keep the main path's subdivision stable away from the seam. Only
        // subdivide the overlap again to add the missing brush footprints.
        cuts.extend(arc.latitude_intersections(inner_seam));
        cuts.extend(arc.latitude_intersections(outer_seam));
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        for interval in cuts.windows(2) {
            let latitude = arc.at((interval[0] + interval[1]) * 0.5).to_lng_lat().1;
            if !(inner_seam..=outer_seam).contains(&latitude.abs()) {
                continue;
            }
            // Each chart clips its own footprint, so no geographic area is
            // counted twice even though the centreline intervals overlap.
            let polar_plane = if latitude > 0.0 {
                BitmapPlane::NorthPolar
            } else {
                BitmapPlane::SouthPolar
            };
            for plane in [BitmapPlane::Mercator, polar_plane] {
                self.add_adaptive_arc(plane, arc, interval[0], interval[1], 0);
            }
        }
    }

    fn add_adaptive_arc(
        &mut self,
        plane: BitmapPlane,
        arc: GreatCircleArc,
        t0: f64,
        t1: f64,
        depth: u8,
    ) {
        let a = arc.at(t0).to_lng_lat();
        let b = arc.at(t1).to_lng_lat();
        let m = arc.at((t0 + t1) * 0.5).to_lng_lat();
        let project = |(lng, lat): (f64, f64)| -> (f64, f64) {
            if plane == BitmapPlane::Mercator {
                project_mercator(lng, lat)
            } else {
                project_polar_laea(lng, lat, plane)
            }
        };
        let pa = project(a);
        let mut pb = project(b);
        let mut pm = project(m);
        if plane == BitmapPlane::Mercator {
            pb.0 = unwrap_mercator_x(pb.0, pa.0);
            pm.0 = unwrap_mercator_x(pm.0, pa.0);
        }
        let error = (pm.0 - (pa.0 + pb.0) * 0.5).hypot(pm.1 - (pa.1 + pb.1) * 0.5);
        // The angular bound also catches inflections whose midpoint happens
        // to lie on the chord. The error is measured before integer rounding.
        let angle = arc.angle * (t1 - t0);
        if depth < 24 && (error > 0.25 || angle > 0.01) {
            let tm = (t0 + t1) * 0.5;
            self.add_adaptive_arc(plane, arc, t0, tm, depth + 1);
            self.add_adaptive_arc(plane, arc, tm, t1, depth + 1);
        } else {
            match plane {
                BitmapPlane::Mercator
                    if m.1.abs() >= MERCATOR_MAX_LATITUDE - seam_overlap_degrees() =>
                {
                    self.add_mercator_seam_line(pa, pb);
                }
                BitmapPlane::Mercator => self.add_mercator_line(
                    a.0,
                    a.1.clamp(-MERCATOR_MAX_LATITUDE, MERCATOR_MAX_LATITUDE),
                    b.0,
                    b.1.clamp(-MERCATOR_MAX_LATITUDE, MERCATOR_MAX_LATITUDE),
                ),
                BitmapPlane::NorthPolar | BitmapPlane::SouthPolar => {
                    self.add_polar_line(plane, a.0, a.1, b.0, b.1)
                }
            }
        }
    }

    fn add_mercator_seam_line(&mut self, a: (f64, f64), b: (f64, f64)) {
        let a = (a.0.floor() as i64, a.1.floor() as i64);
        let b = (b.0.floor() as i64, b.1.floor() as i64);
        self.draw_mercator_brush(a, b);
    }

    /// Stamp complete brushes before partitioning them into blocks and tiles.
    /// Width follows Mercator's scale, including on long meridians. Unlike the
    /// legacy block-local rasterizer, no partition can clip or restart a brush.
    fn draw_mercator_brush(&mut self, a: (i64, i64), b: (i64, i64)) {
        let mercator_width_at_y = |y: i64| {
            let y = y.clamp(0, MERCATOR_WORLD_PIXEL_WIDTH - 1);
            (std::f64::consts::PI * (1.0 - 2.0 * y as f64 / MERCATOR_WORLD_PIXEL_WIDTH as f64))
                .cosh()
                .round() as u8
        };
        let width_a = mercator_width_at_y(a.1);
        let width_b = mercator_width_at_y(b.1);
        let max_width = width_a.max(width_b);
        // Scale is monotonic on either side of the equator. Most segments stay
        // within one integer brush width, so evaluate it once for the stroke.
        let equator = MERCATOR_WORLD_PIXEL_WIDTH / 2;
        let crosses_equator = a.1.min(b.1) < equator && a.1.max(b.1) > equator;
        let constant_width = width_a == width_b && (!crosses_equator || max_width == 1);
        let width_at_y = |y| {
            if constant_width {
                width_a
            } else {
                mercator_width_at_y(y)
            }
        };
        self.draw_mercator_stroke(a, b, max_width, width_at_y);
        // The brush, as well as its centreline, can cross the antimeridian.
        let half_width = i64::from(max_width) / 2;
        if a.0.min(b.0) - half_width < 0 {
            self.draw_mercator_stroke(
                (a.0 + MERCATOR_WORLD_PIXEL_WIDTH, a.1),
                (b.0 + MERCATOR_WORLD_PIXEL_WIDTH, b.1),
                max_width,
                width_at_y,
            );
        }
        if a.0.max(b.0) + half_width >= MERCATOR_WORLD_PIXEL_WIDTH {
            self.draw_mercator_stroke(
                (a.0 - MERCATOR_WORLD_PIXEL_WIDTH, a.1),
                (b.0 - MERCATOR_WORLD_PIXEL_WIDTH, b.1),
                max_width,
                width_at_y,
            );
        }
    }

    fn draw_mercator_stroke(
        &mut self,
        a: (i64, i64),
        b: (i64, i64),
        max_width: u8,
        width_at_y: impl FnMut(i64) -> u8,
    ) {
        let before = i64::from(max_width) / 2;
        let after = i64::from(max_width - 1) / 2;
        let min = (a.0.min(b.0) - before, a.1.min(b.1) - before);
        let max = (a.0.max(b.0) + after, a.1.max(b.1) + after);
        let tile_side = 1 << ALL_OFFSET;
        // When the entire footprint fits in one tile, borrow it for the stroke
        // instead of looking it up for every row. Boundary strokes retain the
        // same walker and the general clipping/partitioning path.
        if min.0 >= 0
            && min.1 >= 0
            && max.0 < MERCATOR_WORLD_PIXEL_WIDTH
            && max.1 < MERCATOR_WORLD_PIXEL_WIDTH
            && min.0 / tile_side == max.0 / tile_side
            && min.1 / tile_side == max.1 / tile_side
        {
            let origin = (min.0 / tile_side * tile_side, min.1 / tile_side * tile_side);
            let key = TileKey::mercator((min.0 / tile_side) as u16, (min.1 / tile_side) as u16);
            self.mutate_tile_or_insert_empty(&key, |tile| {
                let mut changed = false;
                rasterize_stroke(a, b, StrokeBrush::DominantAxis, width_at_y, |x, y| {
                    changed |= tile.fill_rect(
                        x.start - origin.0..x.end - origin.0,
                        y.start - origin.1..y.end - origin.1,
                    );
                });
                ((), changed)
            });
        } else {
            self.draw_line_with_width(
                BitmapPlane::Mercator,
                a,
                b,
                StrokeBrush::DominantAxis,
                width_at_y,
            );
        }
    }

    fn add_polar_line(
        &mut self,
        plane: BitmapPlane,
        start_lng: f64,
        start_lat: f64,
        end_lng: f64,
        end_lat: f64,
    ) {
        let (x0, y0) = project_polar_laea(start_lng, start_lat, plane);
        let (x1, y1) = project_polar_laea(end_lng, end_lat, plane);
        let (x0, y0, x1, y1) = (
            x0.round() as i64,
            y0.round() as i64,
            x1.round() as i64,
            y1.round() as i64,
        );
        self.draw_line(plane, (x0, y0), (x1, y1), polar_brush_width());
    }

    fn add_mercator_line(&mut self, start_lng: f64, start_lat: f64, end_lng: f64, end_lat: f64) {
        use std::f64::consts::PI;

        if !start_lng.is_finite()
            || !end_lng.is_finite()
            || !(-90.0..=90.0).contains(&start_lat)
            || !(-90.0..=90.0).contains(&end_lat)
        {
            return;
        }
        let wrap_lng = |lng: f64| (lng + 180.0).rem_euclid(360.0) - 180.0;
        let start_lng = wrap_lng(start_lng);
        let end_lng = wrap_lng(end_lng);

        let (mut x0, mut y0) =
            utils::lng_lat_to_tile_x_y(start_lng, start_lat, MERCATOR_WORLD_PIXEL_OFFSET as i32);
        let (mut x1, mut y1) =
            utils::lng_lat_to_tile_x_y(end_lng, end_lat, MERCATOR_WORLD_PIXEL_OFFSET as i32);

        let bitmap_pixel_width = MERCATOR_WORLD_PIXEL_WIDTH as i32;
        let x_half = bitmap_pixel_width / 2;

        if x1 - x0 > x_half {
            x0 += 2 * x_half;
        } else if x0 - x1 > x_half {
            x1 += 2 * x_half;
        }

        if (y0 < 0 && y1 < 0) || (y0 >= bitmap_pixel_width && y1 >= bitmap_pixel_width) {
            return;
        }
        let max_y = bitmap_pixel_width - 1;
        let (original_x0, original_y0, original_x1, original_y1) = (x0, y0, x1, y1);
        if !(0..=max_y).contains(&y0) {
            y0 = y0.clamp(0, max_y);
            x0 = interpolate_x_at_y(original_x0, original_y0, original_x1, original_y1, y0);
        }
        if !(0..=max_y).contains(&y1) {
            y1 = y1.clamp(0, max_y);
            x1 = interpolate_x_at_y(original_x0, original_y0, original_x1, original_y1, y1);
        }

        // Single-cell brushes have no footprint to spill into an adjacent
        // partition. Keep their block-batched fast path and pixel tie-breaking.
        let widest_latitude = start_lat.abs().max(end_lat.abs());
        if (1.0 / widest_latitude.to_radians().cos()).round() > 1.0 {
            self.draw_mercator_brush(
                (i64::from(x0), i64::from(y0)),
                (i64::from(x1), i64::from(y1)),
            );
            return;
        }

        // Calculate line deltas
        let dx = x1 as i64 - x0 as i64;
        let dy = y1 as i64 - y0 as i64;
        // Create a positive copy of deltas (makes iterating easier)
        let dx0 = dx.abs();
        let dy0 = dy.abs();
        // Calculate error intervals for both axis
        let px = 2 * dy0 - dx0;
        let py = 2 * dx0 - dy0;

        let xaxis = dy0 <= dx0;
        let quadrants13 = (dx < 0 && dy < 0) || (dx > 0 && dy > 0);

        // Ensure we always draw in the positive primary direction
        let primary_delta = if xaxis { dx } else { dy };
        let ((mut x, mut y), (end_x, end_y)) = if primary_delta >= 0 {
            ((x0 as i64, y0 as i64), (x1 as i64, y1 as i64))
        } else {
            ((x1 as i64, y1 as i64), (x0 as i64, y0 as i64))
        };
        let end = if xaxis { end_x } else { end_y };
        let mut p = if xaxis { px } else { py };

        loop {
            // tile_x is not rounded, it may exceed the antimeridian
            let (tile_x, tile_y) = (x >> ALL_OFFSET, y >> ALL_OFFSET);
            let tile_lat = utils::tile_y_to_lat(y as i32, MERCATOR_WORLD_PIXEL_OFFSET as i32);
            let latirad = tile_lat * PI / 180.0;
            let width: u8 = (1.0 / latirad.cos()).round() as u8;

            let tile_key = TileKey::mercator(tile_x.rem_euclid(MAP_WIDTH) as u16, tile_y as u16);
            let (tile_x_offset, tile_y_offset) = (tile_x << ALL_OFFSET, tile_y << ALL_OFFSET);
            let (next, _) = self.mutate_tile_or_insert_empty(&tile_key, |tile| {
                tile.add_line(
                    x - tile_x_offset,
                    y - tile_y_offset,
                    end - if xaxis { tile_x_offset } else { tile_y_offset },
                    p,
                    dx0,
                    dy0,
                    xaxis,
                    quadrants13,
                    width,
                )
            });
            (x, y, p) = next;

            x += tile_x_offset;
            y += tile_y_offset;

            // A boundary endpoint returned by the tile walker is still
            // undrawn. Visit its owning tile before finishing the stroke.
            if (if xaxis { x } else { y }) >= end
                && x >> ALL_OFFSET == tile_x
                && y >> ALL_OFFSET == tile_y
            {
                break;
            }
        }
    }

    pub fn merge_vector(&mut self, journey_vector: &crate::journey_vector::JourneyVector) {
        for track_segment in &journey_vector.track_segments {
            match track_segment.track_points.as_slice() {
                [] => {}
                [point] => self.add_line(
                    point.longitude,
                    point.latitude,
                    point.longitude,
                    point.latitude,
                ),
                points => {
                    for pair in points.windows(2) {
                        self.add_line(
                            pair[0].longitude,
                            pair[0].latitude,
                            pair[1].longitude,
                            pair[1].latitude,
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journey_bitmap::{BITMAP_WIDTH, TILE_WIDTH};
    use bitvec::prelude::*;
    use std::collections::BTreeSet;

    fn mercator_pixels(bitmap: &JourneyBitmap) -> BTreeSet<(i64, i64)> {
        let mut pixels = BTreeSet::new();
        for key in bitmap.all_tile_keys() {
            let TileKey::Mercator { x, y } = *key else {
                panic!("expected Mercator test coverage, got {key:?}");
            };
            bitmap.peek_tile_without_updating_cache(key, |tile| {
                let tile = tile.unwrap();
                for (block_key, block) in tile.iter() {
                    let origin_x = (x as i64 * TILE_WIDTH + block_key.x() as i64) * BITMAP_WIDTH;
                    let origin_y = (y as i64 * TILE_WIDTH + block_key.y() as i64) * BITMAP_WIDTH;
                    for i in block.raw_data().view_bits::<Msb0>().iter_ones() {
                        pixels.insert((
                            origin_x + i as i64 % BITMAP_WIDTH,
                            origin_y + i as i64 / BITMAP_WIDTH,
                        ));
                    }
                }
            });
        }
        pixels
    }

    #[test]
    fn square_strokes_cover_the_full_brush_across_block_and_tile_boundaries() {
        let tile_side = 1 << ALL_OFFSET;
        for width in [1_u8, 4, 9] {
            for (dx, dy) in [
                (0_i64, 0_i64),
                (12, 0),
                (0, 12),
                (128, 0),
                (0, 128),
                (12, 12),
                (12, -12),
                (12, 5),
                (5, 12),
                (12, -5),
                (5, -12),
                (-12, 5),
                (-5, 12),
                (-12, -5),
                (-5, -12),
            ] {
                for start in [
                    (32, 32),
                    (BITMAP_WIDTH, BITMAP_WIDTH),
                    (tile_side - 1, tile_side - 1),
                    (tile_side, tile_side),
                    (tile_side + 1, tile_side + 1),
                ] {
                    let end = (start.0 + dx, start.1 + dy);
                    let (a, b) = if start <= end {
                        (start, end)
                    } else {
                        (end, start)
                    };
                    let steps = dx.abs().max(dy.abs());
                    let mut expected = BTreeSet::new();
                    // Independent oracle: round points sampled uniformly along
                    // the dominant axis, then stamp each *complete* square.
                    for i in 0..=steps {
                        let t = if steps == 0 {
                            0.0
                        } else {
                            i as f64 / steps as f64
                        };
                        let x = a.0 + ((b.0 - a.0) as f64 * t).round() as i64;
                        let y = a.1 + ((b.1 - a.1) as f64 * t).round() as i64;
                        let before = i64::from(width) / 2;
                        for x in x - before..x - before + i64::from(width) {
                            for y in y - before..y - before + i64::from(width) {
                                expected.insert((x, y));
                            }
                        }
                    }
                    let mut plane = JourneyBitmap::new();
                    let mut editor = plane.edit_tracked();
                    editor.draw_line(BitmapPlane::Mercator, start, end, width);
                    let changed_tiles = editor.finish();
                    assert_eq!(
                        mercator_pixels(&plane),
                        expected,
                        "{start:?}->{end:?}, width={width}"
                    );
                    assert_eq!(changed_tiles.len(), plane.tile_count());

                    // Repeating/reversing a stroke must not invalidate render caches.
                    let mut editor = plane.edit_tracked();
                    editor.draw_line(BitmapPlane::Mercator, end, start, width);
                    assert!(editor.finish().is_empty());
                }
            }
        }
    }

    #[test]
    fn square_brush_clips_at_plane_edges_without_wrapping_or_empty_tiles() {
        let mut plane = JourneyBitmap::new();
        let last = (BitmapPlane::Mercator.grid_width() << ALL_OFFSET) - 1;
        for x in [0, last] {
            for y in [0, last] {
                plane
                    .edit()
                    .draw_line(BitmapPlane::Mercator, (x, y), (x, y), 9);
            }
        }
        let pixels = mercator_pixels(&plane);
        assert_eq!(pixels.len(), 4 * 25);
        assert!(pixels
            .iter()
            .all(|&(x, y)| { (x <= 4 || x >= last - 4) && (y <= 4 || y >= last - 4) }));
        assert_eq!(plane.tile_count(), 4);
    }

    #[test]
    fn ordinary_short_segments_use_direct_mercator_rasterization() {
        for (lng0, lat0, lng1, lat1) in [
            (0.0, 0.0, 0.0002, 0.0002),
            (151.1, -33.8, 151.1001, -33.8001),
            (114.2, 22.6, 114.2, 22.6),
            (179.99999, 54.0, -179.99999, 54.00001),
            (30.0, 84.0, 30.0001, 84.0001),
            (539.99999, -54.0, -179.99999, -54.00001),
            (0.0, 0.0, 0.004, 0.004),
            (151.1, -33.8, 151.105, -33.804),
            (114.2, 22.6, 114.204, 22.604),
            (179.999, 54.0, -179.999, 54.001),
            (30.0, 71.0, 30.004, 71.004),
        ] {
            let mut bitmap = JourneyBitmap::new();
            bitmap.edit().add_line(lng0, lat0, lng1, lat1);
            let mut legacy = JourneyBitmap::new();
            legacy.edit().add_mercator_line(lng0, lat0, lng1, lat1);
            assert_eq!(bitmap, legacy);
            let mut reversed = JourneyBitmap::new();
            reversed.edit().add_line(lng1, lat1, lng0, lat0);
            assert_eq!(bitmap, reversed);
        }
    }

    #[test]
    fn wide_mercator_tracks_keep_complete_brushes_at_blocks_tiles_and_dateline() {
        let n = MERCATOR_WORLD_PIXEL_WIDTH;
        let width_at_y = |y| {
            let latitude = utils::tile_y_to_lat(y as i32, MERCATOR_WORLD_PIXEL_OFFSET as i32);
            (1.0 / latitude.to_radians().cos()).round() as i64
        };
        let transition_y =
            ((1.0 - 2.5_f64.acosh() / std::f64::consts::PI) * n as f64 / 2.0).floor() as i64;
        for start_x in [48, (1 << ALL_OFFSET) - 8, n - 8] {
            // Even/odd widths in both hemispheres, plus the width 3 -> 2 boundary.
            for start_y in [256, n - 400, n / 4, transition_y - 68] {
                for (dx, dy) in [
                    (17, 137),
                    (17, -137),
                    (137, 17),
                    (137, -17),
                    (0, 137),
                    (137, 0),
                    (0, 0),
                ] {
                    let (end_x, end_y) = (start_x + dx, start_y + dy);
                    let (lng0, lat0) = mercator_cell_to_lng_lat(start_x, start_y);
                    let (lng1, lat1) = mercator_cell_to_lng_lat(end_x, end_y);
                    let mut bitmap = JourneyBitmap::new();
                    let mut editor = bitmap.edit_tracked();
                    editor.add_line(lng0, lat0, lng1, lat1);
                    let changed = editor.finish();
                    assert_eq!(changed, bitmap.all_tile_keys().copied().collect());

                    // Independent oracle: sample the dominant axis and stamp complete
                    // perpendicular spans, with caps only at the segment endpoints.
                    // Storage boundaries must neither clip spans nor restart caps.
                    let mut expected = BTreeSet::new();
                    let steps = dx.abs().max(dy.abs());
                    for i in 0..=steps {
                        let t = if steps == 0 {
                            0.0
                        } else {
                            i as f64 / steps as f64
                        };
                        let x = start_x + (dx as f64 * t).round() as i64;
                        let y = start_y + (dy as f64 * t).round() as i64;
                        let width = width_at_y(y);
                        if dx >= dy.abs() {
                            for y in y - width / 2..y - width / 2 + width {
                                expected.insert((x.rem_euclid(n), y));
                            }
                        } else {
                            for x in x - width / 2..x - width / 2 + width {
                                expected.insert((x.rem_euclid(n), y));
                            }
                        }
                    }
                    for (x, y) in [(start_x, start_y), (end_x, end_y)] {
                        let width = width_at_y(y);
                        for x in x - width / 2..x - width / 2 + width {
                            for y in y - width / 2..y - width / 2 + width {
                                expected.insert((x.rem_euclid(n), y));
                            }
                        }
                    }
                    assert_eq!(
                        mercator_pixels(&bitmap),
                        expected,
                        "({start_x}, {start_y}) + ({dx}, {dy})"
                    );
                    let mut reverse = JourneyBitmap::new();
                    reverse.edit().add_line(lng1, lat1, lng0, lat0);
                    assert_eq!(bitmap, reverse);
                    let mut editor = bitmap.edit_tracked();
                    editor.add_line(lng1, lat1, lng0, lat0);
                    assert!(editor.finish().is_empty());
                }
            }
        }
    }

    fn mercator_cell_to_lng_lat(x: i64, y: i64) -> (f64, f64) {
        let n = MERCATOR_WORLD_PIXEL_WIDTH;
        // Stay off integer projection boundaries so inverse roundoff cannot
        // change the source cell selected by the public geographic API.
        let u = (x as f64 + 0.25) / n as f64;
        let v = (y as f64 + 0.25) / n as f64;
        (
            u * 360.0 - 180.0,
            (std::f64::consts::PI * (1.0 - 2.0 * v))
                .sinh()
                .atan()
                .to_degrees(),
        )
    }

    #[test]
    fn single_pixel_strokes_include_endpoints_in_the_next_partition() {
        let n = MERCATOR_WORLD_PIXEL_WIDTH;
        let tile_side = 1 << ALL_OFFSET;
        // End exactly in the next block/tile, including the wrapped world edge.
        for end_x in [n / 2 + BITMAP_WIDTH, n / 2 + tile_side, n] {
            for end_y in [n / 2 + BITMAP_WIDTH, n / 2 + tile_side] {
                for (dx, dy) in [
                    (11_i64, 7_i64),
                    (7, 11),
                    (11, -7),
                    (7, -11),
                    (11, 0),
                    (0, 11),
                ] {
                    let (start_x, start_y) = (end_x - dx, end_y - dy);
                    let a = mercator_cell_to_lng_lat(start_x, start_y);
                    let b = mercator_cell_to_lng_lat(end_x, end_y);
                    let steps = dx.abs().max(dy.abs());
                    let expected = (0..=steps)
                        .map(|i| {
                            let t = i as f64 / steps as f64;
                            (
                                (start_x + (dx as f64 * t).round() as i64).rem_euclid(n),
                                start_y + (dy as f64 * t).round() as i64,
                            )
                        })
                        .collect::<BTreeSet<_>>();
                    let mut bitmap = JourneyBitmap::new();
                    let mut editor = bitmap.edit_tracked();
                    editor.add_line(a.0, a.1, b.0, b.1);
                    let changed = editor.finish();
                    assert_eq!(
                        mercator_pixels(&bitmap),
                        expected,
                        "({start_x}, {start_y}) -> ({end_x}, {end_y})"
                    );
                    assert_eq!(changed, bitmap.all_tile_keys().copied().collect());
                    let mut reverse = JourneyBitmap::new();
                    reverse.edit().add_line(b.0, b.1, a.0, a.1);
                    assert_eq!(bitmap, reverse);
                    let mut editor = bitmap.edit_tracked();
                    editor.add_line(b.0, b.1, a.0, a.1);
                    assert!(editor.finish().is_empty());
                }
            }
        }
    }
}
