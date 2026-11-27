use std::{fs, path::Path, process::Command};

use geo::{
    coordinate_position::CoordPos, dimensions::Dimensions, wkt, Area, BooleanOps, Relate,
    Validation,
};
use geo_data_format::{GeoData, GeoEntityId, GeoEntityKind, Worldview};
use geo_rasterizer::display::{build_display_records, DisplaySource};
use geo_types::{LineString, MultiPolygon, Polygon};

#[test]
fn shared_edge_fallback_applies_to_both_rings() {
    // The triangle collapses at 0.0001°. Both rectangles are large enough
    // to keep that grid if validated alone. Their junction splits the
    // triangle's unsegmented edge; the upper rectangle has an extra vertex.
    let sources = [
        wkt!(POLYGON((0.00004 0.0,0.00004 0.002,0.00003 0.001,0.00004 0.0))),
        wkt!(POLYGON((0.00004 0.001,0.1 0.001,0.1 0.0,0.00004 0.0,0.00004 0.001))),
        wkt!(POLYGON((0.00004 0.002,0.00004 0.0015,0.00004 0.001,0.1 0.001,0.1 0.002,0.00004 0.002))),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, p)| DisplaySource {
        entity_id: GeoEntityId(id as u32),
        polygons: MultiPolygon(vec![p]),
    })
    .collect::<Vec<_>>();
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    assert_eq!(records.len(), 3);
    let edge = |i: usize| {
        let ring = &records[i].polygons[0][0];
        let polygon = polygon_from_grid(&records[i].polygons[0]);
        assert!(polygon.is_valid() && polygon.unsigned_area() > 0.0);
        let mut points = ring[..ring.len() - 1]
            .iter()
            .filter(|p| p[0] == 4)
            .copied()
            .collect::<Vec<_>>();
        points.sort_unstable();
        points
    };
    let lo = edge(1);
    let hi = edge(2);
    assert_eq!(lo, [[4, 0], [4, 100]]);
    assert_eq!(hi.first(), Some(&[4, 100]));
    assert_eq!(hi.last(), Some(&[4, 200]));
    assert_eq!(edge(0), [lo, hi[1..].to_vec()].concat());
}

#[test]
fn retries_both_ids_sharing_a_collapsing_triangle() {
    let triangle = wkt!(POLYGON((0.00004 0.0,0.00004 0.002,0.00003 0.001,0.00004 0.0)));
    let sources = [GeoEntityId(7), GeoEntityId(12)].map(|entity_id| DisplaySource {
        entity_id,
        polygons: MultiPolygon(vec![triangle.clone()]),
    });
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    assert_eq!(
        records.iter().map(|r| r.entity_id).collect::<Vec<_>>(),
        [GeoEntityId(7), GeoEntityId(12)]
    );
    assert_eq!(records[0].polygons, records[1].polygons);
    assert_eq!(
        polygon_from_grid(&records[0].polygons[0]).unsigned_area(),
        100.0
    );
}

#[test]
fn retries_overlap_bounded_by_two_vertex_crossings() {
    // The source diagonal is y = x + 0.00008, above even the apex of
    // the second ring. On the coarse grid it becomes y = x: the two
    // endpoint intersections bound a sliver of new positive area.
    let sources = [
        wkt!(POLYGON((-0.00004 0.00004,0.19996 0.20004,-0.00004 0.20004,-0.00004 0.00004))),
        wkt!(POLYGON((0.01 0.01,0.020011 0.020089,0.03 0.03,0.03 0.008,0.01 0.01))),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, polygon)| DisplaySource {
        entity_id: GeoEntityId(id as u32),
        polygons: MultiPolygon(vec![polygon]),
    })
    .collect::<Vec<_>>();
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    assert_eq!(records.len(), 2);
    let a = polygon_from_grid(&records[0].polygons[0]);
    let b = polygon_from_grid(&records[1].polygons[0]);
    assert!(a.is_valid() && b.is_valid());
    assert_eq!(
        a.intersection(&b).unsigned_area(),
        0.0,
        "rounding invented an overlap"
    );
}

