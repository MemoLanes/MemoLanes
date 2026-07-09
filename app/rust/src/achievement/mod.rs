//! Achievement statistics: explored-area and geo-entity coverage computed from
//! journey data. Grouped by role:
//! - [`contract`] — the [`AchievementStore`](contract::AchievementStore) /
//!   [`AchievementReader`](contract::AchievementReader) traits the store implements.
//! - [`layer`] — the [`AchievementLayer`](layer::AchievementLayer) vocab.
//! - [`compute`] — pure kernels: snapshot (+ geo) → state.
//! - [`read_model`] — joins computed state with the geo tree into UI/wire shapes.
//! - [`backend`] — the compute-on-demand store.
pub mod backend;
pub mod compute;
pub mod contract;
pub mod layer;
pub mod read_model;

use anyhow::Result;

use contract::AchievementStore;

/// Construct the achievement store: a compute-on-demand
/// [`backend::on_demand::OnDemandStore`], mirroring `cache_db::new`.
pub fn new(_cache_dir: &str) -> Result<Box<dyn AchievementStore + Send>> {
    Ok(Box::new(backend::on_demand::OnDemandStore::new()))
}
