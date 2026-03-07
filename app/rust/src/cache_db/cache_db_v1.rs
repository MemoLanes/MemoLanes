use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension};
use std::cmp::Ordering;
use std::path::Path;

use super::{CacheDb, CacheEntry, LayerKind};
use strum::IntoEnumIterator;

use crate::{
    journey_bitmap::JourneyBitmap, journey_data, journey_data::JourneyData,
    journey_header::JourneyKind, main_db, utils,
};

const TABLE_FULL: &str = "journey_cache__full";

fn open_db(cache_dir: &str, file_name: &str) -> Result<Connection> {
    debug!("opening cache db for {file_name}");
    let mut conn = Connection::open(Path::new(cache_dir).join(file_name))?;

    let tx = conn.transaction()?;
    let version = utils::db::init_metadata_and_get_version(&tx)?;

    let target_version = 1;
    debug!("current version = {version}, target_version = {target_version}");
    match version.cmp(&target_version) {
        Ordering::Equal => (),
        Ordering::Greater => {
            bail!(
                "version too high: current version = {version}, target_version = {target_version}"
            );
        }
        Ordering::Less => {
            tx.execute("DROP TABLE IF EXISTS journey_cache;", ())?;
            tx.execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS `{TABLE_FULL}` (
                    kind TEXT PRIMARY KEY NOT NULL UNIQUE,
                    data BLOB NOT NULL
                )"
                ),
                (),
            )?;
            utils::db::set_version_in_metadata(&tx, target_version)?;
        }
    }
    tx.commit()?;
    Ok(conn)
}

fn query_bitmap(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Option<JourneyBitmap>> {
    let mut stmt = conn.prepare(sql)?;
    stmt.query_row(params, |row| {
        let data = row.get_ref(0)?.as_blob()?;
        Ok(journey_data::deserialize_journey_bitmap(data))
    })
    .optional()?
    .transpose()
}

fn serialize_bitmap(bitmap: &JourneyBitmap) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    journey_data::serialize_journey_bitmap(bitmap, &mut data)?;
    Ok(data)
}

fn compute_range_from_txn(
    txn: &main_db::Txn,
    from: NaiveDate,
    to: NaiveDate,
    layer_kind: &LayerKind,
) -> Result<JourneyBitmap> {
    let mut bitmap = JourneyBitmap::new();
    for header in txn.query_journeys(Some(from), Some(to))? {
        let include = match layer_kind {
            LayerKind::All => true,
            LayerKind::JourneyKind(kind) => *kind == header.journey_kind,
        };
        if include {
            let data = txn.get_journey_data(&header.id)?;
            data.merge_into(&mut bitmap);
        }
    }
    Ok(bitmap)
}

/// Simple SQLite-backed implementation of [`CacheDb`] using a single full-table cache.
///
/// Journey bitmaps are cached at one granularity:
/// - **Full** (`journey_cache__full`): one bitmap per `LayerKind`, covering all
///   journeys in the database.
///
/// Only full-range queries (`from: None, to: None`) are cached. Explicit date
/// range queries are always computed directly from the main DB without caching.
pub struct CacheDbV1 {
    conn: Connection,
}

impl CacheDbV1 {
    pub fn open(cache_dir: &str) -> CacheDbV1 {
        let conn = open_db(cache_dir, "cache.db").expect("failed to open cache db");
        CacheDbV1 { conn }
    }

    fn get_full(conn: &Connection, layer_kind: &LayerKind) -> Result<Option<JourneyBitmap>> {
        query_bitmap(
            conn,
            &format!("SELECT data FROM `{TABLE_FULL}` WHERE kind = ?1;"),
            (layer_kind.to_sql(),),
        )
    }

    fn set_full(
        conn: &Connection,
        layer_kind: &LayerKind,
        journey_bitmap: &JourneyBitmap,
    ) -> Result<()> {
        let data = serialize_bitmap(journey_bitmap)?;
        conn.execute(
            &format!("INSERT OR REPLACE INTO `{TABLE_FULL}` (kind, data) VALUES (?1, ?2)"),
            (layer_kind.to_sql(), &data),
        )?;
        Ok(())
    }

    fn delete_full(conn: &Connection, layer_kind: &LayerKind) -> Result<()> {
        conn.execute(
            &format!("DELETE FROM `{TABLE_FULL}` WHERE kind = ?1;"),
            (layer_kind.to_sql(),),
        )?;
        Ok(())
    }
}

impl CacheDb for CacheDbV1 {
    #[auto_context]
    fn get_or_compute(
        &self,
        txn: &main_db::Txn,
        layer_kind: &LayerKind,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<JourneyBitmap> {
        match (from, to) {
            (Some(f), Some(t)) => {
                debug_assert!(f <= t);
                // Explicit range: compute directly, no caching.
                compute_range_from_txn(txn, f, t, layer_kind)
            }
            (None, None) => {
                // Full range: use cache.
                if let Some(bm) = Self::get_full(&self.conn, layer_kind)? {
                    return Ok(bm);
                }

                let result = match *layer_kind {
                    LayerKind::All => {
                        let mut bm = JourneyBitmap::new();
                        for jk in JourneyKind::iter() {
                            bm.merge(self.get_or_compute(
                                txn,
                                &LayerKind::JourneyKind(jk),
                                None,
                                None,
                            )?);
                        }
                        bm
                    }
                    LayerKind::JourneyKind(_) => {
                        // Compute from the full date range in the main DB.
                        match txn.journey_date_range()? {
                            Some((min, max)) => compute_range_from_txn(txn, min, max, layer_kind)?,
                            None => JourneyBitmap::new(),
                        }
                    }
                };

                Self::set_full(&self.conn, layer_kind, &result)?;
                Ok(result)
            }
            _ => bail!("from and to must both be Some or both be None"),
        }
    }

    #[auto_context]
    fn merge_journey(&self, entry: &CacheEntry, data: &JourneyData) -> Result<()> {
        let layer_kind = LayerKind::JourneyKind(entry.kind);

        // Invalidate All aggregate.
        Self::delete_full(&self.conn, &LayerKind::All)?;

        // Merge into the per-kind full cache if it exists.
        if let Some(mut bm) = Self::get_full(&self.conn, &layer_kind)? {
            data.merge_into_with_partial_clone(&mut bm);
            Self::set_full(&self.conn, &layer_kind, &bm)?;
        }

        Ok(())
    }

    #[auto_context]
    fn invalidate(&self, entries: &[CacheEntry]) -> Result<()> {
        // Delete affected kind entries and All entry.
        let mut deleted = std::collections::HashSet::new();
        for entry in entries {
            let layer_kind = LayerKind::JourneyKind(entry.kind);
            if deleted.insert(layer_kind) {
                Self::delete_full(&self.conn, &layer_kind)?;
            }
        }
        Self::delete_full(&self.conn, &LayerKind::All)?;
        Ok(())
    }

    #[auto_context]
    fn clear_all(&self) -> Result<()> {
        self.conn
            .execute(&format!("DELETE FROM `{TABLE_FULL}`;"), ())?;
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        self.conn.cache_flush()?;
        Ok(())
    }
}
