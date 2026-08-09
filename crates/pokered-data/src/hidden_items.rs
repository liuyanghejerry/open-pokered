//! Hidden items — the invisible pickups examined by pressing A while facing
//! their tile, and detected by the ITEMFINDER.
//!
//! Ported from the original disassembly:
//!
//! * `data/events/hidden_item_coords.asm` (`HiddenItemCoords`) — the ordered
//!   (map, y, x) table. An entry's position in this table is its flag index
//!   into `wObtainedHiddenItemsFlags` (save `obtained_hidden_items`); both
//!   `HiddenItems` (`engine/events/hidden_items.asm`,
//!   `FindHiddenItemOrCoinsIndex`) and `HiddenItemNear`
//!   (`engine/items/itemfinder.asm`) count entries from the table start.
//! * `data/events/hidden_events.asm` — the per-map `hidden_event X, Y,
//!   HiddenItems, ITEM` lines that give each coord its item.
//!
//! The table order below matches `HiddenItemCoords` exactly, so a slice index
//! IS the original flag bit index.

use crate::items::ItemId;
use crate::maps::MapId;

/// One hidden item spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HiddenItem {
    pub map: MapId,
    pub x: u8,
    pub y: u8,
    pub item: ItemId,
}

/// All 54 hidden items, in original `HiddenItemCoords` order.
///
/// The original caps the table at `MAX_HIDDEN_ITEMS = 112` (14 flag bytes);
/// only 54 entries exist. Entry 16 (SAFARI_ZONE_GATE NUGGET) is inaccessible
/// in the original and entry 34 sits on UNUSED_MAP_6F — both are kept so the
/// flag indices stay aligned with the original save format.
pub static HIDDEN_ITEMS: [HiddenItem; 54] = [
    // VIRIDIAN_FOREST
    HiddenItem { map: MapId::ViridianForest, x: 1, y: 18, item: ItemId::Potion },
    HiddenItem { map: MapId::ViridianForest, x: 16, y: 42, item: ItemId::Antidote },
    // MT_MOON_B2F
    HiddenItem { map: MapId::MtMoonB2F, x: 18, y: 12, item: ItemId::MoonStone },
    // ROUTE_25
    HiddenItem { map: MapId::Route25, x: 38, y: 3, item: ItemId::Ether },
    // ROUTE_9
    HiddenItem { map: MapId::Route9, x: 14, y: 7, item: ItemId::Ether },
    // SS_ANNE_KITCHEN
    HiddenItem { map: MapId::SSAnneKitchen, x: 13, y: 9, item: ItemId::GreatBall },
    // SS_ANNE_B1F_ROOMS
    HiddenItem { map: MapId::SSAnneB1FRooms, x: 3, y: 1, item: ItemId::HyperPotion },
    // ROUTE_10
    HiddenItem { map: MapId::Route10, x: 9, y: 17, item: ItemId::SuperPotion },
    HiddenItem { map: MapId::Route10, x: 16, y: 53, item: ItemId::MaxEther },
    // ROCKET_HIDEOUT_B1F / B3F / B4F
    HiddenItem { map: MapId::RocketHideoutB1F, x: 21, y: 15, item: ItemId::PpUp },
    HiddenItem { map: MapId::RocketHideoutB3F, x: 27, y: 17, item: ItemId::Nugget },
    HiddenItem { map: MapId::RocketHideoutB4F, x: 25, y: 1, item: ItemId::SuperPotion },
    // POKEMON_TOWER_5F
    HiddenItem { map: MapId::PokemonTower5F, x: 4, y: 12, item: ItemId::Elixer },
    // ROUTE_13
    HiddenItem { map: MapId::Route13, x: 1, y: 14, item: ItemId::PpUp },
    HiddenItem { map: MapId::Route13, x: 16, y: 13, item: ItemId::Calcium },
    // POKEMON_MANSION_B1F
    HiddenItem { map: MapId::PokemonMansionB1F, x: 1, y: 9, item: ItemId::RareCandy },
    // SAFARI_ZONE_GATE (inaccessible in the original)
    HiddenItem { map: MapId::SafariZoneGate, x: 10, y: 1, item: ItemId::Nugget },
    // SAFARI_ZONE_WEST
    HiddenItem { map: MapId::SafariZoneWest, x: 6, y: 5, item: ItemId::Revive },
    // SILPH_CO_5F / 9F
    HiddenItem { map: MapId::SilphCo5F, x: 12, y: 3, item: ItemId::Elixer },
    HiddenItem { map: MapId::SilphCo9F, x: 2, y: 15, item: ItemId::MaxPotion },
    // COPYCATS_HOUSE_2F
    HiddenItem { map: MapId::CopycatsHouse2F, x: 1, y: 1, item: ItemId::Nugget },
    // CERULEAN_CAVE_1F / B1F
    HiddenItem { map: MapId::CeruleanCave1F, x: 14, y: 11, item: ItemId::RareCandy },
    HiddenItem { map: MapId::CeruleanCaveB1F, x: 27, y: 3, item: ItemId::UltraBall },
    // POWER_PLANT
    HiddenItem { map: MapId::PowerPlant, x: 17, y: 16, item: ItemId::MaxElixer },
    HiddenItem { map: MapId::PowerPlant, x: 12, y: 1, item: ItemId::PpUp },
    // SEAFOAM_ISLANDS_B2F / B4F
    HiddenItem { map: MapId::SeafoamIslandsB2F, x: 15, y: 15, item: ItemId::Nugget },
    HiddenItem { map: MapId::SeafoamIslandsB4F, x: 25, y: 17, item: ItemId::UltraBall },
    // POKEMON_MANSION_1F / 3F
    HiddenItem { map: MapId::PokemonMansion1F, x: 8, y: 16, item: ItemId::MoonStone },
    HiddenItem { map: MapId::PokemonMansion3F, x: 1, y: 9, item: ItemId::MaxRevive },
    // ROUTE_23
    HiddenItem { map: MapId::Route23, x: 9, y: 44, item: ItemId::FullRestore },
    HiddenItem { map: MapId::Route23, x: 19, y: 70, item: ItemId::UltraBall },
    HiddenItem { map: MapId::Route23, x: 8, y: 90, item: ItemId::MaxEther },
    // VICTORY_ROAD_2F
    HiddenItem { map: MapId::VictoryRoad2F, x: 5, y: 2, item: ItemId::UltraBall },
    HiddenItem { map: MapId::VictoryRoad2F, x: 26, y: 7, item: ItemId::FullRestore },
    // UNUSED_MAP_6F
    HiddenItem { map: MapId::UnusedMap6F, x: 14, y: 11, item: ItemId::MaxElixer },
    // VIRIDIAN_CITY
    HiddenItem { map: MapId::ViridianCity, x: 14, y: 4, item: ItemId::Potion },
    // ROUTE_11
    HiddenItem { map: MapId::Route11, x: 48, y: 5, item: ItemId::EscapeRope },
    // ROUTE_12
    HiddenItem { map: MapId::Route12, x: 2, y: 63, item: ItemId::HyperPotion },
    // ROUTE_17 (cycling road)
    HiddenItem { map: MapId::Route17, x: 15, y: 14, item: ItemId::RareCandy },
    HiddenItem { map: MapId::Route17, x: 8, y: 45, item: ItemId::FullRestore },
    HiddenItem { map: MapId::Route17, x: 17, y: 72, item: ItemId::PpUp },
    HiddenItem { map: MapId::Route17, x: 4, y: 91, item: ItemId::MaxRevive },
    HiddenItem { map: MapId::Route17, x: 8, y: 121, item: ItemId::MaxElixer },
    // UNDERGROUND_PATH_NORTH_SOUTH
    HiddenItem { map: MapId::UndergroundPathNorthSouth, x: 3, y: 4, item: ItemId::FullRestore },
    HiddenItem { map: MapId::UndergroundPathNorthSouth, x: 4, y: 34, item: ItemId::XSpecial },
    // UNDERGROUND_PATH_WEST_EAST
    HiddenItem { map: MapId::UndergroundPathWestEast, x: 12, y: 2, item: ItemId::Nugget },
    HiddenItem { map: MapId::UndergroundPathWestEast, x: 21, y: 5, item: ItemId::Elixer },
    // CELADON_CITY
    HiddenItem { map: MapId::CeladonCity, x: 48, y: 15, item: ItemId::PpUp },
    // ROUTE_25
    HiddenItem { map: MapId::Route25, x: 10, y: 1, item: ItemId::Elixer },
    // MT_MOON_B2F
    HiddenItem { map: MapId::MtMoonB2F, x: 33, y: 9, item: ItemId::Ether },
    // SEAFOAM_ISLANDS_B3F
    HiddenItem { map: MapId::SeafoamIslandsB3F, x: 9, y: 16, item: ItemId::MaxElixer },
    // VERMILION_CITY
    HiddenItem { map: MapId::VermilionCity, x: 14, y: 11, item: ItemId::MaxEther },
    // CERULEAN_CITY
    HiddenItem { map: MapId::CeruleanCity, x: 15, y: 8, item: ItemId::RareCandy },
    // ROUTE_4
    HiddenItem { map: MapId::Route4, x: 40, y: 3, item: ItemId::GreatBall },
];

