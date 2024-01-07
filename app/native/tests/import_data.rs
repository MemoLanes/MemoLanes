use native::import_data;

#[test]
fn load_fow_sync_data() {
    let (bitmap_1, warnings_1) = import_data::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
    let (bitmap_2, warnings_2) = import_data::load_fow_sync_data("./tests/data/fow_2.zip").unwrap();
    assert_eq!(bitmap_1, bitmap_2);
    assert_eq!(format!("{:?}", warnings_1), "None");
    assert_eq!(
        format!("{:?}", warnings_2),
        "Some(\"unexpected file: Sync/garbage_2\")"
    );
}
