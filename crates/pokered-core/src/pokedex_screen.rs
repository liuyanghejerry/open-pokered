//! Pokédex list + entry screen state machine.
//!
//! Replicates `engine/menus/pokedex.asm` (`ShowPokedexMenu` /
//! `HandlePokedexListMenu` / `HandlePokedexSideMenu` /
//! `ShowPokedexDataInternal`):
//! - Numbered list (1..=`max_seen`, the highest *seen* dex number —
//!   `wDexMaxSeenMon`), 7 visible rows, scroll window.
//! - Owned Pokémon get a Pokéball mark; seen-but-not-owned show their name;
//!   unseen show a dashed line ("----------").
//! - UP/DOWN move one row, LEFT/RIGHT jump 7 rows (the original scrolls
//!   `wListScrollOffset` by 1/7; the cursor-and-window model here produces
//!   the same visible result).
//! - A on a *seen* Pokémon opens the DATA/CRY/AREA/QUIT side menu
//!   (`HandlePokedexSideMenu`); A on an unseen Pokémon does nothing (the
//!   original's side menu exits immediately for unseen entries).
//! - Side menu: DATA opens the entry (sprite + cry + name/number; HT/WT and
//!   the description print only when OWNED — the original shows
//!   "HT  ?′??″ / WT   ???lb" and skips the flavor text otherwise); CRY plays
//!   the species' cry and stays in the menu (`GetCryData`+`PlaySound`, then
//!   `jr .handleMenuInput`); AREA opens the habitat page
//!   (`LoadTownMap_Nest`); QUIT closes the Pokédex; B returns to the list.
//! - After DATA/AREA the list is redrawn at the same cursor (the side menu
//!   restores `wCurrentMenuItem`/`wListScrollOffset` on exit, then
//!   `.setUpGraphics` re-runs the list menu).
//! - B in the list exits; B in the entry view returns to the list; B in the
//!   AREA page returns to the list (the original's `WaitForTextScrollButtonPress`
//!   accepts both buttons).
//!
//! A second entry point, [`PokedexScreenState::new_entry`], opens directly on
//! the entry view — the post-capture "New DEX data will be added…" flow
//! (`ShowPokedexData` in engine/items/item_effects.asm:546). The original
//! shows the entry *without* a side menu there (the side menu only exists in
//! the `ShowPokedexMenu` list flow).

use crate::alloc_prelude::*;
use crate::pokemon::pokedex::Pokedex;
use pokered_data::maps::MapId;
use pokered_data::species::Species;
use pokered_data::wild_data::{area_locations, GameVersion};

/// Visible list rows (`HandlePokedexListMenu` prints 7).
pub const LIST_ROWS: u16 = 7;

/// The side menu's DATA/CRY/AREA/QUIT options (`PokedexMenuItemsText`,
/// engine/menus/pokedex.asm:371-375).
pub const SIDE_MENU_OPTIONS: usize = 4;

/// Reflow the complete Chinese description. Source page boundaries were
/// inherited from English and can split a Chinese word or clause.
pub fn chinese_description_lines(entry: &pokered_data::pokedex::PokedexEntry) -> Vec<String> {
    let text = entry.flavor_text_pages_zh.concat();
    crate::text::zh_dialogue::description_pages(&text, 144, 3, &[])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PokedexScreenInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
}

