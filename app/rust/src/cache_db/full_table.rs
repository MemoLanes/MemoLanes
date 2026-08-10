use anyhow::Result;
use rusqlite::{Connection, Transaction};

use super::bitmap_io::{read_bitmap, to_blob};
use super::LayerKind;
use crate::journey_bitmap::JourneyBitmap;

pub const TABLE: &str = "journey_cache__full";

/// Body of every implementation's `1.0` migration.
pub fn migrate_to_1_0(tx: &Transaction) -> Result<()> {
    tx.execute("DROP TABLE IF EXISTS journey_cache;", ())?;
    tx.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS `{TABLE}` (
                kind TEXT PRIMARY KEY NOT NULL UNIQUE,
                data BLOB NOT NULL
            )"
        ),
        (),
    )?;
    Ok(())
}

pub fn get(conn: &Connection, layer_kind: &LayerKind) -> Result<Option<JourneyBitmap>> {
    read_bitmap(
        conn,
        &format!("SELECT data FROM `{TABLE}` WHERE kind = ?1;"),
        (layer_kind.to_sql(),),
    )
}

pub fn set(conn: &Connection, layer_kind: &LayerKind, bitmap: &mut JourneyBitmap) -> Result<()> {
    let layer_kind_sql = layer_kind.to_sql();
    info!("[cacheDb] setting full cache for layer_kind = {layer_kind_sql}");
    let data = to_blob(bitmap)?;
    conn.execute(
        &format!("INSERT OR REPLACE INTO `{TABLE}` (kind, data) VALUES (?1, ?2)"),
        (layer_kind_sql, &data),
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, layer_kind: &LayerKind) -> Result<()> {
    conn.execute(
        &format!("DELETE FROM `{TABLE}` WHERE kind = ?1;"),
        (layer_kind.to_sql(),),
    )?;
    Ok(())
}

pub fn clear(conn: &Connection) -> Result<()> {
    conn.execute(&format!("DELETE FROM `{TABLE}`;"), ())?;
    Ok(())
}