#[test]
fn retries_new_containment_without_boundary_crossings() {
    // The mainland's shallow inlet opens south. Simplifying the two sides
    // across its mouth swallows the island without intersecting its ring.
    let mainland =
        wkt!(POLYGON((0.0 10.0,0.0 0.0,9.9 0.0,10.0 0.005,10.1 0.0,20.0 0.0,20.0 10.0,0.0 10.0)));
    let island = wkt!(POLYGON((9.996 0.001,10.004 0.001,10.004 0.003,9.996 0.003,9.996 0.001)));
    assert_eq!(mainland.intersection(&island).unsigned_area(), 0.0);
    // Neighbors sharing the two side edges pin the inlet arc at the two
    // bottom corners, independently of how the ring's start is chosen.
    let left = wkt!(POLYGON((-1.0 0.0,0.0 0.0,0.0 10.0,-1.0 10.0,-1.0 0.0)));
    let right = wkt!(POLYGON((20.0 0.0,21.0 0.0,21.0 10.0,20.0 10.0,20.0 0.0)));
    let sources = [mainland, island, left, right]
        .into_iter()
        .enumerate()
        .map(|(id, polygon)| DisplaySource {
            entity_id: GeoEntityId(id as u32),
            polygons: MultiPolygon(vec![polygon]),
        })
        .collect::<Vec<_>>();
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    assert_eq!(records.len(), 4, "the original shore can be recovered");
    let mainland = polygon_from_grid(&records[0].polygons[0]);

    let island = polygon_from_grid(&records[1].polygons[0]);
    assert_eq!(
        mainland.intersection(&island).unsigned_area(),
        0.0,
        "the simplified mainland swallowed the island"
    );
}

#[test]
fn retries_new_overlap_when_a_hole_snaps_inside_an_island() {
    // This extent keeps the mainland's hole on the coarse grid while the
    // tiny island needs the fine grid. The snapped boundaries don't cross.
    let mainland = wkt!(POLYGON((
        -0.02 -0.02,0.02 -0.02,0.02 0.02,-0.02 0.02,-0.02 -0.02
    ),(
        0.00006 0.00006,0.00006 0.00024,0.00024 0.00024,0.00024 0.00006,0.00006 0.00006
    )));
    let island = wkt!(POLYGON((
        0.00007 0.00007,0.00023 0.00007,0.00023 0.00023,0.00007 0.00023,0.00007 0.00007
    )));
    assert_eq!(
        mainland
            .relate(&island)
            .get(CoordPos::Inside, CoordPos::Inside),
        Dimensions::Empty
    );

    // Pass the island first to check the exported stable IDs, not input order.
    let records = build_display_records(
        Worldview::Iso,
        &[
            DisplaySource {
                entity_id: GeoEntityId(42),
                polygons: MultiPolygon(vec![island]),
            },
            DisplaySource {
                entity_id: GeoEntityId(7),
                polygons: MultiPolygon(vec![mainland]),
            },
        ],
    )
    .unwrap();
    assert_eq!(
        records.iter().map(|r| r.entity_id).collect::<Vec<_>>(),
        [GeoEntityId(7), GeoEntityId(42)]
    );
    let [mainland, island] = records.as_slice() else {
        unreachable!()
    };
    assert_eq!(mainland.polygons.len(), 1);
    assert_eq!(island.polygons.len(), 1);
    assert_eq!(mainland.polygons[0].len(), 2);
    assert_eq!(island.polygons[0].len(), 1);
    let hole = &mainland.polygons[0][1];
    let mainland = polygon_from_grid(&mainland.polygons[0]);
    let island = polygon_from_grid(&island.polygons[0]);
    assert!(mainland.is_valid() && island.is_valid());
    assert_eq!(island.unsigned_area(), 256.0);
    assert_eq!(
        mainland.intersection(&island).unsigned_area(),
        0.0,
        "snapping invented an overlap; emitted hole grid vertices: {hole:?}"
    );
    let mut hole_vertices = hole[..hole.len() - 1].to_vec();
    hole_vertices.sort_unstable();
    assert_eq!(hole_vertices, [[6, 6], [6, 24], [24, 6], [24, 24]]);
}

