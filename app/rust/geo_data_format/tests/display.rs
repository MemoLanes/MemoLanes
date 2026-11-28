use std::collections::BTreeMap;

use bitmap_geometry::BitmapPlane;
use geo_data_format::{
    read_header, write_geo_data_planes, write_geo_data_planes_with_display, DisplayBounds,
    DisplayPolygons, DisplayRecord, GeoData, GeoEntity, GeoEntityId, GeoEntityKind, PackedTile,
    PlaneRaster, TileEntry, TileMembership, Worldview, CELLS_PER_TILE, NO_ENTITY,
    PROVENANCE_HASH_END, TILE_COUNT,
};

// The display span is the final outer header entry; inner spans are relative to it.
const DISPLAY_SECTION: usize = PROVENANCE_HASH_END + 4 * 8;
const INDEX_SECTION: usize = 0;
const FRAME_SECTION: usize = 16;
const INDEX_ENTRY_LEN: usize = 52;
const FRAME_OFFSET: usize = 36;
const COMPRESSED_LEN: usize = 44;
const RAW_LEN: usize = 48;

fn u64_at(bytes: &[u8], at: usize) -> usize {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize
}
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn bounds(west: f64) -> DisplayBounds {
    DisplayBounds {
        west,
        south: -10.0,
        east: west + 3.0,
        north: 10.0,
    }
}
fn shape() -> DisplayPolygons {
    DisplayPolygons::Grid(vec![
        vec![
            vec![
                [17990000, -100],
                [18010000, -100],
                [18010000, 100],
                [17990000, -100],
            ],
            vec![
                [18000000, -20],
                [18000000, 20],
                [18000020, 20],
                [18000000, -20],
            ],
        ],
        vec![vec![[-15, -15], [-10, -15], [-10, -10], [-15, -15]]],
    ])
}
fn exact_shape() -> Vec<Vec<Vec<[f64; 2]>>> {
    vec![
        vec![
            vec![
                [179.99999991, -0.0],
                [180.00000002, -1.0],
                [180.00000003, 1.0],
                [179.99999991, -0.0],
            ],
            vec![
                [180.0, -0.00000001],
                [180.000000011, 0.00000002],
                [180.000000012, 0.00000002],
                [180.0, -0.00000001],
            ],
        ],
        vec![vec![[181.0, 0.0], [181.2, 0.0], [181.1, 1.0], [181.0, 0.0]]],
    ]
}
fn record(worldview: Worldview, id: u32, west: f64, polygons: DisplayPolygons) -> DisplayRecord {
    DisplayRecord {
        worldview,
        entity_id: GeoEntityId(id),
        bounds: bounds(west),
        polygons,
    }
}
fn combined(records: &[DisplayRecord]) -> anyhow::Result<Vec<u8>> {
    let entities: Vec<_> = [1, 2, 3]
        .into_iter()
        .map(|id| GeoEntity {
            id: GeoEntityId(id),
            kind: GeoEntityKind::Admin0,
            canonical_code: id.to_string(),
            iso_a3_eh: None,
            name_key: format!("c.{id}"),
            parent_id: None,
            total_area_m2: 1,
        })
        .collect();
    let mut tiles = vec![TileMembership::None; TILE_COUNT];
    tiles[0] = TileMembership::Single(GeoEntityId(1));
    tiles[1] = TileMembership::Border;
    let mut cells = vec![NO_ENTITY; CELLS_PER_TILE];
    cells[5] = 2;
    let blocks = BTreeMap::from([((0, 1), cells)]);
    write_geo_data_planes_with_display(
        &entities,
        "iso",
        &[PlaneRaster {
            plane: BitmapPlane::Mercator,
            tiles: &tiles,
            blocks: &blocks,
        }],
        [5; 32],
        records,
    )
}
fn open(bytes: &[u8]) -> (tempfile::TempDir, GeoData) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("geo_data_iso.bin");
    std::fs::write(&path, bytes).unwrap();
    let geo = GeoData::open(&path).unwrap();
    (dir, geo)
}
fn assert_coverage(geo: &GeoData) {
    assert_eq!(geo.tile_index.get(0), TileEntry::Single(GeoEntityId(1)));
    assert_eq!(geo.tile_index.get(1), TileEntry::Border(0));
    let tile = PackedTile::from_compressed_bytes(&geo.border_blobs.get(0).unwrap()).unwrap();
    assert_eq!(tile.lookup(5), Some(GeoEntityId(2)));
}

