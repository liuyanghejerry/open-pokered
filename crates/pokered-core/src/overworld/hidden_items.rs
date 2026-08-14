//! Hidden items — invisible pickups examined by pressing A while facing
//! their tile (engine/events/hidden_items.asm), and the ITEMFINDER's
//! proximity scan (engine/items/itemfinder.asm).
//!
//! Mechanics ported from the original:
//!
//! * The A-button handler checks the tile IN FRONT of the player
//!   (`CheckIfCoordsInFrontOfPlayerMatch`, engine/overworld/hidden_events.asm)
//!   BEFORE sign/NPC interaction (home/overworld.asm:89-96). A matching
//!   hidden event consumes the A press (`hItemAlreadyFound = 0` loops the
//!   overworld without running the sign/sprite check).
//! * An already-obtained spot shows nothing (`HiddenItems` returns early on
//!   the flag test) but still swallows the A press.
//! * Otherwise "<PLAYER> found X!" shows, then `GiveItem`: on success the
//!   obtained flag is set and `SFX_GET_ITEM_2` plays; on a full bag the
//!   "no more room" text shows and the flag stays clear (data/text/text_2.asm).
//! * No ITEMFINDER is required to pick a hidden item up; the finder only
//!   reports whether an unobtained one is nearby.
//! * Obtained flags are bit `index / 8`, mask `1 << (index % 8)` of the
//!   save's `obtained_hidden_items` bytes (`FlagAction`,
//!   engine/flag_action.asm), indexed by `HiddenItemCoords` position.

use pokered_data::hidden_items;
use pokered_data::items::ItemId;
use pokered_data::maps::MapId;

use crate::save::game_data::{HIDDEN_COINS_BYTES, HIDDEN_ITEMS_BYTES};

/// `FLAG_TEST` on a hidden flag byte slice (engine/flag_action.asm).
pub fn check_obtained<const N: usize>(flags: &[u8; N], index: usize) -> bool {
    flags[index / 8] & (1 << (index % 8)) != 0
}

/// `FLAG_SET` on a hidden flag byte slice (engine/flag_action.asm).
pub fn set_obtained<const N: usize>(flags: &mut [u8; N], index: usize) {
    flags[index / 8] |= 1 << (index % 8);
}

/// Result of pressing A while facing a hidden-item tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HiddenItemFind {
    /// A hidden item lives here but is already obtained — the original
    /// swallows the A press and shows nothing.
    AlreadyObtained,
    /// "<PLAYER> found X!" — with the bag-full outcome precomputed the same
    /// way the script `giveItem` await does (held stack or a free slot).
    Found {
        index: usize,
        item: ItemId,
        bag_full: bool,
    },
}

/// Examine the tile in front of the player. `bag_names` is the screen's
/// per-frame snapshot of bag item const names (`script_bag_names`).
pub fn examine_facing_tile(
    map: MapId,
    facing_x: u8,
    facing_y: u8,
    flags: &[u8; HIDDEN_ITEMS_BYTES],
    bag_names: &[String],
) -> Option<HiddenItemFind> {
    let index = hidden_items::find_hidden_item(map, facing_x, facing_y)?;
    if check_obtained(flags, index) {
        return Some(HiddenItemFind::AlreadyObtained);
    }
    let item = hidden_items::HIDDEN_ITEMS[index].item;
    let held = bag_names.iter().any(|n| *n == item.const_name());
    let bag_full = !held && bag_names.len() >= crate::items::inventory::BAG_ITEM_CAPACITY;
    Some(HiddenItemFind::Found { index, item, bag_full })
}

/// `_FoundHiddenItemText` (data/text/text_2.asm:751): "<PLAYER> found X!"
pub fn found_message(player_name: &str, item: ItemId) -> String {
    let name = pokered_data::item_data::get_item_data(item)
        .map(|d| d.name)
        .unwrap_or("ITEM");
    format!("{} found\n{}!", player_name, name)
}

/// `_HiddenItemBagFullText` (data/text/text_2.asm:758), shown after the found
/// text when `GiveItem` fails: the item is NOT flagged obtained.
pub fn bag_full_message(player_name: &str) -> String {
    format!(
        "But, {} has\nno more room for\nother items!",
        player_name
    )
}

/// `_ItemfinderFoundItemText` (data/text/text_6.asm:119).
pub const ITEMFINDER_FOUND_MESSAGE: &str = "Yes! ITEMFINDER\nindicates there's\nan item nearby.";

/// `_ItemfinderFoundNothingText` (data/text/text_6.asm:125).
pub const ITEMFINDER_NOTHING_MESSAGE: &str = "Nope! ITEMFINDER\nisn't responding.";

