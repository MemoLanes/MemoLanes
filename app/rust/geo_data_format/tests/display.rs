use geo_data_format::{
    write_display_asset, DisplayAsset, DisplayBounds, DisplayRecord, GeoEntityId, Worldview,
};

// Layout offsets are part of the on-disk format contract, not computed by the codec.
const INDEX_SECTION: usize = 24;
const BLOCK_SECTION: usize = 40;
const FRAME_SECTION: usize = 56;
const INDEX_ENTRY_LEN: usize = 41;

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
fn open(bytes: &[u8]) -> (tempfile::TempDir, anyhow::Result<DisplayAsset>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("display.bin");
    std::fs::write(&path, bytes).unwrap();
    (dir, DisplayAsset::open(&path))
}

#[test]
fn round_trips_islands_holes_and_signed_deltas() {
    let polygons = shape();
    let original_bounds = bounds(179.0); // east is unwrapped across the dateline
    let bytes = write_display_asset(
        &[(Worldview::Iso, [5; 32])],
        &[record(Worldview::Iso, 17, 179.0, polygons.clone())],
    )
    .unwrap();
    let (_dir, asset) = open(&bytes);
    let asset = asset.unwrap();
    assert_eq!(asset.raster_provenance(Worldview::Iso), Some([5; 32]));
    assert_eq!(asset.raster_provenance(Worldview::Chn), None);
    let geometry = asset.get(Worldview::Iso, GeoEntityId(17)).unwrap().unwrap();
    assert_eq!(geometry.bounds, original_bounds);
    assert_eq!(geometry.polygons.len(), 2);
    assert_eq!(geometry.polygons[0].len(), 2); // exterior and hole
    assert_eq!(geometry.polygons[1].len(), 1); // island exterior
    assert_eq!(geometry.polygons, polygons); // exact integer units, deltas and closure
}

#[test]
fn deduplicates_equal_frames_across_worldviews() {
    let altered = vec![vec![vec![[0, 0], [10, 0], [10, 10], [0, 0]]]];
    let records = vec![
        record(Worldview::Usa, 17, 30.0, altered.clone()),
        record(Worldview::Chn, 17, 20.0, shape()),
        record(Worldview::Iso, 17, 179.0, shape()),
    ];
    let hashes = [
        (Worldview::Usa, [3; 32]),
        (Worldview::Iso, [1; 32]),
        (Worldview::Chn, [2; 32]),
    ];
    let bytes = write_display_asset(&hashes, &records).unwrap();
    let index = u64_at(&bytes, INDEX_SECTION);
    assert_eq!(
        u32_at(&bytes, index + 37),
        u32_at(&bytes, index + INDEX_ENTRY_LEN + 37)
    );
    assert_ne!(
        u32_at(&bytes, index + 37),
        u32_at(&bytes, index + 2 * INDEX_ENTRY_LEN + 37)
    );
    let (_dir, asset) = open(&bytes);
    let asset = asset.unwrap();
    assert_eq!(
        asset
            .get(Worldview::Iso, GeoEntityId(17))
            .unwrap()
            .unwrap()
            .bounds,
        bounds(179.0)
    );
    assert_eq!(
        asset
            .get(Worldview::Chn, GeoEntityId(17))
            .unwrap()
            .unwrap()
            .bounds,
        bounds(20.0)
    );
    assert_eq!(
        asset
            .get(Worldview::Usa, GeoEntityId(17))
            .unwrap()
            .unwrap()
            .polygons,
        altered
    );
    assert!(asset
        .get(Worldview::Iso, GeoEntityId(999))
        .unwrap()
        .is_none());
}

