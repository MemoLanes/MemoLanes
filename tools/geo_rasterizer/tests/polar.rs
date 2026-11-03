use std::collections::BTreeMap;

use bitmap_geometry::{
    block_pixel_bounds, polar_pixel_area_cm2, polar_rectangle_area_cm2, project_polar_laea,
    BitmapPlane, BITMAP_WIDTH, POLAR_DISK_RADIUS,
};
use geo_data_format::{cell_index, GeoEntityId, TileMembership, NO_ENTITY};
use geo_rasterizer::rasterize::{rasterize_in_plane, BlockLookup, TileLookup};
use geo_types::{polygon, MultiPolygon};

fn owner(
    raster: &(TileLookup, BlockLookup),
    plane: BitmapPlane,
    lng: f64,
    lat: f64,
) -> Option<u32> {
    let (x, y) = project_polar_laea(lng, lat, plane);
    let bx = (x / BITMAP_WIDTH as f64).floor() as usize;
    let by = (y / BITMAP_WIDTH as f64).floor() as usize;
    let (tx, ty) = ((bx / 128) as u16, (by / 128) as u16);
    match raster.0[tx as usize * plane.grid_width() as usize + ty as usize] {
        TileMembership::None => None,
        TileMembership::Single(id) => Some(id.0),
        TileMembership::Border => {
            let id = raster.1[&(tx, ty)][cell_index((bx % 128) as u8, (by % 128) as u8)];
            (id != NO_ENTITY).then_some(id)
        }
    }
}

#[test]
fn pole_closures_fill_caps_without_filling_square_corners() {
    for (plane, s) in [
        (BitmapPlane::NorthPolar, 1.0),
        (BitmapPlane::SouthPolar, -1.0),
    ] {
        let cap = polygon![(x:-180.0,y:s*84.0),(x:-90.0,y:s*84.0),(x:0.0,y:s*84.0),(x:90.0,y:s*84.0),(x:180.0,y:s*84.0),(x:180.0,y:s*90.0),(x:-180.0,y:s*90.0),(x:-180.0,y:s*84.0)];
        let raster = rasterize_in_plane(
            &BTreeMap::from([("CAP".into(), MultiPolygon(vec![cap]))]),
            &BTreeMap::from([("CAP".into(), GeoEntityId(1))]),
            plane,
        );
        for lat in [85.0512, 87.0, 89.999, 90.0] {
            for lng in [-180.0, -179.99, -90.0, 0.0, 90.0, 179.99, 180.0] {
                assert_eq!(
                    owner(&raster, plane, lng, s * lat),
                    Some(1),
                    "{plane:?} {lng} {lat}"
                );
            }
        }
        assert_eq!(raster.0[0], TileMembership::None);
        // Independent disk-area oracle catches missing wedges/radial cuts that
        // a handful of geographic point samples would not detect.
        let mut area_cm2 = 0_i64;
        for (index, membership) in raster.0.iter().enumerate() {
            let tx = (index / 128) as u16;
            let ty = (index % 128) as u16;
            match membership {
                TileMembership::None => {}
                TileMembership::Single(_) => {
                    let side = 128 * BITMAP_WIDTH;
                    let x = tx as i64 * side;
                    let y = ty as i64 * side;
                    area_cm2 += polar_rectangle_area_cm2(x..x + side, y..y + side);
                }
                TileMembership::Border => {
                    for (cell, &id) in raster.1[&(tx, ty)].iter().enumerate() {
                        if id == NO_ENTITY {
                            continue;
                        }
                        let (x, y) =
                            block_pixel_bounds((tx, ty), ((cell / 128) as u8, (cell % 128) as u8));
                        area_cm2 += polar_rectangle_area_cm2(x, y);
                    }
                }
            }
        }
        let expected = std::f64::consts::PI
            * (POLAR_DISK_RADIUS as f64).powi(2)
            * polar_pixel_area_cm2() as f64;
        assert!(
            (area_cm2 as f64 - expected).abs() < 3_000_000.0,
            "cap area differs by more than 300 m²"
        );
    }
}