#[test]
fn preserves_existing_interior_overlap_through_a_hole() {
    // Keep this partial-overlap fixture large enough to retain its corners
    // under the camera-floor budget, with off-grid vertices. Old subpixel
    // vertices are no longer invariant under the camera zoom cap.
    let mainland = wkt!(POLYGON((
        -0.22 -0.22,0.22 -0.22,0.22 0.22,-0.22 0.22,-0.22 -0.22
    ),(
        0.00066 0.00066,0.00066 0.00242,0.00242 0.00242,0.00242 0.00066,0.00066 0.00066
    )));
    let island = wkt!(POLYGON((
        0.00077 0.00077,0.00253 0.00077,0.00253 0.00253,0.00077 0.00253,0.00077 0.00077
    )));
    // The source area already overlaps, even though the island starts in the hole.
    assert_eq!(
        mainland
            .relate(&island)
            .get(CoordPos::Inside, CoordPos::Inside),
        Dimensions::TwoDimensional
    );
    let sources = [mainland, island]
        .into_iter()
        .enumerate()
        .map(|(id, polygon)| DisplaySource {
            entity_id: GeoEntityId(id as u32),
            polygons: MultiPolygon(vec![polygon]),
        })
        .collect::<Vec<_>>();
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    assert_eq!(records.len(), 2);
    assert!(records[0].polygons[0][1].contains(&[240, 240]));
    let mainland = polygon_from_grid(&records[0].polygons[0]);
    let island = polygon_from_grid(&records[1].polygons[0]);
    assert!(mainland.is_valid() && island.is_valid());
    assert_eq!(mainland.intersection(&island).unsigned_area(), 3300.0);
}

#[test]
fn preserves_boundary_only_contact_without_refining() {
    // The diagonal is a shared edge, but the two component bounds overlap
    // in both axes. Boundary-only contact does not justify fine snapping.
    let lower = wkt!(POLYGON((0.01003 0.0,0.1 0.0,0.1 0.1,0.01003 0.0)));
    let upper = wkt!(POLYGON((0.01003 0.0,0.1 0.1,0.01003 0.1,0.01003 0.0)));
    assert!(lower.relate(&upper).is_touches());
    let sources = [lower, upper]
        .into_iter()
        .enumerate()
        .map(|(id, polygon)| DisplaySource {
            entity_id: GeoEntityId(id as u32),
            polygons: MultiPolygon(vec![polygon]),
        })
        .collect::<Vec<_>>();
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    assert_eq!(records.len(), 2);
    assert!(records
        .iter()
        .all(|r| r.polygons[0][0].contains(&[1000, 0])));
    let lower = polygon_from_grid(&records[0].polygons[0]);
    let upper = polygon_from_grid(&records[1].polygons[0]);
    assert!(lower.relate(&upper).is_touches());
}

#[test]
fn preserves_tangent_outline_sharing_a_crossing_arc() {
    // The overlap between A and B persists even after retrying on the fine
    // grid. C shares B's edge from (100, 100) to (300, 80) in grid units,
    // but its continuation at (100, 100) only touches A tangentially.
    let sources = [
        wkt!(POLYGON((-0.000004 0.000004,0.019996 0.020004,-0.000004 0.020004,-0.000004 0.000004))),
        wkt!(POLYGON((0.001 0.001,0.0020011 0.0020089,0.003 0.003,0.003 0.0008,0.001 0.001))),
        wkt!(POLYGON((0.001 0.001,0.003 0.0008,0.0018 0.0005,0.001 0.001))),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, polygon)| DisplaySource {
        entity_id: GeoEntityId(id as u32),
        polygons: MultiPolygon(vec![polygon]),
    })
    .collect::<Vec<_>>();
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    assert_eq!(
        records.iter().map(|r| r.entity_id).collect::<Vec<_>>(),
        [GeoEntityId(0), GeoEntityId(2)]
    );
    let a = polygon_from_grid(&records[0].polygons[0]);
    let c = polygon_from_grid(&records[1].polygons[0]);
    assert!(c.is_valid());
    assert_eq!(a.intersection(&c).unsigned_area(), 0.0);
    assert!(records[1].polygons[0][0].contains(&[100, 100]));
}

