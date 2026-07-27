use std::{fs, path::Path};

use memolanes_core::{
    api::api::{self, InitError},
    utils::db::{init_metadata_and_get_version, set_version_in_metadata, SchemaVersion},
};
use rusqlite::Connection;
use tempdir::TempDir;

#[test]
fn repeated_init_preserves_database_version_too_new_error() {
    let temp_dir = TempDir::new("api-init-newer-major-version").unwrap();
    let sub_dir = |name: &str| {
        let path = temp_dir.path().join(name);
        fs::create_dir(&path).unwrap();
        path.into_os_string().into_string().unwrap()
    };
    let runtime_dir = sub_dir("temp");
    let doc_dir = sub_dir("doc");
    let support_dir = sub_dir("support");
    let cache_dir = sub_dir("cache");

    let mut conn = Connection::open(Path::new(&support_dir).join("main.db")).unwrap();
    let tx = conn.transaction().unwrap();
    init_metadata_and_get_version(&tx).unwrap();
    set_version_in_metadata(&tx, SchemaVersion::new(i32::MAX, 0)).unwrap();
    tx.commit().unwrap();
    drop(conn);

    let call_init = || {
        api::init(
            runtime_dir.clone(),
            doc_dir.clone(),
            support_dir.clone(),
            cache_dir.clone(),
        )
    };

    assert_eq!(call_init(), Err(InitError::DatabaseVersionTooNew));
    assert_eq!(call_init(), Err(InitError::DatabaseVersionTooNew));
}
