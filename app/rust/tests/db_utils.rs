use anyhow::Result;
use memolanes_core::utils::db::{
    migrations_are_strictly_increasing, run_migrations, set_version_in_metadata, DbError,
    Migration, SchemaVersion,
};
use rusqlite::{Connection, OptionalExtension, Transaction};

fn must_not_run(_: &Transaction) -> Result<()> {
    unreachable!()
}

fn no_op(_: &Transaction) -> Result<()> {
    Ok(())
}

fn create_metadata_table(tx: &Transaction) -> Result<()> {
    tx.execute(
        "CREATE TABLE db_metadata (
            key TEXT NOT NULL PRIMARY KEY,
            value TEXT
        )",
        (),
    )?;
    Ok(())
}

fn read_version(tx: &Transaction) -> Result<SchemaVersion> {
    let component = |key| -> Result<i32> {
        let value: Option<String> = tx
            .query_row(
                "SELECT value FROM db_metadata WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.map(|value| value.parse()).transpose()?.unwrap_or(0))
    };
    Ok(SchemaVersion::new(
        component("version")?,
        component("minor_version")?,
    ))
}

#[test]
fn checks_strictly_increasing_migration_versions() {
    assert!(migrations_are_strictly_increasing(&[
        Migration::new(1, 0, &no_op),
        Migration::new(1, 3, &no_op),
        Migration::new(3, 0, &no_op),
    ]));
    assert!(!migrations_are_strictly_increasing(&[
        Migration::new(1, 0, &no_op),
        Migration::new(1, 0, &no_op),
    ]));
    assert!(!migrations_are_strictly_increasing(&[
        Migration::new(2, 0, &no_op),
        Migration::new(1, 5, &no_op),
    ]));
}

#[test]
fn fresh_database_runs_major_then_minor_migrations() -> Result<()> {
    let mut conn = Connection::open_in_memory()?;
    let tx = conn.transaction()?;
    let create_data = |tx: &Transaction| {
        tx.execute("CREATE TABLE data (id INTEGER PRIMARY KEY)", ())?;
        Ok(())
    };
    let add_note = |tx: &Transaction| {
        tx.execute("ALTER TABLE data ADD COLUMN note TEXT", ())?;
        Ok(())
    };
    let migrations = [
        Migration::new(1, 0, &create_data),
        Migration::new(1, 1, &add_note),
    ];

    let version = run_migrations(&tx, "test.db", &migrations)?;

    assert_eq!(version, SchemaVersion::new(1, 1));
    tx.execute("INSERT INTO data (id, note) VALUES (1, 'ready')", ())?;
    Ok(())
}

#[test]
fn missing_minor_version_is_zero_and_migrates() -> Result<()> {
    let mut conn = Connection::open_in_memory()?;
    let tx = conn.transaction()?;
    create_metadata_table(&tx)?;
    tx.execute("CREATE TABLE data (id INTEGER PRIMARY KEY)", ())?;
    tx.execute(
        "INSERT INTO db_metadata (key, value) VALUES ('version', '1')",
        (),
    )?;

    let add_note = |tx: &Transaction| {
        tx.execute("ALTER TABLE data ADD COLUMN note TEXT", ())?;
        Ok(())
    };
    let migrations = [
        Migration::new(1, 0, &must_not_run),
        Migration::new(1, 1, &add_note),
    ];
    let version = run_migrations(&tx, "test.db", &migrations)?;

    assert_eq!(version, SchemaVersion::new(1, 1));
    assert_eq!(read_version(&tx)?, SchemaVersion::new(1, 1));
    tx.execute("INSERT INTO data (id, note) VALUES (1, 'migrated')", ())?;
    Ok(())
}

#[test]
fn legacy_database_records_explicit_minor_zero() -> Result<()> {
    let mut conn = Connection::open_in_memory()?;
    let tx = conn.transaction()?;
    create_metadata_table(&tx)?;
    tx.execute("CREATE TABLE data (id INTEGER PRIMARY KEY)", ())?;
    tx.execute(
        "INSERT INTO db_metadata (key, value) VALUES ('version', '1')",
        (),
    )?;

    let version = run_migrations(&tx, "test.db", &[Migration::new(1, 0, &must_not_run)])?;

    assert_eq!(version, SchemaVersion::new(1, 0));
    let stored_minor: String = tx.query_row(
        "SELECT value FROM db_metadata WHERE key = 'minor_version'",
        (),
        |row| row.get(0),
    )?;
    assert_eq!(stored_minor, "0");
    Ok(())
}

#[test]
fn newer_minor_version_is_accepted_and_preserved() -> Result<()> {
    let mut conn = Connection::open_in_memory()?;
    let tx = conn.transaction()?;
    create_metadata_table(&tx)?;
    tx.execute("CREATE TABLE data (id INTEGER PRIMARY KEY, note TEXT)", ())?;
    set_version_in_metadata(&tx, SchemaVersion::new(1, 2))?;

    let version = run_migrations(
        &tx,
        "test.db",
        &[
            Migration::new(1, 0, &must_not_run),
            Migration::new(1, 1, &must_not_run),
        ],
    )?;

    assert_eq!(version, SchemaVersion::new(1, 2));
    assert_eq!(read_version(&tx)?, SchemaVersion::new(1, 2));
    Ok(())
}

#[test]
fn interleaved_migrations_with_version_gaps_run_in_order() -> Result<()> {
    let mut conn = Connection::open_in_memory()?;
    let tx = conn.transaction()?;
    let record = |tx: &Transaction, version: &str| {
        tx.execute(
            "INSERT INTO migration_order (version) VALUES (?1)",
            [version],
        )?;
        Ok(())
    };
    let major_a = |tx: &Transaction| {
        tx.execute("CREATE TABLE migration_order (version TEXT NOT NULL)", ())?;
        record(tx, "1.0")
    };
    let minor_1 = |tx: &Transaction| record(tx, "1.3");
    let major_b = |tx: &Transaction| record(tx, "3.0");
    let minor_2 = |tx: &Transaction| record(tx, "3.5");
    let migrations = [
        Migration::new(1, 0, &major_a),
        Migration::new(1, 3, &minor_1),
        Migration::new(3, 0, &major_b),
        Migration::new(3, 5, &minor_2),
    ];

    let version = run_migrations(&tx, "test.db", &migrations)?;

    assert_eq!(version, SchemaVersion::new(3, 5));
    assert_eq!(read_version(&tx)?, version);
    let order = tx
        .prepare("SELECT version FROM migration_order ORDER BY rowid")?
        .query_map((), |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    assert_eq!(order, ["1.0", "1.3", "3.0", "3.5"]);
    Ok(())
}

#[test]
fn newer_major_version_is_rejected() -> Result<()> {
    let mut conn = Connection::open_in_memory()?;
    let tx = conn.transaction()?;
    create_metadata_table(&tx)?;
    set_version_in_metadata(&tx, SchemaVersion::new(2, 0))?;

    let error = run_migrations(&tx, "test.db", &[Migration::new(1, 0, &must_not_run)]).unwrap_err();

    assert!(matches!(
        error.downcast_ref::<DbError>(),
        Some(DbError::VersionTooNew)
    ));
    Ok(())
}