#[test]
fn corrupt_frame_does_not_break_other_entity() {
    let mut bytes = write_display_asset(
        &[(Worldview::Iso, [1; 32])],
        &[
            record(Worldview::Iso, 1, 0.0, shape()),
            record(
                Worldview::Iso,
                2,
                0.0,
                vec![vec![vec![[0, 0], [1, 0], [1, 1], [0, 0]]]],
            ),
        ],
    )
    .unwrap();
    let blocks = u64_at(&bytes, BLOCK_SECTION);
    let frames = u64_at(&bytes, FRAME_SECTION);
    let first_frame = frames + u64_at(&bytes, blocks);
    let first_len = u32_at(&bytes, blocks + 8) as usize;
    bytes[first_frame..first_frame + first_len].fill(0xff); // same-size corruption
    let (_dir, asset) = open(&bytes);
    let asset = asset.unwrap();
    assert!(asset.get(Worldview::Iso, GeoEntityId(1)).is_err());
    assert!(asset.get(Worldview::Iso, GeoEntityId(2)).unwrap().is_some());
}

#[test]
fn rejects_out_of_bounds_index() {
    let bytes = write_display_asset(
        &[(Worldview::Iso, [1; 32])],
        &[record(Worldview::Iso, 1, 0.0, shape())],
    )
    .unwrap();
    let index = u64_at(&bytes, INDEX_SECTION);
    let mut bad_ref = bytes.clone();
    bad_ref[index + 37..index + 41].copy_from_slice(&42u32.to_le_bytes());
    assert!(open(&bad_ref).1.is_err());
    let mut overflow = bytes.clone();
    overflow[INDEX_SECTION..INDEX_SECTION + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(open(&overflow).1.is_err());
    let blocks = u64_at(&bytes, BLOCK_SECTION);
    let mut oversized = bytes.clone();
    oversized[blocks + 12..blocks + 16].copy_from_slice(&(16 * 1024 * 1024 + 1u32).to_le_bytes());
    assert!(open(&oversized).1.is_err());
    let mut bad_frame = bytes.clone();
    bad_frame[blocks..blocks + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(open(&bad_frame).1.is_err());
    let mut overlap = bytes;
    overlap[BLOCK_SECTION..BLOCK_SECTION + 8].copy_from_slice(&(index as u64).to_le_bytes());
    assert!(open(&overlap).1.is_err());
}

#[test]
fn reordered_inputs_are_byte_identical() {
    let records = vec![
        record(Worldview::Chn, 5, 5.0, shape()),
        record(Worldview::Iso, 2, 2.0, shape()),
    ];
    let hashes = vec![(Worldview::Chn, [2; 32]), (Worldview::Iso, [1; 32])];
    let mut reversed_records = records.clone();
    reversed_records.reverse();
    let mut reversed_hashes = hashes.clone();
    reversed_hashes.reverse();
    assert_eq!(
        write_display_asset(&hashes, &records).unwrap(),
        write_display_asset(&reversed_hashes, &reversed_records).unwrap()
    );
}

#[test]
fn rejects_malformed_geometry_and_duplicate_keys() {
    let manifest = [(Worldview::Iso, [1; 32])];
    let valid = record(Worldview::Iso, 1, 0.0, shape());
    assert!(write_display_asset(&manifest, &[valid.clone(), valid]).is_err());
    assert!(write_display_asset(&manifest, &[record(Worldview::Chn, 1, 0.0, shape())]).is_err());
    assert!(write_display_asset(&[manifest[0], manifest[0]], &[]).is_err());
    let mut invalid = record(Worldview::Iso, 2, 0.0, shape());
    invalid.polygons[0][0].pop(); // not closed
    assert!(write_display_asset(&manifest, &[invalid]).is_err());
    let mut invalid_bounds = record(Worldview::Iso, 2, 0.0, shape());
    invalid_bounds.bounds.east = f64::INFINITY;
    assert!(write_display_asset(&manifest, &[invalid_bounds]).is_err());
    let mut too_wide = record(Worldview::Iso, 2, -180.0, shape());
    too_wide.bounds.east = 181.0;
    assert!(write_display_asset(&manifest, &[too_wide]).is_err());
}
