use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumCount, EnumIter, EnumString};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Display,
    EnumCount,
    EnumIter,
    EnumString,
    FromPrimitive,
    ToPrimitive,
)]
#[repr(u8)]
pub enum PokemonType {
    Normal = 0x00,
    Fighting = 0x01,
    Flying = 0x02,
    Poison = 0x03,
    Ground = 0x04,
    Rock = 0x05,
    Bird = 0x06,
    Bug = 0x07,
    Ghost = 0x08,
    Fire = 0x14,
    Water = 0x15,
    Grass = 0x16,
    Electric = 0x17,
    Psychic = 0x18,
    Ice = 0x19,
    Dragon = 0x1A,
}

pub const NUM_TYPES: usize = 15;

impl PokemonType {
    pub fn is_physical(self) -> bool {
        (self as u8) < 0x14
    }

    pub fn is_special(self) -> bool {
        (self as u8) >= 0x14
    }

    pub fn from_id(id: u8) -> Self {
        num_traits::FromPrimitive::from_u8(id).unwrap_or(PokemonType::Normal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Effectiveness {
    NoEffect = 0,
    NotVeryEffective = 5,
    Normal = 10,
    SuperEffective = 20,
}

impl Effectiveness {
    pub fn multiplier(self) -> f32 {
        match self {
            Effectiveness::NoEffect => 0.0,
            Effectiveness::NotVeryEffective => 0.5,
            Effectiveness::Normal => 1.0,
            Effectiveness::SuperEffective => 2.0,
        }
    }
}
