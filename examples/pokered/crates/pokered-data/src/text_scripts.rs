/// Special first-byte markers (0xF5-0xFF) dispatched by DisplayTextID.
/// See home/text_script.asm and macros/scripts/text.asm.
/// 0xF8, 0xFA, 0xFB are unused in the original ROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextScriptId {
    /// $FF — Pokémon Center nurse healing script
    PokecenterNurse = 0xFF,
    /// $FE — Poké Mart shopping script. Followed by item count + item IDs + 0xFF terminator.
    Mart = 0xFE,
    /// $FD — Bill's PC access script
    BillsPc = 0xFD,
    /// $FC — Player's PC access script
    PlayersPc = 0xFC,
    // $FB — unused (const_skip)
    // $FA — unused (const_skip)
    /// $F9 — Pokémon Center PC access script (combined Bill's + Player's PC menu)
    PokecenterPc = 0xF9,
    // $F8 — unused (const_skip)
    /// $F7 — Game Corner prize vendor script
    PrizeVendor = 0xF7,
    /// $F6 — Cable Club receptionist script (link trade/battle)
    CableClubReceptionist = 0xF6,
    /// $F5 — Vending machine script (drinks)
    VendingMachine = 0xF5,
}

impl TextScriptId {
    /// Convert a byte to a TextScriptId.
    /// Returns None for bytes outside the valid script ID range.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0xFF => Some(Self::PokecenterNurse),
            0xFE => Some(Self::Mart),
            0xFD => Some(Self::BillsPc),
            0xFC => Some(Self::PlayersPc),
            0xF9 => Some(Self::PokecenterPc),
            0xF7 => Some(Self::PrizeVendor),
            0xF6 => Some(Self::CableClubReceptionist),
            0xF5 => Some(Self::VendingMachine),
            _ => None,
        }
    }

    /// Convert a TextScriptId to its byte value.
    pub fn to_byte(self) -> u8 {
        self as u8
    }

    /// Returns true if the given byte is a text script ID (0xF5-0xFF, excluding gaps).
    pub fn is_text_script(byte: u8) -> bool {
        Self::from_byte(byte).is_some()
    }

    /// Returns a human-readable name for the script.
    pub fn name(self) -> &'static str {
        match self {
            Self::PokecenterNurse => "TX_SCRIPT_POKECENTER_NURSE",
            Self::Mart => "TX_SCRIPT_MART",
            Self::BillsPc => "TX_SCRIPT_BILLS_PC",
            Self::PlayersPc => "TX_SCRIPT_PLAYERS_PC",
            Self::PokecenterPc => "TX_SCRIPT_POKECENTER_PC",
            Self::PrizeVendor => "TX_SCRIPT_PRIZE_VENDOR",
            Self::CableClubReceptionist => "TX_SCRIPT_CABLE_CLUB_RECEPTIONIST",
            Self::VendingMachine => "TX_SCRIPT_VENDING_MACHINE",
        }
    }
}

/// Quick range check: byte >= 0xF5. Does not validate gaps (0xF8, 0xFA, 0xFB).
pub fn is_in_script_range(byte: u8) -> bool {
    byte >= 0xF5
}
