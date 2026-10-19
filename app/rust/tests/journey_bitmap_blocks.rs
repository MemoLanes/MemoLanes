use memolanes_core::journey_bitmap::{Block, BlockKey, BITMAP_SIZE};

// The public mipmap representation packs these levels in descending size.
const MIPMAP_LEVELS: [usize; 6] = [32, 16, 8, 4, 2, 1];

#[test]
fn regenerated_mipmaps_preserve_every_single_pixel() {
    for index in 0..4096 {
        let mut data = [0; BITMAP_SIZE];
        data[index / 8] = 1 << (7 - index % 8);
        let block = Block::new_with_data(data);
        let mipmap = block.regenerate_mipmaps();
        // Exactly one occupied ancestor in each level, with no stray bits.
        assert_eq!(mipmap.count_ones(), MIPMAP_LEVELS.len(), "pixel={index}");
        let mut offset = 0;
        for dim in MIPMAP_LEVELS {
            let x = (index % 64) * dim / 64;
            let y = (index / 64) * dim / 64;
            assert!(mipmap[offset + y * dim + x], "pixel={index}, dim={dim}");
            offset += dim * dim;
        }
    }
}

#[test]
fn regenerated_mipmaps_match_source_regions_at_varied_densities() {
    // Include both sides of the 128-pixel cutoff, empty and full blocks,
    // and clustered/dispersed inputs with exactly the requested occupancy.
    for occupied in [0, 1, 127, 128, 129, 2048, 4096] {
        for clustered in [false, true] {
            let mut data = [0; BITMAP_SIZE];
            for i in 0..occupied {
                let index = if clustered { i } else { (i * 7919) % 4096 };
                data[index / 8] |= 1 << (7 - index % 8);
            }
            let block = Block::new_with_data(data);
            assert_eq!(block.count(), occupied as u32);
            let mipmap = block.regenerate_mipmaps();
            let mut offset = 0;
            for dim in MIPMAP_LEVELS {
                let scale = 64 / dim;
                for y in 0..dim {
                    for x in 0..dim {
                        // Read the original footprint independently of all
                        // generated levels, rather than repeating either path.
                        let expected = (y * scale..(y + 1) * scale).any(|sy| {
                            (x * scale..(x + 1) * scale)
                                .any(|sx| block.is_visited(sx as u8, sy as u8))
                        });
                        assert_eq!(
                            mipmap[offset + y * dim + x],
                            expected,
                            "occupied={occupied}, clustered={clustered}, dim={dim}, x={x}, y={y}"
                        );
                    }
                }
                offset += dim * dim;
            }
        }
    }
}

#[test]
fn block_key_conversion() {
    let test = |(x, y), index| {
        assert_eq!(BlockKey::from_x_y(x, y), BlockKey::from_index(index));
        assert_eq!(BlockKey::from_x_y(x, y).index(), index);
    };
    test((0, 0), 0);
    test((127, 127), 16383);
    test((64, 17), 8209);
}

#[test]
fn block_mipmap() {
    // Verify mipmap correctness at every level for point (10, 10).
    // Each level downsamples by 2x from the original 64×64:
    //   z=6 (64×64): direct data read
    //   z=5 (32×32): (10,10) → (5,5)
    //   z=4 (16×16): → (2,2), z=3 (8×8): → (1,1)
    //   z=2 (4×4):   → (0,0), z=1 (2×2): → (0,0), z=0 (1×1): → (0,0)
    let mut block = Block::new();
    block.set_point(10, 10, true);
    assert!(block.get_at_level(10, 10, 6));
    assert!(!block.get_at_level(11, 10, 6));
    assert!(block.get_at_level(5, 5, 5) && !block.get_at_level(4, 5, 5));
    assert!(block.get_at_level(2, 2, 4) && !block.get_at_level(3, 2, 4));
    assert!(block.get_at_level(1, 1, 3) && !block.get_at_level(0, 1, 3));
    assert!(block.get_at_level(0, 0, 2) && !block.get_at_level(1, 0, 2));
    assert!(block.get_at_level(0, 0, 1) && !block.get_at_level(1, 0, 1));
    assert!(block.get_at_level(0, 0, 0));

    // Results reflect updates (set_point, merge_with, difference_with, intersect_with)
    block.set_point(10, 10, false);
    assert!(!block.get_at_level(0, 0, 0));

    let mut other = Block::new();
    other.set_point(20, 20, true);
    block.merge_with(&other);
    assert!(block.get_at_level(0, 0, 0));

    block.difference_with(&other);
    assert!(!block.get_at_level(0, 0, 0));

    block.merge_with(&other);
    block.intersect_with(&Block::new());
    assert!(!block.get_at_level(0, 0, 0));
}
