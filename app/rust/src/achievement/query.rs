//! Achievement stat queries and typed results. Each stat is one
//! `AchievementQuery` variant (used directly as the cache key), plus an
//! `AchievementValue`; later stats extend these enums.

use flutter_rust_bridge::frb;

use super::scope::AchievementLayer;

#[frb(ignore)]
#[derive(PartialEq, Eq, Hash)]
pub enum AchievementQuery {
    ExploredAreaM2 { layer: AchievementLayer },
}

#[frb(ignore)]
#[derive(Debug, Clone, PartialEq)]
pub enum AchievementValue {
    U64(u64),
}
