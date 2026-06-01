//! Wrap-aware longitude arc on the geographic longitude circle.
//!
//! Used by the achievement system's per-journey `BBox` and aggregate
//! `overall_bbox` to handle journeys that cross the 180° meridian
//! correctly. Other call sites in the codebase (`journey_date_picker`,
//! `api/edit_session`, `renderer`) currently use naive `min`/`max` and
//! can adopt this module in follow-up work.
//!
//! Convention (GeoJSON RFC 7946 §5.2): an `LngArc` starts at `west`,
//! goes *eastward* to `east`. `west > east` means the arc crosses the
//! 180° meridian. Both fields are in `[-180.0, 180.0]`. Inputs of
//! `-180.0` are canonicalized to `180.0` so all callers see the same
//! representative for the antimeridian.

/// A 1-D wrap-aware arc on the longitude circle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LngArc {
    pub west: f64,
    pub east: f64,
}

impl LngArc {
    /// True iff this arc crosses the 180° meridian (i.e. `west > east`).
    pub fn crosses_antimeridian(&self) -> bool {
        self.west > self.east
    }

    /// Width going east from `west` to `east`, in degrees. In `[0.0, 360.0]`.
    pub fn width_deg(&self) -> f64 {
        if self.east >= self.west {
            self.east - self.west
        } else {
            360.0 + self.east - self.west
        }
    }

    /// Smallest enclosing arc of a set of longitudes. `None` if empty.
    ///
    /// O(n log n). Sorts via [`f64::total_cmp`], so non-finite inputs sort
    /// without panicking but their semantics in this geometric context are
    /// undefined; callers should pass finite longitudes in `[-180.0, 180.0]`.
    pub fn from_lngs(lngs: impl IntoIterator<Item = f64>) -> Option<Self> {
        let mut vals: Vec<f64> = lngs.into_iter().map(canonicalize).collect();
        if vals.is_empty() {
            return None;
        }
        vals.sort_unstable_by(f64::total_cmp);

        // Find the largest empty gap on the circle. Internal gaps are
        // between consecutive sorted values; the wrap gap goes from the
        // last value eastward across 180° back to the first value.
        // Tie-breaking: `wrap_gap >= max_gap` (non-strict) prefers a
        // non-crossing arc over a crossing one of equal width — useful
        // for inputs exactly 180° apart, e.g. `[0.0, 180.0]`.
        let n = vals.len();
        let mut max_gap = -1.0_f64;
        let mut max_gap_idx: usize = n - 1; // sentinel: wrap gap wins by default
        for i in 0..n.saturating_sub(1) {
            let gap = vals[i + 1] - vals[i];
            if gap > max_gap {
                max_gap = gap;
                max_gap_idx = i;
            }
        }
        let wrap_gap = 360.0 - (vals[n - 1] - vals[0]);
        if wrap_gap >= max_gap {
            // Wrap gap is largest → arc does NOT cross 180°.
            return Some(Self {
                west: vals[0],
                east: vals[n - 1],
            });
        }
        // Internal gap is largest → arc crosses 180°.
        Some(Self {
            west: vals[max_gap_idx + 1],
            east: vals[max_gap_idx],
        })
    }

    /// Smallest enclosing arc of a *union* of arcs. `None` if empty.
    ///
    /// O(n log n). Splits each input arc into one or two non-wrap
    /// intervals on `[-180.0, 180.0]`, sweeps to merge overlaps, then
    /// finds the largest uncovered arc on the circle — the result is
    /// the gap's complement. Tie-break with `>=` prefers a
    /// non-crossing result over a crossing one of equal width.
    pub fn from_arcs(arcs: impl IntoIterator<Item = LngArc>) -> Option<Self> {
        // 1) Split each arc into one or two non-wrap intervals on
        //    [-180.0, 180.0]. A wrap arc [w, e] (w > e) splits into
        //    [w, 180] ∪ [-180, e].
        let mut intervals: Vec<(f64, f64)> = Vec::new();
        for arc in arcs {
            let w = canonicalize(arc.west);
            let e = canonicalize(arc.east);
            if w <= e {
                intervals.push((w, e));
            } else {
                intervals.push((w, 180.0));
                intervals.push((-180.0, e));
            }
        }
        if intervals.is_empty() {
            return None;
        }

        // 2) Sort by start, then sweep to merge overlapping / touching
        //    intervals.
        intervals.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = Vec::with_capacity(intervals.len());
        for (s, e) in intervals {
            match merged.last_mut() {
                Some(last) if s <= last.1 => last.1 = last.1.max(e),
                _ => merged.push((s, e)),
            }
        }

        // 3) Special case: union covers the full circle.
        let n = merged.len();
        if n == 1 && merged[0].0 == -180.0 && merged[0].1 == 180.0 {
            return Some(Self {
                west: -180.0,
                east: 180.0,
            });
        }

        // 4) Find the largest gap. Internal gaps are between consecutive
        //    merged intervals; the wrap gap goes from the last interval's
        //    end across 180° back to the first interval's start.
        //    Tie-break (`wrap_gap >= max_gap`) prefers non-crossing.
        let mut max_gap = -1.0_f64;
        let mut max_gap_idx: usize = n - 1; // sentinel for wrap gap
        for i in 0..n.saturating_sub(1) {
            let gap = merged[i + 1].0 - merged[i].1;
            if gap > max_gap {
                max_gap = gap;
                max_gap_idx = i;
            }
        }
        let wrap_gap = (180.0 - merged[n - 1].1) + (merged[0].0 - (-180.0));
        if wrap_gap >= max_gap {
            // Wrap gap is largest → arc does NOT cross 180°.
            return Some(Self {
                west: merged[0].0,
                east: merged[n - 1].1,
            });
        }
        // Internal gap is largest → arc crosses 180°.
        Some(Self {
            west: merged[max_gap_idx + 1].0,
            east: merged[max_gap_idx].1,
        })
    }
}

