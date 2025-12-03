extern crate simplelog;
use anyhow::{Context, Result};
use auto_context::auto_context;
use rusqlite::{Connection, OptionalExtension};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::path::Path;

use crate::{journey_bitmap::JourneyBitmap, journey_data, journey_header::JourneyKind, utils};

// TODO: Right now, we keep a cache of all finalized journeys (and fallback to
// compute it by merging all journeys in main db). We clear this cache entirely
// when there is a update to any finalized journey in main db.
// To improve this, we should:
// V1: we want more fine-grained cache invalidation/update rules. e.g. If we are
// only append new finalized journey, then we could just merge that single
// journey with the existing cache.
// V2: we might need multiple caches for different layer (e.g. one for flight,
// one for land).
// V3: we might need multiple caches keyed by `(start_time, end_time]`, and
// have one per year or one per month. So when there is an update to the
// history, we could clear some but not all cache, and re-construct these
//  outdated ones reasonably quickly.

// TODO: we should consider using transaction to get better error handling behavior

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
            if version == 0 {
                tx.execute("DROP TABLE IF EXISTS journey_cache;", ())?;
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS `journey_cache__full` (
                        kind Text PRIMARY KEY NOT NULL UNIQUE,
                        data BLOB NOT NULL
                    )",
                    (),
                )?;
            }
            utils::db::set_version_in_metadata(&tx, target_version)?;
        }
    }
    tx.commit()?;
    Ok(conn)
}

// CacheDb structure
pub struct CacheDb {
    conn: RefCell<Option<Connection>>,
    cache_dir: String,
}

/// flutter_rust_bridge:ignore
#[derive(Eq, Hash, Clone, Copy, Debug, PartialEq)]
pub enum LayerKind {
    All,
    JourneyKind(JourneyKind),
}

impl LayerKind {
    fn to_sql(self) -> &'static str {
        match self {
            LayerKind::All => "All",
            LayerKind::JourneyKind(kind) => match kind {
                JourneyKind::DefaultKind => "Default",
                JourneyKind::Flight => "Flight",
            },
        }
    }
}

impl CacheDb {
    fn with_conn<R>(&self, f: impl FnOnce(&Connection) -> Result<R>) -> Result<R> {
        let conn = self.conn.borrow();
        let conn_ref = conn.as_ref().expect("DB connection is not open");
        f(conn_ref)
    }

    // Method to open and return a CacheDb instance
    pub fn open(cache_dir: &str) -> CacheDb {
        let conn = open_db(cache_dir, "cache.db").expect("failed to open cache db");
        CacheDb {
            conn: RefCell::new(Some(conn)),
            cache_dir: cache_dir.to_string(),
        }
    }

    pub fn flush(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.cache_flush()?;
            Ok(())
        })
    }

    fn reopen(&self) -> Result<()> {
        self.conn.borrow_mut().take();
        let path = Path::new(&self.cache_dir).join("cache.db");
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("Failed to remove corrupt DB: {:?}", path))?;
        }

        let new_conn = open_db(&self.cache_dir, "cache.db")
            .with_context(|| format!("Failed to open DB: {:?}", path))?;

        *self.conn.borrow_mut() = Some(new_conn);

        Ok(())
    }

    fn is_recoverable_error(e: &rusqlite::Error) -> bool {
        use rusqlite::Error::*;
        match e {
            SqliteFailure(err, _) => {
                matches!(
                    err.extended_code,
                    rusqlite::ffi::SQLITE_IOERR
                        | rusqlite::ffi::SQLITE_CORRUPT
                        | rusqlite::ffi::SQLITE_NOTADB
                        | rusqlite::ffi::SQLITE_CANTOPEN
                        | rusqlite::ffi::SQLITE_READONLY_DBMOVED
                )
            }
            _ => false,
        }
    }

    fn retry_fn<T, F>(&self, mut f: F) -> Result<T>
    where
        F: FnMut(&Connection) -> Result<T>,
    {
        match self.with_conn(|conn| f(conn)) {
            Ok(v) => Ok(v),
            Err(e) => {
                if let Some(sql_err) = e.downcast_ref::<rusqlite::Error>() {
                    if Self::is_recoverable_error(sql_err) {
                        self.reopen()?;
                        return self.with_conn(|conn| f(conn));
                    }
                }
                Err(e)
            }
        }
    }

    pub fn get_full_journey_cache(&self, layer_kind: &LayerKind) -> Result<Option<JourneyBitmap>> {
        self.retry_fn(|conn| {
            let sql = "SELECT data FROM `journey_cache__full` WHERE kind = ?1;";
            let mut query = conn.prepare(sql)?;
            let data = query
                .query_row((layer_kind.to_sql(),), |row| {
                    let data = row.get_ref(0)?.as_blob()?;
                    Ok(journey_data::deserialize_journey_bitmap(data))
                })
                .optional()?
                .transpose()?;
            Ok(data)
        })
    }

    pub fn set_full_journey_cache(
        &self,
        layer_kind: &LayerKind,
        journey_bitmap: &JourneyBitmap,
    ) -> Result<()> {
        self.retry_fn(|conn| {
            let mut data = Vec::new();
            journey_data::serialize_journey_bitmap(journey_bitmap, &mut data)?;
            conn.execute(
                "INSERT OR REPLACE INTO `journey_cache__full` (kind, data) VALUES (?1, ?2)",
                (layer_kind.to_sql(), &data),
            )?;
            Ok(())
        })
    }

    // get a merged cache or compute from storage
    #[auto_context]
    pub fn get_full_journey_cache_or_compute<F>(
        &self,
        layer_kind: &LayerKind,
        f: F,
    ) -> Result<JourneyBitmap>
    where
        F: FnOnce() -> Result<JourneyBitmap>,
    {
        match self.get_full_journey_cache(layer_kind)? {
            Some(journey_bitmap) => Ok(journey_bitmap),
            None => {
                let journey_bitmap = f()?;
                self.set_full_journey_cache(layer_kind, &journey_bitmap)?;

                Ok(journey_bitmap)
            }
        }
    }

    #[auto_context]
    pub fn clear_all_cache(&self) -> Result<()> {
        // TODO: in v3, use more fine-grained delete with year/month
        self.retry_fn(|conn| {
            conn.execute("DELETE FROM `journey_cache__full`;", ())?;
            Ok(())
        })
    }

    #[auto_context]
    pub fn delete_full_journey_cache(&self, layer_kind: &LayerKind) -> Result<()> {
        self.retry_fn(|conn| {
            conn.execute(
                "DELETE FROM `journey_cache__full` WHERE kind = ?1;",
                (layer_kind.to_sql(),),
            )?;
            Ok(())
        })
    }

    #[auto_context]
    pub fn update_full_journey_cache_if_exists<F>(&self, layer_kind: &LayerKind, f: F) -> Result<()>
    where
        F: FnOnce(&mut JourneyBitmap) -> Result<()>,
    {
        match self.get_full_journey_cache(layer_kind)? {
            None => (),
            Some(mut journey_bitmap) => {
                f(&mut journey_bitmap)?;
                self.set_full_journey_cache(layer_kind, &journey_bitmap)?;
            }
        }
        Ok(())
    }
}