#[test]
fn resolves_each_shared_crossing_path_as_one_side() {
    // C shares B's boundaries at both crossings. B and D use the identical
    // ring there, but D's detached island makes it wider than A. Compare B
    // and D as one side: removing A resolves both crossings without losing B.
    let a =
        wkt!(POLYGON((-0.000004 0.000004,0.019996 0.020004,-0.000004 0.020004,-0.000004 0.000004)));
    let b = wkt!(POLYGON((0.001 0.001,0.0020011 0.0020089,0.003 0.003,0.003 0.0008,0.001 0.001)));
    let c = wkt!(POLYGON((0.001 0.001,0.003 0.0008,0.003 0.003,0.0028 0.001,0.001 0.001)));
    let island = wkt!(POLYGON((0.04 0.0,0.041 0.0,0.04 0.001,0.04 0.0)));
    let sources = [vec![a], vec![b.clone()], vec![c], vec![b, island]]
        .into_iter()
        .enumerate()
        .map(|(id, polygons)| DisplaySource {
            entity_id: GeoEntityId(id as u32),
            polygons: MultiPolygon(polygons),
        })
        .collect::<Vec<_>>();
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    assert_eq!(
        records.iter().map(|r| r.entity_id).collect::<Vec<_>>(),
        [GeoEntityId(1), GeoEntityId(2), GeoEntityId(3)]
    );
    assert_eq!(records[0].polygons[0], records[2].polygons[0]);
    assert_eq!(records[2].polygons.len(), 2);
}

#[test]
fn a_tangent_point_does_not_force_fine_grid() {
    // Coarse snapping brings the apex to the diagonal, but both rays
    // stay on the same side. This changes no area between the shapes.
    let sources = [
        wkt!(POLYGON((-0.00004 0.00004,0.19996 0.20004,-0.00004 0.20004,-0.00004 0.00004))),
        wkt!(POLYGON((0.01 0.008,0.020011 0.020009,0.03 0.008,0.01 0.008))),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, polygon)| DisplaySource {
        entity_id: GeoEntityId(id as u32),
        polygons: MultiPolygon(vec![polygon]),
    })
    .collect::<Vec<_>>();
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    assert_eq!(records.len(), 2);
    assert!(records
        .iter()
        .flat_map(|r| r.polygons.iter().flatten().flatten())
        .flatten()
        .all(|n| n % 10 == 0));
    assert!(records[1].polygons[0][0].contains(&[2000, 2000]));
}

fn polygon_from_grid(rings: &[Vec<[i32; 2]>]) -> Polygon<f64> {
    let mut lines = rings.iter().map(|ring| {
        LineString::from(
            ring.iter()
                .map(|p| (f64::from(p[0]), f64::from(p[1])))
                .collect::<Vec<_>>(),
        )
    });
    Polygon::new(lines.next().unwrap(), lines.collect())
}