impl PokedexScreenInput {
    pub fn none() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokedexScreenAction {
    /// Still open.
    Active,
    /// Leave the Pokédex entirely (back to the start menu, or — after a
    /// post-capture entry — back to the overworld).
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokedexScreenMode {
    /// The scrolling CONTENTS list.
    List,
    /// The DATA/CRY/AREA/QUIT menu at (15,10) (`HandlePokedexSideMenu`).
    SideMenu,
    /// A single Pokémon's entry (data page).
    Entry,
    /// The habitat page (`LoadTownMap_Nest`).
    Area,
}

#[derive(Debug, Clone)]
pub struct PokedexScreenState {
    pub language: crate::game_state::Lang,
    dex: Pokedex,
    /// Highest seen dex number (`wDexMaxSeenMon`); the list ends here.
    max_seen: u16,
    /// Currently selected dex number (1..=`max_seen`).
    cursor: u16,
    /// First visible row is `scroll + 1` (`wListScrollOffset`).
    scroll: u16,
    mode: PokedexScreenMode,
    /// Side-menu item (0=DATA, 1=CRY, 2=AREA, 3=QUIT).
    side_menu_cursor: u8,
    /// Description page within the entry view.
    entry_page: usize,
    /// True when reached from the list (entry-B returns to the list); false
    /// for the post-capture standalone entry (any close exits the screen).
    from_list: bool,
    /// Set when the entry view opens or the side-menu CRY item is chosen —
    /// the frontend plays the species' cry once (`PlayCry` in
    /// `ShowPokedexDataInternal`, `GetCryData`+`PlaySound` in the side menu).
    cry_pending: bool,
    /// Game version — selects the wild-encounter tables for the AREA page
    /// (`FindWildLocationsOfMon` reads the ROM's own tables).
    version: GameVersion,
}

impl PokedexScreenState {
    /// Open the CONTENTS list (start-menu POKéDEX).
    pub fn new(dex: Pokedex, version: GameVersion) -> Self {
        let max_seen = Self::compute_max_seen(&dex);
        Self {
            language: crate::game_state::Lang::En,
            dex,
            max_seen,
            cursor: 1,
            scroll: 0,
            mode: PokedexScreenMode::List,
            side_menu_cursor: 0,
            entry_page: 0,
            from_list: true,
            cry_pending: false,
            version,
        }
    }

    /// Open directly on a species' entry — the post-capture flow. In the
    /// original the species was just added to the owned flags, so the entry
    /// always shows full data.
    pub fn new_entry(dex: Pokedex, species: Species, version: GameVersion) -> Self {
        let max_seen = Self::compute_max_seen(&dex);
        let cursor = (species as u16).clamp(1, max_seen);
        Self {
            language: crate::game_state::Lang::En,
            dex,
            max_seen,
            cursor,
            scroll: 0,
            mode: PokedexScreenMode::Entry,
            side_menu_cursor: 0,
            entry_page: 0,
            from_list: false,
            cry_pending: true,
            version,
        }
    }

    /// `wDexMaxSeenMon`: highest dex number whose seen bit is set (≥1 so an
    /// empty dex still shows row 1 as "----------").
    fn compute_max_seen(dex: &Pokedex) -> u16 {
        let mut max = 1u16;
        for n in 1..=(crate::pokemon::pokedex::NUM_POKEMON as u16) {
            if dex.is_seen(Species::from_index_id(n as u8)) {
                max = n;
            }
        }
        max
    }

    pub fn mode(&self) -> PokedexScreenMode {
        self.mode
    }

    pub fn cursor(&self) -> u16 {
        self.cursor
    }

    /// Selected side-menu item (0=DATA, 1=CRY, 2=AREA, 3=QUIT).
    pub fn side_menu_cursor(&self) -> u8 {
        self.side_menu_cursor
    }

    pub fn scroll_offset(&self) -> u16 {
        self.scroll
    }

    pub fn max_seen(&self) -> u16 {
        self.max_seen
    }

    pub fn from_list(&self) -> bool {
        self.from_list
    }

    pub fn entry_page(&self) -> usize {
        self.entry_page
    }

    pub fn version(&self) -> GameVersion {
        self.version
    }

    pub fn seen_count(&self) -> u32 {
        self.dex.seen_count()
    }

    pub fn owned_count(&self) -> u32 {
        self.dex.owned_count()
    }

    /// Is dex number `n` (1-based) marked seen?
    pub fn is_seen(&self, n: u16) -> bool {
        self.dex.is_seen(Species::from_index_id(n as u8))
    }

    /// Is dex number `n` (1-based) marked owned?
    pub fn is_owned(&self, n: u16) -> bool {
        self.dex.is_owned(Species::from_index_id(n as u8))
    }

    /// The species the cursor currently points at.
    pub fn cursor_species(&self) -> Species {
        Species::from_index_id(self.cursor as u8)
    }

    /// Maps where the cursor's species can be found in the wild
    /// (`FindWildLocationsOfMon` + the Cerulean Cave exclusion in
    /// `DisplayWildLocations`); empty → the AREA page prints "AREA UNKNOWN".
    pub fn area_maps(&self) -> Vec<MapId> {
        area_locations(self.cursor_species(), self.version)
    }

    /// Frontend hook: take the pending "play this species' cry" request.
    pub fn take_cry_pending(&mut self) -> bool {
        let p = self.cry_pending;
        self.cry_pending = false;
        p
    }

    /// Description page count for the entry under the cursor. Seen-but-not-
    /// owned entries print no flavor text (`ShowPokedexDataInternal` skips it),
    /// so they have a single page.
    pub fn entry_total_pages(&self) -> usize {
        if !self.is_owned(self.cursor) {
            return 1;
        }
        pokered_data::pokedex::get_pokedex_entry(self.cursor_species())
            .map(|e| if self.language == crate::game_state::Lang::Zh {
                (chinese_description_lines(e).len() / 3).max(1)
            } else { e.flavor_text_pages.len().max(1) })
            .unwrap_or(1)
    }

    /// Advance one frame. `input` booleans are edge-detected (just-pressed),
    /// matching the `TownMapScreenInput` convention.
    pub fn update_frame(&mut self, input: PokedexScreenInput) -> PokedexScreenAction {
        match self.mode {
            PokedexScreenMode::List => self.update_list(input),
            PokedexScreenMode::SideMenu => self.update_side_menu(input),
            PokedexScreenMode::Entry => self.update_entry(input),
            PokedexScreenMode::Area => self.update_area(input),
        }
    }

    fn update_list(&mut self, input: PokedexScreenInput) -> PokedexScreenAction {
        if input.b {
            return PokedexScreenAction::Closed;
        }
        // HandleMenuInput moves within the visible rows first; UP wins over
        // DOWN. A boundary attempt or a watched A/LEFT/RIGHT key then returns
        // to HandlePokedexListMenu, whose own direction priority comes first.
        let visible_rows = (self.max_seen - self.scroll).min(LIST_ROWS);
        let mut row = self.cursor - 1 - self.scroll;
        let moving_out = if input.up {
            row == 0
        } else {
            input.down && row + 1 >= visible_rows
        };
        if input.up {
            row = row.saturating_sub(1);
        } else if input.down {
            row = (row + 1).min(visible_rows.saturating_sub(1));
        }
        let watched = input.a || input.left || input.right;
        if moving_out || watched {
            let max_scroll = self.max_seen.saturating_sub(LIST_ROWS);
            if input.up {
                self.scroll = self.scroll.saturating_sub(1);
            } else if input.down {
                self.scroll = (self.scroll + 1).min(max_scroll);
            } else if input.right {
                self.scroll = (self.scroll + LIST_ROWS).min(max_scroll);
            } else if input.left {
                self.scroll = self.scroll.saturating_sub(LIST_ROWS);
            } else if input.a && self.is_seen(self.cursor) {
                // Unseen entries return to the list without opening DATA.
                self.mode = PokedexScreenMode::SideMenu;
                self.side_menu_cursor = 0;
            }
        }
        self.cursor = self.scroll + row + 1;
        self.clamp_scroll();
        PokedexScreenAction::Active
    }

    /// `HandlePokedexSideMenu`: menu at (15,10), items DATA/CRY/AREA/QUIT.
    /// A on DATA → entry; A on CRY → play the cry, stay in the menu; A on
    /// AREA → habitat page; A on QUIT → close the Pokédex; B → back to list.
    fn update_side_menu(&mut self, input: PokedexScreenInput) -> PokedexScreenAction {
        if input.b {
            self.mode = PokedexScreenMode::List;
            return PokedexScreenAction::Active;
        }
        // HandleMenuInput tests UP before DOWN even when UP cannot move.
        if input.up {
            self.side_menu_cursor = self.side_menu_cursor.saturating_sub(1);
        } else if input.down {
            self.side_menu_cursor = (self.side_menu_cursor + 1).min(SIDE_MENU_OPTIONS as u8 - 1);
        }
        if input.a {
            match self.side_menu_cursor {
                0 => {
                    // .choseData → ShowPokedexDataInternal (plays the cry).
                    self.mode = PokedexScreenMode::Entry;
                    self.entry_page = 0;
                    self.cry_pending = true;
                }
                1 => {
                    // .choseCry → GetCryData + PlaySound, then loop back to
                    // the menu input.
                    self.cry_pending = true;
                }
                2 => {
                    // .choseArea → predef LoadTownMap_Nest.
                    self.mode = PokedexScreenMode::Area;
                }
                _ => {
                    // QUIT → b=1 → .exitPokedex.
                    return PokedexScreenAction::Closed;
                }
            }
        }
        PokedexScreenAction::Active
    }

    /// The habitat page closes on A or B (`WaitForTextScrollButtonPress`),
    /// returning to the list (the side menu exits with b=0 → `.setUpGraphics`).
    fn update_area(&mut self, input: PokedexScreenInput) -> PokedexScreenAction {
        if input.a || input.b {
            self.mode = PokedexScreenMode::List;
        }
        PokedexScreenAction::Active
    }

    fn update_entry(&mut self, input: PokedexScreenInput) -> PokedexScreenAction {
        let close = input.b
            || (input.a && self.entry_page + 1 >= self.entry_total_pages());
        if close {
            if self.from_list {
                self.mode = PokedexScreenMode::List;
                return PokedexScreenAction::Active;
            }
            return PokedexScreenAction::Closed;
        }
        if input.a {
            self.entry_page += 1;
        }
        PokedexScreenAction::Active
    }

    /// Keep the cursor inside the 7-row window; clamp the window to the list.
    fn clamp_scroll(&mut self) {
        let max_scroll = self.max_seen.saturating_sub(LIST_ROWS);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
        if self.cursor <= self.scroll {
            self.scroll = self.cursor - 1;
        }
        if self.cursor > self.scroll + LIST_ROWS {
            self.scroll = self.cursor - LIST_ROWS;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dex_with(seen: &[u8], owned: &[u8]) -> Pokedex {
        let mut dex = Pokedex::new();
        for &n in seen {
            dex.set_seen(Species::from_index_id(n));
        }
        for &n in owned {
            dex.set_owned(Species::from_index_id(n));
        }
        dex
    }

    fn screen(dex: Pokedex) -> PokedexScreenState {
        PokedexScreenState::new(dex, GameVersion::Red)
    }

    fn press(input: PokedexScreenInput) -> PokedexScreenInput {
        input
    }

    fn a() -> PokedexScreenInput {
        PokedexScreenInput {
            a: true,
            ..Default::default()
        }
    }

    fn b() -> PokedexScreenInput {
        PokedexScreenInput {
            b: true,
            ..Default::default()
        }
    }

    /// Open the side menu for the current cursor's species, then pick DATA.
    fn open_entry(s: &mut PokedexScreenState) {
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::SideMenu);
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::Entry);
    }

    #[test]
    fn list_mixed_keys_follow_original_inner_menu_and_outer_caller_priority() {
        // Matched original emulator controls (audit285) exercise both levels:
        // HandleMenuInput moves a row, then the caller handles watched keys.
        // DOWN+A therefore moves one row and scrolls once, without opening DATA.
        let cases = [
            ("down", PokedexScreenInput { down: true, ..Default::default() }, (2, 0, PokedexScreenMode::List)),
            ("down-a", PokedexScreenInput { down: true, a: true, ..Default::default() }, (3, 1, PokedexScreenMode::List)),
            ("up-right", PokedexScreenInput { up: true, right: true, ..Default::default() }, (1, 0, PokedexScreenMode::List)),
            ("down-right", PokedexScreenInput { down: true, right: true, ..Default::default() }, (3, 1, PokedexScreenMode::List)),
            ("right", PokedexScreenInput { right: true, ..Default::default() }, (8, 7, PokedexScreenMode::List)),
            ("a-right", PokedexScreenInput { a: true, right: true, ..Default::default() }, (8, 7, PokedexScreenMode::List)),
            ("a", PokedexScreenInput { a: true, ..Default::default() }, (1, 0, PokedexScreenMode::SideMenu)),
            ("left-a", PokedexScreenInput { left: true, a: true, ..Default::default() }, (1, 0, PokedexScreenMode::List)),
        ];
        let seen: Vec<u8> = (1..=25).collect();
        let mut actual = Vec::new();
        let mut expected = Vec::new();
        for (name, input, endpoint) in cases {
            let mut state = screen(dex_with(&seen, &[]));
            state.update_frame(input);
            let result = (state.cursor(), state.scroll_offset(), state.mode());
            if matches!(name, "down" | "right" | "a") {
                assert_eq!(result, endpoint, "single-key control {name}");
            }
            actual.push((name, result));
            expected.push((name, endpoint));
        }
        assert_eq!(actual, expected, "original paired two-level menu controls");
    }

    #[test]
    fn opposite_vertical_keys_keep_the_original_up_priority() {
        let seen: Vec<u8> = (1..=25).collect();
        let mut endpoints = Vec::new();
        for initial_down in [0, 3] {
            let mut state = screen(dex_with(&seen, &[]));
            for _ in 0..initial_down {
                state.update_frame(PokedexScreenInput { down: true, ..Default::default() });
            }
            state.update_frame(PokedexScreenInput { up: true, down: true, ..Default::default() });
            endpoints.push((state.cursor(), state.scroll_offset(), state.mode()));
        }
        assert_eq!(endpoints, vec![(1, 0, PokedexScreenMode::List), (3, 0, PokedexScreenMode::List)]);
    }

    #[test]
    fn down_with_a_keeps_a_short_list_without_opening_the_side_menu() {
        let mut state = screen(dex_with(&[1, 2, 3, 4, 5, 6], &[]));
        state.update_frame(PokedexScreenInput { down: true, a: true, ..Default::default() });
        assert_eq!((state.cursor(), state.scroll_offset(), state.mode()), (2, 0, PokedexScreenMode::List));
    }

    #[test]
    fn side_menu_opposite_vertical_keys_use_up_priority_at_top_and_inside() {
        let seen: Vec<u8> = (1..=25).collect();
        let mut endpoints = Vec::new();
        for initial_down in [0, 2] {
            let mut state = screen(dex_with(&seen, &[]));
            state.update_frame(a());
            for _ in 0..initial_down {
                state.update_frame(PokedexScreenInput { down: true, ..Default::default() });
            }
            state.update_frame(PokedexScreenInput { up: true, down: true, ..Default::default() });
            assert_eq!(state.cursor(), 1, "side navigation preserves selected species");
            assert!(!state.take_cry_pending(), "directions do not trigger a cry");
            endpoints.push((state.side_menu_cursor(), state.mode()));
        }
        assert_eq!(endpoints, vec![(0, PokedexScreenMode::SideMenu), (1, PokedexScreenMode::SideMenu)]);
    }

    #[test]
    fn max_seen_is_highest_seen_dex_number() {
        let s = screen(dex_with(&[1, 4, 7, 25], &[1]));
        assert_eq!(s.max_seen(), 25);
        assert_eq!(s.seen_count(), 4);
        assert_eq!(s.owned_count(), 1);
    }

    #[test]
    fn empty_dex_shows_row_one() {
        let s = screen(Pokedex::new());
        assert_eq!(s.max_seen(), 1);
        assert!(!s.is_seen(1));
    }

    #[test]
    fn up_down_move_cursor_within_bounds() {
        let mut s = screen(dex_with(&[1, 2, 3], &[]));
        s.update_frame(press(PokedexScreenInput {
            up: true,
            ..Default::default()
        }));
        assert_eq!(s.cursor(), 1, "can't move above 1");
        s.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        assert_eq!(s.cursor(), 2);
        s.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        s.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        assert_eq!(s.cursor(), 3, "can't move past max_seen");
    }

    #[test]
    fn left_right_jump_seven_rows_clamped() {
        let dex = dex_with(&(1u8..=20).collect::<Vec<_>>(), &[]);
        let mut s = screen(dex);
        s.update_frame(press(PokedexScreenInput {
            right: true,
            ..Default::default()
        }));
        assert_eq!(s.cursor(), 8);
        s.update_frame(press(PokedexScreenInput {
            right: true,
            ..Default::default()
        }));
        s.update_frame(press(PokedexScreenInput {
            right: true,
            ..Default::default()
        }));
        assert_eq!((s.cursor(), s.scroll_offset()), (14, 13), "first row of final page");
        s.update_frame(press(PokedexScreenInput {
            left: true,
            ..Default::default()
        }));
        assert_eq!((s.cursor(), s.scroll_offset()), (7, 6));
        s.update_frame(press(PokedexScreenInput {
            left: true,
            ..Default::default()
        }));
        s.update_frame(press(PokedexScreenInput {
            left: true,
            ..Default::default()
        }));
        assert_eq!(s.cursor(), 1, "clamped at 1");
    }

    #[test]
    fn horizontal_pages_keep_cursor_row_at_both_bounds() {
        for max_seen in [1u8, 6, 7, 8, 25, 151] {
            for row in [0u16, 3, 6].into_iter().filter(|r| *r < u16::from(max_seen)) {
                let mut s = screen(dex_with(&(1..=max_seen).collect::<Vec<_>>(), &[]));
                for _ in 0..row {
                    s.update_frame(PokedexScreenInput { down: true, ..Default::default() });
                }
                let max_scroll = u16::from(max_seen).saturating_sub(LIST_ROWS);
                for press in 1..=24 {
                    s.update_frame(PokedexScreenInput { right: true, ..Default::default() });
                    let scroll = (press * LIST_ROWS).min(max_scroll);
                    assert_eq!((s.cursor(), s.scroll_offset()), (scroll + row + 1, scroll),
                        "max{max_seen} row{row} right{press}");
                    assert_eq!(s.seen_count(), u32::from(max_seen));
                    assert_eq!(s.owned_count(), 0);
                    assert!(!s.take_cry_pending());
                }
                for press in 1..=24 {
                    s.update_frame(PokedexScreenInput { left: true, ..Default::default() });
                    let scroll = max_scroll.saturating_sub(press * LIST_ROWS);
                    assert_eq!((s.cursor(), s.scroll_offset()), (scroll + row + 1, scroll),
                        "max{max_seen} row{row} left{press}");
                }
            }
        }
    }

    #[test]
    fn horizontal_right_priority_and_data_selection_follow_retained_row() {
        let mut s = screen(dex_with(&(1..=25).collect::<Vec<_>>(), &[]));
        s.update_frame(PokedexScreenInput { left: true, right: true, ..Default::default() });
        assert_eq!((s.cursor(), s.scroll_offset()), (8, 7), "source RIGHT priority");
        for _ in 0..3 {
            s.update_frame(PokedexScreenInput { right: true, ..Default::default() });
        }
        assert_eq!((s.cursor(), s.scroll_offset()), (19, 18));
        open_entry(&mut s);
        assert_eq!(s.cursor_species(), Species::Rattata, "DATA opens retained row, not page-end Pikachu");
        assert!(s.take_cry_pending());
    }

    #[test]
    fn scroll_window_follows_cursor() {
        let dex = dex_with(&(1u8..=20).collect::<Vec<_>>(), &[]);
        let mut s = screen(dex);
        for _ in 0..7 {
            s.update_frame(press(PokedexScreenInput {
                down: true,
                ..Default::default()
            }));
        }
        assert_eq!(s.cursor(), 8);
        assert_eq!(s.scroll_offset(), 1, "row 8 is the last visible row");
        for _ in 0..7 {
            s.update_frame(press(PokedexScreenInput {
                up: true,
                ..Default::default()
            }));
        }
        assert_eq!(s.cursor(), 1);
        assert_eq!(s.scroll_offset(), 0);
    }

    #[test]
    fn a_on_seen_opens_side_menu_then_data_entry_with_cry() {
        let mut s = screen(dex_with(&[1, 2], &[1]));
        assert_eq!(
            s.update_frame(press(a())),
            PokedexScreenAction::Active
        );
        assert_eq!(s.mode(), PokedexScreenMode::SideMenu);
        assert_eq!(s.side_menu_cursor(), 0, "menu opens on DATA");
        assert!(!s.take_cry_pending(), "side menu alone doesn't play a cry");
        // DATA → entry, which plays the cry.
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::Entry);
        assert!(s.take_cry_pending(), "entry opening plays the cry");
        assert!(!s.take_cry_pending(), "cry plays once");
    }

    #[test]
    fn a_on_unseen_does_nothing() {
        // dex has 1,3 seen; cursor 2 is unseen.
        let mut s2 = screen(dex_with(&[1, 3], &[]));
        s2.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        assert_eq!(s2.cursor(), 2);
        assert!(!s2.is_seen(2));
        s2.update_frame(press(a()));
        assert_eq!(s2.mode(), PokedexScreenMode::List, "unseen can't be selected");
        assert!(!s2.take_cry_pending());
    }

    #[test]
    fn b_in_list_closes() {
        let mut s = screen(dex_with(&[1], &[1]));
        assert_eq!(
            s.update_frame(press(b())),
            PokedexScreenAction::Closed
        );
    }

    #[test]
    fn b_in_entry_returns_to_list() {
        let mut s = screen(dex_with(&[1], &[1]));
        open_entry(&mut s);
        assert_eq!(
            s.update_frame(press(b())),
            PokedexScreenAction::Active
        );
        assert_eq!(s.mode(), PokedexScreenMode::List);
    }

    #[test]
    fn entry_a_pages_description_then_returns_to_list() {
        // Bulbasaur (dex 1) has 2 flavor pages in the real data.
        let mut s = screen(dex_with(&[1], &[1]));
        open_entry(&mut s);
        let pages = s.entry_total_pages();
        assert_eq!(pages, 2, "Bulbasaur entry has 2 flavor pages");
        s.update_frame(press(a()));
        assert_eq!(s.entry_page(), 1);
        assert_eq!(s.mode(), PokedexScreenMode::Entry);
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::List, "last page A exits entry");
    }

