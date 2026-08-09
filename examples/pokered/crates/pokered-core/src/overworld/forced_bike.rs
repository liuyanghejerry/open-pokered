//! Cycling Road forced-bike state — `wStatusFlags6` `BIT_ALWAYS_ON_BIKE`.
//!
//! Gen-1 references:
//! - engine/overworld/player_state.asm — `CheckForceBikeOrSurf` (34-82) runs
//!   from `EnterMap` (home/overworld.asm:31) on every map entry and locks the
//!   player onto the bike when they stand on one of the `ForcedBikeOrSurfMaps`
//!   tiles (data/maps/force_bike_surf.asm).
//! - scripts/Route16Gate1F.asm:1-3 and scripts/Route18Gate1F.asm:1-3 — both
//!   gate scripts begin with `res BIT_ALWAYS_ON_BIKE`: entering either gate
//!   releases the lock. The gate maps use the GATE tileset, which is not in
//!   `BikeRidingTilesets` (data/tilesets/bike_riding_tilesets.asm), so
//!   `LoadPlayerSpriteGraphics` (home/overworld.asm:804-830) restores the
//!   walking state — the player auto-dismounts at the gates.
//! - home/overworld.asm:783-793 (`HandleFlyWarpOrDungeonWarp`) and
//!   home/text_script.asm:200-202 / engine/battle/core.asm:1160-1162
//!   (blackout) clear the bit and reset the walk/bike state.
//! - engine/menus/start_sub_menus.asm:374-379 — the BICYCLE item is refused
//!   with "You can't get off here." while the bit is set; engine/overworld/
//!   field_move_messages.asm:27-29,42-46 — SURF is refused with the
//!   "Cycling is fun!" text (`IsSurfingAllowed`).

use pokered_data::maps::MapId;

/// `ForcedBikeOrSurfMaps` (data/maps/force_bike_surf.asm) — the Cycling Road
/// tiles (map, x, y) that lock the player onto the bike: the road ends on
/// Route 16 (west of the gate) and Route 18 (east of the gate).
pub const FORCED_BIKE_TILES: &[(MapId, u8, u8)] = &[
    (MapId::Route16, 17, 10),
    (MapId::Route16, 17, 11),
    (MapId::Route18, 33, 8),
    (MapId::Route18, 33, 9),
];

/// The Seafoam Islands entries of `ForcedBikeOrSurfMaps`
/// (data/maps/force_bike_surf.asm:10-13): the B3F/B4F strong-current tiles.
/// Standing on one forces SURF (`wWalkBikeSurfState = 2`,
/// player_state.asm:78-82) — the player arrives there without having surfed
/// (e.g. falling through a floor hole into the water) and the current sweeps
/// them along (the MOVE_OBJECT map scripts).
pub const FORCED_SURF_TILES: &[(MapId, u8, u8)] = &[
    (MapId::SeafoamIslandsB3F, 18, 7),
    (MapId::SeafoamIslandsB3F, 19, 7),
    (MapId::SeafoamIslandsB4F, 4, 14),
    (MapId::SeafoamIslandsB4F, 5, 14),
];

/// What a map entry did to the player's transport state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForcedBikeMapEntry {
    /// Nothing to change (not a forced tile, or already on the bike).
    Keep,
    /// The player stepped onto a Cycling Road tile: mount the bike.
    Mount,
    /// The player arrived on a Seafoam Islands current tile: force the surf
    /// state (`wWalkBikeSurfState = 2`).
    ForceSurf,
    /// The player entered a gate (or left by FLY/DIG/TELEPORT/blackout):
    /// restore walking.
    Dismount,
}

/// `wStatusFlags6` `BIT_ALWAYS_ON_BIKE` — locked onto the BICYCLE while on
/// the Cycling Road (Route 16/17/18). While active the player cannot get off
/// the bike and cannot SURF; the lock survives connection walks (the whole
/// road keeps you on the bike) and is released by the gates, FLY/DIG/TELEPORT
/// and blackout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ForcedBikeState {
    /// Whether `BIT_ALWAYS_ON_BIKE` is currently set.
    pub active: bool,
}

