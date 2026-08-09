//! Fishing — the OLD/GOOD/SUPER ROD bag items.
//!
//! Gen-1 references:
//! - engine/items/item_effects.asm — `ItemUseOldRod` / `ItemUseGoodRod` /
//!   `ItemUseSuperRod` / `RodResponse` (:1826-1889), `FishingInit`
//!   (:1891-1916), `IsNextTileShoreOrWater` (:2826-2851), `ReadSuperRodData`
//!   (:2855-2898)
//! - data/wild/good_rod.asm — `GoodRodMons` (global 2-entry table)
//! - data/wild/super_rod.asm — `SuperRodData` (per-map fishing groups)
//! - engine/overworld/player_animations.asm — `FishingAnim` (:378-469) with
//!   the NoNibble / NothingHere / ItsABite texts
//! - home/overworld.asm:64-66 — after the item menu closes, `wCurOpponent != 0`
//!   starts the hooked mon's wild battle (`.newBattle`)
//!
//! The pure gating/roll logic is free functions (unit-tested); the
//! [`OverworldScreen`] impl wires them to the live screen. The rod-sprite
//! `FishingAnim` (rod OAM + player shake + "!" bubble) is ported as
//! `presentation::FishingAnimState`: the response is rolled here at item
//! use, the animation plays after the item-use text closes, and the result
//! text (plus the hooked battle on a bite) is queued when it finishes.

use dotzuki_engine::overworld::types::TransportMode;
use dotzuki_engine::GameData;
use pokered_data::items::ItemId;
use pokered_data::maps::MapId;
use pokered_data::species::Species;
use pokered_data::tilesets::TilesetId;
use pokered_data::wild_data::{good_rod_data, super_rod_group_index_for_map, super_rod_groups};

use super::screen::OverworldScreen;

/// Gen-1 water tile ID ($14 — `IsNextTileShoreOrWater`).
const WATER_TILE: u8 = 0x14;
/// Eastern shoreline tiles that also count for fishing ($32 usual, $48
/// Safari Zone), except on the Vermilion Dock (SHIP_PORT) tileset.
const SHORE_TILE_USUAL: u8 = 0x32;
const SHORE_TILE_SAFARI: u8 = 0x48;

/// The three fishing rods (bag items OLD ROD / GOOD ROD / SUPER ROD).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RodKind {
    Old,
    Good,
    Super,
}

impl RodKind {
    pub fn from_item(item: ItemId) -> Option<RodKind> {
        match item {
            ItemId::OldRod => Some(RodKind::Old),
            ItemId::GoodRod => Some(RodKind::Good),
            ItemId::SuperRod => Some(RodKind::Super),
            _ => None,
        }
    }
}

/// `wRodResponse` (ram/wram.asm:848-851): `0` = no bite, `1` = bite,
/// `2` = no fish on this map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RodResponse {
    /// "Not even a nibble!" (`_NoNibbleText`, data/text/text_1.asm:21-23).
    NoBite,
    /// "Oh! It's a bite!" (`_ItsABiteText`, text_1.asm:30-33) — a wild battle
    /// with the hooked mon follows.
    Bite { species: Species, level: u8 },
    /// "Looks like there's nothing here." (`_NothingHereText`,
    /// text_1.asm:25-28) — Super Rod on a map with no fishing group.
    NothingHere,
}

/// `IsNextTileShoreOrWater` (item_effects.asm:2829-2851): fishing (and
/// surfing) requires a water tileset and a water tile ($14) or an eastern
/// shore tile ($32 usual / $48 Safari Zone) in front of the player; the shore
/// tiles are skipped on the SHIP_PORT (Vermilion Dock) tileset.
pub fn is_fishing_tile(tileset: TilesetId, tile_in_front: u8) -> bool {
    pokered_data::tileset_data::is_water_tileset(tileset)
        && (tile_in_front == WATER_TILE
            || (tileset != TilesetId::ShipPort
                && (tile_in_front == SHORE_TILE_USUAL || tile_in_front == SHORE_TILE_SAFARI)))
}