#[test]
fn combined_round_trip_includes_coverage_and_display_geometry() {
    let polygons = shape();
    let bytes = combined(&[record(Worldview::Iso, 1, 179.0, polygons.clone())]).unwrap();
    let (dir, geo) = open(&bytes);
    assert_eq!(geo.provenance_hash, [5; 32]);
    assert_eq!(geo.worldview_id, "iso");
    assert_eq!(geo.entities().unwrap()[0].id, GeoEntityId(1));
    assert_coverage(&geo);

    let header = read_header(&dir.path().join("geo_data_iso.bin"))
        .unwrap()
        .unwrap();
    assert_eq!(
        header.display.off,
        header.border_blobs.off + header.border_blobs.len
    );
    assert_ne!(header.display.len, 0);
    assert_eq!(header.display.off + header.display.len, bytes.len());
    let geometry = geo.display_geometry(GeoEntityId(1)).unwrap().unwrap();
    assert_eq!(geometry.bounds, bounds(179.0)); // unwrapped across the dateline
    assert_eq!(geometry.polygons, polygons); // islands, holes, signed deltas and closure
    assert!(geo.display_geometry(GeoEntityId(2)).unwrap().is_none());
    assert!(geo.display_geometry(GeoEntityId(999)).unwrap().is_none());
}

#[test]
fn exact_display_polygons_round_trip_every_coordinate_bit() {
    let exact = exact_shape();
    let bytes = combined(&[
        record(Worldview::Iso, 2, 0.0, shape()),
        record(
            Worldview::Iso,
            1,
            179.0,
            DisplayPolygons::Exact(exact.clone()),
        ),
    ])
    .unwrap();
    let (_dir, geo) = open(&bytes);
    assert_eq!(geo.display_ids().unwrap(), [GeoEntityId(1), GeoEntityId(2)]);
    let mut visited = Vec::new();
    geo.validate_display(|id, geometry| {
        visited.push((id, geometry.bounds, geometry.polygons));
        Ok(())
    })
    .unwrap();
    assert_eq!(
        visited,
        [
            (
                GeoEntityId(1),
                bounds(179.0),
                DisplayPolygons::Exact(exact.clone())
            ),
            (GeoEntityId(2), bounds(0.0), shape())
        ]
    );
    assert_eq!(
        geo.display_geometry(GeoEntityId(2))
            .unwrap()
            .unwrap()
            .polygons,
        shape()
    );
    let geometry = geo.display_geometry(GeoEntityId(1)).unwrap().unwrap();
    assert_eq!(geometry.bounds, bounds(179.0));
    assert!(matches!(&geometry.polygons, DisplayPolygons::Exact(_)));
    let degrees = geometry.polygons.into_degrees();
    assert_eq!(degrees, exact);
    for (actual, expected) in degrees
        .iter()
        .flatten()
        .flatten()
        .zip(exact.iter().flatten().flatten())
    {
        assert_eq!(actual.map(f64::to_bits), expected.map(f64::to_bits));
    }
    let degrees = shape().into_degrees();
    assert_eq!(degrees[0][0][0], [179.9, -0.001]);
    assert_eq!(degrees[0][0][1], [180.1, -0.001]);
}

