//! Active worldview state shared across the achievement layer.
//!
//! Owns two statics moved here from `api/achievement.rs` to keep
//! `storage.rs` free of dependencies on `api/*`:
//!
//! - `GEO_LOOKUP`     — swappable via `set_geo_lookup`; first installed at
//!   `init_achievement_system`, replaced on POV switch.
//! - `ACTIVE_WORLDVIEW_CTX` — bundled snapshot of the above + the active
//!   worldview id; `merge_journey` reads this from `Storage::with_db_txn`.

use std::sync::{Arc, OnceLock, RwLock};

use crate::achievement::geo_lookup::GeoLookupTable;

/// Snapshot bundle passed to cache write hooks. `lookup` is set-once
/// and bundled here so `merge_journey` and friends can take a single
/// `Option<&ActiveWorldviewCtx>` parameter without `cache_db` reaching
/// directly into `GEO_LOOKUP` (which would invert layering).
#[derive(Clone)]
pub struct ActiveWorldviewCtx {
    pub worldview_id: String,
    pub lookup: Arc<GeoLookupTable>,
}

pub static GEO_LOOKUP: OnceLock<RwLock<Arc<GeoLookupTable>>> = OnceLock::new();
pub static ACTIVE_WORLDVIEW_CTX: OnceLock<RwLock<Arc<ActiveWorldviewCtx>>> = OnceLock::new();

/// Install (or replace) the active geo lookup. First call initializes the
/// `RwLock`; later calls overwrite the inner `Arc` (worldview switch).
pub fn set_geo_lookup(lookup: Arc<GeoLookupTable>) {
    // The init closure runs only on the very first call; the write below
    // always overwrites, so later calls replace (POV switch).
    let lock = GEO_LOOKUP.get_or_init(|| RwLock::new(lookup.clone()));
    *lock.write().expect("GEO_LOOKUP poisoned") = lookup;
}

/// Current active geo lookup, or `None` before the first install.
pub fn geo_lookup() -> Option<Arc<GeoLookupTable>> {
    Some(
        GEO_LOOKUP
            .get()?
            .read()
            .expect("GEO_LOOKUP poisoned")
            .clone(),
    )
}

/// Read the current active context. Returns `None` until
/// `init_achievement_system` has run.
pub fn current() -> Option<Arc<ActiveWorldviewCtx>> {
    let lock = ACTIVE_WORLDVIEW_CTX.get()?;
    Some(lock.read().expect("ACTIVE_WORLDVIEW_CTX poisoned").clone())
}

/// Replace the active worldview snapshot. Used by `install_active`
/// (`init_achievement_system` / `switch_worldview`). Initializes the
/// underlying `RwLock` on first call; subsequent calls overwrite the
/// inner `Arc` (last-writer-wins).
pub fn set_active(ctx: ActiveWorldviewCtx) {
    let arc = Arc::new(ctx);
    let lock = ACTIVE_WORLDVIEW_CTX.get_or_init(|| RwLock::new(arc.clone()));
    let mut w = lock.write().expect("ACTIVE_WORLDVIEW_CTX poisoned");
    *w = arc;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> Arc<GeoLookupTable> {
        Arc::new(
            GeoLookupTable::load_from_bytes(
                &crate::achievement::geo_lookup::test_support::tiny_geo_bin([1u8; 32]),
            )
            .unwrap(),
        )
    }

    #[test]
    fn geo_lookup_set_then_get_then_replace() {
        let a = tiny();
        set_geo_lookup(a.clone());
        assert!(Arc::ptr_eq(&geo_lookup().unwrap(), &a));
        let b = tiny();
        set_geo_lookup(b.clone());
        assert!(
            Arc::ptr_eq(&geo_lookup().unwrap(), &b),
            "set_geo_lookup must replace, not error"
        );
    }
}
