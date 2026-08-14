//! Hidden coins — the 12 floor-coin spots in the Game Corner
//! (`data/events/hidden_coins.asm` + the `HiddenCoins` entries in
//! `data/events/hidden_events.asm`).
//!
//! Mechanics ported from `engine/events/hidden_items.asm::HiddenCoins`:
//!
//! * Picking one up requires a COIN CASE in the bag (no case, no text).
//! * The A-press on a matching tile is consumed even when the spot was
//!   already collected (the original returns silently on the flag test).
//! * The spot at (11,7) stores 40 but only awards 20 — the original jumps to
//!   `.bcd20` for both 20 and 40 ("should be bcd40" — deliberate Gen-1 bug).
//! * Obtained flags are bits of `wObtainedHiddenCoinsFlags`, indexed by
//!   `HiddenCoinCoords` position (0..12).

use crate::maps::MapId;

/// The intended coin amount of each spot; `40` degrades to `20` at pickup.
pub const NUM_HIDDEN_COINS: usize = 12;

pub struct HiddenCoinSpot {
    pub map: MapId,
    pub x: u8,
    pub y: u8,
    pub amount: u8,
}

/// `data/events/hidden_coins.asm:5-17` in table order (flag index == index).
pub const HIDDEN_COIN_SPOTS: [HiddenCoinSpot; NUM_HIDDEN_COINS] = [
    HiddenCoinSpot { map: MapId::GameCorner, x: 0, y: 8, amount: 10 },
    HiddenCoinSpot { map: MapId::GameCorner, x: 1, y: 16, amount: 10 },
    HiddenCoinSpot { map: MapId::GameCorner, x: 3, y: 11, amount: 20 },
    HiddenCoinSpot { map: MapId::GameCorner, x: 3, y: 14, amount: 10 },
    HiddenCoinSpot { map: MapId::GameCorner, x: 4, y: 12, amount: 10 },
    HiddenCoinSpot { map: MapId::GameCorner, x: 9, y: 12, amount: 20 },
    HiddenCoinSpot { map: MapId::GameCorner, x: 9, y: 15, amount: 10 },
    HiddenCoinSpot { map: MapId::GameCorner, x: 16, y: 14, amount: 10 },
    HiddenCoinSpot { map: MapId::GameCorner, x: 10, y: 16, amount: 10 },
    HiddenCoinSpot { map: MapId::GameCorner, x: 11, y: 7, amount: 40 },
    HiddenCoinSpot { map: MapId::GameCorner, x: 15, y: 8, amount: 100 },
    HiddenCoinSpot { map: MapId::GameCorner, x: 12, y: 15, amount: 10 },
];

/// Find the spot index whose tile the player is facing, if any.
pub fn find_hidden_coin(map: MapId, x: u8, y: u8) -> Option<usize> {
    HIDDEN_COIN_SPOTS
        .iter()
        .position(|s| s.map == map && s.x == x && s.y == y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_spots_in_asm_order() {
        assert_eq!(NUM_HIDDEN_COINS, 12);
        // hidden_events.asm:287-298 amounts: 10,10,20,10,10,20,10,10,10,40,100,10
        assert_eq!(
            HIDDEN_COIN_SPOTS.iter().map(|s| s.amount).collect::<Vec<_>>(),
            vec![10, 10, 20, 10, 10, 20, 10, 10, 10, 40, 100, 10]
        );
    }

    #[test]
    fn finds_spot_by_tile() {
        assert_eq!(find_hidden_coin(MapId::GameCorner, 0, 8), Some(0));
        assert_eq!(find_hidden_coin(MapId::GameCorner, 12, 15), Some(11));
        assert_eq!(find_hidden_coin(MapId::GameCorner, 1, 1), None);
        assert_eq!(find_hidden_coin(MapId::CeladonCity, 0, 8), None);
    }
}
