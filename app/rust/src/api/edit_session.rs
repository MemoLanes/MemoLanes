use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use flutter_rust_bridge::frb;

use crate::journey_bitmap::JourneyBitmap;
use crate::journey_data::JourneyData;
use crate::journey_vector::{JourneyVector, TrackSegment};

use super::api::{get, MapRendererProxy};
use crate::renderer::MapRenderer;
use crate::utils::{get_bounds_from_journey_bitmap, MapBounds};

// TODO: This is a bit sus, it is comparing the lng/lat and doesn't handle anti-meridian.
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
        bitmap.merge_vector(vector);
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

    fn segment_intersections(
        a: &crate::journey_vector::TrackPoint,
        b: &crate::journey_vector::TrackPoint,
        min_lat: f64,
        max_lat: f64,
        min_lng: f64,
        max_lng: f64,
    ) -> Vec<(f64, crate::journey_vector::TrackPoint)> {
        let x0 = a.longitude;
        let y0 = a.latitude;
        let x1 = b.longitude;
        let y1 = b.latitude;
        let dx = x1 - x0;
        let dy = y1 - y0;

        let mut hits: Vec<(f64, crate::journey_vector::TrackPoint)> = Vec::new();

        let mut push_hit = |t: f64, x: f64, y: f64| {
            if !(-EPS..=1.0 + EPS).contains(&t) {
                return;
            }
            let t = t.clamp(0.0, 1.0);
            if hits.iter().any(|(t0, _)| (*t0 - t).abs() < DEDUP_EPS) {
                return;
            }
            hits.push((
                t,
                crate::journey_vector::TrackPoint {
                    latitude: y,
                    longitude: x,
                },
            ));
        };

        if dx.abs() > EPS {
            for x_edge in [min_lng, max_lng] {
                let t = (x_edge - x0) / dx;
                let y = y0 + t * dy;
                if y >= min_lat - EPS && y <= max_lat + EPS {
                    push_hit(t, x_edge, y);
                }
            }
        }

        if dy.abs() > EPS {
            for y_edge in [min_lat, max_lat] {
                let t = (y_edge - y0) / dy;
                let x = x0 + t * dx;
                if x >= min_lng - EPS && x <= max_lng + EPS {
                    push_hit(t, x, y_edge);
                }
            }
        }

        hits.sort_by(|(t1, _), (t2, _)| t1.total_cmp(t2));
        hits
    }

    fn delete_points_in_box_segments(
        segments: &[crate::journey_vector::TrackSegment],
        min_lat: f64,
        max_lat: f64,
        min_lng: f64,
        max_lng: f64,
    ) -> Vec<crate::journey_vector::TrackSegment> {
        let mut new_segments: Vec<crate::journey_vector::TrackSegment> = Vec::new();

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

            let mut current: Vec<crate::journey_vector::TrackPoint> = Vec::new();
            if !Self::point_in_box(&pts[0], min_lat, max_lat, min_lng, max_lng) {
                current.push(pts[0].clone());
            }

            for i in 0..(pts.len() - 1) {
                let a = &pts[i];
                let b = &pts[i + 1];
                let inside_a = Self::point_in_box(a, min_lat, max_lat, min_lng, max_lng);
                let inside_b = Self::point_in_box(b, min_lat, max_lat, min_lng, max_lng);
                let hits = Self::segment_intersections(a, b, min_lat, max_lat, min_lng, max_lng);

                match (inside_a, inside_b) {
                    (false, false) => {
                        if hits.len() >= 2 {
                            let entry = hits.first().unwrap().1.clone();
                            let exit = hits.last().unwrap().1.clone();

                            if current.last() != Some(&entry) {
                                current.push(entry);
                            }
                            if current.len() >= 2 {
                                new_segments.push(crate::journey_vector::TrackSegment {
                                    track_points: current,
                                });
                            }

                            current = vec![exit];
                            if current.last() != Some(b) {
                                current.push(b.clone());
                            }
                        } else {
                            if current.is_empty() {
                                current.push(a.clone());
                            }
                            if current.last() != Some(b) {
                                current.push(b.clone());
                            }
                        }
                    }
                    (true, true) => {}
                    (false, true) => {
                        if let Some(hit) = hits.first().map(|x| x.1.clone()) {
                            if current.is_empty() {
                                current.push(a.clone());
                            }
                            if current.last() != Some(&hit) {
                                current.push(hit);
                            }
                        }
                        if current.len() >= 2 {
                            new_segments.push(crate::journey_vector::TrackSegment {
                                track_points: current,
                            });
                        }
                        current = Vec::new();
                    }
                    (true, false) => {
                        if let Some(hit) = hits.last().map(|x| x.1.clone()) {
                            current = vec![hit];
                        } else {
                            current = Vec::new();
                        }
                        if current.last() != Some(b) {
                            current.push(b.clone());
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
        map_renderer.update(|journey_bitmap, tile_changed| {
            for window in render_points.windows(2) {
                let (start_lat, start_lng) = window[0];
                let (end_lat, end_lng) = window[1];

                if (start_lat - end_lat).abs() < EPS && (start_lng - end_lng).abs() < EPS {
                    continue;
                }

                journey_bitmap.add_line_with_change_callback(
                    start_lng,
                    start_lat,
                    end_lng,
                    end_lat,
                    &mut *tile_changed,
                );
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

    #[test]
    fn box_deletion_splits_crossings_at_ordered_intersections() {
        for (start, entry, exit, end) in [
            ((1.0, -1.0), (1.0, 0.0), (1.0, 2.0), (1.0, 3.0)),
            ((-1.0, 1.0), (0.0, 1.0), (2.0, 1.0), (3.0, 1.0)),
            // Corner intersections must be deduplicated across the two edges.
            ((-1.0, -1.0), (0.0, 0.0), (2.0, 2.0), (3.0, 3.0)),
        ] {
            for (start, entry, exit, end) in [(start, entry, exit, end), (end, exit, entry, start)]
            {
                assert_eq!(
                    EditSession::delete_points_in_box_segments(
                        &[segment(&[start, end])],
                        0.0,
                        2.0,
                        0.0,
                        2.0,
                    ),
                    vec![segment(&[start, entry]), segment(&[exit, end])]
                );
            }
        }
    }

    #[test]
    fn box_deletion_clips_entering_and_leaving_segments() {
        assert_eq!(
            EditSession::delete_points_in_box_segments(
                &[segment(&[(1.0, -1.0), (1.0, 1.0), (1.0, 3.0)])],
                0.0,
                2.0,
                0.0,
                2.0,
            ),
            vec![
                segment(&[(1.0, -1.0), (1.0, 0.0)]),
                segment(&[(1.0, 2.0), (1.0, 3.0)]),
            ]
        );
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
}
