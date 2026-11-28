use geo_data_format::{GeoEntityId, Worldview};
use geo_rasterizer::display::{build_display_records, DisplaySource};
use geo_types::{wkt, MultiPolygon, Rect};

fn rectangle(id: u32, west: f64, south: f64, east: f64, north: f64) -> DisplaySource {
    DisplaySource {
        entity_id: GeoEntityId(id),
        polygons: MultiPolygon(vec![Rect::new((west, south), (east, north)).to_polygon()]),
    }
}

#[test]
fn quantization_keeps_a_channel_open_across_the_dateline() {
    let sources = [
        rectangle(1, 179.0, 0.0, 179.99999, 1.0),
        rectangle(2, -179.99999, 0.0, -179.0, 1.0),
    ];
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    let outlines: Vec<_> = records
        .into_iter()
        .map(|r| r.polygons.into_degrees())
        .collect();
    let east = outlines[0][0][0]
        .iter()
        .map(|p| p[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let west = outlines[1][0][0]
        .iter()
        .map(|p| p[0])
        .fold(f64::INFINITY, f64::min);
    assert_eq!((east, west), (179.99999, -179.99999));
}

#[test]
fn simplifying_a_bowed_shore_keeps_the_dateline_channel_between_its_endpoints() {
    let sources = [
        DisplaySource {
            entity_id: GeoEntityId(1),
            polygons: MultiPolygon(vec![wkt!(POLYGON((
                179.0 0.0, 180.0 0.0, 179.9999 0.5, 180.0 1.0, 179.0 1.0, 179.0 0.0
            )))]),
        },
        rectangle(2, -180.0, 0.0, -179.0, 1.0),
    ];
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    let shore = records[0].polygons.clone().into_degrees();
    let ring = &shore[0][0];
    assert!(ring.contains(&[180.0, 0.0]) && ring.contains(&[180.0, 1.0]));
    assert!(ring.contains(&[179.9999, 0.5]));
    assert!(!ring
        .windows(2)
        .any(|edge| edge.iter().all(|p| p[0] == 180.0)));
}

#[test]
fn opposite_dateline_contacts_keep_their_order_and_shared_endpoints() {
    let sources = [
        rectangle(1, 179.0, 0.0, 180.0, 1.000014),
        rectangle(2, -180.0, 1.000016, -179.0, 2.0),
        rectangle(3, -180.0, 0.25, -179.0, 0.75),
    ];
    let records = build_display_records(Worldview::Iso, &sources).unwrap();
    let seam_latitudes = |id: usize| {
        let polygons = records[id].polygons.clone().into_degrees();
        let mut latitudes: Vec<_> = polygons[0][0]
            .iter()
            .filter(|p| p[0].abs() == 180.0)
            .map(|p| p[1])
            .collect();
        latitudes.sort_unstable_by(f64::total_cmp);
        latitudes.dedup();
        latitudes
    };
    let southern = seam_latitudes(0);
    let northern = seam_latitudes(1);
    assert!(southern.last().unwrap() < northern.first().unwrap());
    assert_eq!(seam_latitudes(2), [0.25, 0.75]);
    assert!(southern.windows(2).any(|edge| edge == [0.25, 0.75]));
}

#[test]
fn differently_segmented_crossings_export_the_same_shared_path() {
    let north = wkt!(POLYGON((150.0 -25.0, -174.0 -15.0, -174.0 0.0, 150.0 0.0, 150.0 -25.0)));
    let south = wkt!(POLYGON((
        -174.0 - 15.0,
        168.0 - 20.0,
        150.0 - 25.0,
        150.0 - 35.0,
        -174.0 - 35.0,
        -174.0 - 15.0
    )));
    let sources = [north, south]
        .into_iter()
        .enumerate()
        .map(|(i, polygon)| DisplaySource {
            entity_id: GeoEntityId(i as u32 + 1),
            polygons: MultiPolygon(vec![polygon]),
        })
        .collect::<Vec<_>>();
    let mut paths = Vec::new();
    for record in build_display_records(Worldview::Iso, &sources).unwrap() {
        let mut path = Vec::new();
        for polygon in record.polygons.into_degrees() {
            for edge in polygon[0].windows(2) {
                if edge[0][0] != edge[1][0] && edge.iter().all(|p| (-25.0..=-15.0).contains(&p[1]))
                {
                    let [a, b] = [edge[0], edge[1]];
                    path.push(if a[0] < b[0] { [a, b] } else { [b, a] });
                }
            }
        }
        path.sort_unstable_by(|a, b| a[0][0].total_cmp(&b[0][0]));
        assert!(path.iter().any(|edge| edge[0][0] == -180.0));
        assert!(path.iter().any(|edge| edge[1][0] == 180.0));
        paths.push(path);
    }
    assert_eq!(paths[0], paths[1]);
}

#[test]
fn ordinary_neighbor_shares_only_the_collinear_part_of_a_crossing_edge() {
    let north = wkt!(POLYGON((150.0 -25.0, -174.0 -15.0, -174.0 0.0, 150.0 0.0, 150.0 -25.0)));
    let neighbor = wkt!(POLYGON((
        159.0 - 22.5,
        168.0 - 20.0,
        162.0 - 25.0,
        159.0 - 22.5
    )));
    let sources = [north, neighbor]
        .into_iter()
        .enumerate()
        .map(|(i, polygon)| DisplaySource {
            entity_id: GeoEntityId(i as u32 + 1),
            polygons: MultiPolygon(vec![polygon]),
        })
        .collect::<Vec<_>>();
    let expected = [[159., -22.5], [168., -20.]];
    for record in build_display_records(Worldview::Iso, &sources).unwrap() {
        let polygons = record.polygons.into_degrees();
        assert!(
            polygons
                .iter()
                .flat_map(|p| p[0].windows(2))
                .any(|e| e == expected || e == [expected[1], expected[0]]),
            "shared interval missing from ID {}",
            record.entity_id.0
        );
    }
}