    #[test]
    fn seen_not_owned_entry_has_single_page() {
        // Seen but not owned: no HT/WT/description in the original → 1 page.
        let mut s = screen(dex_with(&[1], &[]));
        open_entry(&mut s);
        assert_eq!(s.entry_total_pages(), 1);
        // A on the only page exits back to the list.
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::List);
    }

    #[test]
    fn post_catch_entry_closes_to_caller() {
        let s_dex = dex_with(&[25], &[25]);
        let mut s = PokedexScreenState::new_entry(
            s_dex,
            Species::from_index_id(25),
            GameVersion::Red,
        );
        assert_eq!(s.mode(), PokedexScreenMode::Entry);
        assert!(!s.from_list());
        assert!(s.take_cry_pending(), "post-catch entry plays the cry");
        // B closes the whole screen (not back to a list that never opened).
        assert_eq!(
            s.update_frame(press(b())),
            PokedexScreenAction::Closed
        );
    }

    // ── side menu (HandlePokedexSideMenu) ──────────────────────────────

    #[test]
    fn side_menu_cursor_moves_within_four_options() {
        let mut s = screen(dex_with(&[1, 2], &[1]));
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::SideMenu);
        // Down to CRY, AREA, QUIT; can't go past QUIT.
        for expected in [1u8, 2, 3] {
            s.update_frame(press(PokedexScreenInput {
                down: true,
                ..Default::default()
            }));
            assert_eq!(s.side_menu_cursor(), expected);
        }
        s.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        assert_eq!(s.side_menu_cursor(), 3, "clamped at QUIT");
        // Up back to DATA; can't go above it.
        for expected in [2u8, 1, 0] {
            s.update_frame(press(PokedexScreenInput {
                up: true,
                ..Default::default()
            }));
            assert_eq!(s.side_menu_cursor(), expected);
        }
        s.update_frame(press(PokedexScreenInput {
            up: true,
            ..Default::default()
        }));
        assert_eq!(s.side_menu_cursor(), 0, "clamped at DATA");
    }

    #[test]
    fn side_menu_cry_plays_cry_and_stays_in_menu() {
        let mut s = screen(dex_with(&[1, 2], &[1]));
        s.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::SideMenu);
        s.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        assert_eq!(s.side_menu_cursor(), 1); // CRY
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::SideMenu, "CRY stays in the menu");
        assert!(s.take_cry_pending(), "CRY plays the species' cry");
        assert!(!s.take_cry_pending(), "cry fires once");
        // Still usable afterwards — pick AREA next.
        s.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::Area);
    }

    #[test]
    fn side_menu_data_opens_entry_with_cry() {
        let mut s = screen(dex_with(&[1, 2], &[1]));
        s.update_frame(press(a()));
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::Entry);
        assert!(s.take_cry_pending());
    }

    #[test]
    fn side_menu_area_opens_area_page_and_returns_to_list() {
        let mut s = screen(dex_with(&[1, 2], &[1]));
        s.update_frame(press(a()));
        // Move DATA → CRY → AREA.
        s.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        s.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        assert_eq!(s.side_menu_cursor(), 2);
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::Area);
        // A or B closes the area page back to the list (WaitForTextScrollButtonPress).
        assert_eq!(
            s.update_frame(press(b())),
            PokedexScreenAction::Active
        );
        assert_eq!(s.mode(), PokedexScreenMode::List);
        // And the list cursor survived (side menu restores wCurrentMenuItem).
        assert_eq!(s.cursor(), 1);
        // Area also closes on A.
        s.update_frame(press(a()));
        s.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        s.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::Area);
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::List);
    }

    #[test]
    fn side_menu_quit_closes_pokedex() {
        let mut s = screen(dex_with(&[1, 2], &[1]));
        s.update_frame(press(a()));
        for _ in 0..3 {
            s.update_frame(press(PokedexScreenInput {
                down: true,
                ..Default::default()
            }));
        }
        assert_eq!(s.side_menu_cursor(), 3); // QUIT
        assert_eq!(
            s.update_frame(press(a())),
            PokedexScreenAction::Closed
        );
    }

    #[test]
    fn side_menu_b_returns_to_list() {
        let mut s = screen(dex_with(&[1, 2], &[1]));
        s.update_frame(press(a()));
        assert_eq!(s.mode(), PokedexScreenMode::SideMenu);
        s.update_frame(press(PokedexScreenInput {
            down: true,
            ..Default::default()
        }));
        s.update_frame(press(b()));
        assert_eq!(s.mode(), PokedexScreenMode::List, "B in side menu → list");
        // The selected row is unchanged (b=2 → .doPokemonListMenu).
        assert_eq!(s.cursor(), 1);
    }

    #[test]
    fn area_maps_reports_habitats_and_unknown() {
        // Pidgey (dex 16): found on Routes 1-3, 5-8, 12-15, 21, 24, 25 in Red.
        let mut s = screen(dex_with(&[16], &[16]));
        for _ in 1..16 {
            s.update_frame(press(PokedexScreenInput {
                down: true,
                ..Default::default()
            }));
        }
        assert_eq!(s.cursor_species(), Species::Pidgey);
        assert_eq!(
            s.area_maps(),
            vec![
                MapId::Route1, MapId::Route2, MapId::Route3, MapId::Route5,
                MapId::Route6, MapId::Route7, MapId::Route8, MapId::Route12,
                MapId::Route13, MapId::Route14, MapId::Route15, MapId::Route21,
                MapId::Route24, MapId::Route25,
            ]
        );
        // Kadabra (dex 64): only in Cerulean Cave, whose nest icon
        // DisplayWildLocations skips → "AREA UNKNOWN".
        let s64 = screen(dex_with(&(1..=64u8).collect::<Vec<_>>(), &[16]));
        let mut s64 = s64;
        for _ in 1..64 {
            s64.update_frame(press(PokedexScreenInput {
                down: true,
                ..Default::default()
            }));
        }
        assert_eq!(s64.cursor_species(), Species::Kadabra);
        assert!(s64.area_maps().is_empty(), "Kadabra → AREA UNKNOWN");
        // Ditto (dex 132): Routes 13/14/15/23 grass; Cerulean Cave excluded.
        let s132 = screen(dex_with(&(1..=132u8).collect::<Vec<_>>(), &[16]));
        let mut s132 = s132;
        for _ in 1..132 {
            s132.update_frame(press(PokedexScreenInput {
                down: true,
                ..Default::default()
            }));
        }
        assert_eq!(s132.cursor_species(), Species::Ditto);
        assert_eq!(
            s132.area_maps(),
            vec![
                MapId::Route13, MapId::Route14, MapId::Route15, MapId::Route23
            ]
        );
    }

    /// The Blue tables differ (e.g. Route 2 has Caterpie there, not Weedle) —
    /// the version is threaded through to the data layer.
    #[test]
    fn area_maps_follows_game_version() {
        let blue = screen(dex_with(&[10], &[10]));
        let _ = blue;
        let mut blue_caterpie =
            PokedexScreenState::new(dex_with(&[10], &[10]), GameVersion::Blue);
        for _ in 1..10 {
            blue_caterpie.update_frame(press(PokedexScreenInput {
                down: true,
                ..Default::default()
            }));
        }
        assert_eq!(blue_caterpie.cursor_species(), Species::Caterpie);
        assert!(
            blue_caterpie.area_maps().contains(&MapId::Route2),
            "Blue: Caterpie is on Route 2"
        );
        let mut red_caterpie =
            PokedexScreenState::new(dex_with(&[10], &[10]), GameVersion::Red);
        for _ in 1..10 {
            red_caterpie.update_frame(press(PokedexScreenInput {
                down: true,
                ..Default::default()
            }));
        }
        assert!(
            !red_caterpie.area_maps().contains(&MapId::Route2),
            "Red: Route 2 has Weedle, not Caterpie"
        );
    }
}
