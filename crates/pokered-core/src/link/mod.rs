pub mod link_battle;
pub mod link_trade;
pub mod protocol;
pub mod rng;
pub mod transport;

#[cfg(test)]
mod link_battle_tests;
#[cfg(test)]
mod link_trade_tests;

/// The player's role on a link connection — re-exported from the engine
/// (`dotzuki_engine::link::LinkRole`), which names the two sides neutrally:
/// [`LinkRole::Host`] is the original's internal-clock side and
/// [`LinkRole::Guest`] the external-clock side (`hSerialConnectionStatus`,
/// USING_INTERNAL_CLOCK vs USING_EXTERNAL_CLOCK).
///
/// The role decides the entry spot and the remote player's sprite placement
/// in the Cable Club rooms (`scripts/TradeCenter.asm` positions the opponent
/// by connection status: internal clock → x=10 facing left, external →
/// x=7 facing right). The host (`--link-listen`) is the internal clock;
/// the client (`--link-connect`) is the external clock.
pub use dotzuki_engine::link::LinkRole;

/// Overworld placement of the remote player's avatar in the Cable Club rooms.
///
/// Coordinates are in map blocks (16 px), the same units as `NpcRuntimeState`
/// positions. Derived from `TradeCenter_Script` (scripts/TradeCenter.asm:17-30):
/// the opponent sprite is placed at sprite coords (MapY=8, MapX=10) for the
/// internal-clock player and (MapY=8, MapX=7) for the external-clock player;
/// RBY sprite map coordinates carry a +4 offset (`CheckBoulderCoords` in
/// home/map_objects.asm:141-150 subtracts 4 "because sprite coordinates are
/// offset by 4"), so those map to map tiles (4,6) / (4,3) = blocks (3,2) /
/// (1,2) — the two ends of the room's table, facing each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkOpponentPresence {
    pub x: u16,
    pub y: u16,
    pub facing: dotzuki_engine::overworld::Direction,
}

impl LinkOpponentPresence {
    pub fn for_role(role: LinkRole) -> Self {
        match role {
            LinkRole::Host => LinkOpponentPresence {
                x: 6,
                y: 4,
                facing: dotzuki_engine::overworld::Direction::Left,
            },
            LinkRole::Guest => LinkOpponentPresence {
                x: 3,
                y: 4,
                facing: dotzuki_engine::overworld::Direction::Right,
            },
        }
    }
}

/// Special room warps use tile coordinates (data/maps/special_warps.asm).
/// Sprite map bytes include the object_event +4 offset: the opponent's
/// (10,8)/(7,8) bytes in TradeCenter_Script are tiles (6,4)/(3,4).
pub const fn cable_room_entry(role: LinkRole) -> (u16, u16) {
    match role {
        LinkRole::Host => (3, 4),
        LinkRole::Guest => (6, 4),
    }
}

/// CableClubLeft/RightGameboy require the corresponding clocking role
/// and direction. Hidden-event coordinates are in front of the player.
pub fn can_use_gameboy(role: LinkRole, x: u16, y: u16, facing: dotzuki_engine::overworld::Direction) -> bool {
    let expected_facing = match role {
        LinkRole::Host => dotzuki_engine::overworld::Direction::Right,
        LinkRole::Guest => dotzuki_engine::overworld::Direction::Left,
    };
    (x, y) == cable_room_entry(role) && facing == expected_facing
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    None,
    InCableClub,
    StartTrade,
    StartBattle,
    Battling,
    Trading,
}

impl LinkState {
    pub fn is_connected(&self) -> bool {
        !matches!(self, LinkState::None)
    }

    pub fn is_battling(&self) -> bool {
        matches!(self, LinkState::Battling)
    }

    pub fn is_trading(&self) -> bool {
        matches!(self, LinkState::Trading)
    }
}