impl ForcedBikeState {
    /// `CheckForceBikeOrSurf` on map entry: if the bit is already set it
    /// stays (`ret nz`, player_state.asm:36-37); otherwise standing on a
    /// `ForcedBikeOrSurfMaps` tile sets it, `wWalkBikeSurfState = 1` and the
    /// bike sprite is loaded (`ForceBikeOrSurf`) — except on the Seafoam
    /// Islands tiles, which force the surf state (`wWalkBikeSurfState = 2`,
    /// player_state.asm:78-82) without setting the bike bit. Entering either
    /// gate map clears the bit (the gate scripts' `res BIT_ALWAYS_ON_BIKE`).
    pub fn enter_map(&mut self, map: MapId, x: u16, y: u16) -> ForcedBikeMapEntry {
        if matches!(map, MapId::Route16Gate1F | MapId::Route18Gate1F) {
            self.active = false;
            return ForcedBikeMapEntry::Dismount;
        }
        if self.active {
            return ForcedBikeMapEntry::Keep;
        }
        if FORCED_SURF_TILES.contains(&(map, x as u8, y as u8)) {
            return ForcedBikeMapEntry::ForceSurf;
        }
        if FORCED_BIKE_TILES.contains(&(map, x as u8, y as u8)) {
            self.active = true;
            return ForcedBikeMapEntry::Mount;
        }
        ForcedBikeMapEntry::Keep
    }

    /// Clear the bit (and restore walking) — `HandleFlyWarpOrDungeonWarp`
    /// (home/overworld.asm:791-793) and blackout
    /// (home/text_script.asm:200-202, engine/battle/core.asm:1160-1162).
    pub fn clear(&mut self) -> ForcedBikeMapEntry {
        self.active = false;
        ForcedBikeMapEntry::Dismount
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forced_tiles_mount_the_bike() {
        for (map, x, y) in FORCED_BIKE_TILES {
            let mut state = ForcedBikeState::default();
            assert_eq!(state.enter_map(*map, *x as u16, *y as u16), ForcedBikeMapEntry::Mount);
            assert!(state.active, "{map:?} ({x},{y}) sets the bit");
        }
    }

    #[test]
    fn seafoam_tiles_force_surf_state() {
        // CheckForceBikeOrSurf (player_state.asm:57-82): the Seafoam entries
        // set wWalkBikeSurfState = 2 — surf — WITHOUT setting
        // BIT_ALWAYS_ON_BIKE.
        for (map, x, y) in FORCED_SURF_TILES {
            let mut state = ForcedBikeState::default();
            assert_eq!(
                state.enter_map(*map, *x as u16, *y as u16),
                ForcedBikeMapEntry::ForceSurf,
                "{map:?} ({x},{y}) forces surf"
            );
            assert!(!state.active, "{map:?} ({x},{y}) does not set the bike bit");
        }
    }

    #[test]
    fn off_road_tile_leaves_walking() {
        let mut state = ForcedBikeState::default();
        // The other Route 16 gate door (24,10) is NOT a forced tile per asm.
        assert_eq!(
            state.enter_map(MapId::Route16, 24, 10),
            ForcedBikeMapEntry::Keep
        );
        assert!(!state.active);
        assert_eq!(
            state.enter_map(MapId::PalletTown, 5, 5),
            ForcedBikeMapEntry::Keep
        );
    }

    #[test]
    fn already_forced_keeps_throughout_the_road() {
        let mut state = ForcedBikeState::default();
        state.enter_map(MapId::Route16, 17, 10);
        // The whole Route 17 (and the rest of 16/18) keeps the lock: the asm
        // returns early while the bit is set, so any tile is a Keep.
        assert_eq!(
            state.enter_map(MapId::Route17, 3, 40),
            ForcedBikeMapEntry::Keep
        );
        assert!(state.active);
        assert_eq!(
            state.enter_map(MapId::Route18, 10, 4),
            ForcedBikeMapEntry::Keep
        );
        assert!(state.active);
    }

    #[test]
    fn entering_a_gate_releases_the_lock() {
        let mut state = ForcedBikeState::default();
        state.enter_map(MapId::Route16, 17, 10);
        assert!(state.active);
        // Route16Gate1F_Script / Route18Gate1F_Script start with
        // `res BIT_ALWAYS_ON_BIKE`.
        assert_eq!(
            state.enter_map(MapId::Route16Gate1F, 7, 8),
            ForcedBikeMapEntry::Dismount
        );
        assert!(!state.active);
        assert_eq!(
            state.enter_map(MapId::Route18Gate1F, 0, 4),
            ForcedBikeMapEntry::Dismount
        );
        assert!(!state.active);
    }

    #[test]
    fn clear_releases_without_a_map_change() {
        let mut state = ForcedBikeState::default();
        state.enter_map(MapId::Route16, 17, 10);
        assert_eq!(state.clear(), ForcedBikeMapEntry::Dismount);
        assert!(!state.active);
        // The lock stays released on subsequent entries.
        assert_eq!(
            state.enter_map(MapId::Route16, 17, 10),
            ForcedBikeMapEntry::Mount
        );
    }
}
