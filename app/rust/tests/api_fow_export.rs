use chrono::NaiveDate;
use memolanes_core::{
    api::api::{self, ExportResult, ExportType},
    import_data,
    journey_bitmap::JourneyBitmap,
    journey_data::JourneyData,
    journey_header::JourneyKind,
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    main_db::{MainDb, NewJourney},
};
use tempdir::TempDir;

fn insert(main: &mut MainDb, day: u32, data: JourneyData) -> String {
    main.with_txn(|txn| {
        txn.create_and_insert_journey(NewJourney {
            journey_date: NaiveDate::from_ymd_opt(2025, 1, day).unwrap(),
            start: None,
            end: None,
            created_at: None,
            journey_kind: JourneyKind::DefaultKind,
            note: None,
            journey_data: data,
            raw_data: None,
        })
    })
    .unwrap()
}

#[test]
fn fow_api_exports_mercator_and_reports_polar_only_content_as_empty() {
    let dir = TempDir::new("api_fow_export").unwrap();
    let sub = |name| {
        let path = dir.path().join(name);
        std::fs::create_dir(&path).unwrap();
        path.to_str().unwrap().to_owned()
    };
    let support = sub("support");
    let mut main = MainDb::open(&support).unwrap();
    let north = JourneyVector {
        track_segments: vec![TrackSegment {
            track_points: vec![TrackPoint {
                longitude: 0.0,
                latitude: 90.0,
            }],
        }],
    };
    let mut south = JourneyBitmap::new();
    south.edit().add_line(0.0, -90.0, 0.0, -90.0);
    let mut mercator = JourneyBitmap::new();
    mercator.edit().add_line(10.0, 22.0, 10.001, 22.001);
    let mut mixed = mercator.clone();
    mixed.edit().merge_vector(&north);
    mixed.edit().merge(south.clone());
    let north_id = insert(&mut main, 1, JourneyData::Vector(north));
    let south_id = insert(&mut main, 2, JourneyData::Bitmap(south));
    let mixed_id = insert(&mut main, 3, JourneyData::Bitmap(mixed));
    drop(main);
    api::init(sub("temp"), sub("doc"), support, sub("cache")).unwrap();

    for (id, filename) in [(north_id, "north.fwss"), (south_id, "south.fwss")] {
        let path = dir.path().join(filename);
        assert!(matches!(
            api::export_journey(
                path.to_str().unwrap().to_owned(),
                id,
                ExportType::FWSS,
                false,
            )
            .unwrap(),
            ExportResult::DataIsEmpty
        ));
        assert!(
            !path.exists(),
            "no file should be created for empty FoW content"
        );
    }

    let mixed_path = dir.path().join("mixed.fwss");
    assert!(matches!(
        api::export_journey(
            mixed_path.to_str().unwrap().to_owned(),
            mixed_id.clone(),
            ExportType::FWSS,
            false,
        )
        .unwrap(),
        ExportResult::Succeed
    ));
    let (imported, warnings) =
        import_data::fow::load_fow_snapshot_data(mixed_path.to_str().unwrap()).unwrap();
    assert!(warnings.is_none());
    assert_eq!(imported, mercator);

    let all_path = dir.path().join("all.fwss");
    assert!(matches!(
        api::export_all_journeys_as_fwss(all_path.to_str().unwrap().to_owned()).unwrap(),
        ExportResult::Succeed
    ));
    assert_eq!(
        std::fs::read(&all_path).unwrap(),
        std::fs::read(&mixed_path).unwrap()
    );

    api::delete_journey(&mixed_id).unwrap();
    // Empty export must also preserve an existing destination file.
    std::fs::write(&all_path, b"keep existing file").unwrap();
    assert!(matches!(
        api::export_all_journeys_as_fwss(all_path.to_str().unwrap().to_owned()).unwrap(),
        ExportResult::DataIsEmpty
    ));
    assert_eq!(std::fs::read(&all_path).unwrap(), b"keep existing file");
}
