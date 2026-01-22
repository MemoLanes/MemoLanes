use anyhow::{bail, Result};
use flutter_rust_bridge::frb;

use crate::journey_data::JourneyData;

use super::api::{get, CameraOption, MapRendererProxy};

#[frb(opaque)]
pub struct EditSession {
    journey_id: String,
    data: JourneyData,
    undo_stack: Vec<JourneyData>,
}

impl EditSession {
    pub fn new(journey_id: String) -> Result<Self> {
        let state = get();
        let journey_data = state
            .storage
            .with_db_txn(|txn| txn.get_journey_data(&journey_id))?;

        Ok(Self {
            journey_id,
            data: journey_data,
            undo_stack: Vec::new(),
        })
    }

    #[frb(sync)]
    pub fn journey_id(&self) -> String {
        self.journey_id.clone()
    }

    #[frb(sync)]
    pub fn is_vector(&self) -> bool {
        matches!(self.data, JourneyData::Vector(_))
    }

    #[frb(sync)]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn push_undo_checkpoint(&mut self) -> bool {
        if self.undo_stack.last() != Some(&self.data) {
            self.undo_stack.push(self.data.clone());
            return true;
        }
        false
    }

    pub fn get_map_renderer_proxy(&self) -> Result<(MapRendererProxy, Option<CameraOption>)> {
        super::api::get_map_renderer_proxy_for_journey_data_internal(self.data.clone())
    }

    pub fn undo(&mut self) -> Result<(MapRendererProxy, Option<CameraOption>)> {
        if let Some(previous) = self.undo_stack.pop() {
            self.data = previous;
            return self.get_map_renderer_proxy();
        }
        bail!("Nothing to undo")
    }

    pub fn delete_points_in_box(
        &mut self,
        start_lat: f64,
        start_lng: f64,
        end_lat: f64,
        end_lng: f64,
    ) -> Result<(MapRendererProxy, Option<CameraOption>)> {
        if let JourneyData::Vector(vector) = &mut self.data {
            let min_lat = start_lat.min(end_lat);
            let max_lat = start_lat.max(end_lat);
            let min_lng = start_lng.min(end_lng);
            let max_lng = start_lng.max(end_lng);

            let inside = |p: &crate::journey_vector::TrackPoint| -> bool {
                p.latitude >= min_lat
                    && p.latitude <= max_lat
                    && p.longitude >= min_lng
                    && p.longitude <= max_lng
            };

            let segment_intersections =
                |a: &crate::journey_vector::TrackPoint,
                 b: &crate::journey_vector::TrackPoint|
                 -> Vec<(f64, crate::journey_vector::TrackPoint)> {
                    let x0 = a.longitude;
                    let y0 = a.latitude;
                    let x1 = b.longitude;
                    let y1 = b.latitude;
                    let dx = x1 - x0;
                    let dy = y1 - y0;

                    let mut hits: Vec<(f64, crate::journey_vector::TrackPoint)> = Vec::new();
                    let eps = 1e-12_f64;

                    let mut push_hit = |t: f64, x: f64, y: f64| {
                        if t < -eps || t > 1.0 + eps {
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

                    if dx.abs() > eps {
                        for x_edge in [min_lng, max_lng] {
                            let t = (x_edge - x0) / dx;
                            let y = y0 + t * dy;
                            if y >= min_lat - eps && y <= max_lat + eps {
                                push_hit(t, x_edge, y);
                            }
                        }
                    }

                    if dy.abs() > eps {
                        for y_edge in [min_lat, max_lat] {
                            let t = (y_edge - y0) / dy;
                            let x = x0 + t * dx;
                            if x >= min_lng - eps && x <= max_lng + eps {
                                push_hit(t, x, y_edge);
                            }
                        }
                    }

                    hits.sort_by(|(t1, _), (t2, _)| t1.partial_cmp(t2).unwrap());
                    hits
                };

            let mut new_segments: Vec<crate::journey_vector::TrackSegment> = Vec::new();

            for segment in &vector.track_segments {
                let pts = &segment.track_points;
                if pts.len() < 2 {
                    continue;
                }

                let mut current: Vec<crate::journey_vector::TrackPoint> = Vec::new();
                if !inside(&pts[0]) {
                    current.push(pts[0].clone());
                }

                for i in 0..(pts.len() - 1) {
                    let a = &pts[i];
                    let b = &pts[i + 1];
                    let inside_a = inside(a);
                    let inside_b = inside(b);
                    let hits = segment_intersections(a, b);

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

            vector.track_segments = new_segments;
            return self.get_map_renderer_proxy();
        }

        bail!("Bitmap journey is not editable")
    }

    pub fn add_line(
        &mut self,
        start_lat: f64,
        start_lng: f64,
        end_lat: f64,
        end_lng: f64,
    ) -> Result<(MapRendererProxy, Option<CameraOption>)> {
        if let JourneyData::Vector(vector) = &mut self.data {
            let eps = 1e-12_f64;
            if (start_lat - end_lat).abs() < eps && (start_lng - end_lng).abs() < eps {
                return self.get_map_renderer_proxy();
            }

            vector
                .track_segments
                .push(crate::journey_vector::TrackSegment {
                    track_points: vec![
                        crate::journey_vector::TrackPoint {
                            latitude: start_lat,
                            longitude: start_lng,
                        },
                        crate::journey_vector::TrackPoint {
                            latitude: end_lat,
                            longitude: end_lng,
                        },
                    ],
                });

            return self.get_map_renderer_proxy();
        }

        bail!("Bitmap journey is not editable")
    }

    pub fn commit(&self) -> Result<()> {
        let state = get();
        let journey_id = self.journey_id.clone();
        let journey_data = self.data.clone();

        state.storage.with_db_txn(|txn| {
            txn.update_journey_data(&journey_id, journey_data, None)?;
            txn.action = Some(crate::main_db::Action::CompleteRebuilt);
            Ok(())
        })
    }

    #[frb(sync)]
    pub fn discard(&self) {
        // Intentionally a no-op. Dropping the EditSession on the Dart side
        // abandons any uncommitted changes.
    }
}
