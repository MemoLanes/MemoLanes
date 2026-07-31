use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::NaiveDate;
use rusqlite::Connection;

use super::{full_table, range, CacheDb, CacheEntry, LayerKind};
use strum::IntoEnumIterator;

use crate::{
    achievement::on_demand::OnDemandReader, achievement::AchievementReader, geo::GeoLookup,
    journey_bitmap::JourneyBitmap, journey_data::JourneyData, journey_header::JourneyKind,
    journey_snapshot::JourneySnapshot, main_db, utils,
};

fn migrations() -> [utils::db::Migration<'static>; 1] {
    [utils::db::Migration::new(1, 0, &full_table::migrate_to_1_0)]
}

/// Simple SQLite-backed implementation of [`CacheDb`] using a single full-table cache.
///
/// Journey bitmaps are cached at one granularity:
/// - **Full** (`journey_cache__full`): one bitmap per `LayerKind`, covering all
///   journeys in the database.
///
/// Only full-range queries (`range: None`) are cached. Explicit date range
/// queries are always computed directly from the main DB without caching.
/// Nothing derived is persisted here, so achievement answers are computed per
/// read and this holds no achievement state at all.
pub struct CacheDbV1 {
    conn: Connection,
}

impl CacheDbV1 {
    pub fn open(cache_dir: &str) -> CacheDbV1 {
        let conn =
            super::open_or_recreate(cache_dir, &migrations()).expect("failed to open cache db");
        CacheDbV1 { conn }
    }
}

impl CacheDb for CacheDbV1 {
    #[auto_context]
    fn get_or_compute(
        &mut self,
        txn: &main_db::Txn,
        layer_kind: &LayerKind,
        range: Option<(NaiveDate, NaiveDate)>,
    ) -> Result<JourneyBitmap> {
        match range {
            Some((from, to)) => {
                debug_assert!(from <= to);
                // Explicit range: compute directly, no caching.
                range::compute(txn, from, to, layer_kind)
            }
            None => {
                // Full range: use cache.
                if let Some(bm) = full_table::get(&self.conn, layer_kind)? {
                    return Ok(bm);
                }

                let mut result = match *layer_kind {
                    LayerKind::All => {
                        let mut bm = JourneyBitmap::new();
                        for jk in JourneyKind::iter() {
                            bm.merge(self.get_or_compute(
                                txn,
                                &LayerKind::JourneyKind(jk),
                                None,
                            )?);
                        }
                        bm
                    }
                    LayerKind::JourneyKind(_) => {
                        // Compute from the full date range in the main DB.
                        match txn.journey_date_range()? {
                            Some((min, max)) => range::compute(txn, min, max, layer_kind)?,
                            None => JourneyBitmap::new(),
                        }
                    }
                };

                full_table::set(&self.conn, layer_kind, &mut result)?;
                Ok(result)
            }
        }
    }

    #[auto_context]
    fn merge_journey(
        &mut self,
        entry: &CacheEntry,
        data: &JourneyData,
        // Nothing derived is stored, so there is nothing to attribute here.
        _geo: Option<&dyn GeoLookup>,
    ) -> Result<()> {
        let layer_kind = LayerKind::JourneyKind(entry.kind);

        // Invalidate All aggregate.
        full_table::delete(&self.conn, &LayerKind::All)?;

        // Merge into the per-kind full cache if it exists.
        if let Some(mut bm) = full_table::get(&self.conn, &layer_kind)? {
            data.merge_into_with_partial_clone(&mut bm);
            full_table::set(&self.conn, &layer_kind, &mut bm)?;
        }

        Ok(())
    }

    #[auto_context]
    fn invalidate(&mut self, entries: &[CacheEntry]) -> Result<()> {
        // Delete affected kind entries and All entry.
        let mut deleted = std::collections::HashSet::new();
        for entry in entries {
            let layer_kind = LayerKind::JourneyKind(entry.kind);
            if deleted.insert(layer_kind) {
                full_table::delete(&self.conn, &layer_kind)?;
            }
        }
        full_table::delete(&self.conn, &LayerKind::All)?;
        Ok(())
    }

    #[auto_context]
    fn clear_all(&mut self) -> Result<()> {
        full_table::clear(&self.conn)
    }

    fn flush(&self) -> Result<()> {
        self.conn.cache_flush()?;
        Ok(())
    }

    #[auto_context]
    fn achievement_reader<'a>(
        &'a mut self,
        txn: &'a main_db::Txn<'_>,
        geo: Option<&'a dyn GeoLookup>,
    ) -> Result<Box<dyn AchievementReader + 'a>> {
        Ok(Box::new(OnDemandReader::new(
            JourneySnapshot::new(txn, self),
            geo,
        )))
    }
}

#[cfg(test)]
mod migration_tests {
    #[test]
    fn migrations_are_in_order() {
        assert!(crate::utils::db::migrations_are_strictly_increasing(
            &super::migrations()
        ));
    }
}
