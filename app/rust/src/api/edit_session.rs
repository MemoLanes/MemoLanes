use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use flutter_rust_bridge::frb;

use crate::journey_bitmap::JourneyBitmap;
use crate::journey_data::JourneyData;
use crate::journey_vector::{JourneyVector, TrackPoint, TrackSegment};
use crate::spherical_geometry::{GreatCircleArc, UnitPoint};

use super::api::{get, MapRendererProxy};
use crate::renderer::MapRenderer;
use crate::utils::{get_bounds_from_journey_bitmap, MapBounds};

const EPS: f64 = 1e-12_f64;
const DEDUP_EPS: f64 = 1e-9_f64;
const LINK_SNAP_DISTANCE_RATIO_THRESHOLD: f64 = 3.0_f64;

#[frb(opaque)]
pub struct EditSession {
    journey_id: String,
    journey_revision: String,
    map_renderer: Arc<Mutex<MapRenderer>>,
    initial_bounds: Option<MapBounds>,
    data: JourneyVector,
    undo_stack: Vec<JourneyVector>,
}

pub enum AddLinesOutcome {
    Added,
    Ignored,
    LinkedDrawTooFar,
    LinkedDrawNeedsMultipleTracks,
    LinkedDrawInvalidLinkTargets,
}

#[derive(Debug)]
enum PrepareTrackPointsError {
    TooFar,
    NeedsMultipleTracks,
    InvalidLinkTargets,
}

impl std::fmt::Display for PrepareTrackPointsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFar => write!(f, "linked draw too far"),
            Self::NeedsMultipleTracks => write!(f, "linked draw needs multiple tracks"),
            Self::InvalidLinkTargets => write!(f, "linked draw invalid link targets"),
        }
    }
}

impl std::error::Error for PrepareTrackPointsError {}

impl EditSession {
    fn point_distance(
        a: &crate::journey_vector::TrackPoint,
        b: &crate::journey_vector::TrackPoint,
    ) -> f64 {
        let lat_delta = a.latitude - b.latitude;
        let lng_delta = a.longitude - b.longitude;
        (lat_delta * lat_delta + lng_delta * lng_delta).sqrt()
    }

    fn point_distance_sq(
        a: &crate::journey_vector::TrackPoint,
        b: &crate::journey_vector::TrackPoint,
    ) -> f64 {
        let lat_delta = a.latitude - b.latitude;
        let lng_delta = a.longitude - b.longitude;
        lat_delta * lat_delta + lng_delta * lng_delta
    }

    fn points_equal(
        a: &crate::journey_vector::TrackPoint,
        b: &crate::journey_vector::TrackPoint,
    ) -> bool {
        (a.latitude - b.latitude).abs() < DEDUP_EPS && (a.longitude - b.longitude).abs() < DEDUP_EPS
    }

    fn dedup_adjacent_track_points(
        points: Vec<crate::journey_vector::TrackPoint>,
    ) -> Vec<crate::journey_vector::TrackPoint> {
        let mut deduped: Vec<crate::journey_vector::TrackPoint> = Vec::with_capacity(points.len());
        for point in points {
            if deduped
                .last()
                .is_some_and(|last| Self::points_equal(last, &point))
            {
                continue;
            }
            deduped.push(point);
        }
        deduped
    }

    /// Nearest endpoint of any existing polyline (`TrackSegment`), paired with its segment index.
    /// Only segment endpoints are considered — not interior edge points.
    fn find_nearest_endpoint_on_existing_tracks(
        &self,
        target: &crate::journey_vector::TrackPoint,
    ) -> Option<(crate::journey_vector::TrackPoint, usize)> {
        let mut best_match: Option<(f64, crate::journey_vector::TrackPoint, usize)> = None;

        for (segment_index, segment) in self.data.track_segments.iter().enumerate() {
            let pts = &segment.track_points;
            if pts.is_empty() {
                continue;
            }

            let mut consider = |point: &crate::journey_vector::TrackPoint| {
                let distance_sq = Self::point_distance_sq(target, point);
                let should_replace = match &best_match {
                    Some((best_distance_sq, _, _)) => distance_sq < *best_distance_sq,
                    None => true,
                };
                if should_replace {
                    best_match = Some((distance_sq, point.clone(), segment_index));
                }
            };

            consider(&pts[0]);
            if pts.len() >= 2 && !Self::points_equal(&pts[0], &pts[pts.len() - 1]) {
                consider(&pts[pts.len() - 1]);
            }
        }

        best_match.map(|(_, point, segment_index)| (point, segment_index))
    }

