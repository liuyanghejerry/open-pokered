#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum SlotSymbol {
    Seven = 0x0200,
    Bar = 0x0604,
    Cherry = 0x0A08,
    Fish = 0x0E0C,
    Bird = 0x1210,
    Mouse = 0x1614,
}

impl SlotSymbol {
    pub fn high_byte(self) -> u8 {
        ((self as u16) >> 8) as u8
    }

    pub fn low_byte(self) -> u8 {
        (self as u16 & 0xFF) as u8
    }

    pub fn from_high_byte(b: u8) -> Option<Self> {
        match b {
            0x02 => Some(Self::Seven),
            0x06 => Some(Self::Bar),
            0x0A => Some(Self::Cherry),
            0x0E => Some(Self::Fish),
            0x12 => Some(Self::Bird),
            0x16 => Some(Self::Mouse),
            _ => None,
        }
    }
}

pub const WHEEL_SIZE: usize = 18;

pub const SLOT_MACHINE_WHEEL1: [SlotSymbol; WHEEL_SIZE] = [
    SlotSymbol::Seven,
    SlotSymbol::Mouse,
    SlotSymbol::Fish,
    SlotSymbol::Bar,
    SlotSymbol::Cherry,
    SlotSymbol::Seven,
    SlotSymbol::Fish,
    SlotSymbol::Bird,
    SlotSymbol::Bar,
    SlotSymbol::Cherry,
    SlotSymbol::Seven,
    SlotSymbol::Mouse,
    SlotSymbol::Bird,
    SlotSymbol::Bar,
    SlotSymbol::Cherry,
    SlotSymbol::Seven,
    SlotSymbol::Mouse,
    SlotSymbol::Fish,
];

pub const SLOT_MACHINE_WHEEL2: [SlotSymbol; WHEEL_SIZE] = [
    SlotSymbol::Seven,
    SlotSymbol::Fish,
    SlotSymbol::Cherry,
    SlotSymbol::Bird,
    SlotSymbol::Mouse,
    SlotSymbol::Bar,
    SlotSymbol::Cherry,
    SlotSymbol::Fish,
    SlotSymbol::Bird,
    SlotSymbol::Cherry,
    SlotSymbol::Bar,
    SlotSymbol::Fish,
    SlotSymbol::Bird,
    SlotSymbol::Cherry,
    SlotSymbol::Mouse,
    SlotSymbol::Seven,
    SlotSymbol::Fish,
    SlotSymbol::Cherry,
];

pub const SLOT_MACHINE_WHEEL3: [SlotSymbol; WHEEL_SIZE] = [
    SlotSymbol::Seven,
    SlotSymbol::Bird,
    SlotSymbol::Fish,
    SlotSymbol::Cherry,
    SlotSymbol::Mouse,
    SlotSymbol::Bird,
    SlotSymbol::Fish,
    SlotSymbol::Cherry,
    SlotSymbol::Mouse,
    SlotSymbol::Bird,
    SlotSymbol::Fish,
    SlotSymbol::Cherry,
    SlotSymbol::Mouse,
    SlotSymbol::Bird,
    SlotSymbol::Bar,
    SlotSymbol::Seven,
    SlotSymbol::Bird,
    SlotSymbol::Fish,
];

pub const BIT_SLOTS_CAN_WIN: u8 = 6;
pub const BIT_SLOTS_CAN_WIN_WITH_7_OR_BAR: u8 = 7;

pub const SLOTS_CAN_WIN: u8 = 1 << BIT_SLOTS_CAN_WIN;
pub const SLOTS_CAN_WIN_WITH_7_OR_BAR: u8 = 1 << BIT_SLOTS_CAN_WIN_WITH_7_OR_BAR;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotReward {
    pub symbol: SlotSymbol,
    pub payout: u16,
    pub flash_count: u8,
}

pub const SLOT_REWARDS: [SlotReward; 6] = [
    SlotReward {
        symbol: SlotSymbol::Seven,
        payout: 300,
        flash_count: 20,
    },
    SlotReward {
        symbol: SlotSymbol::Bar,
        payout: 100,
        flash_count: 8,
    },
    SlotReward {
        symbol: SlotSymbol::Cherry,
        payout: 8,
        flash_count: 2,
    },
    SlotReward {
        symbol: SlotSymbol::Fish,
        payout: 15,
        flash_count: 4,
    },
    SlotReward {
        symbol: SlotSymbol::Bird,
        payout: 15,
        flash_count: 4,
    },
    SlotReward {
        symbol: SlotSymbol::Mouse,
        payout: 15,
        flash_count: 4,
    },
];

pub fn reward_for_symbol(sym: SlotSymbol) -> &'static SlotReward {
    SLOT_REWARDS
        .iter()
        .find(|r| r.symbol == sym)
        .unwrap_or(&SLOT_REWARDS[5])
}

pub const SLOTS_OUTOFORDER: u8 = 0;
pub const SLOTS_OUTTOLUNCH: u8 = 1;
pub const SLOTS_SOMEONESKEYS: u8 = 2;

pub const WHEEL_OFFSET_MAX: u8 = 30;
pub const INITIAL_SLIP_COUNTER: u8 = 4;
pub const INITIAL_WHEEL_OFFSET: u8 = 0x1C;

pub const SEVEN_AND_BAR_MODE_LUCKY: u8 = 250;
pub const SEVEN_AND_BAR_MODE_NORMAL: u8 = 253;
pub const ALLOW_MATCHES_DURATION: u8 = 60;