/// Frames between ITEMFINDER dings (approximates the original's
/// PlaySoundWaitForCurrent blocking; same 30-frame spacing the Poké Center
/// healing machine uses per SFX).
pub const ITEMFINDER_DING_FRAMES: u8 = 30;

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_flags() -> [u8; HIDDEN_ITEMS_BYTES] {
        [0; HIDDEN_ITEMS_BYTES]
    }

    #[test]
    fn flag_round_trip_matches_original_bit_layout() {
        let mut flags = empty_flags();
        assert!(!check_obtained(&flags, 0));
        set_obtained(&mut flags, 0);
        assert_eq!(flags[0], 0b0000_0001);
        set_obtained(&mut flags, 53); // last entry: ROUTE_4
        assert_eq!(flags[6], 0b0010_0000); // 53 = byte 6, bit 5
        assert!(check_obtained(&flags, 0));
        assert!(check_obtained(&flags, 53));
        assert!(!check_obtained(&flags, 1));
    }

    #[test]
    fn examine_finds_item_on_facing_tile() {
        // Viridian Forest POTION at (1, 18), player facing it.
        let flags = empty_flags();
        let bag: Vec<String> = Vec::new();
        let find = examine_facing_tile(MapId::ViridianForest, 1, 18, &flags, &bag);
        assert_eq!(
            find,
            Some(HiddenItemFind::Found {
                index: 0,
                item: ItemId::Potion,
                bag_full: false,
            })
        );
    }

    #[test]
    fn examine_returns_none_off_tile() {
        let flags = empty_flags();
        let bag: Vec<String> = Vec::new();
        assert_eq!(
            examine_facing_tile(MapId::ViridianForest, 2, 18, &flags, &bag),
            None
        );
        // Right coords, wrong map.
        assert_eq!(
            examine_facing_tile(MapId::PalletTown, 1, 18, &flags, &bag),
            None
        );
    }

    #[test]
    fn examine_already_obtained_shows_nothing() {
        let mut flags = empty_flags();
        set_obtained(&mut flags, 0);
        let bag: Vec<String> = Vec::new();
        assert_eq!(
            examine_facing_tile(MapId::ViridianForest, 1, 18, &flags, &bag),
            Some(HiddenItemFind::AlreadyObtained)
        );
    }

    #[test]
    fn examine_bag_full_unless_stack_held() {
        let flags = empty_flags();
        let full_bag: Vec<String> = (0..crate::items::inventory::BAG_ITEM_CAPACITY)
            .map(|i| format!("ITEM_{i}"))
            .collect();
        assert_eq!(
            examine_facing_tile(MapId::ViridianForest, 1, 18, &flags, &full_bag),
            Some(HiddenItemFind::Found {
                index: 0,
                item: ItemId::Potion,
                bag_full: true,
            })
        );
        // A held POTION stack leaves room for one more.
        let mut bag_with_stack = full_bag;
        bag_with_stack[0] = "POTION".to_string();
        assert_eq!(
            examine_facing_tile(MapId::ViridianForest, 1, 18, &flags, &bag_with_stack),
            Some(HiddenItemFind::Found {
                index: 0,
                item: ItemId::Potion,
                bag_full: false,
            })
        );
    }

    #[test]
    fn found_message_uses_display_name() {
        assert_eq!(found_message("RED", ItemId::Nugget), "RED found\nNUGGET!");
    }
}

// ── Hidden coins (Game Corner floor spots) ────────────────────────────────
// engine/events/hidden_items.asm::HiddenCoins + data/events/hidden_coins.asm.

/// Result of pressing A while facing a hidden-coin tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HiddenCoinFind {
    /// Spot already collected — the original swallows the A press silently.
    AlreadyObtained,
    /// Award `amount` coins (the 40-spot's Gen-1 bug degrades 40 → 20).
    Found { index: usize, amount: u16 },
}

/// Examine the tile in front of the player for a Game Corner floor coin.
pub fn examine_facing_coin_tile(
    map: MapId,
    facing_x: u8,
    facing_y: u8,
    flags: &[u8; HIDDEN_COINS_BYTES],
) -> Option<HiddenCoinFind> {
    let index = pokered_data::hidden_coins::find_hidden_coin(map, facing_x, facing_y)?;
    if check_obtained(flags, index) {
        return Some(HiddenCoinFind::AlreadyObtained);
    }
    let stored = pokered_data::hidden_coins::HIDDEN_COIN_SPOTS[index].amount;
    // hidden_items.asm: `cp 40 / jr z, .bcd20 ; should be bcd40` — the 40
    // spot awards 20 in the original.
    let amount = if stored == 40 { 20 } else { stored as u16 };
    Some(HiddenCoinFind::Found { index, amount })
}

/// `_FoundHiddenCoinsText` (data/text/text_2.asm:764): "<PLAYER> found @NN coins!"
pub fn found_coins_message(player_name: &str, amount: u16) -> String {
    format!("{} found\n@{} coins!", player_name, amount)
}

/// `_DroppedHiddenCoinsText` (data/text/text_2.asm:774): shown right after the
/// found text when the coin total hit the 9999 cap.
pub fn dropped_coins_message() -> String {
    "Oops! Dropped\nsome coins!".to_string()
}