#[test]
fn preserves_holes_islands_and_crossing_bounds() {
    let records = build_display_records(
        Worldview::Iso,
        &[DisplaySource {
            entity_id: GeoEntityId(42),
            polygons: wkt!(MULTIPOLYGON(
                ((179.0 0.0,-179.0 0.0,-179.0 4.0,179.0 4.0,179.0 0.0),
                 (179.2 1.0,179.2 2.0,179.7 2.0,179.7 1.0,179.2 1.0)),
                ((179.2 5.0,179.5 5.0,179.5 6.0,179.2 6.0,179.2 5.0))
            )),
        }],
    )
    .unwrap();
    let r = &records[0];
    assert_eq!((r.bounds.west, r.bounds.east), (179.0, 181.0));
    assert_eq!((r.bounds.south, r.bounds.north), (0.0, 6.0));
    let polygons = r
        .polygons
        .iter()
        .map(|p| polygon_from_grid(p))
        .collect::<Vec<_>>();
    assert_eq!(
        polygons.iter().map(|p| p.interiors().len()).sum::<usize>(),
        1
    );
    assert!(polygons
        .iter()
        .any(|p| p.exterior().0.iter().all(|v| v.y >= 500_000.0)));
    assert!(polygons.iter().all(|p| p.is_valid()));
    assert!(r
        .polygons
        .iter()
        .flatten()
        .flat_map(|r| r.windows(2))
        .all(|e| (e[0][0] - e[1][0]).abs() <= 200_000));
}

#[test]
fn polar_and_broad_bounds_are_not_short_wrapped_intervals() {
    let records = build_display_records(
        Worldview::Iso,
        &[
            DisplaySource {
                entity_id: GeoEntityId(1),
                polygons: wkt!(MULTIPOLYGON(((-180.0 -90.0,180.0 -90.0,180.0 -75.0,0.0 -70.0,-180.0 -75.0,-180.0 -90.0)))),
            },
            DisplaySource {
                entity_id: GeoEntityId(2),
                polygons: wkt!(MULTIPOLYGON(((-150.0 0.0,0.0 0.0,150.0 0.0,150.0 40.0,0.0 40.0,-150.0 40.0,-150.0 0.0)))),
            },
        ],
    )
    .unwrap();
    assert_eq!(records.len(), 2);
    let a = records[0].bounds;
    assert_eq!((a.west, a.east, a.south), (-180.0, 180.0, -90.0));
    let b = records[1].bounds;
    assert_eq!((b.west, b.east), (-150.0, 150.0));
}

// Two synthetic worldviews exercise batching without changing the existing
// single-worldview fixture or its golden binary. China-specific production
// policy expects source features that the synthetic input does not model.
const FIXTURE_WORLDVIEWS: [Worldview; 2] = [Worldview::Iso, Worldview::Usa];
fn write_test_fixture_inputs(dir: &Path) {
    let fixtures = Path::new("tests/fixtures");
    for (destination, inputs) in [
        ("geo_entity_registry", "synthetic_registry"),
        ("admin2", "synthetic_admin2"),
    ] {
        fs::create_dir(dir.join(destination)).unwrap();
        for entry in fs::read_dir(fixtures.join(inputs)).unwrap() {
            let entry = entry.unwrap();
            fs::copy(entry.path(), dir.join(destination).join(entry.file_name())).unwrap();
        }
    }
    let natural_earth = dir.join("natural_earth");
    fs::create_dir(&natural_earth).unwrap();
    for worldview in FIXTURE_WORLDVIEWS {
        fs::copy(
            fixtures.join("synthetic.geojson"),
            natural_earth.join(worldview.spec().source_filename),
        )
        .unwrap();
    }
    fs::copy(
        fixtures.join("synthetic_admin1.geojson"),
        natural_earth.join(geo_data_format::ADMIN1_SOURCE_FILENAME),
    )
    .unwrap();
}

fn run_test_fixture_batch(dir: &Path) -> anyhow::Result<()> {
    use geo_rasterizer::batch::{rasterize_batch, RasterizeMode, RasterizePaths};
    rasterize_batch(
        FIXTURE_WORLDVIEWS.map(|worldview| {
            (
                worldview,
                RasterizePaths {
                    countries: dir
                        .join("natural_earth")
                        .join(worldview.spec().source_filename),
                    admin1: dir
                        .join("natural_earth")
                        .join(geo_data_format::ADMIN1_SOURCE_FILENAME),
                    registry: dir.join("geo_entity_registry"),
                    admin2_dir: dir.join("admin2"),
                    output: raster(dir, worldview),
                },
            )
        }),
        RasterizeMode::Build {
            ensure_source: false,
        },
    )
}