/// `FindHiddenItemOrCoinsIndex` (engine/events/hidden_items.asm:134-161):
/// the table index of the hidden item at (`x`, `y`) on `map`, if any.
pub fn find_hidden_item(map: MapId, x: u8, y: u8) -> Option<usize> {
    HIDDEN_ITEMS
        .iter()
        .position(|e| e.map == map && e.x == x && e.y == y)
}

/// `HiddenItemNear` (engine/items/itemfinder.asm): is any unobtained hidden
/// item on `map` within the ITEMFINDER's scan window of the player at
/// (`px`, `py`)?
///
/// The original window is asymmetric (itemfinder.asm:26-42, "check if the
/// item is within 4-5 tiles depending on the direction of item"):
/// `py - 4 <= item_y <= py + 4` and `px - 4 <= item_x <= px + 5` (the low
/// sides subtract 5 clamped to 0 and require strict `>`, i.e. `>= p - 4`).
pub fn hidden_item_near(
    map: MapId,
    px: u8,
    py: u8,
    is_obtained: impl Fn(usize) -> bool,
) -> bool {
    let (px, py) = (px as i32, py as i32);
    HIDDEN_ITEMS.iter().enumerate().any(|(i, e)| {
        e.map == map
            && !is_obtained(i)
            && (py - 4..=py + 4).contains(&(e.y as i32))
            && (px - 4..=px + 5).contains(&(e.x as i32))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_count_matches_hidden_item_coords() {
        // data/events/hidden_item_coords.asm: 54 hidden_item lines before `db -1`.
        assert_eq!(HIDDEN_ITEMS.len(), 54);
    }

    #[test]
    fn spot_check_entries_against_asm() {
        // First entry: hidden_item VIRIDIAN_FOREST, 1, 18 → POTION.
        assert_eq!(
            HIDDEN_ITEMS[0],
            HiddenItem { map: MapId::ViridianForest, x: 1, y: 18, item: ItemId::Potion }
        );
        // Index 16: SAFARI_ZONE_GATE 10, 1 → NUGGET (asm comment: inaccessible).
        assert_eq!(
            HIDDEN_ITEMS[16],
            HiddenItem { map: MapId::SafariZoneGate, x: 10, y: 1, item: ItemId::Nugget }
        );
        // Index 34: UNUSED_MAP_6F 14, 11 → MAX_ELIXER.
        assert_eq!(
            HIDDEN_ITEMS[34],
            HiddenItem { map: MapId::UnusedMap6F, x: 14, y: 11, item: ItemId::MaxElixer }
        );
        // Last entry: ROUTE_4 40, 3 → GREAT_BALL.
        assert_eq!(
            HIDDEN_ITEMS[53],
            HiddenItem { map: MapId::Route4, x: 40, y: 3, item: ItemId::GreatBall }
        );
    }

    #[test]
    fn find_hidden_item_returns_table_index() {
        assert_eq!(find_hidden_item(MapId::ViridianForest, 1, 18), Some(0));
        assert_eq!(find_hidden_item(MapId::Route4, 40, 3), Some(53));
        assert_eq!(find_hidden_item(MapId::ViridianForest, 2, 18), None);
        assert_eq!(find_hidden_item(MapId::PalletTown, 1, 18), None);
    }

    #[test]
    fn itemfinder_window_is_asymmetric() {
        // ROUTE_4 item at (40, 3). Player four tiles below-left is inside.
        assert!(hidden_item_near(MapId::Route4, 36, 7, |_| false));
        // Item five tiles to the RIGHT of the player is in (high side px + 5)...
        assert!(hidden_item_near(MapId::Route4, 35, 3, |_| false));
        // ...but five tiles to the LEFT is out (low side is px - 4).
        assert!(!hidden_item_near(MapId::Route4, 45, 3, |_| false));
        // Six tiles to the right is out too.
        assert!(!hidden_item_near(MapId::Route4, 34, 3, |_| false));
        // Five tiles above/below is out (y window is exactly ±4).
        assert!(!hidden_item_near(MapId::Route4, 40, 8, |_| false));
        // Obtained items are skipped.
        assert!(!hidden_item_near(MapId::Route4, 40, 3, |i| i == 53));
        // Other maps never match (Route25 has no items near (40, 20)).
        assert!(!hidden_item_near(MapId::Route25, 40, 20, |_| false));
    }
}