/// Fold `-180.0` into `180.0` so both representations of the
/// antimeridian compare equal. Other inputs are returned unchanged.
/// Caller is expected to pass values in `[-180.0, 180.0]`.
fn canonicalize(lng: f64) -> f64 {
    if lng == -180.0 {
        180.0
    } else {
        lng
    }
}

/// True iff `arc` contains `lng` going east from `arc.west` to `arc.east`.
/// Test-only; shared across the achievement module's test suites so the
/// containment convention is defined in one place.
#[cfg(test)]
pub(crate) fn arc_contains(arc: &LngArc, lng: f64) -> bool {
    // Full-circle arc returned by from_arcs when the input union
    // covers all 360°. Canonicalizing collapses both endpoints to
    // 180.0, so this case must be detected before canonicalization.
    if arc.west == -180.0 && arc.east == 180.0 {
        return true;
    }
    let w = canonicalize(arc.west);
    let e = canonicalize(arc.east);
    let l = canonicalize(lng);
    if w <= e {
        l >= w && l <= e
    } else {
        l >= w || l <= e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_deg_non_wrap() {
        let a = LngArc {
            west: 10.0,
            east: 30.0,
        };
        assert!(!a.crosses_antimeridian());
        assert!((a.width_deg() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn width_deg_wrap() {
        let a = LngArc {
            west: 170.0,
            east: -170.0,
        };
        assert!(a.crosses_antimeridian());
        assert!((a.width_deg() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn width_deg_single_point() {
        let a = LngArc {
            west: 5.0,
            east: 5.0,
        };
        assert!(!a.crosses_antimeridian());
        assert_eq!(a.width_deg(), 0.0);
    }

    #[test]
    fn width_deg_full_circle() {
        let a = LngArc {
            west: -180.0,
            east: 180.0,
        };
        assert!(!a.crosses_antimeridian());
        assert!((a.width_deg() - 360.0).abs() < 1e-9);
    }

    #[test]
    fn canonicalize_minus_180_to_180() {
        assert_eq!(canonicalize(-180.0), 180.0);
        assert_eq!(canonicalize(180.0), 180.0);
        assert_eq!(canonicalize(0.0), 0.0);
        assert_eq!(canonicalize(-179.9), -179.9);
    }

    #[test]
    fn from_lngs_empty() {
        let v: Vec<f64> = vec![];
        assert_eq!(LngArc::from_lngs(v), None);
    }

    #[test]
    fn from_lngs_single() {
        let a = LngArc::from_lngs(vec![5.0]).unwrap();
        assert_eq!(
            a,
            LngArc {
                west: 5.0,
                east: 5.0
            }
        );
    }

    #[test]
    fn from_lngs_simple_no_wrap() {
        let a = LngArc::from_lngs(vec![10.0, 30.0, 20.0]).unwrap();
        assert_eq!(
            a,
            LngArc {
                west: 10.0,
                east: 30.0
            }
        );
        assert!(!a.crosses_antimeridian());
    }

    #[test]
    fn from_lngs_simple_wrap() {
        let a = LngArc::from_lngs(vec![179.0, -179.0]).unwrap();
        assert!(a.crosses_antimeridian());
        assert!((a.width_deg() - 2.0).abs() < 1e-9);
        assert_eq!(
            a,
            LngArc {
                west: 179.0,
                east: -179.0
            }
        );
    }

    #[test]
    fn from_lngs_collapses_minus_180() {
        // -180 and 180 should canonicalize to the same point.
        let a = LngArc::from_lngs(vec![-180.0, 180.0]).unwrap();
        assert_eq!(
            a,
            LngArc {
                west: 180.0,
                east: 180.0
            }
        );
        assert_eq!(a.width_deg(), 0.0);
    }

    #[test]
    fn from_lngs_three_points_wrap() {
        // Points at -170, -179, 179. sorted = [-179, -170, 179]. Internal
        // gaps: 9°, 349°. Wrap gap = 360 - (179 - (-179)) = 2°. Largest =
        // 349 (between -170 and 179) → arc wraps from 179 east to -170,
        // covering 11°.
        let a = LngArc::from_lngs(vec![-170.0, -179.0, 179.0]).unwrap();
        assert!(a.crosses_antimeridian());
        assert!((a.width_deg() - 11.0).abs() < 1e-9, "got {}", a.width_deg());
        assert_eq!(
            a,
            LngArc {
                west: 179.0,
                east: -170.0
            }
        );
    }

    #[test]
    fn from_lngs_two_points_180_apart_prefers_non_wrap() {
        // Internal gap (180°) and wrap gap (180°) tie. The non-strict
        // `wrap_gap >= max_gap` tie-break prefers the non-crossing arc.
        let a = LngArc::from_lngs(vec![0.0, 180.0]).unwrap();
        assert!(!a.crosses_antimeridian());
        assert!((a.width_deg() - 180.0).abs() < 1e-9);
        assert_eq!(
            a,
            LngArc {
                west: 0.0,
                east: 180.0
            }
        );
    }

    #[test]
    fn from_lngs_all_equal() {
        // All inputs at the same longitude collapse to a single-point arc.
        let a = LngArc::from_lngs(vec![45.0, 45.0, 45.0]).unwrap();
        assert!(!a.crosses_antimeridian());
        assert_eq!(a.width_deg(), 0.0);
        assert_eq!(
            a,
            LngArc {
                west: 45.0,
                east: 45.0
            }
        );
    }

    #[test]
    fn from_arcs_empty() {
        let v: Vec<LngArc> = vec![];
        assert_eq!(LngArc::from_arcs(v), None);
    }

    #[test]
    fn from_arcs_single_no_wrap_round_trips() {
        let a = LngArc {
            west: 10.0,
            east: 30.0,
        };
        assert_eq!(LngArc::from_arcs(vec![a]).unwrap(), a);
    }

    #[test]
    fn from_arcs_single_wrap_round_trips() {
        let a = LngArc {
            west: 170.0,
            east: -170.0,
        };
        assert_eq!(LngArc::from_arcs(vec![a]).unwrap(), a);
    }

    #[test]
    fn from_arcs_two_disjoint_no_wrap() {
        // Two arcs: [10, 20] and [30, 40]. Gaps: [-180, 10] (190°),
        // [20, 30] (10°), [40, 180] (140°). Largest = 190° → arc is
        // [10, 40] going east (no wrap).
        let a = LngArc {
            west: 10.0,
            east: 20.0,
        };
        let b = LngArc {
            west: 30.0,
            east: 40.0,
        };
        let merged = LngArc::from_arcs(vec![a, b]).unwrap();
        assert_eq!(
            merged,
            LngArc {
                west: 10.0,
                east: 40.0
            }
        );
    }

    #[test]
    fn from_arcs_two_disjoint_with_wrap_picks_wrap() {
        // Two arcs near opposite sides of antimeridian: [170, 175] and
        // [-175, -170]. Their union should wrap, west=170 east=-170.
        let a = LngArc {
            west: 170.0,
            east: 175.0,
        };
        let b = LngArc {
            west: -175.0,
            east: -170.0,
        };
        let merged = LngArc::from_arcs(vec![a, b]).unwrap();
        assert!(merged.crosses_antimeridian(), "got {:?}", merged);
        assert_eq!(
            merged,
            LngArc {
                west: 170.0,
                east: -170.0
            }
        );
    }

    #[test]
    fn from_arcs_overlapping_merges() {
        // [10, 30] and [20, 40] → [10, 40].
        let a = LngArc {
            west: 10.0,
            east: 30.0,
        };
        let b = LngArc {
            west: 20.0,
            east: 40.0,
        };
        let merged = LngArc::from_arcs(vec![a, b]).unwrap();
        assert_eq!(
            merged,
            LngArc {
                west: 10.0,
                east: 40.0
            }
        );
    }

    #[test]
    fn from_arcs_full_circle() {
        // Two complementary arcs covering the whole circle:
        // [0, 90] (90°) and [90, 0] (270°, wrap). Together: [-180, 180].
        let a = LngArc {
            west: 0.0,
            east: 90.0,
        };
        let b = LngArc {
            west: 90.0,
            east: 0.0,
        };
        let merged = LngArc::from_arcs(vec![a, b]).unwrap();
        // Documented degenerate case: full coverage returns the maximal
        // east-going arc {west: -180.0, east: 180.0} — pinning both the
        // width and the canonical representation.
        assert!((merged.width_deg() - 360.0).abs() < 1e-9);
        assert_eq!(
            merged,
            LngArc {
                west: -180.0,
                east: 180.0
            }
        );
    }

    #[test]
    fn from_arcs_two_overlapping_wrap_arcs() {
        // Both arcs wrap and overlap each other.
        //   a: [160, -10] wraps → (160, 180), (-180, -10)
        //   b: [170, 0]   wraps → (170, 180), (-180, 0)
        // Sort: (-180,-10), (-180,0), (160,180), (170,180)
        // Merge: (-180, 0), (160, 180)
        // Internal gap = 160, wrap_gap = 0 → internal wins.
        // Result: west = 160, east = 0 (still wrapping, wider envelope).
        let a = LngArc {
            west: 160.0,
            east: -10.0,
        };
        let b = LngArc {
            west: 170.0,
            east: 0.0,
        };
        let merged = LngArc::from_arcs(vec![a, b]).unwrap();
        assert!(merged.crosses_antimeridian(), "got {:?}", merged);
        assert_eq!(
            merged,
            LngArc {
                west: 160.0,
                east: 0.0
            }
        );
    }

    #[test]
    fn from_arcs_extends_wrap_arc_with_non_wrap() {
        // a wraps: [160, -170] → (160, 180), (-180, -170)
        // b non-wrap on east side: [-170, -160] → (-170, -160)
        // Sort: (-180,-170), (-170,-160), (160,180)
        // Merge: -170 <= -170 → (-180, -160); 160 > -160 → (160, 180)
        // Internal gap = 160 - (-160) = 320, wrap_gap = 0 → internal wins.
        // Result: west = 160, east = -160.
        let a = LngArc {
            west: 160.0,
            east: -170.0,
        };
        let b = LngArc {
            west: -170.0,
            east: -160.0,
        };
        let merged = LngArc::from_arcs(vec![a, b]).unwrap();
        assert!(merged.crosses_antimeridian(), "got {:?}", merged);
        assert_eq!(
            merged,
            LngArc {
                west: 160.0,
                east: -160.0
            }
        );
    }

    use proptest::prelude::*;

    #[test]
    fn arc_contains_full_circle() {
        // Pinning the shared helper's full-circle behaviour. The
        // achievement-side test modules also rely on full-circle arcs
        // (legitimately returned by from_arcs) containing every longitude.
        let full = LngArc {
            west: -180.0,
            east: 180.0,
        };
        for lng in [-180.0, -90.0, 0.0, 90.0, 180.0, 12.34, -56.78] {
            assert!(arc_contains(&full, lng), "full circle should contain {lng}");
        }
    }

    proptest! {
        #[test]
        fn from_lngs_contains_every_input(
            lngs in prop::collection::vec(-180.0_f64..=180.0_f64, 1..=20)
        ) {
            let arc = LngArc::from_lngs(lngs.iter().copied()).expect("non-empty");
            for &l in &lngs {
                prop_assert!(arc_contains(&arc, l), "arc {:?} missing lng {}", arc, l);
            }
        }

        #[test]
        fn from_lngs_width_in_range(
            lngs in prop::collection::vec(-180.0_f64..=180.0_f64, 1..=20)
        ) {
            let arc = LngArc::from_lngs(lngs).expect("non-empty");
            prop_assert!(arc.width_deg() >= 0.0);
            prop_assert!(arc.width_deg() <= 360.0 + 1e-9);
        }

        #[test]
        fn from_arcs_idempotent_on_singleton(
            west in -180.0_f64..=180.0_f64,
            east in -180.0_f64..=180.0_f64,
        ) {
            let a = LngArc { west, east };
            let merged = LngArc::from_arcs(vec![a]).expect("non-empty");
            // Allow ±180.0 normalization differences.
            let to_canon = |x: f64| if x == -180.0 { 180.0 } else { x };
            prop_assert_eq!(to_canon(merged.west), to_canon(a.west));
            prop_assert_eq!(to_canon(merged.east), to_canon(a.east));
        }

        #[test]
        fn from_arcs_order_independent(
            arcs in prop::collection::vec(
                (-180.0_f64..=180.0_f64, -180.0_f64..=180.0_f64)
                    .prop_map(|(w, e)| LngArc { west: w, east: e }),
                1..=10,
            )
        ) {
            let a = LngArc::from_arcs(arcs.iter().copied()).expect("non-empty");
            let mut reversed = arcs.clone();
            reversed.reverse();
            let b = LngArc::from_arcs(reversed).expect("non-empty");
            prop_assert_eq!(a, b);
        }
    }
}