    fn prepare_track_points(
        &self,
        points: &[(f64, f64)],
        snap_endpoints: bool,
    ) -> Result<Vec<crate::journey_vector::TrackPoint>> {
        let mut track_points: Vec<crate::journey_vector::TrackPoint> = points
            .iter()
            .map(|(lat, lng)| crate::journey_vector::TrackPoint {
                latitude: *lat,
                longitude: *lng,
            })
            .collect();

        if snap_endpoints {
            if self.data.track_segments.len() < 2 {
                return Err(anyhow!(PrepareTrackPointsError::NeedsMultipleTracks));
            }

            let original_first = track_points.first().unwrap();
            let original_last = track_points.last().unwrap();

            let Some((snapped_first_pt, seg_first)) =
                self.find_nearest_endpoint_on_existing_tracks(original_first)
            else {
                return Err(anyhow!(PrepareTrackPointsError::NeedsMultipleTracks));
            };
            let Some((snapped_last_pt, seg_last)) =
                self.find_nearest_endpoint_on_existing_tracks(original_last)
            else {
                return Err(anyhow!(PrepareTrackPointsError::NeedsMultipleTracks));
            };

            let stroke_span = Self::point_distance(original_first, original_last);
            let snap_distance_sum = Self::point_distance(original_first, &snapped_first_pt)
                + Self::point_distance(original_last, &snapped_last_pt);

            if snap_distance_sum > stroke_span * LINK_SNAP_DISTANCE_RATIO_THRESHOLD {
                return Err(anyhow!(PrepareTrackPointsError::TooFar));
            }

            if Self::points_equal(&snapped_first_pt, &snapped_last_pt) {
                return Err(anyhow!(PrepareTrackPointsError::InvalidLinkTargets));
            }
            if seg_first == seg_last {
                return Err(anyhow!(PrepareTrackPointsError::InvalidLinkTargets));
            }

            if let Some(first_point) = track_points.first_mut() {
                *first_point = snapped_first_pt;
            }
            if let Some(last_point) = track_points.last_mut() {
                *last_point = snapped_last_pt;
            }
        }

        Ok(Self::dedup_adjacent_track_points(track_points))
    }

    fn build_bitmap_from_vector(vector: &JourneyVector) -> JourneyBitmap {
        let mut bitmap = JourneyBitmap::new();
        bitmap.edit().merge_vector(vector);
        bitmap
    }

    fn sync_renderer_from_data(&self) -> Result<()> {
        let bitmap = Self::build_bitmap_from_vector(&self.data);
        let mut map_renderer = self.map_renderer.lock().unwrap();
        map_renderer.replace(bitmap);
        Ok(())
    }

    fn normalize_box(
        start_lat: f64,
        start_lng: f64,
        end_lat: f64,
        end_lng: f64,
    ) -> (f64, f64, f64, f64) {
        (
            start_lat.min(end_lat),
            start_lat.max(end_lat),
            start_lng.min(end_lng),
            start_lng.max(end_lng),
        )
    }

    fn point_in_box(
        p: &crate::journey_vector::TrackPoint,
        min_lat: f64,
        max_lat: f64,
        min_lng: f64,
        max_lng: f64,
    ) -> bool {
        p.latitude >= min_lat
            && p.latitude <= max_lat
            && p.longitude >= min_lng
            && p.longitude <= max_lng
    }

    /// A conservative bound for the entire spherical arc, without trigonometry.
    fn segment_bounds(a: &TrackPoint, b: &TrackPoint) -> Option<(f64, f64, f64, f64)> {
        if ![a.latitude, b.latitude]
            .iter()
            .all(|lat| (-90.0..=90.0).contains(lat))
            || ![a.longitude, b.longitude]
                .iter()
                .all(|lng| (-180.0..=180.0).contains(lng))
        {
            return None;
        }
        let longitude_span = (b.longitude - a.longitude).abs();
        // A meridian/parallel route bounds the shortest arc's angular length.
        // Every point is at most that distance from BOTH endpoints, including
        // when the great circle bows beyond their latitude range.
        let angle_bound =
            (b.latitude - a.latitude).abs() + longitude_span.min(360.0 - longitude_span);
        let min_lat = (a.latitude.max(b.latitude) - angle_bound).max(-90.0);
        let max_lat = (a.latitude.min(b.latitude) + angle_bound).min(90.0);
        let (min_lng, max_lng) = if longitude_span < 180.0
            && a.latitude.abs().max(b.latitude.abs()) + angle_bound < 90.0 - EPS
        {
            // Away from poles, longitude is monotone along the shorter arc.
            (a.longitude.min(b.longitude), a.longitude.max(b.longitude))
        } else {
            // Dateline/polar arcs need no longitude shortcut to stay safe.
            (-180.0, 180.0)
        };
        Some((min_lat, max_lat, min_lng, max_lng))
    }

