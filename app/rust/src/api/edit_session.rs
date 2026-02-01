use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Result};
use flutter_rust_bridge::frb;

use crate::journey_bitmap::JourneyBitmap;
use crate::journey_data::JourneyData;
use crate::journey_vector::{JourneyVector, TrackSegment};

use super::api::{get, CameraOption, MapRendererProxy};
use crate::merged_journey_builder;
use crate::renderer::get_default_camera_option_from_journey_bitmap;
use crate::renderer::MapRenderer;

const EPS: f64 = 1e-12_f64;

// TODO: we want some test coverage here.

#[frb(opaque)]
pub struct EditSession {
    journey_id: String,
    journey_revision: String,
    map_renderer: Arc<Mutex<MapRenderer>>,
    initial_camera_option: Option<CameraOption>,
    data: JourneyVector,
    undo_stack: Vec<JourneyVector>,
}

impl EditSession {
    fn build_bitmap_from_vector(vector: &JourneyVector) -> JourneyBitmap {
        let mut bitmap = JourneyBitmap::new();
        merged_journey_builder::add_journey_vector_to_journey_bitmap(&mut bitmap, vector);
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
            if hits.iter().any(|(t0, _)| (*t0 - t).abs() < 1e-9) {
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
            if pts.len() < 2 {
                continue;
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
        let journey_data = state
            .storage
            .with_db_txn(|txn| txn.get_journey_data(&journey_id))?;
        let journey_revision = state.storage.with_db_txn(|txn| {
            txn.get_journey_header(&journey_id)?
                .ok_or_else(|| anyhow!("Missing journey header"))
                .map(|header| header.revision)
        })?;

        let journey_vector = match journey_data {
            JourneyData::Vector(vector) => vector,
            JourneyData::Bitmap(_) => {
                // Cannot edit bitmap journeys.
                return Ok(None);
            }
        };

        let bitmap = Self::build_bitmap_from_vector(&journey_vector);
        let initial_camera_option = get_default_camera_option_from_journey_bitmap(&bitmap);
        let map_renderer = Arc::new(Mutex::new(MapRenderer::new(bitmap)));

        Ok(Some(Self {
            journey_id,
            journey_revision,
            map_renderer,
            initial_camera_option,
            data: journey_vector,
            undo_stack: Vec::new(),
        }))
    }

    #[frb(sync)]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    fn push_undo_checkpoint(&mut self, prev_data: JourneyVector) -> () {
        self.undo_stack.push(prev_data);
    }

    pub fn get_map_renderer_proxy(&self) -> Result<(MapRendererProxy, Option<CameraOption>)> {
        Ok((
            MapRendererProxy::DynamicRenderer(self.map_renderer.clone()),
            self.initial_camera_option,
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
        // TODO Unable to properly handle cases spanning ±180° of longitude.

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

    pub fn add_lines(&mut self, points: &[(f64, f64)]) -> Result<()> {
        // TODO: we could run the post processor here to simplify the added points first.
        if points.len() < 2 {
            return Ok(());
        }

        self.push_undo_checkpoint(self.data.clone());
        self.data.track_segments.push(TrackSegment {
            track_points: points
                .iter()
                .map(|(lat, lng)| crate::journey_vector::TrackPoint {
                    latitude: *lat,
                    longitude: *lng,
                })
                .collect(),
        });

        let mut map_renderer = self.map_renderer.lock().unwrap();
        map_renderer.update(|journey_bitmap, tile_changed| {
            for window in points.windows(2) {
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

        Ok(())
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
            txn.action = Some(crate::main_db::Action::CompleteRebuilt);
            Ok(())
        })
    }
}
