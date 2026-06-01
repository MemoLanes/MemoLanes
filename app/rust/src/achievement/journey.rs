use std::sync::OnceLock;

use chrono::{DateTime, NaiveDate, Utc};

use crate::journey_data::JourneyData;
use crate::journey_header::JourneyKind;

/// Stable identifier for a Journey (matches main_db journey.id, a UUID).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JourneyId(pub String);

impl JourneyId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Bounding box for a journey's GPS extent.
///
/// Latitude is a plain interval (`lat_min..=lat_max`). Longitude is a
/// wrap-aware arc on the longitude circle: the bbox starts at `lng_west`
/// and goes *eastward* to `lng_east`. `lng_west > lng_east` means the
/// bbox crosses the 180° meridian. See `crate::lng_arc::LngArc`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBox {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_west: f64,
    pub lng_east: f64,
}

/// Per-journey derived facts. Computed once per Journey instance.
/// Successor to the existing `metrics::JourneyMetrics`.
#[derive(Debug, Clone, PartialEq)]
pub struct JourneyFacts {
    /// None for bitmap-only imported journeys.
    pub distance_m: Option<f64>,
    /// None when `start`/`end` headers are missing.
    pub duration_sec: Option<i64>,
    pub is_loop: bool,
    pub bbox: Option<BBox>,
}

/// One finalized journey, loaded into memory.
///
/// `raw_data` is populated eagerly at construction time (see
/// `from_header_with_data` and `from_parts`). `bitmap` and `facts` use
/// `std::sync::OnceLock` (NOT `OnceCell`) for thread-safe lazy init on
/// first access.
///
/// THREADING: `Journey` is `!Sync` because `JourneyBitmap` (V2) carries a
/// `RefCell` mipmap cache inside `Block`. The achievement pipeline runs on
/// a single thread, so `Arc<Journey>` is used purely for cheap refcount
/// sharing — never sent across threads. The `arc_with_non_send_sync`
/// clippy lint is suppressed at the module level in achievement callers.
/// If you need to move achievement work off-thread, either switch to
/// `Rc<Journey>` (single-threaded) or wrap the bitmap mipmap in a
/// thread-safe cell.
pub struct Journey {
    pub id: JourneyId,
    pub date: NaiveDate,
    pub kind: JourneyKind,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    raw_data: OnceLock<JourneyData>,
    bitmap: OnceLock<crate::journey_bitmap::JourneyBitmap>,
    facts: OnceLock<JourneyFacts>,
}

impl std::fmt::Debug for Journey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Journey")
            .field("id", &self.id)
            .field("date", &self.date)
            .field("kind", &self.kind)
            .field("start_time", &self.start_time)
            .field("end_time", &self.end_time)
            .finish_non_exhaustive()
    }
}

impl Journey {
    /// Test-only constructor. Not for production use.
    /// Builds a Journey with `raw_data` already populated. Used by unit
    /// tests across the achievement module to construct fixtures without
    /// touching Storage.
    #[doc(hidden)]
    pub fn from_parts(
        id: JourneyId,
        date: NaiveDate,
        kind: JourneyKind,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        raw_data: JourneyData,
    ) -> Self {
        let raw_cell = OnceLock::new();
        let _ = raw_cell.set(raw_data);
        Self {
            id,
            date,
            kind,
            start_time,
            end_time,
            raw_data: raw_cell,
            bitmap: OnceLock::new(),
            facts: OnceLock::new(),
        }
    }

    /// Returns the eagerly-populated raw data.
    /// Internal — `bitmap()`/`facts()` go through this.
    fn raw_data(&self) -> &JourneyData {
        self.raw_data
            .get()
            .expect("Journey raw_data must be pre-populated")
    }

    /// Returns the bitmap. For `JourneyData::Bitmap` variants this is a
    /// clone of the stored bitmap; for `JourneyData::Vector` this builds
    /// the bitmap by calling `JourneyBitmap::merge_vector(&v)`.
    /// Result is cached; subsequent calls are O(1).
    pub fn bitmap(&self) -> &crate::journey_bitmap::JourneyBitmap {
        self.bitmap.get_or_init(|| {
            let mut bm = crate::journey_bitmap::JourneyBitmap::new();
            self.raw_data().clone().merge_into(&mut bm);
            bm
        })
    }

    /// Per-journey derived facts. Computed once per Journey instance.
    pub fn facts(&self) -> &JourneyFacts {
        self.facts.get_or_init(|| self.compute_facts())
    }

