//! Frozen against the pre-polar rasterizer, including integer endpoint
//! clamping at the Mercator domain edges before the DDA edge walk.
use geo_data_format::{GeoEntityId, TileMembership};
use geo_rasterizer::rasterize::rasterize;
use geo_types::{polygon, MultiPolygon};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[test]
fn mercator_domain_edges_keep_their_legacy_cells() {
    let geoms = BTreeMap::from([
        (
            "SOUTH".into(),
            MultiPolygon(vec![
                polygon![(x:-170.0,y:-84.0),(x:-160.0,y:-80.0),(x:-150.0,y:-90.0),(x:-170.0,y:-84.0)],
            ]),
        ),
        (
            "DATE".into(),
            MultiPolygon(vec![
                polygon![(x:179.0,y:70.0),(x:180.0,y:70.0),(x:180.0,y:84.0),(x:179.0,y:70.0)],
            ]),
        ),
    ]);
    let ids = BTreeMap::from([
        ("SOUTH".into(), GeoEntityId(1)),
        ("DATE".into(), GeoEntityId(2)),
    ]);
    let (tiles, blocks) = rasterize(&geoms, &ids);
    let mut hash = Sha256::new();
    for tile in tiles {
        let value = match tile {
            TileMembership::None => u32::MAX,
            TileMembership::Border => u32::MAX - 1,
            TileMembership::Single(id) => id.0,
        };
        hash.update(value.to_le_bytes());
    }
    for ((x, y), cells) in blocks {
        hash.update(x.to_le_bytes());
        hash.update(y.to_le_bytes());
        for cell in cells {
            hash.update(cell.to_le_bytes());
        }
    }
    assert_eq!(
        hash.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "804d9706cf33282672f1a593d63b0d66f5a0d9a4f63cab67b779b84424c4f6c3"
    );
}