#[test]
fn grid_display_frame_keeps_legacy_bytes() {
    let polygons = DisplayPolygons::Grid(vec![vec![vec![[-1, 2], [2, 0], [2, 1], [-1, 2]]]]);
    let bytes = combined(&[record(Worldview::Iso, 1, 0.0, polygons)]).unwrap();
    let display = u32_at(&bytes, DISPLAY_SECTION) as usize;
    let frames = display + u64_at(&bytes, display + FRAME_SECTION);
    let legacy_payload = [1, 1, 3, 1, 4, 6, 3, 0, 2];
    assert_eq!(
        bytes[frames..],
        zstd::bulk::compress(&legacy_payload, 3).unwrap()
    );
}

#[test]
fn malformed_outer_display_span_does_not_disable_installed_coverage() {
    let bytes = combined(&[record(Worldview::Iso, 1, 0.0, shape())]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("geo_data_iso.bin");
    let mut invalid_offset = bytes.clone();
    invalid_offset[DISPLAY_SECTION] ^= 1;
    let mut invalid_length = bytes.clone();
    invalid_length[DISPLAY_SECTION + 4] ^= 1;
    let mut overlap = bytes.clone();
    let border_blobs = DISPLAY_SECTION - 8;
    let start = u32_at(&bytes, border_blobs);
    overlap[DISPLAY_SECTION..DISPLAY_SECTION + 4].copy_from_slice(&start.to_le_bytes());
    overlap[DISPLAY_SECTION + 4..DISPLAY_SECTION + 8]
        .copy_from_slice(&(bytes.len() as u32 - start).to_le_bytes());

    for (case, damaged) in [
        ("invalid offset", invalid_offset.as_slice()),
        ("invalid length", invalid_length.as_slice()),
        ("overlapping raster", overlap.as_slice()),
        ("truncated display tail", &bytes[..bytes.len() - 1]),
    ] {
        std::fs::write(&path, damaged).unwrap();
        assert!(read_header(&path).unwrap().is_none(), "{case}");
        assert!(GeoData::open(&path).is_err(), "{case}");

        let geo =
            GeoData::open_for_coverage(&path).unwrap_or_else(|error| panic!("{case}: {error:#}"));
        assert_coverage(&geo);
        assert!(geo.display_geometry(GeoEntityId(1)).is_err(), "{case}");
        assert!(geo.display_ids().is_err(), "{case}");
    }
}

#[test]
fn both_open_modes_reject_truncation_inside_raster() {
    let mut bytes = combined(&[record(Worldview::Iso, 1, 0.0, shape())]).unwrap();
    let raster_end = u32_at(&bytes, DISPLAY_SECTION) as usize;
    bytes.truncate(raster_end - 1);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("geo_data_iso.bin");
    std::fs::write(&path, bytes).unwrap();
    assert!(GeoData::open(&path).is_err());
    assert!(GeoData::open_for_coverage(&path).is_err());
}

#[test]
fn empty_display_section_differs_from_raster_only() {
    let (combined_dir, geo) = open(&combined(&[]).unwrap());
    assert!(geo.display_geometry(GeoEntityId(1)).unwrap().is_none());
    assert!(geo.display_ids().unwrap().is_empty());
    let header = read_header(&combined_dir.path().join("geo_data_iso.bin"))
        .unwrap()
        .unwrap();
    assert_ne!(header.display.len, 0);

    let bytes = write_geo_data_planes(&[], "iso", &[], [5; 32]).unwrap();
    let (raster_dir, geo) = open(&bytes);
    assert!(geo.display_geometry(GeoEntityId(1)).unwrap().is_none());
    assert!(geo.display_ids().unwrap().is_empty());
    let header = read_header(&raster_dir.path().join("geo_data_iso.bin"))
        .unwrap()
        .unwrap();
    assert_eq!(header.display.len, 0);
    assert_eq!(header.display.off, bytes.len());
}

#[test]
fn each_entity_has_an_independent_frame_in_id_order() {
    let altered = DisplayPolygons::Grid(vec![vec![vec![[0, 0], [10, 0], [10, 10], [0, 0]]]]);
    let records = vec![
        record(Worldview::Iso, 3, 30.0, altered.clone()),
        record(Worldview::Iso, 2, 20.0, shape()),
        record(Worldview::Iso, 1, 179.0, shape()),
    ];
    let bytes = combined(&records).unwrap();
    let display = u32_at(&bytes, DISPLAY_SECTION) as usize;
    let index = display + u64_at(&bytes, display + INDEX_SECTION);
    assert_eq!(u32_at(&bytes, index), 1);
    assert_eq!(u32_at(&bytes, index + INDEX_ENTRY_LEN), 2);
    assert_eq!(u32_at(&bytes, index + INDEX_ENTRY_LEN * 2), 3);
    let frames = display + u64_at(&bytes, display + FRAME_SECTION);
    let first_len = u32_at(&bytes, index + COMPRESSED_LEN) as usize;
    assert!(first_len > 0);
    assert_eq!(u64_at(&bytes, index + FRAME_OFFSET), 0);
    assert_eq!(
        u64_at(&bytes, index + INDEX_ENTRY_LEN + FRAME_OFFSET),
        first_len
    );
    assert_eq!(
        u64_at(&bytes, index + 2 * INDEX_ENTRY_LEN + FRAME_OFFSET),
        2 * first_len
    );
    assert_eq!(
        &bytes[frames..frames + first_len],
        &bytes[frames + first_len..frames + 2 * first_len]
    );
    let (_dir, geo) = open(&bytes);
    assert_eq!(
        geo.display_geometry(GeoEntityId(1))
            .unwrap()
            .unwrap()
            .bounds,
        bounds(179.0)
    );
    assert_eq!(
        geo.display_geometry(GeoEntityId(2))
            .unwrap()
            .unwrap()
            .bounds,
        bounds(20.0)
    );
    assert_eq!(
        geo.display_geometry(GeoEntityId(3))
            .unwrap()
            .unwrap()
            .polygons,
        altered
    );
    assert_eq!(
        bytes,
        combined(&records.into_iter().rev().collect::<Vec<_>>()).unwrap()
    );
}

#[test]
fn malformed_display_frame_does_not_break_coverage_or_other_frame() {
    let mut bytes = combined(&[
        record(
            Worldview::Iso,
            1,
            179.0,
            DisplayPolygons::Exact(exact_shape()),
        ),
        record(Worldview::Iso, 2, 0.0, shape()),
    ])
    .unwrap();
    let display = u32_at(&bytes, DISPLAY_SECTION) as usize;
    let frames = display + u64_at(&bytes, display + FRAME_SECTION);
    bytes[frames] ^= 1; // invalidate zstd frame magic without changing its size
    let (_dir, geo) = open(&bytes);
    assert_eq!(geo.display_ids().unwrap(), [GeoEntityId(1), GeoEntityId(2)]);
    assert!(geo.display_geometry(GeoEntityId(2)).unwrap().is_some());
    assert!(geo.display_geometry(GeoEntityId(1)).is_err());
    assert!(geo.validate_display(|_, _| Ok(())).is_err());
    assert_coverage(&geo);
}

#[test]
fn rejects_malformed_exact_display_payloads() {
    let bytes = combined(&[record(
        Worldview::Iso,
        1,
        179.0,
        DisplayPolygons::Exact(exact_shape()),
    )])
    .unwrap();
    let display = u32_at(&bytes, DISPLAY_SECTION) as usize;
    let index = display + u64_at(&bytes, display + INDEX_SECTION);
    let frames = display + u64_at(&bytes, display + FRAME_SECTION);
    let mut excessive_vertices = vec![0, 1, 1, 0xc1, 0x84, 0x3d]; // 1,000,001 vertices
    excessive_vertices.resize(excessive_vertices.len() + 2 * 1_000_001, 0);
    for (raw, error) in [
        (&[0][..], "truncated varint"),
        (&[0, 0], "invalid polygon count"),
        (&[0, 1, 0], "invalid ring count"),
        (&[0, 1, 1, 2, 0, 0, 0, 0], "invalid vertex count"),
        (&[0, 1, 1, 3, 0, 0, 0, 0, 0, 128], "truncated varint"),
        (&[0, 1, 1, 3, 0, 0, 0, 0, 0, 0, 1], "trailing frame data"),
        // First x delta zigzags the f64 bits of positive infinity.
        (
            &[
                0, 1, 1, 3, 128, 128, 128, 128, 128, 128, 128, 240, 255, 1, 0, 0, 0, 0, 0,
            ],
            "non-finite coordinate",
        ),
        (
            &[
                0, 1, 1, 3, 128, 128, 128, 128, 128, 128, 128, 128, 128, 2, 0, 0, 0, 0, 0,
            ],
            "varint overflow",
        ),
        (excessive_vertices.as_slice(), "too many vertices"),
    ] {
        // Replace the sole frame and keep the file and table lengths valid.
        let mut corrupt = bytes[..frames].to_vec();
        let payload = zstd::bulk::compress(raw, 3).unwrap();
        corrupt.extend_from_slice(&payload);
        let display_len = (corrupt.len() - display) as u32;
        corrupt[DISPLAY_SECTION + 4..DISPLAY_SECTION + 8]
            .copy_from_slice(&display_len.to_le_bytes());
        corrupt[display + FRAME_SECTION + 8..display + FRAME_SECTION + 16]
            .copy_from_slice(&(payload.len() as u64).to_le_bytes());
        corrupt[index + COMPRESSED_LEN..index + COMPRESSED_LEN + 4]
            .copy_from_slice(&(payload.len() as u32).to_le_bytes());
        corrupt[index + RAW_LEN..index + RAW_LEN + 4]
            .copy_from_slice(&(raw.len() as u32).to_le_bytes());
        let (_dir, geo) = open(&corrupt);
        let err = geo.display_geometry(GeoEntityId(1)).unwrap_err();
        assert!(
            format!("{err:#}").contains(error),
            "expected {error}: {err:#}"
        );
        assert!(geo.validate_display(|_, _| Ok(())).is_err());
        assert_coverage(&geo);
    }
}

#[test]
fn display_index_and_frames_are_bounded_without_affecting_coverage() {
    let bytes = combined(&[record(Worldview::Iso, 1, 0.0, shape())]).unwrap();
    let display = u32_at(&bytes, DISPLAY_SECTION) as usize;
    let index = display + u64_at(&bytes, display + INDEX_SECTION);
    let frames = display + u64_at(&bytes, display + FRAME_SECTION);
    let frame_len = (bytes.len() - frames) as u32;
    for (at, bad) in [
        (index + 4, f64::NAN.to_le_bytes().to_vec()),
        (index + FRAME_OFFSET, u64::MAX.to_le_bytes().to_vec()),
        (index + RAW_LEN, 0u32.to_le_bytes().to_vec()),
        (
            index + RAW_LEN,
            (16 * 1024 * 1024 + 1u32).to_le_bytes().to_vec(),
        ),
        (
            index + COMPRESSED_LEN,
            (frame_len + 1).to_le_bytes().to_vec(),
        ),
        (
            index + COMPRESSED_LEN,
            (frame_len - 1).to_le_bytes().to_vec(),
        ),
        (display + INDEX_SECTION, u64::MAX.to_le_bytes().to_vec()),
        (display + INDEX_SECTION + 8, u64::MAX.to_le_bytes().to_vec()),
        (
            display + FRAME_SECTION,
            (index as u64).to_le_bytes().to_vec(),
        ),
        (display + FRAME_SECTION + 8, u64::MAX.to_le_bytes().to_vec()),
    ] {
        let mut corrupt = bytes.clone();
        corrupt[at..at + bad.len()].copy_from_slice(&bad);
        let (_dir, geo) = open(&corrupt);
        assert!(geo.display_geometry(GeoEntityId(1)).is_err());
        assert!(geo.display_ids().is_err());
        assert_coverage(&geo);
    }
}

#[test]
fn display_index_is_not_read_until_the_first_lookup() {
    use std::io::{Seek, SeekFrom, Write};

    let bytes = combined(&[record(Worldview::Iso, 1, 0.0, shape())]).unwrap();
    let (dir, geo) = open(&bytes);
    let display = u32_at(&bytes, DISPLAY_SECTION) as usize;
    let index = display + u64_at(&bytes, display + INDEX_SECTION);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(dir.path().join("geo_data_iso.bin"))
        .unwrap();
    file.seek(SeekFrom::Start((index + FRAME_OFFSET) as u64))
        .unwrap();
    file.write_all(&42u64.to_le_bytes()).unwrap();
    assert!(geo.display_geometry(GeoEntityId(1)).is_err());
    assert_coverage(&geo);
}

#[test]
fn display_lookup_uses_the_original_file_after_path_replacement() {
    let polygons = shape();
    let bytes = combined(&[record(Worldview::Iso, 1, 0.0, polygons.clone())]).unwrap();
    let (dir, geo) = open(&bytes);
    let path = dir.path().join("geo_data_iso.bin");
    let replacement = dir.path().join("replacement.bin");
    std::fs::write(&replacement, combined(&[]).unwrap()).unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    assert!(GeoData::open(&path)
        .unwrap()
        .display_geometry(GeoEntityId(1))
        .unwrap()
        .is_none());
    assert_eq!(
        geo.display_geometry(GeoEntityId(1))
            .unwrap()
            .unwrap()
            .polygons,
        polygons
    );
    assert_coverage(&geo);
}

#[test]
fn rejects_record_from_another_worldview() {
    let err = combined(&[record(Worldview::Chn, 1, 0.0, shape())]).unwrap_err();
    assert!(err.to_string().contains("worldview"), "{err}");
}

#[test]
fn rejects_malformed_geometry_and_duplicate_keys() {
    let valid = record(Worldview::Iso, 1, 0.0, shape());
    assert!(combined(&[valid.clone(), valid]).is_err());
    let invalid = DisplayPolygons::Grid(vec![vec![vec![[0, 0], [1, 0], [1, 1], [0, 1]]]]);
    assert!(combined(&[record(Worldview::Iso, 2, 0.0, invalid)]).is_err());
    let mut invalid_bounds = record(Worldview::Iso, 2, 0.0, shape());
    invalid_bounds.bounds.east = f64::INFINITY;
    assert!(combined(&[invalid_bounds]).is_err());
    let mut too_wide = record(Worldview::Iso, 2, -180.0, shape());
    too_wide.bounds.east = 181.0;
    assert!(combined(&[too_wide]).is_err());
}

#[test]
fn exact_display_writer_rejects_invalid_geometry() {
    let valid = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]];
    let alter = |index: usize, point| {
        let mut ring = valid;
        ring[index] = point;
        vec![vec![ring.to_vec()]]
    };
    for (polygons, error) in [
        (vec![], "empty geometry"),
        (vec![vec![]], "empty polygon"),
        (
            vec![vec![valid[..3].to_vec()]],
            "ring not closed or too short",
        ),
        (alter(3, [0.0, 1.0]), "ring not closed or too short"),
        (alter(3, [-0.0, 0.0]), "ring not closed or too short"),
        (alter(1, [f64::NAN, 0.0]), "non-finite coordinate"),
        (alter(3, [f64::INFINITY, 0.0]), "non-finite coordinate"),
        (vec![vec![vec![[0.0, 0.0]; 1_000_002]]], "too many vertices"),
    ] {
        let err = combined(&[record(
            Worldview::Iso,
            1,
            0.0,
            DisplayPolygons::Exact(polygons),
        )])
        .unwrap_err();
        assert!(
            format!("{err:#}").contains(error),
            "expected {error}: {err:#}"
        );
    }
}
