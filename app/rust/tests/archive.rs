pub mod test_utils;

use rust_lib::{archive, gps_processor, main_db::MainDb};
use core::arch;
use std::{fs::File, path::Path};
use tempdir::TempDir;

#[test]
fn basic() {
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
            
    let mut file = File::create(temp_dir.path().join("archive.zip")).unwrap();    
    archive::archive_all_as_zip(&mut main_db, &mut file).unwrap();
}

#[test]
fn recover(){
    let zip_file_path = "./tests/data/archive.zip";    
    let temp_dir = TempDir::new("main_db-recover").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    match archive::recover_archive_file(zip_file_path,&mut main_db) {
        Ok(())=>{
            println!("recover success");
        },
        Err(e)=>{
            println!("错误：{}",e);
        }
    }
    
}