fn raster(dir: &Path, w: Worldview) -> std::path::PathBuf {
    dir.join(format!("geo_data_{}.bin", w.spec().id))
}

#[test]
fn each_worldview_contains_its_raster_entities_and_display() {
    let dir = tempfile::tempdir().unwrap();
    run_fixture(dir.path());
    for worldview in FIXTURE_WORLDVIEWS {
        let path = raster(dir.path(), worldview);
        let asset = GeoData::open(&path).unwrap();
        assert_eq!(asset.worldview_id, worldview.spec().id);
        let entities = asset.entities().unwrap();
        assert!(entities.len() > 1);
        for entity in entities {
            assert_eq!(
                asset.display_geometry(entity.id).unwrap().is_some(),
                entity.kind != GeoEntityKind::Continent,
                "{} {:?}",
                worldview.spec().id,
                entity.id
            );
        }
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 5);
}

fn run_fixture(dir: &Path) {
    write_test_fixture_inputs(dir);
    run_test_fixture_batch(dir).unwrap();
}

#[test]
fn cache_reuses_complete_assets_and_rebuilds_damaged_or_raster_only_files() {
    let dir = tempfile::tempdir().unwrap();
    run_fixture(dir.path());
    let [iso, usa] = FIXTURE_WORLDVIEWS.map(|w| raster(dir.path(), w));
    let expected_iso = fs::read(&iso).unwrap();
    let expected_usa = fs::read(&usa).unwrap();
    let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_234_567);
    fs::OpenOptions::new()
        .write(true)
        .open(&usa)
        .unwrap()
        .set_modified(old)
        .unwrap();
    fs::write(&iso, &expected_iso[..expected_iso.len() - 1]).unwrap();
    run_test_fixture_batch(dir.path()).unwrap();
    assert_eq!(fs::read(&iso).unwrap(), expected_iso);
    assert_eq!(fs::metadata(&usa).unwrap().modified().unwrap(), old);

    let mut damaged = expected_iso.clone();
    *damaged.last_mut().unwrap() ^= 1;
    fs::write(&iso, damaged).unwrap();
    run_test_fixture_batch(dir.path()).unwrap();
    assert_eq!(fs::read(&iso).unwrap(), expected_iso);

    let geo = GeoData::open(&iso).unwrap();
    let raster_only = geo_data_format::write_geo_data_planes(
        &geo.entities().unwrap(),
        "iso",
        &[],
        geo.provenance_hash,
    )
    .unwrap();
    fs::write(&iso, raster_only).unwrap();
    run_test_fixture_batch(dir.path()).unwrap();
    assert_eq!(fs::read(&iso).unwrap(), expected_iso);
    assert_eq!(fs::read(&usa).unwrap(), expected_usa);
}

#[test]
fn joins_touching_source_components_without_losing_the_detached_island() {
    let source = DisplaySource {
        entity_id: GeoEntityId(42),
        polygons: wkt!(MULTIPOLYGON(
            ((0.0 0.0,1.0 0.0,1.0 1.0,0.0 1.0,0.0 0.0)),
            ((1.0 0.0,2.0 0.0,2.0 1.0,1.0 1.0,1.0 0.0)),
            ((3.0 0.0,4.0 0.0,4.0 1.0,3.0 1.0,3.0 0.0))
        )),
    };
    let records = build_display_records(Worldview::Iso, &[source]).unwrap();
    assert_eq!(records.len(), 1);
    let polygons = MultiPolygon(
        records[0]
            .polygons
            .iter()
            .map(|p| polygon_from_grid(p))
            .collect(),
    );
    assert!(polygons.is_valid());
    assert_eq!(polygons.0.len(), 2);
    assert_eq!(polygons.unsigned_area(), 30_000_000_000.0);
}

