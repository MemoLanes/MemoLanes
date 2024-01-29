pub mod test_utils;

use rust_lib::{archive, gps_processor, journey_data::JourneyData, main_db::MainDb};
use std::fs::File;
use tempdir::TempDir;

#[test]
fn basic() {
    let test_error_zip = false;
    let test_data = test_utils::load_raw_gpx_data_for_test();

    let temp_dir = TempDir::new("main_db-basic").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    for (i, raw_data) in test_data.iter().enumerate() {
        if i > 1000 && i % 1000 == 0 {
            main_db
                .with_txn(|txn| txn.finalize_ongoing_journey())
                .unwrap();
        }
        main_db
            .record(raw_data, gps_processor::ProcessResult::Append)
            .unwrap();
    }
    main_db
        .with_txn(|txn| txn.finalize_ongoing_journey())
        .unwrap();

    let journeys = main_db.with_txn(|txn| txn.list_all_journeys()).unwrap();
    let mut journey_datas: Vec<JourneyData> = Vec::new();
    for journey in journeys.clone().into_iter() {
        let journey_data = main_db
            .with_txn(|txn| txn.get_journey(&journey.id))
            .unwrap();
        journey_datas.push(journey_data);
    }
    let zip_file_path = temp_dir.path().join("archive.zip");
    let mut file = File::create(zip_file_path.clone()).unwrap();
    if !test_error_zip {
        archive::archive_all_as_zip(&mut main_db, &mut file).unwrap();
    }

    match archive::recover_archive_file(zip_file_path.to_str().unwrap(), &mut main_db) {
        Ok(_) => {}
        Err(e) => {
            println!("{}", e);
        }
    }

    let recover_journeys = main_db.with_txn(|txn| txn.list_all_journeys()).unwrap();
    assert_eq!(journeys, recover_journeys);

    let mut recover_journey_datas: Vec<JourneyData> = Vec::new();
    for journey in journeys {
        let journey_data = main_db
            .with_txn(|txn| txn.get_journey(&journey.id))
            .unwrap();
        recover_journey_datas.push(journey_data);
    }
    assert_eq!(journey_datas, recover_journey_datas);
}
