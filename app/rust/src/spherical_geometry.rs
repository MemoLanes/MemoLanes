//! Projection-independent geometry for interpolating sparse GPS/flight fixes.

pub(crate) const EARTH_RADIUS_METERS: f64 = 6_371_000.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct UnitPoint {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) z: f64,
}

impl UnitPoint {
    pub(crate) fn from_lng_lat(lng: f64, lat: f64) -> Self {
        let lng = lng.to_radians();
        let lat = lat.to_radians();
        Self {
            x: lat.cos() * lng.cos(),
            y: lat.cos() * lng.sin(),
            z: lat.sin(),
        }
    }
    pub(crate) fn to_lng_lat(self) -> (f64, f64) {
        (
            self.y.atan2(self.x).to_degrees(),
            self.z.atan2(self.x.hypot(self.y)).to_degrees(),
        )
    }
    pub(crate) fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }
    fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }
    fn norm(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub(crate) fn normalized(self) -> Self {
        let norm = self.norm();
        Self {
            x: self.x / norm,
            y: self.y / norm,
            z: self.z / norm,
        }
    }
    pub(crate) fn angle_to(self, other: Self) -> f64 {
        // atan2 retains small GPS distances that acos(dot) rounds down to zero.
        self.cross(other)
            .norm()
            .atan2(self.dot(other).clamp(-1.0, 1.0))
    }
    pub(crate) fn scale_add(self, scale: f64, other: Self, other_scale: f64) -> Self {
        Self {
            x: self.x * scale + other.x * other_scale,
            y: self.y * scale + other.y * other_scale,
            z: self.z * scale + other.z * other_scale,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GreatCircleArc {
    start: UnitPoint,
    end: UnitPoint,
    tangent: UnitPoint,
    pub(crate) angle: f64,
}
impl GreatCircleArc {
    pub(crate) fn new(start: UnitPoint, end: UnitPoint) -> Self {
        let angle = start.angle_to(end);
        let mut axis = start.cross(end);
        if axis.norm() < 1e-12 {
            // Antipodal fixes do not determine a unique arc. Use a stable
            // orthogonal axis; coincident fixes do not interpolate at all.
            let reference = if start.z.abs() < 0.9 {
                UnitPoint {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                }
            } else {
                UnitPoint {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                }
            };
            axis = start.cross(reference);
        }
        Self {
            start,
            end,
            tangent: axis.normalized().cross(start),
            angle,
        }
    }
    pub(crate) fn at(self, t: f64) -> UnitPoint {
        if t <= 0.0 {
            return self.start;
        }
        if t >= 1.0 {
            return self.end;
        }
        if self.angle < 1e-12 {
            return self.start;
        }
        self.start
            .scale_add((self.angle * t).cos(), self.tangent, (self.angle * t).sin())
            .normalized()
    }
    pub(crate) fn enters_latitude_cap(self, latitude: f64) -> bool {
        let threshold = latitude.to_radians().sin();
        if self.start.z.abs() > threshold || self.end.z.abs() > threshold {
            return true;
        }
        if self.angle < 1e-12 {
            return false;
        }
        let phase = self.tangent.z.atan2(self.start.z);
        for k in -2..=2 {
            let u = phase + k as f64 * std::f64::consts::PI;
            if (0.0..=self.angle).contains(&u) {
                let z = self.start.z * u.cos() + self.tangent.z * u.sin();
                if z.abs() > threshold {
                    return true;
                }
            }
        }
        false
    }
    /// All interior intersections with the two latitude seams, in arc fractions.
    pub(crate) fn latitude_intersections(self, latitude: f64) -> Vec<f64> {
        if self.angle < 1e-12 {
            return Vec::new();
        }
        let threshold = latitude.to_radians().sin();
        let amplitude = self.start.z.hypot(self.tangent.z);
        let phase = self.tangent.z.atan2(self.start.z);
        let mut cuts = Vec::new();
        for seam in [threshold, -threshold] {
            if amplitude < seam.abs() {
                continue;
            }
            let offset = (seam / amplitude).clamp(-1.0, 1.0).acos();
            for sign in [-1.0, 1.0] {
                for turn in -1..=1 {
                    let u = phase + sign * offset + turn as f64 * std::f64::consts::TAU;
                    if u > 0.0 && u < self.angle {
                        cuts.push(u / self.angle);
                    }
                }
            }
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        cuts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_distances_remain_nonzero() {
        let a = UnitPoint::from_lng_lat(10.0, 20.0);
        let b = UnitPoint::from_lng_lat(10.0000001, 20.0);
        let metres = a.angle_to(b) * EARTH_RADIUS_METERS;
        assert!(metres > 0.010 && metres < 0.011);
    }
    #[test]
    fn arc_reversal_preserves_the_path_including_antipodal_fixes() {
        for (a, b) in [
            ((0.0, 0.0), (180.0, 0.0)),
            ((170.0, 88.0), (-170.0, 88.2)),
            ((0.0, 89.0), (180.0, 89.0)),
        ] {
            let a = UnitPoint::from_lng_lat(a.0, a.1);
            let b = UnitPoint::from_lng_lat(b.0, b.1);
            for i in 0..=100 {
                let p = GreatCircleArc::new(a, b).at(i as f64 / 100.0);
                let q = GreatCircleArc::new(b, a).at(1.0 - i as f64 / 100.0);
                assert!((p.x - q.x).abs() + (p.y - q.y).abs() + (p.z - q.z).abs() < 1e-10);
            }
        }
    }
}