#[test]
fn does_not_snap_distinct_lines_together() {
    let sources = [
        wkt!(POLYGON((0.0 0.0,0.00102 0.0,0.00102 0.1,0.0 0.1,0.0 0.0))),
        wkt!(POLYGON((0.00104 0.0,0.1 0.0,0.1 0.1,0.00104 0.1,0.00104 0.0))),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, polygon)| DisplaySource {
        entity_id: GeoEntityId(i as u32),
        polygons: MultiPolygon(vec![polygon]),
    })
    .collect::<Vec<_>>();
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[0].polygons[0][0].iter().map(|p| p[0]).max(),
        Some(102)
    );
    assert_eq!(
        records[1].polygons[0][0].iter().map(|p| p[0]).min(),
        Some(104)
    );
}

#[test]
fn shipped_display_covers_key_entities_in_all_worldviews() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/assets/geo");
    for &w in Worldview::ALL {
        let raster = GeoData::open(&raster(&dir, w)).unwrap();
        let entities = raster.entities().unwrap();
        let china = entities
            .iter()
            .find(|e| e.canonical_code == "CHN" && e.kind == GeoEntityKind::Admin0)
            .unwrap();
        let guangdong = entities
            .iter()
            .find(|e| e.canonical_code == "CHN-1180")
            .unwrap();
        let guangzhou = entities
            .iter()
            .find(|e| e.canonical_code == "CN-4401")
            .unwrap();
        let macao = entities
            .iter()
            .find(|e| e.name_key == "country.MAC")
            .unwrap();
        let antarctica = entities.iter().find(|e| e.canonical_code == "ATA").unwrap();
        let fiji = entities.iter().find(|e| e.canonical_code == "FJI").unwrap();
        let vatican = entities
            .iter()
            .find(|e| e.canonical_code == "VAT" && e.kind == GeoEntityKind::Admin0)
            .unwrap();
        assert_eq!(guangdong.parent_id, Some(china.id));
        assert_eq!(guangzhou.parent_id, Some(guangdong.id));
        assert_eq!(macao.parent_id == Some(china.id), w == Worldview::Chn);
        for e in [
            china, guangdong, guangzhou, macao, antarctica, fiji, vatican,
        ] {
            let shape = raster.display_geometry(e.id).unwrap().unwrap_or_else(|| {
                panic!("{} missing {} ({})", w.spec().id, e.canonical_code, e.id.0)
            });
            let geometry = MultiPolygon(
                shape
                    .polygons
                    .iter()
                    .map(|p| polygon_from_grid(p))
                    .collect(),
            );
            assert!(geometry.is_valid() && geometry.unsigned_area() > 0.0);
            if e == antarctica {
                assert_eq!(
                    (shape.bounds.west, shape.bounds.east, shape.bounds.south),
                    (-180.0, 180.0, -90.0)
                );
            } else if e == fiji {
                assert!(shape.bounds.east > 180.0 && shape.bounds.east - shape.bounds.west < 10.0);
            }
        }
    }
}

#[test]
fn cli_rejects_test_batch_fixture_flag() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("fixture");
    fs::create_dir(&root).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_geo_rasterizer"))
        .arg("--batch-fixture")
        .arg(&root)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("unexpected argument '--batch-fixture'"),
        "{stderr}"
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
}

#[test]
fn subpixel_eligible_shape_survives_display_fit_budget() {
    let records = build_display_records(
        Worldview::Iso,
        &[DisplaySource {
            entity_id: GeoEntityId(42),
            polygons: wkt!(MULTIPOLYGON(((12.000003 40.000003,12.000023 40.000003,12.000023 40.000023,12.000003 40.000023,12.000003 40.000003)))),
        }],
    ).unwrap();
    assert_eq!(records.len(), 1);
    let polygon = polygon_from_grid(&records[0].polygons[0]);
    assert!(polygon.is_valid() && polygon.unsigned_area() > 0.0);
}
