pub mod test_utils;
use memolanes_core::{import_data, journey_area_utils, journey_bitmap::JourneyBitmap, renderer::*};
use std::collections::HashMap;
//use std::cell::RefCell;

const START_LNG: f64 = 151.1435370795134;
const START_LAT: f64 = -33.793291910360125;
const END_LNG: f64 = 132.1435370795134;
const END_LAT: f64 = -55.793291910360125;

#[test]
fn test_compute_journey_bitmap_area() {
    let (bitmap_import, _warnings) =
        import_data::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
    let mut dummy_map: HashMap<(u16, u16), f64> = HashMap::new();
    let calculated_area =
        journey_area_utils::compute_journey_bitmap_area(&bitmap_import, &mut dummy_map);
    assert_eq!(calculated_area, 3035670); // area unit: m^2
}

#[test]
fn partial_update_use_cached_and_recompute_touched_tiles_only() {
    let journey_bitmap = JourneyBitmap::new();
    let mut map_renderer = MapRenderer::new(journey_bitmap);
    //let touched = RefCell::new(Vec::<(u16, u16)>::new());
    map_renderer.update(|bitmap, cb| bitmap.add_line(START_LNG, START_LAT, END_LNG, END_LAT, cb));
    let _ = map_renderer.get_current_area();

    map_renderer.update(|bitmap, cb| {
        bitmap.add_line(
            START_LNG, END_LAT, END_LNG, START_LAT,
            cb, //|tile_pos| {
               //    touched.borrow_mut().push(tile_pos);
               //    cb(tile_pos);
               //},
        )
    });
    let update_area = map_renderer.get_current_area();

    //let tiles = touched.borrow();
    //let count = tiles.len();
    //println!("Touched {} tiles: {:?}", count, &*tiles);

    //assert!(count > 0, "expected to touch at least one tile, got {}", count);

    let mut dummy_update_map = HashMap::new();
    let mut full_journey_bitmap = JourneyBitmap::new();
    full_journey_bitmap.add_line(START_LNG, START_LAT, END_LNG, END_LAT, |_| {});
    full_journey_bitmap.add_line(START_LNG, END_LAT, END_LNG, START_LAT, |_| {});
    let full_area = journey_area_utils::compute_journey_bitmap_area(
        &full_journey_bitmap,
        &mut dummy_update_map,
    );

    println!("update_area = {}", update_area);
    println!("full_area = {}", full_area);
    assert_eq!(
        update_area, full_area,
        "updated area after partial-update must match a full compute"
    );
}
