#[cfg(geo_bins_present)]
pub mod test_utils;

use bitmap_geometry::{polar_pixel_area_cm2, MERCATOR_MAX_LATITUDE};
use memolanes_core::journey_bitmap::{area::journey_bitmap_area_cm2, JourneyBitmap};

fn line(a: (f64, f64), b: (f64, f64)) -> JourneyBitmap {
    let mut bitmap = JourneyBitmap::new();
    bitmap.edit().add_line(a.0, a.1, b.0, b.1);
    bitmap
}

#[test]
fn exact_poles_ignore_longitude_and_overlapping_aliases_count_once() {
    for latitude in [-90.0, 90.0] {
        let expected = line((0.0, latitude), (0.0, latitude));
        assert_eq!(
            journey_bitmap_area_cm2(&expected, None),
            81 * polar_pixel_area_cm2()
        );
        let mut merged = expected.clone();
        for longitude in [-180.0, -179.999, -90.0, 0.0, 90.0, 179.999, 180.0] {
            let point = line((longitude, latitude), (longitude, latitude));
            assert_eq!(point, expected, "({longitude}, {latitude})");
            merged.edit().merge(point);
        }
        assert_eq!(merged, expected);
    }
}

#[test]
fn dateline_point_aliases_match_at_poles_caps_and_both_seams() {
    for latitude in [
        -90.0,
        -89.999,
        -88.0,
        -MERCATOR_MAX_LATITUDE - 1e-6,
        -MERCATOR_MAX_LATITUDE,
        -MERCATOR_MAX_LATITUDE + 1e-6,
        0.0,
        MERCATOR_MAX_LATITUDE - 1e-6,
        MERCATOR_MAX_LATITUDE,
        MERCATOR_MAX_LATITUDE + 1e-6,
        88.0,
        89.999,
        90.0,
    ] {
        let west = line((-180.0, latitude), (-180.0, latitude));
        let east = line((180.0, latitude), (180.0, latitude));
        assert_eq!(west, east, "latitude {latitude}");
        assert!(journey_bitmap_area_cm2(&west, None) > 0);
        let mut merged = west.clone();
        merged.edit().merge(east);
        assert_eq!(merged, west, "duplicate area at latitude {latitude}");
    }
}

#[test]
fn tracks_ending_at_or_crossing_a_pole_preserve_dateline_aliases() {
    for (pole, near) in [(-90.0, -89.999), (90.0, 89.999)] {
        let expected = line((180.0, near), (0.0, pole));
        for start_lng in [-180.0, 180.0] {
            for pole_lng in [-180.0, -90.0, 0.0, 90.0, 180.0] {
                assert_eq!(line((start_lng, near), (pole_lng, pole)), expected);
                assert_eq!(line((pole_lng, pole), (start_lng, near)), expected);
            }
        }
        for end_lng in [-180.0, 180.0] {
            let arc = line((0.0, near), (end_lng, near));
            assert_eq!(arc, line((end_lng, near), (0.0, near)));
            let mut with_pole = arc.clone();
            with_pole.edit().merge(line((0.0, pole), (0.0, pole)));
            assert_eq!(with_pole, arc, "arc lost its exact pole crossing");
        }
    }
}

#[cfg(geo_bins_present)]
#[test]
fn real_assets_resolve_exact_poles_and_dateline_aliases_in_every_worldview() {
    use bitmap_geometry::{project_polar_laea, BitmapPlane, BITMAP_WIDTH};
    use geo_data_format::Worldview;
    use memolanes_core::{
        geo::{GeoIndex, GeoLookup},
        journey_bitmap::{BlockKey, TileKey},
    };

    for &worldview in Worldview::ALL {
        let geo = GeoIndex::open(&test_utils::geo_asset(worldview.spec().id)).unwrap();
        for (plane, sign) in [
            (BitmapPlane::NorthPolar, 1.0),
            (BitmapPlane::SouthPolar, -1.0),
        ] {
            for latitude in [88.0, 89.999, 90.0] {
                let mut first = None;
                for longitude in [-180.0, 180.0] {
                    let (x, y) = project_polar_laea(longitude, sign * latitude, plane);
                    let bx = (x / BITMAP_WIDTH as f64).floor() as u32;
                    let by = (y / BITMAP_WIDTH as f64).floor() as u32;
                    let tile = TileKey::new(plane, (bx / 128) as u16, (by / 128) as u16);
                    let block = BlockKey::from_x_y((bx % 128) as u8, (by % 128) as u8);
                    let owner = geo.entity_of_block(tile, block).unwrap();
                    if plane == BitmapPlane::NorthPolar {
                        assert!(owner.is_none(), "Arctic ocean got land credit");
                    } else {
                        let id = owner.expect("Antarctica must include the South Pole");
                        assert_eq!(geo.describe(&[id]).unwrap()[&id].canonical_code, "ATA");
                        let ancestors = geo.ancestors(id);
                        assert!(geo
                            .describe(&ancestors)
                            .unwrap()
                            .values()
                            .any(|e| e.canonical_code == "AN"));
                    }
                    if let Some(previous) = first {
                        assert_eq!((tile, block.index(), owner), previous);
                    }
                    first = Some((tile, block.index(), owner));
                }
            }
        }
    }
}
