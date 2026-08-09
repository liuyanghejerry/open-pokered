use crate::battle::state::Pokemon;
use crate::pokemon::party::Party;
use serde::{Deserialize, Serialize};

pub const LINK_RANDOM_LIST_SIZE: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkAction {
    UseMove(u8),
    Switch(u8),
    Run,
    Struggle,
    NoAction,
}

impl LinkAction {
    pub fn to_wire_byte(self) -> u8 {
        match self {
            LinkAction::UseMove(idx) => idx,
            LinkAction::Switch(pokemon_idx) => pokemon_idx + 4,
            LinkAction::Run => 0x0F,
            LinkAction::Struggle => 0x0E,
            LinkAction::NoAction => 0x0D,
        }
    }

    pub fn from_wire_byte(byte: u8) -> Self {
        match byte {
            0x0F => LinkAction::Run,
            0x0E => LinkAction::Struggle,
            0x0D => LinkAction::NoAction,
            b if b >= 4 => LinkAction::Switch(b - 4),
            b => LinkAction::UseMove(b),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyExchangeData {
    pub trainer_name: Vec<u8>,
    pub party: Party,
    pub random_numbers: [u8; LINK_RANDOM_LIST_SIZE],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkMessage {
    Hello { version: u8 },
    HelloAck { version: u8 },

    RequestBattle,
    AcceptBattle,
    DeclineBattle,

    PartyData(PartyExchangeData),

    TurnAction(LinkAction),

    /// Sent by each side when its battle ends (locally resolved outcome).
    /// Informational/confirmatory: both sides compute the same result from
    /// the shared resolution, so this never gates battle logic — it gives the
    /// app an authoritative end-of-battle signal from the peer.
    BattleResult(LinkBattleResult),

    RequestTrade,
    AcceptTrade,
    DeclineTrade,
    SelectMon(u8),
    ConfirmTrade,
    CancelTrade,
    TradeComplete(Pokemon),

    Disconnect,
}

/// The local side's outcome of a finished link battle, mirroring the
/// original `wBattleResult` (0 = win, 1 = loss, 2 = draw) as shown by
/// `EndOfBattle` (engine/battle/end_of_battle.asm:28-34): "YOU WIN" /
/// "YOU LOSE" / "DRAW" inside the versus text box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkBattleResult {
    Win,
    Lose,
    Draw,
}

impl NetworkMessage {
    /// Wire protocol version. Bumped from 1 to 2 when `BattleResult` was
    /// added; handshakes accept both versions (an old peer simply never
    /// sends `BattleResult`).
    pub const PROTOCOL_VERSION: u8 = 2;

    /// Versions this implementation can talk to (v1 peers predate
    /// `BattleResult`; the message is purely additive).
    pub fn is_compatible_version(version: u8) -> bool {
        version == 1 || version == Self::PROTOCOL_VERSION
    }

    pub fn hello() -> Self {
        NetworkMessage::Hello {
            version: Self::PROTOCOL_VERSION,
        }
    }

    pub fn hello_ack() -> Self {
        NetworkMessage::HelloAck {
            version: Self::PROTOCOL_VERSION,
        }
    }
}
