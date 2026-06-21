//! In-memory cache for achievement stats: one cell per `AchievementQuery`
//! holding a typed `AchievementValue`, cleared on each journey-write commit.

use std::collections::HashMap;
use std::sync::Mutex;

use flutter_rust_bridge::frb;

use super::query::{AchievementQuery, AchievementValue};

#[frb(ignore)]
#[derive(Default)]
pub struct StatCache {
    inner: Mutex<HashMap<AchievementQuery, AchievementValue>>,
}

impl StatCache {
    pub fn get(&self, query: &AchievementQuery) -> Option<AchievementValue> {
        self.inner
            .lock()
            .expect("stat cache poisoned")
            .get(query)
            .cloned()
    }

    pub fn put(&self, query: AchievementQuery, value: AchievementValue) {
        self.inner
            .lock()
            .expect("stat cache poisoned")
            .insert(query, value);
    }

    /// Evict all cached stats — called on each committed journey mutation.
    pub fn invalidate(&self) {
        self.inner.lock().expect("stat cache poisoned").clear();
    }
}
