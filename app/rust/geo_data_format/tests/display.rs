use std::collections::BTreeMap;

use bitmap_geometry::BitmapPlane;
use geo_data_format::{
    read_header, write_geo_data_planes, write_geo_data_planes_with_display, DisplayBounds,
    DisplayRecord, GeoData, GeoEntity, GeoEntityId, GeoEntityKind, PackedTile, PlaneRaster,
    TileEntry, TileMembership, Worldview, CELLS_PER_TILE, NO_ENTITY, PROVENANCE_HASH_END,
    TILE_COUNT,
};

// The display span is the final outer header entry; inner spans are relative to it.
const DISPLAY_SECTION: usize = PROVENANCE_HASH_END + 4 * 8;
const INDEX_SECTION: usize = 0;
const BLOCK_SECTION: usize = 16;
const FRAME_SECTION: usize = 32;
const INDEX_ENTRY_LEN: usize = 40;

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
fn shape() -> Vec<Vec<Vec<[i32; 2]>>> {
    vec![
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
    ]
}
fn record(
    worldview: Worldview,
    id: u32,
    west: f64,
    polygons: Vec<Vec<Vec<[i32; 2]>>>,
) -> DisplayRecord {
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
fn empty_display_section_differs_from_raster_only() {
    let (combined_dir, geo) = open(&combined(&[]).unwrap());
    assert!(geo.display_geometry(GeoEntityId(1)).unwrap().is_none());
    let header = read_header(&combined_dir.path().join("geo_data_iso.bin"))
        .unwrap()
        .unwrap();
    assert_ne!(header.display.len, 0);

    let bytes = write_geo_data_planes(&[], "iso", &[], [5; 32]).unwrap();
    let (raster_dir, geo) = open(&bytes);
    assert!(geo.display_geometry(GeoEntityId(1)).unwrap().is_none());
    let header = read_header(&raster_dir.path().join("geo_data_iso.bin"))
        .unwrap()
        .unwrap();
    assert_eq!(header.display.len, 0);
    assert_eq!(header.display.off, bytes.len());
}

#[test]
fn deduplicates_equal_frames_and_sorts_entity_ids() {
    let altered = vec![vec![vec![[0, 0], [10, 0], [10, 10], [0, 0]]]];
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
    assert_eq!(
        u32_at(&bytes, index + 36),
        u32_at(&bytes, index + INDEX_ENTRY_LEN + 36)
    );
    assert_ne!(
        u32_at(&bytes, index + 36),
        u32_at(&bytes, index + INDEX_ENTRY_LEN * 2 + 36)
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
        record(Worldview::Iso, 1, 0.0, shape()),
        record(
            Worldview::Iso,
            2,
            0.0,
            vec![vec![vec![[0, 0], [1, 0], [1, 1], [0, 0]]]],
        ),
    ])
    .unwrap();
    let display = u32_at(&bytes, DISPLAY_SECTION) as usize;
    let blocks = display + u64_at(&bytes, display + BLOCK_SECTION);
    let frames = display + u64_at(&bytes, display + FRAME_SECTION);
    let first_frame = frames + u64_at(&bytes, blocks);
    assert_eq!(bytes[first_frame + 4] & 0b100, 0); // zstd content checksum flag
    bytes[first_frame] ^= 1; // invalidate zstd frame magic without changing its size
    let (_dir, geo) = open(&bytes);
    assert!(geo.display_geometry(GeoEntityId(1)).is_err());
    assert_coverage(&geo);
    assert!(geo.display_geometry(GeoEntityId(2)).unwrap().is_some());
}

#[test]
fn display_index_and_frames_are_bounded_without_affecting_coverage() {
    let bytes = combined(&[record(Worldview::Iso, 1, 0.0, shape())]).unwrap();
    let display = u32_at(&bytes, DISPLAY_SECTION) as usize;
    let index = u64_at(&bytes, display + INDEX_SECTION);
    let blocks = display + u64_at(&bytes, display + BLOCK_SECTION);
    let frames = display + u64_at(&bytes, display + FRAME_SECTION);
    // Exercise malformed refs, overflowing spans, oversized frames, frames
    // past the display boundary, and offsets that only work at the file origin.
    for (at, bad) in [
        (display + index + 36, 42u32.to_le_bytes().to_vec()),
        (display + INDEX_SECTION, u64::MAX.to_le_bytes().to_vec()),
        (
            blocks + 12,
            (16 * 1024 * 1024 + 1u32).to_le_bytes().to_vec(),
        ),
        (blocks, u64::MAX.to_le_bytes().to_vec()),
        (
            blocks + 8,
            ((bytes.len() - frames + 1) as u32).to_le_bytes().to_vec(),
        ),
        (
            display + BLOCK_SECTION,
            (index as u64).to_le_bytes().to_vec(),
        ),
        (
            display + INDEX_SECTION,
            ((display - 1) as u64).to_le_bytes().to_vec(),
        ),
    ] {
        let mut corrupt = bytes.clone();
        corrupt[at..at + bad.len()].copy_from_slice(&bad);
        let (_dir, geo) = open(&corrupt);
        assert!(geo.display_geometry(GeoEntityId(1)).is_err());
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
    file.seek(SeekFrom::Start((index + 36) as u64)).unwrap();
    file.write_all(&42u32.to_le_bytes()).unwrap();
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
    let mut invalid = record(Worldview::Iso, 2, 0.0, shape());
    invalid.polygons[0][0].pop(); // not closed
    assert!(combined(&[invalid]).is_err());
    let mut invalid_bounds = record(Worldview::Iso, 2, 0.0, shape());
    invalid_bounds.bounds.east = f64::INFINITY;
    assert!(combined(&[invalid_bounds]).is_err());
    let mut too_wide = record(Worldview::Iso, 2, -180.0, shape());
    too_wide.bounds.east = 181.0;
    assert!(combined(&[too_wide]).is_err());
}
