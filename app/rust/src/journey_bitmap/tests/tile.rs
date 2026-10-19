use super::*;

#[test]
fn rectangle_fill_matches_point_writes_with_cached_mipmaps() {
    let mut block = Block::new();
    let mut expected = Block::new();
    for (x, y) in [
        (7..10, 8..12),
        (8..12, 9..13),
        (0..64, 1..2),
        (62..64, 62..64),
        (0..1, 0..64),
        (0..64, 0..64),
    ] {
        block.get_at_level(0, 0, 0); // Cache mipmaps before each mutation.
        let mut changed = false;
        for y in y.clone() {
            for x in x.clone() {
                changed |= !expected.is_visited(x as u8, y as u8);
                expected.set_point(x as u8, y as u8, true);
            }
        }
        assert_eq!(block.fill_rect(x.clone(), y.clone()), changed);
        assert_eq!(block, expected);
        assert_mipmap_matches_regenerated(&block);
        assert!(!block.fill_rect(x, y));
        assert_mipmap_matches_regenerated(&block);
    }
}

#[test]
fn block_key_bitset_iterates_only_set_bits_in_order() {
    let expected = [0, 1, 7, 8, 127, 128, 8_209, 16_383];
    let mut bitset = super::BlockKeyBitset::new();
    for &index in &expected {
        bitset.set(BlockKey::from_index(index));
    }

    let actual = bitset.iter().map(|key| key.index()).collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn assert_mipmap_matches_regenerated(block: &Block) {
    let cached = block.mipmap.borrow();
    let cached = cached.as_ref().expect("mipmap should be cached");
    let fresh = block.regenerate_mipmaps();
    assert_eq!(
        **cached, fresh,
        "incremental mipmap does not match regenerated mipmap"
    );
}

fn has_mipmap(block: &Block) -> bool {
    block.mipmap.borrow().is_some()
}

#[test]
fn incremental_mipmap_set_true() {
    let mut block = Block::new();
    // Trigger initial mipmap generation
    block.get_at_level(0, 0, 0);
    assert!(has_mipmap(&block));

    for (x, y) in [(10, 10), (50, 30), (0, 0), (63, 63), (32, 32), (1, 62)] {
        block.set_point(x, y, true);
        assert!(has_mipmap(&block));
        assert_mipmap_matches_regenerated(&block);
    }
}

#[test]
fn incremental_mipmap_set_false() {
    let mut block = Block::new();
    let points: &[(u8, u8)] = &[(10, 10), (10, 11), (20, 20), (0, 0), (63, 63)];
    for &(x, y) in points {
        block.set_point(x, y, true);
    }
    // Generate mipmap
    block.get_at_level(0, 0, 0);
    assert!(has_mipmap(&block));

    for &(x, y) in points {
        block.set_point(x, y, false);
        assert!(!has_mipmap(&block));
    }
}

#[test]
fn incremental_mipmap_merge() {
    let mut block_a = Block::new();
    block_a.set_point(5, 5, true);
    block_a.set_point(10, 10, true);
    block_a.get_at_level(0, 0, 0);
    assert!(has_mipmap(&block_a));

    // Merge with block that has no mipmap
    let mut block_b = Block::new();
    block_b.set_point(30, 30, true);
    block_b.set_point(50, 50, true);
    block_a.merge_with(&block_b);
    assert!(!has_mipmap(&block_a));

    // Merge with block that has a mipmap
    block_a.get_at_level(0, 0, 0);
    assert!(has_mipmap(&block_a));
    let mut block_c = Block::new();
    block_c.set_point(0, 0, true);
    block_c.set_point(63, 63, true);
    block_c.get_at_level(0, 0, 0);
    block_a.merge_with(&block_c);
    assert!(has_mipmap(&block_a));
    assert_mipmap_matches_regenerated(&block_a);
}