    fn compute_facts(&self) -> JourneyFacts {
        match self.raw_data() {
            JourneyData::Vector(v) => compute_facts_from_vector(v, self.start_time, self.end_time),
            JourneyData::Bitmap(b) => compute_facts_from_bitmap(b, self.start_time, self.end_time),
        }
    }

    /// Construct a Journey with raw_data already populated. Used by the
    /// eager loader path.
    pub fn from_header_with_data(
        header: &crate::journey_header::JourneyHeader,
        data: JourneyData,
    ) -> Self {
        let raw_cell = OnceLock::new();
        let _ = raw_cell.set(data);
        Self {
            id: JourneyId(header.id.clone()),
            date: header.journey_date,
            kind: header.journey_kind,
            start_time: header.start,
            end_time: header.end,
            raw_data: raw_cell,
            bitmap: OnceLock::new(),
            facts: OnceLock::new(),
        }
    }

    pub fn distance_m(&self) -> Option<f64> {
        self.facts().distance_m
    }
    pub fn duration_sec(&self) -> Option<i64> {
        self.facts().duration_sec
    }
    pub fn is_loop(&self) -> bool {
        self.facts().is_loop
    }
    pub fn bbox(&self) -> Option<BBox> {
        self.facts().bbox
    }
}

/// Haversine distance in meters between two lat/lng points.
fn haversine_m(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    const EARTH_RADIUS_M: f64 = 6_371_000.0;
    let to_rad = |d: f64| d.to_radians();
    let dlat = to_rad(lat2 - lat1);
    let dlng = to_rad(lng2 - lng1);
    let a = (dlat / 2.0).sin().powi(2)
        + to_rad(lat1).cos() * to_rad(lat2).cos() * (dlng / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    EARTH_RADIUS_M * c
}

fn compute_facts_from_vector(
    v: &crate::journey_vector::JourneyVector,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
) -> JourneyFacts {
    let mut total_distance = 0.0;
    let mut lat_min = f64::INFINITY;
    let mut lat_max = f64::NEG_INFINITY;
    let mut lngs: Vec<f64> = Vec::new();
    let mut first_pt = None;
    let mut last_pt = None;

    for seg in &v.track_segments {
        let mut prev: Option<&crate::journey_vector::TrackPoint> = None;
        for p in &seg.track_points {
            if first_pt.is_none() {
                first_pt = Some((p.latitude, p.longitude));
            }
            last_pt = Some((p.latitude, p.longitude));
            lat_min = lat_min.min(p.latitude);
            lat_max = lat_max.max(p.latitude);
            lngs.push(p.longitude);
            if let Some(prev_p) = prev {
                total_distance +=
                    haversine_m(prev_p.latitude, prev_p.longitude, p.latitude, p.longitude);
            }
            prev = Some(p);
        }
    }

    let bbox = crate::lng_arc::LngArc::from_lngs(lngs.iter().copied()).map(|arc| BBox {
        lat_min,
        lat_max,
        lng_west: arc.west,
        lng_east: arc.east,
    });

    let is_loop = match (first_pt, last_pt) {
        (Some(a), Some(b)) if total_distance > 100.0 => haversine_m(a.0, a.1, b.0, b.1) < 100.0,
        _ => false,
    };

    let duration_sec = match (start_time, end_time) {
        (Some(s), Some(e)) => Some((e - s).num_seconds()),
        _ => None,
    };

    JourneyFacts {
        distance_m: Some(total_distance),
        duration_sec,
        is_loop,
        bbox,
    }
}

fn compute_facts_from_bitmap(
    b: &crate::journey_bitmap::JourneyBitmap,
    _start_time: Option<DateTime<Utc>>,
    _end_time: Option<DateTime<Utc>>,
) -> JourneyFacts {
    // Bitmap-only imports have no track points, so distance/duration/loop
    // are not derivable from the bitmap. bbox is approximated from the
    // tile envelope (tile-level precision, ~78km at the equator).
    let bbox = if b.is_empty() {
        None
    } else {
        let mut tx_set: std::collections::BTreeSet<u16> = std::collections::BTreeSet::new();
        let mut ty_min = u16::MAX;
        let mut ty_max = 0u16;
        for tile_key in b.all_tile_keys() {
            tx_set.insert(tile_key.x);
            ty_min = ty_min.min(tile_key.y);
            ty_max = ty_max.max(tile_key.y);
        }
        let map_w = crate::journey_bitmap::MAP_WIDTH as f64;
        // Both helpers take u32 so `+ 1` on the extreme tile coordinate
        // (`MAP_WIDTH - 1`) cannot overflow — `512` fits comfortably,
        // and `to_lng(MAP_WIDTH)` correctly evaluates to 180.0°.
        let to_lng = |tx: u32| (tx as f64) / map_w * 360.0 - 180.0;
        let to_lat = |ty: u32| {
            let n = std::f64::consts::PI - 2.0 * std::f64::consts::PI * ty as f64 / map_w;
            n.sinh().atan().to_degrees()
        };
        let arcs = tx_set.iter().map(|&tx| crate::lng_arc::LngArc {
            west: to_lng(tx as u32),
            east: to_lng(tx as u32 + 1),
        });
        let lng_arc = crate::lng_arc::LngArc::from_arcs(arcs).expect("non-empty tile set ⇒ Some");
        // Note: y axis is flipped (tile y=0 is north). max_lat corresponds
        // to ty_min; lat_min corresponds to the bottom edge of ty_max.
        Some(BBox {
            lat_min: to_lat(ty_max as u32 + 1),
            lat_max: to_lat(ty_min as u32),
            lng_west: lng_arc.west,
            lng_east: lng_arc.east,
        })
    };
    JourneyFacts {
        distance_m: None,
        duration_sec: None,
        is_loop: false,
        bbox,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::achievement::test_strategies::{arb_journey_bitmap, Grid};
    use crate::journey_bitmap::JourneyBitmap;
    use crate::journey_data::JourneyData;
    use crate::journey_header::JourneyKind;
    use crate::journey_vector::{JourneyVector, TrackPoint, TrackSegment};
    use proptest::prelude::*;

    fn make_bitmap_journey(bm: JourneyBitmap, kind: JourneyKind) -> Journey {
        Journey::from_parts(
            JourneyId("p".into()),
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            kind,
            None,
            None,
            JourneyData::Bitmap(bm),
        )
    }

    use crate::lng_arc::arc_contains;

    fn make_vector_journey(longitude_deg: f64) -> Journey {
        Journey::from_parts(
            JourneyId("v".into()),
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            JourneyKind::DefaultKind,
            None,
            None,
            JourneyData::Vector(JourneyVector {
                track_segments: vec![TrackSegment {
                    track_points: vec![
                        TrackPoint {
                            latitude: 0.0,
                            longitude: 0.0,
                        },
                        TrackPoint {
                            latitude: 0.0,
                            longitude: longitude_deg,
                        },
                    ],
                }],
            }),
        )
    }

    #[test]
    fn vector_journey_crossing_antimeridian_has_short_bbox() {
        // Two points 0.2° apart across the antimeridian: 179.9 → -179.9.
        let j = Journey::from_parts(
            JourneyId("am".into()),
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            JourneyKind::DefaultKind,
            None,
            None,
            JourneyData::Vector(JourneyVector {
                track_segments: vec![TrackSegment {
                    track_points: vec![
                        TrackPoint {
                            latitude: 0.0,
                            longitude: 179.9,
                        },
                        TrackPoint {
                            latitude: 0.0,
                            longitude: -179.9,
                        },
                    ],
                }],
            }),
        );
        let bbox = j.facts().bbox.expect("vector should have bbox");
        let arc = crate::lng_arc::LngArc {
            west: bbox.lng_west,
            east: bbox.lng_east,
        };
        assert!(arc.crosses_antimeridian(), "bbox {:?} should wrap", bbox);
        assert!(
            arc.width_deg() < 1.0,
            "bbox span {}° should be ~0.2°",
            arc.width_deg()
        );
    }

    #[test]
    fn bitmap_journey_crossing_antimeridian_has_short_bbox() {
        use crate::journey_bitmap::{Block, BlockKey, JourneyBitmap, TileKey, MAP_WIDTH};
        // Two tiles, one at tile_x = 0 (just east of antimeridian) and
        // one at tile_x = MAP_WIDTH - 1 (just west of antimeridian).
        // Naive min/max would span ~360°; wrap-aware should span ~2 tiles.
        let mut bm = JourneyBitmap::new();
        for &tx in &[0_u16, (MAP_WIDTH as u16) - 1] {
            let tile = bm.get_tile_mut_or_insert_empty(&TileKey::new(tx, 256));
            let mut block = Block::new();
            block.set_point(0, 0, true);
            tile.set(&BlockKey::from_x_y(0, 0), block);
        }
        let j = Journey::from_parts(
            JourneyId("am-bm".into()),
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            JourneyKind::Flight,
            None,
            None,
            JourneyData::Bitmap(bm),
        );
        let bbox = j.facts().bbox.expect("non-empty bitmap");
        let arc = crate::lng_arc::LngArc {
            west: bbox.lng_west,
            east: bbox.lng_east,
        };
        assert!(arc.crosses_antimeridian(), "bbox {:?} should wrap", bbox);
        // Two tile-widths: 360 / MAP_WIDTH * 2 ≈ 1.4°
        let two_tiles_deg = 360.0 / MAP_WIDTH as f64 * 2.0;
        assert!(
            arc.width_deg() <= two_tiles_deg + 1e-6,
            "bbox span {}° should be at most {}°",
            arc.width_deg(),
            two_tiles_deg,
        );
    }

    proptest! {
        // §8.6 bitmap journey ⇒ no distance / duration.
        #[test]
        fn bitmap_no_distance_no_duration(bm in arb_journey_bitmap(Grid::default_4x4())) {
            let j = make_bitmap_journey(bm, JourneyKind::Flight);
            let f = j.facts();
            prop_assert!(f.distance_m.is_none());
            prop_assert!(f.duration_sec.is_none());
        }

        // §8.6 empty bitmap (i.e. tiles HashMap empty) ⇒ no bbox.
        #[test]
        fn empty_bitmap_no_bbox(_seed in any::<u8>()) {
            let j = make_bitmap_journey(JourneyBitmap::new(), JourneyKind::Flight);
            prop_assert!(j.facts().bbox.is_none());
        }

        // §8.6 caching: facts() returns the same reference twice.
        #[test]
        fn facts_caching(longitude in 0.001f64..1.0) {
            let j = make_vector_journey(longitude);
            let f1 = j.facts() as *const _;
            let f2 = j.facts() as *const _;
            prop_assert_eq!(f1, f2);
        }

        // §8.6 vector journey distance ≥ 0.
        #[test]
        fn vector_distance_non_negative(longitude in 0.0f64..1.0) {
            let j = make_vector_journey(longitude);
            if let Some(d) = j.facts().distance_m {
                prop_assert!(d >= 0.0);
            }
        }

        // §8.6 non-empty bitmap ⇒ Some(bbox) with lat well-ordered and
        // lng forming a valid wrap-aware LngArc that contains every set
        // tile's longitude midpoint. Run under two grids so the wrap
        // branch in LngArc::from_arcs gets probabilistic coverage.
        #[test]
        fn nonempty_bitmap_has_well_ordered_bbox(bm in arb_journey_bitmap(Grid::default_4x4())) {
            check_nonempty_bitmap_invariants(&bm)?;
        }

        #[test]
        fn nonempty_bitmap_antimeridian_has_well_ordered_bbox(
            bm in arb_journey_bitmap(Grid::antimeridian_4x4())
        ) {
            check_nonempty_bitmap_invariants(&bm)?;
        }
    }

    /// Body of the bbox proptest, factored so the same invariants run
    /// under both `Grid::default_4x4()` and `Grid::antimeridian_4x4()`.
    fn check_nonempty_bitmap_invariants(
        bm: &JourneyBitmap,
    ) -> Result<(), proptest::test_runner::TestCaseError> {
        // Skip the empty case — that's covered by empty_bitmap_no_bbox.
        if bm.is_empty() {
            return Ok(());
        }
        let j = make_bitmap_journey(bm.clone(), JourneyKind::Flight);
        let f = j.facts();
        let bbox = f.bbox.expect("non-empty bitmap should have bbox");
        prop_assert!(
            bbox.lat_max >= bbox.lat_min,
            "lat: max={} min={}",
            bbox.lat_max,
            bbox.lat_min
        );
        prop_assert!(bbox.lat_min.is_finite() && bbox.lat_max.is_finite());
        prop_assert!(bbox.lng_west.is_finite() && bbox.lng_east.is_finite());
        prop_assert!((-180.0..=180.0).contains(&bbox.lng_west));
        prop_assert!((-180.0..=180.0).contains(&bbox.lng_east));

        // The bbox arc must contain every set tile's longitude midpoint.
        let arc = crate::lng_arc::LngArc {
            west: bbox.lng_west,
            east: bbox.lng_east,
        };
        let map_w = crate::journey_bitmap::MAP_WIDTH as f64;
        for tile_key in bm.all_tile_keys() {
            let tx = tile_key.x;
            let mid_lng = (tx as f64 + 0.5) / map_w * 360.0 - 180.0;
            prop_assert!(
                arc_contains(&arc, mid_lng),
                "arc {:?} should contain tile_x={} mid_lng={}",
                arc,
                tx,
                mid_lng
            );
        }
        Ok(())
    }
}
