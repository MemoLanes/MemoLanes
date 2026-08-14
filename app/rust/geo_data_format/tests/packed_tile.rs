use geo_data_format::{GeoEntityId, PackedTile, CELLS_PER_TILE};

fn cells_from(per_quarter: [Option<GeoEntityId>; 4]) -> Vec<Option<GeoEntityId>> {
    // 16_384 cells split into four contiguous quarters, one value each.
    let mut out = Vec::with_capacity(CELLS_PER_TILE);
    let chunk = CELLS_PER_TILE / 4;
    for v in per_quarter {
        for _ in 0..chunk {
            out.push(v);
        }
    }
    out
}

#[test]
fn round_trip_two_unique() {
    let cells = cells_from([None, Some(GeoEntityId(1)), None, Some(GeoEntityId(1))]);
    let pt = PackedTile::from_dense(&cells);
    assert_eq!(pt.bits_per_cell(), 1);
    for (i, expected) in cells.iter().enumerate() {
        assert_eq!(pt.lookup(i), *expected, "cell {i}");
    }
}

#[test]
fn round_trip_three_unique_uses_two_bits() {
    let cells = cells_from([None, Some(GeoEntityId(1)), Some(GeoEntityId(2)), None]);
    let pt = PackedTile::from_dense(&cells);
    assert_eq!(pt.bits_per_cell(), 2);
    for (i, expected) in cells.iter().enumerate() {
        assert_eq!(pt.lookup(i), *expected, "cell {i}");
    }
}

#[test]
fn round_trip_five_unique_uses_four_bits() {
    let cells = cells_from([
        Some(GeoEntityId(1)),
        Some(GeoEntityId(2)),
        Some(GeoEntityId(3)),
        Some(GeoEntityId(4)),
    ]);
    // Insert one cell of a 5th value at index 0.
    let mut cells = cells;
    cells[0] = Some(GeoEntityId(5));
    let pt = PackedTile::from_dense(&cells);
    assert_eq!(pt.bits_per_cell(), 4);
    for (i, expected) in cells.iter().enumerate() {
        assert_eq!(pt.lookup(i), *expected, "cell {i}");
    }
}

#[test]
fn to_dense_round_trips() {
    let cells = cells_from([
        None,
        Some(GeoEntityId(7)),
        Some(GeoEntityId(7)),
        Some(GeoEntityId(13)),
    ]);
    let pt = PackedTile::from_dense(&cells);
    assert_eq!(pt.to_dense(), cells);
}

#[test]
fn zstd_round_trip_preserves_lookups() {
    let cells = cells_from([
        None,
        Some(GeoEntityId(7)),
        Some(GeoEntityId(7)),
        Some(GeoEntityId(13)),
    ]);
    let pt = PackedTile::from_dense(&cells);
    let bytes = pt.to_compressed_bytes();
    // Should be much smaller than the dense form (16_384 × 8 B = 131_072).
    assert!(bytes.len() < 2_000, "compressed too large: {}", bytes.len());
    let pt2 = PackedTile::from_compressed_bytes(&bytes);
    for (i, expected) in cells.iter().enumerate() {
        assert_eq!(pt2.lookup(i), *expected, "cell {i}");
    }
}