/// Roll the rod response for `rod` used on `map`.
///
/// `next_random` supplies the original's `Random` bytes; the per-rod loops
/// below reproduce the asm's consumption pattern exactly (`srl a` puts bit 0
/// in carry — no bite — and `and %11` then masks the SHIFTED byte, redrawing
/// when the index runs past the table).
pub fn roll_rod_response(
    rod: RodKind,
    map: MapId,
    next_random: &mut dyn FnMut() -> u8,
) -> RodResponse {
    match rod {
        // ItemUseOldRod (item_effects.asm:1826-1831): always a bite, fixed
        // MAGIKARP level 5 (`lb bc, 5, MAGIKARP`); no RNG at all.
        RodKind::Old => RodResponse::Bite {
            species: Species::Magikarp,
            level: 5,
        },
        // ItemUseGoodRod (item_effects.asm:1833-1857): 50% "no bite" (bit 0 of
        // the random byte); otherwise bits 1-2 index the global two-entry
        // GoodRodMons table (10 GOLDEEN / 10 POLIWAG), redrawing on 2/3.
        RodKind::Good => {
            let mons = good_rod_data();
            loop {
                let r = next_random();
                if r & 1 != 0 {
                    return RodResponse::NoBite;
                }
                if let Some(m) = mons.get(((r >> 1) & 0b11) as usize) {
                    return RodResponse::Bite {
                        species: m.species,
                        level: m.level,
                    };
                }
            }
        }
        // ItemUseSuperRod + ReadSuperRodData (item_effects.asm:1861-1865,
        // 2855-2898): a map with no fishing group yields e=2 ("nothing here",
        // no RNG); otherwise 50% "no bite" (`ret c` — "50% chance of no
        // battle"), then a uniform pick from the map's group.
        RodKind::Super => {
            let Some(group_index) = super_rod_group_index_for_map(map) else {
                return RodResponse::NothingHere;
            };
            let mons = &super_rod_groups()[group_index].mons;
            loop {
                let r = next_random();
                if r & 1 != 0 {
                    return RodResponse::NoBite;
                }
                if let Some(m) = mons.get(((r >> 1) & 0b11) as usize) {
                    return RodResponse::Bite {
                        species: m.species,
                        level: m.level,
                    };
                }
            }
        }
    }
}

/// FishingAnim's result text (player_animations.asm:459-469 →
/// data/text/text_1.asm:21-33).
pub fn response_text(response: RodResponse) -> &'static str {
    match response {
        RodResponse::NoBite => "Not even a nibble!",
        RodResponse::NothingHere => "Looks like there's\nnothing here.",
        RodResponse::Bite { .. } => "Oh!\nIt's a bite!",
    }
}

/// A rod use whose `FishingAnim` has not played yet. The response is rolled
/// at item use time (`RodResponse`, item_effects.asm:1869-1877) and held here
/// while the "You used the <ROD>!" dialogue shows and the animation runs;
/// when the animation completes, the result text (and, on a bite, the hooked
/// mon's deferred wild battle) is emitted from this record.
pub(crate) struct PendingFishing {
    /// The pre-rolled rod response.
    pub(crate) response: RodResponse,
}

impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    /// Use a fishing rod from the bag (`ItemUseOldRod` / `ItemUseGoodRod` /
    /// `ItemUseSuperRod` + the shared `FishingInit`, item_effects.asm:1826-1916).
    ///
    /// Returns the message to display. On a bite, the hooked mon's wild battle
    /// is queued in `post_dialogue_battle` so it starts once the player
    /// dismisses the "Oh! It's a bite!" text (home/overworld.asm `.newBattle`).
    ///
    /// The returned message is the item-use text only ("<PLAYER> used
    /// <ITEM>!" — `ItemUseText00`); the animation (`FishingAnim`) plays after
    /// that dialogue closes, and the rod response text is shown when it
    /// finishes. The response is rolled NOW (as the original's `RodResponse`
    /// runs before `FishingAnim`).
    pub(crate) fn use_fishing_rod(&mut self, rod: RodKind) -> String {
        // FishingInit: can't fish while surfing (`wWalkBikeSurfState == 2` →
        // carry) or unless the tile in front is shore/water
        // (IsNextTileShoreOrWater). Both failures route to ItemUseNotTime.
        let facing_water = self
            .tiles_in_front()
            .map(|(_, tile_in_front, _, _)| {
                let tileset = self.map_data.as_ref().expect("map_data present").tileset;
                is_fishing_tile(tileset, tile_in_front)
            })
            .unwrap_or(false);
        if self.state.player.transport == TransportMode::Surfing || !facing_water {
            return "This isn't the\ntime to use that!".to_string();
        }

        // FishingInit's success path prints ItemUseText00 ("<PLAYER> used
        // <ITEM>!") and plays SFX_HEAL_AILMENT + an 80-frame delay BEFORE the
        // rod animation (item_effects.asm:1906-1911). The SFX and the delay
        // are deferred to when the text is dismissed (update_frame's
        // `pending_fishing` handling) so the sound does not overlap the box.
        let response = {
            use rand::Rng;
            let rng = &mut self.rng;
            roll_rod_response(rod, self.state.current_map, &mut || rng.gen_range(0u8..=255))
        };

        let rod_name = pokered_data::item_data::get_item_data(match rod {
            RodKind::Old => ItemId::OldRod,
            RodKind::Good => ItemId::GoodRod,
            RodKind::Super => ItemId::SuperRod,
        })
        .map(|d| d.name)
        .unwrap_or("ROD");

        // The response is consumed when the rod animation completes — the
        // result text ("Not even a nibble!" / "Oh! It's a bite!" /
        // "Nothing here.") is queued then, and a bite arms
        // `post_dialogue_battle` so home/overworld.asm's `.newBattle` fires
        // once that text is dismissed.
        self.pending_fishing = Some(PendingFishing { response });

        format!("You used the\n{}!", rod_name)
    }
}
