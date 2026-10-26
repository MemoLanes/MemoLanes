//! Small, synthetic flight shapes complement the behavioral tests.
//! Hash baselines share image_hashes.lock and gpx_hashes.lock with other tests.
//! Failed GPX/PNG files are saved to the ignored for_inspection directory.
//! Review them before copying the reported hashes into the corresponding baseline.
//! To export passing cases too: GENERATE_INSPECTION_FILES=1 cargo test --test flight_track_snapshots
//! Images show direct great circles on the left and preprocessing on the right.

pub mod test_utils;

use image::{Rgb, RgbImage};
use journey_kernel::tile_range::decode_tile_range_response;
use memolanes_core::{
    export_data, flight_track_processor,
    gps::{Point, RawGPSPoint},
    journey_bitmap::JourneyBitmap,
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    renderer::MapRenderer,
    utils::lng_lat_to_tile_x_y,
};
use std::io::Cursor;

struct Case {
    name: &'static str,
    // Deliberately small, synthetic inputs: each case has one visible purpose.
    observations: &'static [(f64, f64)],
    center: (f64, f64),
    zoom: i16,
}

const CASES: &[Case] = &[
    Case {
        name: "right_angle",
        observations: &[(0.0, 0.0), (0.8, 0.0), (0.8, 0.8)],
        center: (0.4, 0.4),
        zoom: 9,
    },
    Case {
        // Four times the spacing should retain a similar smoothing shape.
        name: "wide_right_angle",
        observations: &[(0.0, 0.0), (3.2, 0.0), (3.2, 3.2)],
        center: (1.6, 1.6),
        zoom: 7,
    },
    Case {
        // A longer second leg makes distance weighting visible.
        name: "unequal_spacing",
        observations: &[(0.0, 0.0), (0.8, 0.0), (0.8, 2.4)],
        center: (0.4, 1.2),
        zoom: 8,
    },
    Case {
        name: "dateline_turn",
        observations: &[(179.4, 0.0), (-179.8, 0.0), (-179.8, 0.8)],
        center: (179.8, 0.4),
        zoom: 9,
    },
    Case {
        name: "high_latitude_turn",
        observations: &[(178.0, 70.0), (179.6, 70.0), (-179.8, 70.8)],
        center: (179.3, 70.4),
        zoom: 7,
    },
    Case {
        // Two observations should bow poleward, with no extra turn smoothing.
        name: "arctic_arc",
        observations: &[(-60.0, 70.0), (60.0, 70.0)],
        center: (0.0, 78.0),
        zoom: 2,
    },
];

#[test]
fn flight_shapes_match_reviewed_gpx_and_images() {
    let mut failures = Vec::new();
    for case in CASES {
        let raw: Vec<_> = case
            .observations
            .iter()
            .map(|&(longitude, latitude)| RawGPSPoint {
                point: Point {
                    longitude,
                    latitude,
                },
                timestamp_ms: None,
                accuracy: None,
                altitude: None,
                speed: None,
            })
            .collect();
        let processed = flight_track_processor::process(&[raw]).unwrap();
        let mut gpx = Vec::new();
        export_data::gpx::journeys_to_gpx_file(
            &[test_utils::vector_export(processed.clone())],
            &mut gpx,
        )
        .unwrap();

        let direct = JourneyVector {
            track_segments: vec![TrackSegment {
                track_points: case
                    .observations
                    .iter()
                    .map(|&(longitude, latitude)| TrackPoint {
                        longitude,
                        latitude,
                    })
                    .collect(),
            }],
        };
        // Left: direct great-circle connections. Right: flight preprocessing.
        let mut image = RgbImage::from_pixel(2 * PANEL_SIZE + 8, PANEL_SIZE, Rgb([220; 3]));
        image::imageops::replace(&mut image, &render(case, &direct), 0, 0);
        image::imageops::replace(
            &mut image,
            &render(case, &processed),
            i64::from(PANEL_SIZE + 8),
            0,
        );
        // Match the existing image tests: hash the encoded PNG bytes.
        let mut png = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let key = format!("flight_track_{}", case.name);
        for result in [
            test_utils::check_gpx(&key, &gpx),
            test_utils::check_image(&key, &png),
        ] {
            if let Err(error) = result {
                failures.push(error);
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

const TILE_SIZE_POWER: i16 = 7;
const TILE_SIZE: u32 = 1 << TILE_SIZE_POWER;
const PANEL_SIZE: u32 = 3 * TILE_SIZE;

fn render(case: &Case, vector: &JourneyVector) -> RgbImage {
    let mut bitmap = JourneyBitmap::new();
    bitmap.edit().merge_vector(vector);
    let mut renderer = MapRenderer::new(bitmap);
    let (cx, cy) = lng_lat_to_tile_x_y(case.center.0, case.center.1, case.zoom.into());
    let tiles_per_world = 1 << case.zoom;
    let left = cx - 1;
    let top = (cy - 1).clamp(0, tiles_per_world - 3);
    let response = renderer
        .get_tile_range_response(left.into(), top.into(), case.zoom, 3, 3, TILE_SIZE_POWER)
        .unwrap();
    let mut image = RgbImage::from_pixel(PANEL_SIZE, PANEL_SIZE, Rgb([250; 3]));
    for (tile_x, tile_y, pixels) in decode_tile_range_response(&response).unwrap() {
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                if pixels.get(x as usize, y as usize) {
                    image.put_pixel(
                        (tile_x - left) as u32 * TILE_SIZE + x,
                        (tile_y - top) as u32 * TILE_SIZE + y,
                        Rgb([25, 80, 170]),
                    );
                }
            }
        }
    }
    // Orange disks mark original observations, never generated samples or split endpoints.
    for &(longitude, latitude) in case.observations {
        let (x, y) =
            lng_lat_to_tile_x_y(longitude, latitude, i32::from(case.zoom + TILE_SIZE_POWER));
        let x = (x - left * TILE_SIZE as i32).rem_euclid(tiles_per_world * TILE_SIZE as i32);
        let y = y - top * TILE_SIZE as i32;
        for dy in -2..=2 {
            for dx in -2..=2 {
                if dx * dx + dy * dy <= 4
                    && (0..PANEL_SIZE as i32).contains(&(x + dx))
                    && (0..PANEL_SIZE as i32).contains(&(y + dy))
                {
                    image.put_pixel((x + dx) as u32, (y + dy) as u32, Rgb([235, 110, 30]));
                }
            }
        }
    }
    image
}