#[test]
fn holes_dateline_splits_and_nonpolar_geometries() {
    let west = polygon![(x:170.0,y:87.0),(x:180.0,y:87.0),(x:180.0,y:89.0),(x:170.0,y:89.0),(x:170.0,y:87.0)];
    let east = polygon![(x:-180.0,y:87.0),(x:-170.0,y:87.0),(x:-170.0,y:89.0),(x:-180.0,y:89.0),(x:-180.0,y:87.0)];
    let holed = polygon!(exterior:[(x:-40.0,y:86.0),(x:40.0,y:86.0),(x:40.0,y:89.0),(x:-40.0,y:89.0),(x:-40.0,y:86.0)],interiors:[[(x:-10.0,y:87.0),(x:10.0,y:87.0),(x:10.0,y:88.0),(x:-10.0,y:88.0),(x:-10.0,y:87.0)]]);
    let nonpolar =
        polygon![(x:-5.0,y:10.0),(x:5.0,y:10.0),(x:5.0,y:20.0),(x:-5.0,y:20.0),(x:-5.0,y:10.0)];
    let geoms: BTreeMap<String, MultiPolygon<f64>> = BTreeMap::from([
        ("DATE".into(), MultiPolygon(vec![west, east])),
        ("HOLE".into(), MultiPolygon(vec![holed])),
        ("LOW".into(), MultiPolygon(vec![nonpolar])),
    ]);
    let ids = BTreeMap::from([
        ("DATE".into(), GeoEntityId(1)),
        ("HOLE".into(), GeoEntityId(2)),
        ("LOW".into(), GeoEntityId(3)),
    ]);
    use geo::MapCoords;
    use geo_types::Coord;
    for (plane, sign) in [
        (BitmapPlane::NorthPolar, 1.0),
        (BitmapPlane::SouthPolar, -1.0),
    ] {
        let mirrored = geoms
            .iter()
            .map(|(code, geometry)| {
                (
                    code.clone(),
                    geometry.map_coords(|Coord { x, y }| Coord { x, y: sign * y }),
                )
            })
            .collect();
        let r = rasterize_in_plane(&mirrored, &ids, plane);
        for lng in [-180.0, -179.9, 179.9, 180.0] {
            assert_eq!(owner(&r, plane, lng, sign * 88.0), Some(1));
        }
        assert_eq!(owner(&r, plane, 0.0, sign * 87.5), None);
        assert_eq!(owner(&r, plane, 25.0, sign * 87.5), Some(2));
        assert_eq!(owner(&r, plane, 90.0, sign * 88.0), None);
    }
}

#[test]
fn retained_province_guard_rejects_both_polar_caps() {
    use geo_rasterizer::polar::validate_no_polar_provinces;
    use geo_types::{Coord, Rect};

    let geometries = BTreeMap::from([
        (
            "NORTH".into(),
            MultiPolygon(vec![Rect::new(
                Coord { x: 0.0, y: 85.0 },
                Coord { x: 1.0, y: 86.0 },
            )
            .to_polygon()]),
        ),
        (
            "SOUTH".into(),
            MultiPolygon(vec![Rect::new(
                Coord { x: 0.0, y: -86.0 },
                Coord { x: 1.0, y: -85.0 },
            )
            .to_polygon()]),
        ),
    ]);
    let error = validate_no_polar_provinces(&geometries)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("NORTH") && error.contains("SOUTH"),
        "{error}"
    );
    assert!(
        error.contains("polar province refinement is not supported"),
        "{error}"
    );
}

#[test]
fn retained_province_guard_accepts_mercator_edges_and_empty_input() {
    use bitmap_geometry::MERCATOR_MAX_LATITUDE;
    use geo_rasterizer::polar::validate_no_polar_provinces;
    use geo_types::{Coord, Rect};

    validate_no_polar_provinces(&BTreeMap::new()).unwrap();
    let geometries = BTreeMap::from([(
        "EDGE".into(),
        MultiPolygon(vec![Rect::new(
            Coord {
                x: -10.0,
                y: -MERCATOR_MAX_LATITUDE,
            },
            Coord {
                x: 10.0,
                y: MERCATOR_MAX_LATITUDE,
            },
        )
        .to_polygon()]),
    )]);
    validate_no_polar_provinces(&geometries).unwrap();
}

#[test]
fn rasterizer_refuses_a_retained_polar_province_before_writing_an_asset() {
    let mut admin1: serde_json::Value = serde_json::from_reader(
        std::fs::File::open("tests/fixtures/synthetic_admin1.geojson").unwrap(),
    )
    .unwrap();
    admin1["features"][0]["geometry"]["coordinates"] = serde_json::json!([[
        [0.0, 87.0],
        [1.0, 87.0],
        [1.0, 88.0],
        [0.0, 88.0],
        [0.0, 87.0]
    ]]);
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("polar_admin1.geojson");
    let output = dir.path().join("geo.bin");
    std::fs::write(&source, serde_json::to_vec(&admin1).unwrap()).unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_geo_rasterizer"))
        .args([
            "--worldview",
            "iso",
            "--countries",
            "tests/fixtures/synthetic.geojson",
        ])
        .arg("--admin1")
        .arg(source)
        .args(["--registry", "tests/fixtures/synthetic_registry"])
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{error}");
    assert!(error.contains("AAA-001"), "{error}");
    assert!(
        error.contains("polar province refinement is not supported"),
        "{error}"
    );
    assert!(!output.exists());
}
