use memolanes_core::achievement::query::{AchievementQuery, AchievementValue};
use memolanes_core::achievement::scope::AchievementLayer;
use memolanes_core::achievement::stat_cache::StatCache;

fn v(n: u64) -> AchievementValue {
    AchievementValue::U64(n)
}

fn q(layer: AchievementLayer) -> AchievementQuery {
    AchievementQuery::ExploredAreaM2 { layer }
}

#[test]
fn round_trips_and_isolates_keys() {
    let c = StatCache::default();
    c.put(q(AchievementLayer::Default), v(1));
    c.put(q(AchievementLayer::Flight), v(2));
    assert_eq!(c.get(&q(AchievementLayer::Default)), Some(v(1)));
    assert_eq!(c.get(&q(AchievementLayer::Flight)), Some(v(2)));
    assert_eq!(c.get(&q(AchievementLayer::All)), None);
}

#[test]
fn invalidate_evicts_all() {
    let c = StatCache::default();
    c.put(q(AchievementLayer::Default), v(1));
    c.put(q(AchievementLayer::Flight), v(2));
    c.invalidate();
    assert_eq!(c.get(&q(AchievementLayer::Default)), None);
    assert_eq!(c.get(&q(AchievementLayer::Flight)), None);
}
