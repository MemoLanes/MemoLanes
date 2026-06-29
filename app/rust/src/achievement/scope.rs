use crate::cache_db::LayerKind;
use crate::journey_header::JourneyKind;

/// The journey layer an achievement query is computed over. Flat,
/// closed wire enum (FRB-exposed via `api/achievement.rs`) — kept
/// deliberately separate from the structurally-open
/// `cache_db::LayerKind`, so a future `JourneyKind` variant widens the
/// achievement surface only through a compile error in the exhaustive
/// matches below, never silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AchievementLayer {
    /// Ground journeys only (`JourneyKind::DefaultKind`) — the headline
    /// layer; what "visited" means by default.
    Default,
    /// Flight journeys only ("flown over").
    Flight,
    /// Union of every journey kind.
    All,
}

impl AchievementLayer {
    pub const ALL_LAYERS: [AchievementLayer; 3] = [
        AchievementLayer::Default,
        AchievementLayer::Flight,
        AchievementLayer::All,
    ];

    /// Whether a journey of `kind` contributes to this layer's coverage
    /// and first_visited. Exhaustive over BOTH enums on purpose — this
    /// is the match that makes the type-level promise above real.
    pub fn includes_kind(self, kind: JourneyKind) -> bool {
        match (self, kind) {
            (AchievementLayer::All, _) => true,
            (AchievementLayer::Default, JourneyKind::DefaultKind) => true,
            (AchievementLayer::Default, JourneyKind::Flight) => false,
            (AchievementLayer::Flight, JourneyKind::Flight) => true,
            (AchievementLayer::Flight, JourneyKind::DefaultKind) => false,
        }
    }

    /// The layers a journey of `kind` contributes to (its own kind's
    /// layer plus `All`).
    pub fn layers_including(kind: JourneyKind) -> impl Iterator<Item = AchievementLayer> {
        Self::ALL_LAYERS
            .into_iter()
            .filter(move |l| l.includes_kind(kind))
    }

    /// The cache_db layer holding this layer's merged bitmap.
    pub fn to_layer_kind(self) -> LayerKind {
        match self {
            AchievementLayer::Default => LayerKind::JourneyKind(JourneyKind::DefaultKind),
            AchievementLayer::Flight => LayerKind::JourneyKind(JourneyKind::Flight),
            AchievementLayer::All => LayerKind::All,
        }
    }
}
