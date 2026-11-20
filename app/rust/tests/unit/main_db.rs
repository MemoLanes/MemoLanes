use super::*;

#[test]
fn opens_and_reopens_existing_database_with_wal_full() -> Result<()> {
    let dir = tempdir::TempDir::new("main-db-rollback-to-wal")?;
    let path = dir.path().to_str().unwrap();
    let conn = utils::db::open_and_migrate(path, "main.db", &migrations())?;
    let mode: String = conn.query_row("PRAGMA journal_mode = DELETE", (), |row| row.get(0))?;
    assert_eq!(mode, "delete");
    conn.execute(
        "INSERT INTO setting (key, value) VALUES ('RAW_DATA_MODE', 'true')",
        (),
    )?;
    drop(conn);

    // Cover conversion from DELETE and reopening an existing WAL database.
    for _ in 0..2 {
        let mut db = MainDb::open(path)?;
        let mode: String = db
            .conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        let synchronous: i32 = db
            .conn
            .pragma_query_value(None, "synchronous", |row| row.get(0))?;
        assert_eq!(mode, "wal");
        assert_eq!(synchronous, 2);
        assert!(db.get_setting_with_default(Setting::RawDataMode, false));
    }
    Ok(())
}

#[test]
fn migrations_are_in_order() {
    assert!(crate::utils::db::migrations_are_strictly_increasing(
        &super::migrations()
    ));
}

#[test]
fn migrate_to_2_0_backfills_journey_kind_and_creates_composite_index() -> Result<()> {
    let mut connection = Connection::open_in_memory()?;
    let tx = connection.transaction()?;
    utils::db::run_migrations(
        &tx,
        "main.db",
        &[utils::db::Migration::new(1, 0, &migrate_to_1_0)],
    )?;

    let header = JourneyHeader {
        id: "flight-journey".to_owned(),
        revision: "revision".to_owned(),
        journey_date: NaiveDate::from_ymd_opt(2026, 7, 25).unwrap(),
        created_at: Utc::now(),
        updated_at: None,
        start: None,
        end: None,
        journey_type: JourneyType::Vector,
        journey_kind: JourneyKind::Flight,
        note: None,
        postprocessor_algo: None,
        has_raw_data: false,
        transport_details: None,
    };
    let header_bytes = header.clone().to_proto().write_to_bytes()?;
    tx.execute(
        "INSERT INTO journey (id, journey_date, type, header, data) VALUES (?1, ?2, ?3, ?4, ?5)",
        (
            &header.id,
            utils::date_to_days_since_epoch(header.journey_date),
            header.journey_type.to_int(),
            header_bytes,
            Vec::<u8>::new(),
        ),
    )?;

    assert_eq!(
        utils::db::run_migrations(&tx, "main.db", &migrations())?,
        utils::db::SchemaVersion::new(2, 1)
    );

    let journey_kind: i8 = tx.query_row(
        "SELECT journey_kind FROM journey WHERE id = ?1",
        [&header.id],
        |row| row.get(0),
    )?;
    assert_eq!(journey_kind, JourneyKind::Flight.to_int());
    let not_null: i8 = tx.query_row(
        "SELECT \"notnull\" FROM pragma_table_info('journey') WHERE name = 'journey_kind'",
        (),
        |row| row.get(0),
    )?;
    assert_eq!(not_null, 1);
    assert!(tx
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'journey_kind_date_index'",
            (),
            |_| Ok(()),
        )
        .is_ok());
    Ok(())
}