    fn segment_intersections(arc: GreatCircleArc, planes: &[(UnitPoint, f64); 4]) -> Vec<f64> {
        let mut cuts = Vec::new();
        for &(normal, offset) in planes {
            cuts.extend(arc.plane_intersections(normal, offset));
        }
        // Meridian planes also intersect the opposite longitude. These extra
        // cuts are harmless: classify intervals, then merge retained spans.
        cuts.retain(|t| *t > EPS && *t < 1.0 - EPS);
        cuts.extend([0.0, 1.0]);
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < EPS);
        cuts
    }

    fn delete_points_in_box_segments(
        segments: &[crate::journey_vector::TrackSegment],
        min_lat: f64,
        max_lat: f64,
        min_lng: f64,
        max_lng: f64,
    ) -> Vec<crate::journey_vector::TrackSegment> {
        let mut new_segments: Vec<crate::journey_vector::TrackSegment> = Vec::new();
        // The selection is constant for every segment; compute its planes once.
        let latitude_plane = |latitude: f64| {
            (
                UnitPoint {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                latitude.to_radians().sin(),
            )
        };
        let longitude_plane = |longitude: f64| {
            let (sin, cos) = longitude.to_radians().sin_cos();
            (
                UnitPoint {
                    x: -sin,
                    y: cos,
                    z: 0.0,
                },
                0.0,
            )
        };
        let planes = [
            latitude_plane(min_lat),
            latitude_plane(max_lat),
            longitude_plane(min_lng),
            longitude_plane(max_lng),
        ];

        for segment in segments {
            let pts = &segment.track_points;
            match pts.as_slice() {
                [] => continue,
                [point] => {
                    if !Self::point_in_box(point, min_lat, max_lat, min_lng, max_lng) {
                        new_segments.push(segment.clone());
                    }
                    continue;
                }
                _ => {}
            }

            let mut current = Vec::new();
            for pair in pts.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                if let Some((south, north, west, east)) = Self::segment_bounds(a, b) {
                    if north < min_lat - EPS
                        || south > max_lat + EPS
                        || east < min_lng - EPS
                        || west > max_lng + EPS
                    {
                        for point in [a, b] {
                            if current.last() != Some(point) {
                                current.push(point.clone());
                            }
                        }
                        continue;
                    }
                    if south > min_lat + EPS
                        && north < max_lat - EPS
                        && west > min_lng + EPS
                        && east < max_lng - EPS
                    {
                        if current.len() >= 2 {
                            new_segments.push(TrackSegment {
                                track_points: std::mem::take(&mut current),
                            });
                        } else {
                            current.clear();
                        }
                        continue;
                    }
                }
                // Match the renderer's canonical direction, including its
                // deterministic choice for antipodal endpoints.
                let reversed = (a.longitude, a.latitude) > (b.longitude, b.latitude);
                let (start, end) = if reversed { (b, a) } else { (a, b) };
                let arc = GreatCircleArc::new(
                    UnitPoint::from_lng_lat(start.longitude, start.latitude),
                    UnitPoint::from_lng_lat(end.longitude, end.latitude),
                );
                let point_at = |t: f64| {
                    // Preserve original observations exactly at the endpoints.
                    if t <= 0.0 {
                        return a.clone();
                    }
                    if t >= 1.0 {
                        return b.clone();
                    }
                    let (longitude, latitude) =
                        arc.at(if reversed { 1.0 - t } else { t }).to_lng_lat();
                    TrackPoint {
                        latitude,
                        longitude,
                    }
                };
                let mut cuts = Self::segment_intersections(arc, &planes);
                if reversed {
                    cuts = cuts.into_iter().rev().map(|t| 1.0 - t).collect();
                }
                let mut kept_start = 0.0;
                for interval in cuts.windows(2) {
                    let midpoint = point_at((interval[0] + interval[1]) * 0.5);
                    if !Self::point_in_box(
                        &midpoint,
                        min_lat - EPS,
                        max_lat + EPS,
                        min_lng - EPS,
                        max_lng + EPS,
                    ) {
                        continue;
                    }
                    if interval[0] > kept_start {
                        for t in [kept_start, interval[0]] {
                            let point = point_at(t);
                            if current.last() != Some(&point) {
                                current.push(point);
                            }
                        }
                    }
                    if current.len() >= 2 {
                        new_segments.push(TrackSegment {
                            track_points: std::mem::take(&mut current),
                        });
                    } else {
                        current.clear();
                    }
                    kept_start = interval[1];
                }
                if kept_start < 1.0 {
                    for t in [kept_start, 1.0] {
                        let point = point_at(t);
                        if current.last() != Some(&point) {
                            current.push(point);
                        }
                    }
                }
            }

            if current.len() >= 2 {
                new_segments.push(crate::journey_vector::TrackSegment {
                    track_points: current,
                });
            }
        }

        new_segments
    }

    pub fn new(journey_id: String) -> Result<Option<Self>> {
        let state = get();
        let (journey_data, journey_revision) = state.storage.with_db_txn(|txn| {
            Ok((
                txn.get_journey_data(&journey_id)?,
                txn.get_journey_header(&journey_id)?
                    .ok_or_else(|| anyhow!("Missing journey header"))?
                    .revision,
            ))
        })?;

        let journey_vector = match journey_data {
            JourneyData::Vector(vector) => vector,
            JourneyData::Bitmap(_) => {
                // Cannot edit bitmap journeys.
                return Ok(None);
            }
        };

        let mut bitmap = Self::build_bitmap_from_vector(&journey_vector);
        let initial_bounds = get_bounds_from_journey_bitmap(&mut bitmap);
        let map_renderer = Arc::new(Mutex::new(MapRenderer::new(bitmap)));

        Ok(Some(Self {
            journey_id,
            journey_revision,
            map_renderer,
            initial_bounds,
            data: journey_vector,
            undo_stack: Vec::new(),
        }))
    }

    #[frb(sync)]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    fn push_undo_checkpoint(&mut self, prev_data: JourneyVector) {
        self.undo_stack.push(prev_data);
    }

    pub fn get_map_renderer_proxy(&self) -> Result<(MapRendererProxy, Option<MapBounds>)> {
        Ok((
            MapRendererProxy::DynamicRenderer(self.map_renderer.clone()),
            self.initial_bounds,
        ))
    }

    pub fn undo(&mut self) -> Result<()> {
        if let Some(previous) = self.undo_stack.pop() {
            self.data = previous;
            self.sync_renderer_from_data()?;
        }
        Ok(())
    }

    pub fn delete_points_in_box(
        &mut self,
        start_lat: f64,
        start_lng: f64,
        end_lat: f64,
        end_lng: f64,
    ) -> Result<()> {
        // TODO: Unable to properly handle cases spanning ±180° of longitude.

        let (min_lat, max_lat, min_lng, max_lng) =
            Self::normalize_box(start_lat, start_lng, end_lat, end_lng);
        let new_segments = Self::delete_points_in_box_segments(
            &self.data.track_segments,
            min_lat,
            max_lat,
            min_lng,
            max_lng,
        );

        // TODO: This equality check can be very expensive.
        if new_segments != self.data.track_segments {
            self.push_undo_checkpoint(self.data.clone());
            self.data.track_segments = new_segments;
            self.sync_renderer_from_data()?;
        }

        Ok(())
    }

    pub fn add_lines(
        &mut self,
        points: &[(f64, f64)],
        snap_endpoints: bool,
    ) -> Result<AddLinesOutcome> {
        // TODO: we could run the post processor here to simplify the added points first.
        if points.len() < 2 {
            return Ok(AddLinesOutcome::Ignored);
        }

        let track_points = match self.prepare_track_points(points, snap_endpoints) {
            Ok(track_points) => track_points,
            Err(error) => {
                match error.downcast_ref::<PrepareTrackPointsError>() {
                    Some(PrepareTrackPointsError::TooFar) => {
                        return Ok(AddLinesOutcome::LinkedDrawTooFar);
                    }
                    Some(PrepareTrackPointsError::NeedsMultipleTracks) => {
                        return Ok(AddLinesOutcome::LinkedDrawNeedsMultipleTracks);
                    }
                    Some(PrepareTrackPointsError::InvalidLinkTargets) => {
                        return Ok(AddLinesOutcome::LinkedDrawInvalidLinkTargets);
                    }
                    None => {}
                }
                return Err(error);
            }
        };
        if track_points.len() < 2 {
            return Ok(AddLinesOutcome::Ignored);
        }

        let render_points: Vec<(f64, f64)> = track_points
            .iter()
            .map(|point| (point.latitude, point.longitude))
            .collect();

        self.push_undo_checkpoint(self.data.clone());
        self.data.track_segments.push(TrackSegment { track_points });

        let mut map_renderer = self.map_renderer.lock().unwrap();
        map_renderer.update(|editor| {
            for window in render_points.windows(2) {
                let (start_lat, start_lng) = window[0];
                let (end_lat, end_lng) = window[1];

                if (start_lat - end_lat).abs() < EPS && (start_lng - end_lng).abs() < EPS {
                    continue;
                }

                editor.add_line(start_lng, start_lat, end_lng, end_lat);
            }
        });
        drop(map_renderer);

        Ok(AddLinesOutcome::Added)
    }

    pub fn commit(&self) -> Result<()> {
        let state = get();

        state.storage.with_db_txn(|txn| {
            let current_revision = txn
                .get_journey_header(&self.journey_id)?
                .ok_or_else(|| anyhow!("Missing journey header"))?
                .revision;
            if current_revision != self.journey_revision {
                bail!("Journey has been modified. Please reopen the editor.")
            }
            txn.update_journey_data_with_latest_postprocessor(
                &self.journey_id,
                // TODO: probably we could make this function drop self to avoid the clone.
                JourneyData::Vector(self.data.clone()),
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journey_vector::TrackPoint;

    fn segment(points: &[(f64, f64)]) -> TrackSegment {
        TrackSegment {
            track_points: points
                .iter()
                .map(|&(latitude, longitude)| TrackPoint {
                    latitude,
                    longitude,
                })
                .collect(),
        }
    }

    fn session(track_segments: Vec<TrackSegment>) -> EditSession {
        let data = JourneyVector { track_segments };
        let bitmap = EditSession::build_bitmap_from_vector(&data);
        EditSession {
            journey_id: String::new(),
            journey_revision: String::new(),
            map_renderer: Arc::new(Mutex::new(MapRenderer::new(bitmap))),
            initial_bounds: None,
            data,
            undo_stack: Vec::new(),
        }
    }

    #[test]
    fn segment_bounds_contain_arcs_including_dateline_poles_and_antipodes() {
        let mut points = Vec::new();
        for latitude in [-90.0, -89.999, -60.0, 0.0, 60.0, 89.999, 90.0] {
            for longitude in [-180.0, -179.0, -90.0, 0.0, 90.0, 179.0, 180.0] {
                points.push(TrackPoint {
                    latitude,
                    longitude,
                });
            }
        }
        for a in &points {
            for b in &points {
                let (south, north, west, east) = EditSession::segment_bounds(a, b).unwrap();
                let (start, end) = if (a.longitude, a.latitude) <= (b.longitude, b.latitude) {
                    (a, b)
                } else {
                    (b, a)
                };
                let arc = GreatCircleArc::new(
                    UnitPoint::from_lng_lat(start.longitude, start.latitude),
                    UnitPoint::from_lng_lat(end.longitude, end.latitude),
                );
                for i in 0..=16 {
                    let (longitude, latitude) = arc.at(i as f64 / 16.0).to_lng_lat();
                    assert!(
                        latitude >= south - EPS && latitude <= north + EPS
                            && longitude >= west - EPS && longitude <= east + EPS,
                        "arc {a:?} -> {b:?} escaped its conservative bounds at {longitude}, {latitude}"
                    );
                }
            }
        }
    }

    #[test]
    fn box_deletion_preserves_disjoint_segments_including_single_points() {
        let segments = vec![
            segment(&[(-1.0, -1.0)]),
            segment(&[(3.0, 3.0), (4.0, 4.0)]),
            segment(&[(1.0, 3.0)]),
        ];
        assert_eq!(
            EditSession::delete_points_in_box_segments(&segments, 0.0, 2.0, 0.0, 2.0),
            segments
        );
    }

    #[test]
    fn box_deletion_removes_empty_and_contained_segments() {
        assert!(EditSession::delete_points_in_box_segments(&[], 0.0, 2.0, 0.0, 2.0).is_empty());
        for points in [
            vec![],
            vec![(1.0, 1.0)],
            vec![(0.0, 1.0)],
            vec![(2.0, 2.0)],
            vec![(0.5, 0.5), (1.5, 1.5)],
        ] {
            assert!(EditSession::delete_points_in_box_segments(
                &[segment(&points)],
                0.0,
                2.0,
                0.0,
                2.0
            )
            .is_empty());
        }
    }

    fn assert_segments_close(actual: &[TrackSegment], expected: &[TrackSegment]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.track_points.len(), expected.track_points.len());
            for (actual, expected) in actual.track_points.iter().zip(&expected.track_points) {
                assert!(
                    (actual.latitude - expected.latitude).abs() < 1e-9,
                    "{actual:?} != {expected:?}"
                );
                assert!(
                    (actual.longitude - expected.longitude).abs() < 1e-9,
                    "{actual:?} != {expected:?}"
                );
            }
        }
    }

    #[test]
    fn box_deletion_splits_crossings_at_ordered_intersections() {
        // For equal-latitude endpoints around longitude 1, the great circle
        // satisfies tan(lat) = tan(1 degree) * cos(lng - 1) / cos(2 degrees).
        let hit_lat = (1.0_f64.to_radians().tan() * 1.0_f64.to_radians().cos()
            / 2.0_f64.to_radians().cos())
        .atan()
        .to_degrees();
        for (start, entry, exit, end) in [
            ((1.0, -1.0), (hit_lat, 0.0), (hit_lat, 2.0), (1.0, 3.0)),
            ((-1.0, 1.0), (0.0, 1.0), (2.0, 1.0), (3.0, 1.0)),
            // The equator lies exactly along the inclusive box boundary.
            ((0.0, -1.0), (0.0, 0.0), (0.0, 2.0), (0.0, 3.0)),
        ] {
            for (start, entry, exit, end) in [(start, entry, exit, end), (end, exit, entry, start)]
            {
                assert_segments_close(
                    &EditSession::delete_points_in_box_segments(
                        &[segment(&[start, end])],
                        0.0,
                        2.0,
                        0.0,
                        2.0,
                    ),
                    &[segment(&[start, entry]), segment(&[exit, end])],
                );
            }
        }
    }

    #[test]
    fn box_deletion_clips_entering_and_leaving_segments() {
        let hit_lat = (1.0_f64.to_radians().tan() / 1.0_f64.to_radians().cos())
            .atan()
            .to_degrees();
        assert_segments_close(
            &EditSession::delete_points_in_box_segments(
                &[segment(&[(1.0, -1.0), (1.0, 1.0), (1.0, 3.0)])],
                0.0,
                2.0,
                0.0,
                2.0,
            ),
            &[
                segment(&[(1.0, -1.0), (hit_lat, 0.0)]),
                segment(&[(hit_lat, 2.0), (1.0, 3.0)]),
            ],
        );
    }

    #[test]
    fn box_deletion_handles_corner_entry_and_tangency() {
        for points in [
            vec![(-1.0, 1.0), (1.0, -1.0)],
            vec![(1.0, -1.0), (-1.0, 1.0)],
        ] {
            let original = vec![segment(&points)];
            assert_eq!(
                EditSession::delete_points_in_box_segments(&original, 0.0, 2.0, 0.0, 2.0),
                original
            );
        }
        assert_segments_close(
            &EditSession::delete_points_in_box_segments(
                &[segment(&[(-1.0, -1.0), (1.0, 1.0)])],
                0.0,
                2.0,
                0.0,
                2.0,
            ),
            &[segment(&[(-1.0, -1.0), (0.0, 0.0)])],
        );
    }

    #[test]
    fn box_deletion_preserves_an_arc_bowing_outside_with_both_endpoints_inside() {
        for longitudes in [[0.0, 1.0], [1.0, 0.0]] {
            let edited = EditSession::delete_points_in_box_segments(
                &[segment(&[(60.0, longitudes[0]), (60.0, longitudes[1])])],
                59.9,
                60.0005,
                -0.1,
                1.1,
            );
            assert_eq!(edited.len(), 1);
            let points = &edited[0].track_points;
            assert_eq!(points.len(), 2);
            // The surviving middle arc meets the upper parallel twice.
            let delta = (60.0005_f64.to_radians().tan() * 0.5_f64.to_radians().cos()
                / 60.0_f64.to_radians().tan())
            .acos()
            .to_degrees();
            let expected = if longitudes[0] < longitudes[1] {
                [0.5 - delta, 0.5 + delta]
            } else {
                [0.5 + delta, 0.5 - delta]
            };
            for (point, longitude) in points.iter().zip(expected) {
                assert!((point.latitude - 60.0005).abs() < 1e-9);
                assert!((point.longitude - longitude).abs() < 1e-8);
            }
        }
    }

    #[test]
    fn reversed_box_corners_and_undo_restore_single_points() {
        let original = vec![segment(&[(0.001, 0.001)]), segment(&[(0.003, 0.003)])];
        let mut session = session(original.clone());
        // A disjoint selection must not add an undo checkpoint.
        session
            .delete_points_in_box(0.01, 0.01, 0.02, 0.02)
            .unwrap();
        assert_eq!(session.data.track_segments, original);
        assert!(!session.can_undo());

        session
            .delete_points_in_box(0.002, 0.002, 0.0, 0.0)
            .unwrap();
        assert_eq!(session.data.track_segments, vec![original[1].clone()]);
        assert!(session.can_undo());
        session.undo().unwrap();
        assert_eq!(session.data.track_segments, original);
        assert!(!session.can_undo());
    }

    #[test]
    fn drawing_deduplicates_adjacent_points_and_supports_undo() {
        let mut session = session(vec![]);
        assert!(matches!(
            session.add_lines(&[], false).unwrap(),
            AddLinesOutcome::Ignored
        ));
        assert!(matches!(
            session.add_lines(&[(0.0, 0.0), (0.0, 0.0)], false).unwrap(),
            AddLinesOutcome::Ignored
        ));
        assert!(!session.can_undo());
        assert!(matches!(
            session
                .add_lines(&[(0.0, 0.0), (0.0, 0.0), (0.001, 0.001)], false)
                .unwrap(),
            AddLinesOutcome::Added
        ));
        assert_eq!(
            session.data.track_segments,
            vec![segment(&[(0.0, 0.0), (0.001, 0.001)])]
        );
        session.undo().unwrap();
        assert!(session.data.track_segments.is_empty());
        assert!(!session.can_undo());
    }

    #[test]
    fn linked_drawing_snaps_to_different_track_endpoints() {
        let mut session = session(vec![
            segment(&[(0.0, 0.0), (0.0, 0.001)]),
            segment(&[(0.0, 0.003), (0.0, 0.004)]),
        ]);
        assert!(matches!(
            session
                .add_lines(&[(0.0, 0.0011), (0.0, 0.0029)], true)
                .unwrap(),
            AddLinesOutcome::Added
        ));
        assert_eq!(
            session.data.track_segments[2],
            segment(&[(0.0, 0.001), (0.0, 0.003)])
        );
        session.undo().unwrap();
        let original = session.data.clone();
        assert!(matches!(
            session
                .add_lines(&[(0.0, 0.0), (0.0, 0.001)], true)
                .unwrap(),
            AddLinesOutcome::LinkedDrawInvalidLinkTargets
        ));
        assert!(matches!(
            session
                .add_lines(&[(1.0, 1.0), (1.0, 1.001)], true)
                .unwrap(),
            AddLinesOutcome::LinkedDrawTooFar
        ));
        assert_eq!(session.data, original);
        assert!(!session.can_undo());
    }

    use crate::flight_track_processor;
    use crate::gps::{Point, RawGPSPoint};

    #[test]
    fn deleting_a_remote_box_preserves_dateline_flights() {
        for longitudes in [[179.99, -179.99], [-179.999, 179.999]] {
            let source = longitudes
                .map(|longitude| RawGPSPoint {
                    point: Point {
                        latitude: 0.0,
                        longitude,
                    },
                    timestamp_ms: None,
                    accuracy: None,
                    altitude: None,
                    speed: None,
                })
                .to_vec();
            let flight = flight_track_processor::process(&[source]).unwrap();
            let edited = EditSession::delete_points_in_box_segments(
                &flight.track_segments,
                -1.0,
                1.0,
                -1.0,
                1.0,
            );
            assert_eq!(edited, flight.track_segments);
        }
    }
}
