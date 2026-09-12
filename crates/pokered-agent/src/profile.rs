//! [`ObservationProfile`]: which snapshot sections are populated.
//!
//! Levels follow the design doc: 1 = runtime state only (map, position,
//! facing, party, bag, badges — progress flags ride along via
//! `get_flags`), 2 = + dialogue/battle detail, 3 = + nearby entities
//! (the full symbolic observation), 4 = + allowance for global world
//! data (flag only; the world model arrives in M3).

use serde::{Deserialize, Serialize};

use crate::nearby::DEFAULT_NEARBY_RADIUS;

/// Canned observation detail levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationLevel {
    /// 1 — runtime state only (map, position, party, bag, badges).
    RuntimeState,
    /// 2 — + dialogue and battle detail.
    Interaction,
    /// 3 — + nearby entities (full symbolic observation; the default).
    FullSymbolic,
    /// 4 — + global world data allowance (`allow_world_data` flag only
    /// until the world model lands in M3).
    WorldModel,
}

impl ObservationLevel {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::RuntimeState => 1,
            Self::Interaction => 2,
            Self::FullSymbolic => 3,
            Self::WorldModel => 4,
        }
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::RuntimeState),
            2 => Some(Self::Interaction),
            3 => Some(Self::FullSymbolic),
            4 => Some(Self::WorldModel),
            _ => None,
        }
    }
}

/// Which snapshot sections to populate. Construct via
/// [`ObservationProfile::for_level`] and flip individual toggles to
/// customize, or deserialize a complete profile from the wire.
///
/// serde fills missing fields from the level-3 default, so a partial
/// wire profile inherits full-symbolic toggles — pass complete profiles
/// or use the plain `level` parameter instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservationProfile {
    pub level: ObservationLevel,
    pub include_party: bool,
    pub include_bag: bool,
    pub include_badges: bool,
    pub include_dialogue: bool,
    pub include_battle: bool,
    pub include_nearby: bool,
    /// Level-4 allowance for attaching global world data once M3 exists;
    /// v1 snapshots carry no world-data sections either way.
    pub allow_world_data: bool,
    /// Radius (step units) for the nearby-entities section.
    pub nearby_radius: i32,
}

impl ObservationProfile {
    pub fn for_level(level: ObservationLevel) -> Self {
        let detail = level.as_u8() >= 2;
        Self {
            level,
            include_party: true,
            include_bag: true,
            include_badges: true,
            include_dialogue: detail,
            include_battle: detail,
            include_nearby: level.as_u8() >= 3,
            allow_world_data: level.as_u8() >= 4,
            nearby_radius: DEFAULT_NEARBY_RADIUS,
        }
    }
}

impl Default for ObservationProfile {
    /// Full symbolic observation (level 3).
    fn default() -> Self {
        Self::for_level(ObservationLevel::FullSymbolic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_round_trip() {
        for value in 1..=4u8 {
            let level = ObservationLevel::from_u8(value).unwrap();
            assert_eq!(level.as_u8(), value);
        }
        assert_eq!(ObservationLevel::from_u8(0), None);
        assert_eq!(ObservationLevel::from_u8(5), None);
    }

    #[test]
    fn level_section_gating() {
        let l1 = ObservationProfile::for_level(ObservationLevel::RuntimeState);
        assert!(l1.include_party && l1.include_bag && l1.include_badges);
        assert!(!l1.include_dialogue && !l1.include_battle && !l1.include_nearby);
        assert!(!l1.allow_world_data);

        let l2 = ObservationProfile::for_level(ObservationLevel::Interaction);
        assert!(l2.include_dialogue && l2.include_battle);
        assert!(!l2.include_nearby);

        let l3 = ObservationProfile::for_level(ObservationLevel::FullSymbolic);
        assert!(l3.include_nearby && !l3.allow_world_data);

        let l4 = ObservationProfile::for_level(ObservationLevel::WorldModel);
        assert!(l4.include_nearby && l4.allow_world_data);
    }

    #[test]
    fn default_is_full_symbolic() {
        assert_eq!(
            ObservationProfile::default(),
            ObservationProfile::for_level(ObservationLevel::FullSymbolic)
        );
    }

    #[test]
    fn level_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&ObservationLevel::FullSymbolic).unwrap(),
            "\"full_symbolic\""
        );
    }

    #[test]
    fn profile_round_trips_through_json() {
        let profile = ObservationProfile::for_level(ObservationLevel::Interaction);
        let json = serde_json::to_value(&profile).unwrap();
        assert_eq!(json["level"], "interaction");
        let back: ObservationProfile = serde_json::from_value(json).unwrap();
        assert_eq!(back, profile);
    }
}
