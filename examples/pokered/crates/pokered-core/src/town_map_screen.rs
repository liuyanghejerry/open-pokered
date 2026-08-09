//! Town Map viewer screen state machine.
//!
//! Opened from the bag's TOWN MAP item. Read-only: shows the Kanto map with a
//! flashing "you are here" marker at the player's current location, and lets the
//! player scroll a cursor through the fly-destination landmarks to read each
//! one's name. Pure logic (no rendering) — mirrors `bag_screen::BagScreenState`.
//!
//! FLY mode (party-menu FLY, `LoadTownMap_Fly` in engine/items/town_map.asm):
//! the cursor walks the *visited-city* list and A picks a warp destination.

use pokered_data::maps::MapId;
use pokered_data::town_map_data::TOWN_MAP_ORDER;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TownMapScreenInput {
    pub up: bool,
    pub down: bool,
    pub a: bool,
    pub b: bool,
}

impl TownMapScreenInput {
    pub fn none() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TownMapScreenAction {
    /// Still open.
    Active,
    /// Player closed the map (return to the overworld — or, after cancelling
    /// a FLY pick, back to the party menu per the original flow).
    Closed,
    /// FLY mode: the player chose a destination (A button). The caller warps
    /// there via the map's fly point.
    FlyTo(MapId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TownMapMode {
    /// Read-only Kanto map (bag TOWN MAP item).
    View,
    /// FLY destination picker (party-menu FLY).
    Fly,
}

#[derive(Debug, Clone)]
pub struct TownMapScreenState {
    /// The player's current map — drawn with the flashing "you are here" marker.
    current_map: MapId,
    /// Cursor index into [`TOWN_MAP_ORDER`] (View) or `fly_destinations` (Fly).
    cursor: usize,
    mode: TownMapMode,
    /// FLY mode: visited city maps in map-ID order (BuildFlyLocationsList).
    fly_destinations: Vec<MapId>,
}

impl TownMapScreenState {
    pub fn new(current_map: MapId) -> Self {
        // Start the browse cursor on the player's own landmark when it appears in
        // the fly-order list, otherwise at the top.
        let cursor = TOWN_MAP_ORDER
            .iter()
            .position(|&m| m == current_map)
            .unwrap_or(0);
        Self {
            current_map,
            cursor,
            mode: TownMapMode::View,
            fly_destinations: Vec::new(),
        }
    }

    /// Open in FLY mode over the visited-city list. The cursor starts on the
    /// first entry (the original starts at the head of wFlyLocationsList).
    pub fn new_fly(current_map: MapId, fly_destinations: Vec<MapId>) -> Self {
        Self {
            current_map,
            cursor: 0,
            mode: TownMapMode::Fly,
            fly_destinations,
        }
    }

    pub fn mode(&self) -> TownMapMode {
        self.mode
    }

    pub fn current_map(&self) -> MapId {
        self.current_map
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The landmark the browse cursor currently points at.
    pub fn selected_map(&self) -> MapId {
        match self.mode {
            TownMapMode::View => TOWN_MAP_ORDER[self.cursor],
            TownMapMode::Fly => self
                .fly_destinations
                .get(self.cursor)
                .copied()
                .unwrap_or(self.current_map),
        }
    }

    /// Advance one frame. `input` booleans are edge-detected (just-pressed) by
    /// the caller, matching the `BagScreenInput` convention.
    pub fn update_frame(&mut self, input: TownMapScreenInput) -> TownMapScreenAction {
        match self.mode {
            TownMapMode::View => self.update_view(input),
            TownMapMode::Fly => self.update_fly(input),
        }
    }

    fn update_view(&mut self, input: TownMapScreenInput) -> TownMapScreenAction {
        if input.a || input.b {
            return TownMapScreenAction::Closed;
        }
        if input.up && self.cursor > 0 {
            self.cursor -= 1;
        }
        if input.down && self.cursor + 1 < TOWN_MAP_ORDER.len() {
            self.cursor += 1;
        }
        TownMapScreenAction::Active
    }

    /// LoadTownMap_Fly input loop: UP moves to the next entry in the fly list,
    /// DOWN to the previous, both wrapping (the original increments the list
    /// pointer on UP — the list runs south-to-north through the cities).
    /// A picks the destination; B cancels back to the party menu.
    fn update_fly(&mut self, input: TownMapScreenInput) -> TownMapScreenAction {
        if input.b {
            return TownMapScreenAction::Closed;
        }
        if input.a {
            return match self.fly_destinations.get(self.cursor) {
                Some(&dest) => TownMapScreenAction::FlyTo(dest),
                None => TownMapScreenAction::Closed,
            };
        }
        let count = self.fly_destinations.len();
        if count == 0 {
            return TownMapScreenAction::Active;
        }
        if input.up {
            self.cursor = (self.cursor + 1) % count;
        }
        if input.down {
            self.cursor = (self.cursor + count - 1) % count;
        }
        TownMapScreenAction::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_cursor_on_current_landmark() {
        let s = TownMapScreenState::new(MapId::CeladonCity);
        assert_eq!(s.selected_map(), MapId::CeladonCity);
    }

    #[test]
    fn new_falls_back_to_top_for_unlisted_map() {
        // An interior map not in the fly order → cursor 0 (Pallet Town).
        let s = TownMapScreenState::new(MapId::RedsHouse1F);
        assert_eq!(s.cursor(), 0);
        assert_eq!(s.selected_map(), TOWN_MAP_ORDER[0]);
    }

    #[test]
    fn up_down_scroll_within_bounds() {
        let mut s = TownMapScreenState::new(TOWN_MAP_ORDER[0]);
        assert_eq!(s.cursor(), 0);
        // Can't scroll above the top.
        s.update_frame(TownMapScreenInput {
            up: true,
            ..Default::default()
        });
        assert_eq!(s.cursor(), 0);
        // Down moves one.
        s.update_frame(TownMapScreenInput {
            down: true,
            ..Default::default()
        });
        assert_eq!(s.cursor(), 1);
        // Can't scroll past the end.
        let mut end = TownMapScreenState::new(TOWN_MAP_ORDER[TOWN_MAP_ORDER.len() - 1]);
        let last = end.cursor();
        end.update_frame(TownMapScreenInput {
            down: true,
            ..Default::default()
        });
        assert_eq!(end.cursor(), last);
    }

    #[test]
    fn a_or_b_closes() {
        let mut s = TownMapScreenState::new(MapId::PalletTown);
        assert_eq!(
            s.update_frame(TownMapScreenInput {
                b: true,
                ..Default::default()
            }),
            TownMapScreenAction::Closed
        );
        assert_eq!(
            s.update_frame(TownMapScreenInput {
                a: true,
                ..Default::default()
            }),
            TownMapScreenAction::Closed
        );
    }

    // ── FLY mode (LoadTownMap_Fly) ───────────────────────────────────

    fn fly_screen() -> TownMapScreenState {
        TownMapScreenState::new_fly(
            MapId::Route1,
            vec![
                MapId::PalletTown,
                MapId::ViridianCity,
                MapId::CeruleanCity,
            ],
        )
    }

    #[test]
    fn fly_mode_starts_on_first_visited_city() {
        let s = fly_screen();
        assert_eq!(s.mode(), TownMapMode::Fly);
        assert_eq!(s.cursor(), 0);
        assert_eq!(s.selected_map(), MapId::PalletTown);
    }

    #[test]
    fn fly_mode_up_moves_forward_with_wrap() {
        // The original increments the fly-list pointer on UP (the list runs
        // south-to-north), wrapping at the end.
        let mut s = fly_screen();
        s.update_frame(TownMapScreenInput {
            up: true,
            ..Default::default()
        });
        assert_eq!(s.selected_map(), MapId::ViridianCity);
        s.update_frame(TownMapScreenInput {
            up: true,
            ..Default::default()
        });
        assert_eq!(s.selected_map(), MapId::CeruleanCity);
        s.update_frame(TownMapScreenInput {
            up: true,
            ..Default::default()
        });
        assert_eq!(s.selected_map(), MapId::PalletTown, "wraps to the top");
    }

    #[test]
    fn fly_mode_down_moves_backward_with_wrap() {
        let mut s = fly_screen();
        s.update_frame(TownMapScreenInput {
            down: true,
            ..Default::default()
        });
        assert_eq!(s.selected_map(), MapId::CeruleanCity, "wraps to the end");
    }

    #[test]
    fn fly_mode_a_chooses_destination() {
        let mut s = fly_screen();
        s.update_frame(TownMapScreenInput {
            up: true,
            ..Default::default()
        });
        assert_eq!(
            s.update_frame(TownMapScreenInput {
                a: true,
                ..Default::default()
            }),
            TownMapScreenAction::FlyTo(MapId::ViridianCity)
        );
    }

    #[test]
    fn fly_mode_b_cancels() {
        let mut s = fly_screen();
        assert_eq!(
            s.update_frame(TownMapScreenInput {
                b: true,
                ..Default::default()
            }),
            TownMapScreenAction::Closed
        );
    }

    #[test]
    fn fly_mode_empty_list_a_closes() {
        let mut s = TownMapScreenState::new_fly(MapId::Route1, vec![]);
        assert_eq!(
            s.update_frame(TownMapScreenInput {
                a: true,
                ..Default::default()
            }),
            TownMapScreenAction::Closed
        );
    }
}
