use flate2::read::ZlibDecoder;
use memolanes_core::export_data;
use memolanes_core::import_data;
use memolanes_core::journey_bitmap::{Block, BlockKey, JourneyBitmap, Tile, TileKey};
use memolanes_core::renderer::MapRenderer;
use memolanes_core::utils;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Cursor, Read};

pub mod test_utils;

const SOURCE_SNAPSHOT_PATH: &str = "./tests/data/Snapshot-20260601T232045+0800.fwss";
const ROUNDTRIP_SNAPSHOT_PATH: &str =
    "./tests/for_inspection/generated_fow_snapshot_roundtrip.fwss";
const RENDER_SNAPSHOT_PATH: &str =
    "./tests/for_inspection/generated_fow_snapshot_render_roundtrip.fwss";
const TILE_INDEX_PATH: &str = "Model/#/3389dae361";

fn load_source_bitmap() -> JourneyBitmap {
    let (bitmap, warnings) =
        import_data::fow::load_fow_snapshot_data(SOURCE_SNAPSHOT_PATH).unwrap();
    assert_eq!(format!("{warnings:?}"), "None");
    bitmap
}

fn export_source_snapshot() -> Vec<u8> {
    let bitmap = load_source_bitmap();
    let mut fwss = Cursor::new(Vec::new());
    export_data::fow::journey_bitmap_to_fwss_file(&bitmap, &mut fwss).unwrap();
    fwss.into_inner()
}

#[test]
fn fow_export_omits_polar_tiles_from_payloads_index_layers_and_area() {
    let mut block = Block::new();
    block.set_point(10, 20, true);
    let mut tile = Tile::new();
    tile.set(&BlockKey::from_x_y(0, 0), block);
    let mut mercator = JourneyBitmap::new();
    mercator
        .edit()
        .insert_tile(&TileKey::Mercator { x: 64, y: 64 }, tile.clone());
    let mut mixed = mercator.clone();
    // Both polar keys deliberately reuse the Mercator tile's coordinates.
    for key in [
        TileKey::NorthPolar { x: 64, y: 64 },
        TileKey::SouthPolar { x: 64, y: 64 },
    ] {
        mixed.edit().insert_tile(&key, tile.clone());
    }
    let mut expected = Cursor::new(Vec::new());
    let mut actual = Cursor::new(Vec::new());
    export_data::fow::journey_bitmap_to_fwss_file(&mercator, &mut expected).unwrap();
    export_data::fow::journey_bitmap_to_fwss_file(&mixed, &mut actual).unwrap();

    // Compare the entire archive, including derived layers and area metadata.
    assert_eq!(actual.into_inner(), expected.into_inner());
}

#[test]
fn fow_export_with_no_mercator_coverage_has_no_data_to_export() {
    for latitude in [90.0, -90.0] {
        let mut bitmap = JourneyBitmap::new();
        bitmap.edit().add_line(0.0, latitude, 0.0, latitude);
        let mut output = Cursor::new(Vec::new());
        let error =
            export_data::fow::journey_bitmap_to_fwss_file(&bitmap, &mut output).unwrap_err();
        assert_eq!(error.to_string(), "No Fog of World data to export");
        assert!(output.get_ref().is_empty());
    }
}

fn zip_entry_names_from_path(path: &str) -> BTreeSet<String> {
    let mut zip = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
    (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned())
        .collect()
}

fn zip_entry_names_from_bytes(fwss: &[u8]) -> BTreeSet<String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(fwss)).unwrap();
    (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned())
        .collect()
}

fn inflate_zip_entry(fwss: &[u8], path: &str) -> Vec<u8> {
    let mut zip = zip::ZipArchive::new(Cursor::new(fwss)).unwrap();
    let mut compressed = Vec::new();
    zip.by_name(path)
        .unwrap_or_else(|_| panic!("missing zip entry {path}"))
        .read_to_end(&mut compressed)
        .unwrap();

    let mut decoder = ZlibDecoder::new(compressed.as_slice());
    let mut inflated = Vec::new();
    decoder.read_to_end(&mut inflated).unwrap();
    inflated
}

