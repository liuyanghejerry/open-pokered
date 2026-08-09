use crate::types::{Effectiveness, PokemonType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeMatchup {
    pub attacker: PokemonType,
    pub defender: PokemonType,
    pub effectiveness: Effectiveness,
}

pub const TYPE_CHART: [TypeMatchup; 82] = [
    TypeMatchup {
        attacker: PokemonType::Water,
        defender: PokemonType::Fire,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fire,
        defender: PokemonType::Grass,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fire,
        defender: PokemonType::Ice,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Grass,
        defender: PokemonType::Water,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Electric,
        defender: PokemonType::Water,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Water,
        defender: PokemonType::Rock,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ground,
        defender: PokemonType::Flying,
        effectiveness: Effectiveness::NoEffect,
    },
    TypeMatchup {
        attacker: PokemonType::Water,
        defender: PokemonType::Water,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fire,
        defender: PokemonType::Fire,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Electric,
        defender: PokemonType::Electric,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ice,
        defender: PokemonType::Ice,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Grass,
        defender: PokemonType::Grass,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Psychic,
        defender: PokemonType::Psychic,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fire,
        defender: PokemonType::Water,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Grass,
        defender: PokemonType::Fire,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Water,
        defender: PokemonType::Grass,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Electric,
        defender: PokemonType::Grass,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Normal,
        defender: PokemonType::Rock,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Normal,
        defender: PokemonType::Ghost,
        effectiveness: Effectiveness::NoEffect,
    },
    TypeMatchup {
        attacker: PokemonType::Ghost,
        defender: PokemonType::Ghost,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fire,
        defender: PokemonType::Bug,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fire,
        defender: PokemonType::Rock,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Water,
        defender: PokemonType::Ground,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Electric,
        defender: PokemonType::Ground,
        effectiveness: Effectiveness::NoEffect,
    },
    TypeMatchup {
        attacker: PokemonType::Electric,
        defender: PokemonType::Flying,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Grass,
        defender: PokemonType::Ground,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Grass,
        defender: PokemonType::Bug,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Grass,
        defender: PokemonType::Poison,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Grass,
        defender: PokemonType::Rock,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Grass,
        defender: PokemonType::Flying,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ice,
        defender: PokemonType::Water,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ice,
        defender: PokemonType::Grass,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ice,
        defender: PokemonType::Ground,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ice,
        defender: PokemonType::Flying,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fighting,
        defender: PokemonType::Normal,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fighting,
        defender: PokemonType::Poison,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fighting,
        defender: PokemonType::Flying,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fighting,
        defender: PokemonType::Psychic,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fighting,
        defender: PokemonType::Bug,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fighting,
        defender: PokemonType::Rock,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fighting,
        defender: PokemonType::Ice,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Fighting,
        defender: PokemonType::Ghost,
        effectiveness: Effectiveness::NoEffect,
    },
    TypeMatchup {
        attacker: PokemonType::Poison,
        defender: PokemonType::Grass,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Poison,
        defender: PokemonType::Poison,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Poison,
        defender: PokemonType::Ground,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Poison,
        defender: PokemonType::Bug,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Poison,
        defender: PokemonType::Rock,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Poison,
        defender: PokemonType::Ghost,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ground,
        defender: PokemonType::Fire,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ground,
        defender: PokemonType::Electric,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ground,
        defender: PokemonType::Grass,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ground,
        defender: PokemonType::Bug,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ground,
        defender: PokemonType::Rock,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ground,
        defender: PokemonType::Poison,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Flying,
        defender: PokemonType::Electric,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Flying,
        defender: PokemonType::Fighting,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Flying,
        defender: PokemonType::Bug,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Flying,
        defender: PokemonType::Grass,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Flying,
        defender: PokemonType::Rock,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Psychic,
        defender: PokemonType::Fighting,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Psychic,
        defender: PokemonType::Poison,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Bug,
        defender: PokemonType::Fire,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Bug,
        defender: PokemonType::Grass,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Bug,
        defender: PokemonType::Fighting,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Bug,
        defender: PokemonType::Flying,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Bug,
        defender: PokemonType::Psychic,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Bug,
        defender: PokemonType::Ghost,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Bug,
        defender: PokemonType::Poison,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Rock,
        defender: PokemonType::Fire,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Rock,
        defender: PokemonType::Fighting,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Rock,
        defender: PokemonType::Ground,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Rock,
        defender: PokemonType::Flying,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Rock,
        defender: PokemonType::Bug,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Rock,
        defender: PokemonType::Ice,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ghost,
        defender: PokemonType::Normal,
        effectiveness: Effectiveness::NoEffect,
    },
    TypeMatchup {
        attacker: PokemonType::Ghost,
        defender: PokemonType::Psychic,
        effectiveness: Effectiveness::NoEffect,
    },
    TypeMatchup {
        attacker: PokemonType::Fire,
        defender: PokemonType::Dragon,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Water,
        defender: PokemonType::Dragon,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Electric,
        defender: PokemonType::Dragon,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Grass,
        defender: PokemonType::Dragon,
        effectiveness: Effectiveness::NotVeryEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Ice,
        defender: PokemonType::Dragon,
        effectiveness: Effectiveness::SuperEffective,
    },
    TypeMatchup {
        attacker: PokemonType::Dragon,
        defender: PokemonType::Dragon,
        effectiveness: Effectiveness::SuperEffective,
    },
];

pub fn get_effectiveness(attacker: PokemonType, defender: PokemonType) -> Effectiveness {
    for matchup in &TYPE_CHART {
        if matchup.attacker == attacker && matchup.defender == defender {
            return matchup.effectiveness;
        }
    }
    Effectiveness::Normal
}