fn render_first_tile(bitmap: JourneyBitmap) -> Vec<u8> {
    let tile_key = bitmap
        .all_tile_keys()
        .min_by_key(|tile_key| match **tile_key {
            TileKey::Mercator { x, y } => (y, x),
            TileKey::NorthPolar { .. } | TileKey::SouthPolar { .. } => {
                panic!("snapshot fixture requires Mercator tiles");
            }
        })
        .copied()
        .unwrap_or_else(|| panic!("snapshot fixture should contain at least one tile"));
    render_tile(bitmap, tile_key)
}

fn render_tile(bitmap: JourneyBitmap, tile_key: TileKey) -> Vec<u8> {
    let TileKey::Mercator { x, y } = tile_key else {
        panic!("snapshot rendering requires a Mercator tile, got {tile_key:?}");
    };
    let (left, top) = utils::tile_x_y_to_lng_lat(x as i32, y as i32, 9);
    let (right, bottom) = utils::tile_x_y_to_lng_lat(x as i32 + 1, y as i32 + 1, 9);
    let mut renderer = MapRenderer::new(bitmap);
    test_utils::render_map_overlay(&mut renderer, 9, left, top, right, bottom).data
}

#[test]
fn generated_fwss_uses_stored_zip_entries() {
    let fwss = export_source_snapshot();
    let mut zip = zip::ZipArchive::new(Cursor::new(fwss)).unwrap();

    for i in 0..zip.len() {
        let file = zip.by_index(i).unwrap();
        assert_eq!(file.compression(), zip::CompressionMethod::Stored);
    }
}

#[test]
fn generated_fwss_preserves_official_snapshot_model_entries() {
    let generated_fwss = export_source_snapshot();
    assert_eq!(
        zip_entry_names_from_bytes(&generated_fwss),
        zip_entry_names_from_path(SOURCE_SNAPSHOT_PATH)
    );
}

#[test]
fn generated_fwss_tile_index_matches_exported_base_tiles() {
    let bitmap = load_source_bitmap();
    let fwss = export_source_snapshot();
    let tile_index = inflate_zip_entry(&fwss, TILE_INDEX_PATH);

    assert_eq!(tile_index.len(), 512 * 512 / 8);
    assert_eq!(
        tile_index.iter().map(|byte| byte.count_ones()).sum::<u32>() as usize,
        bitmap.tile_count()
    );

    for tile_key in bitmap.all_tile_keys() {
        let TileKey::Mercator { x, y } = *tile_key else {
            panic!("snapshot tile index requires a Mercator tile, got {tile_key:?}");
        };
        let offset = ((y as usize * 512) + x as usize) / 8;
        assert_ne!(
            tile_index[offset] & (1 << (x % 8)),
            0,
            "missing low-bit-first index bit for tile ({}, {})",
            x,
            y
        );
    }
}

#[test]
fn generated_fwss_roundtrips_through_importer() {
    let bitmap = load_source_bitmap();
    let fwss = export_source_snapshot();
    std::fs::write(ROUNDTRIP_SNAPSHOT_PATH, fwss).unwrap();

    let (roundtripped_bitmap, warnings) =
        import_data::fow::load_fow_snapshot_data(ROUNDTRIP_SNAPSHOT_PATH).unwrap();
    assert_eq!(bitmap, roundtripped_bitmap);
    assert_eq!(format!("{warnings:?}"), "None");
}

#[test]
fn generated_fwss_roundtrip_preserves_rendered_bitmap() {
    let bitmap = load_source_bitmap();
    let original_render = render_first_tile(bitmap.clone());

    let fwss = export_source_snapshot();
    std::fs::write(RENDER_SNAPSHOT_PATH, fwss).unwrap();

    let (roundtripped_bitmap, warnings) =
        import_data::fow::load_fow_snapshot_data(RENDER_SNAPSHOT_PATH).unwrap();
    let roundtripped_render = render_first_tile(roundtripped_bitmap);

    assert_eq!(format!("{warnings:?}"), "None");
    assert_eq!(original_render, roundtripped_render);
}
