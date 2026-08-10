#![allow(dead_code)]
//! Shared test helpers for integration tests.

use pokered_core::battle::effects::EffectRandoms;
use pokered_core::battle::move_execution::MoveRandoms;
use pokered_core::battle::state::*;
use pokered_core::battle::turn::TurnRandoms;
use pokered_data::move_data::MoveData;
use pokered_data::moves::{MoveEffect, MoveId};
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

pub fn make_pokemon(
    species: Species,
    level: u8,
    hp: u16,
    attack: u16,
    defense: u16,
    speed: u16,
    special: u16,
    type1: PokemonType,
    type2: PokemonType,
    moves: [MoveId; 4],
    pp: [u8; 4],
) -> Pokemon {
    Pokemon {
        species,
        nickname: [0x50; 11],
        level,
        hp,
        max_hp: hp,
        attack,
        defense,
        speed,
        special,
        type1,
        type2,
        moves,
        pp,
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

pub fn make_pikachu(level: u8, hp: u16, speed: u16) -> Pokemon {
    make_pokemon(
        Species::Pikachu,
        level,
        hp,
        55,
        30,
        speed,
        50,
        PokemonType::Electric,
        PokemonType::Electric,
        [
            MoveId::Thundershock,
            MoveId::QuickAttack,
            MoveId::ThunderWave,
            MoveId::None,
        ],
        [30, 30, 20, 0],
    )
}

pub fn make_charmander(level: u8, hp: u16) -> Pokemon {
    make_pokemon(
        Species::Charmander,
        level,
        hp,
        52,
        43,
        65,
        50,
        PokemonType::Fire,
        PokemonType::Fire,
        [MoveId::Scratch, MoveId::Ember, MoveId::None, MoveId::None],
        [35, 25, 0, 0],
    )
}

pub fn make_bulbasaur(level: u8, hp: u16) -> Pokemon {
    make_pokemon(
        Species::Bulbasaur,
        level,
        hp,
        49,
        49,
        45,
        65,
        PokemonType::Grass,
        PokemonType::Poison,
        [MoveId::VineWhip, MoveId::Tackle, MoveId::None, MoveId::None],
        [10, 35, 0, 0],
    )
}

pub fn make_geodude(level: u8, hp: u16) -> Pokemon {
    make_pokemon(
        Species::Geodude,
        level,
        hp,
        80,
        100,
        20,
        30,
        PokemonType::Rock,
        PokemonType::Ground,
        [
            MoveId::RockThrow,
            MoveId::Tackle,
            MoveId::None,
            MoveId::None,
        ],
        [15, 35, 0, 0],
    )
}

pub fn make_rattata(level: u8, hp: u16) -> Pokemon {
    make_pokemon(
        Species::Rattata,
        level,
        hp,
        56,
        35,
        72,
        25,
        PokemonType::Normal,
        PokemonType::Normal,
        [
            MoveId::Tackle,
            MoveId::QuickAttack,
            MoveId::None,
            MoveId::None,
        ],
        [35, 30, 0, 0],
    )
}

pub fn thundershock_data() -> MoveData {
    MoveData {
        id: MoveId::Thundershock,
        effect: MoveEffect::NoAdditionalEffect,
        power: 40,
        move_type: PokemonType::Electric,
        accuracy: 255,
        pp: 30,
    }
}

pub fn scratch_data() -> MoveData {
    MoveData {
        id: MoveId::Scratch,
        effect: MoveEffect::NoAdditionalEffect,
        power: 40,
        move_type: PokemonType::Normal,
        accuracy: 255,
        pp: 35,
    }
}

pub fn ember_data() -> MoveData {
    MoveData {
        id: MoveId::Ember,
        effect: MoveEffect::BurnSideEffect1,
        power: 40,
        move_type: PokemonType::Fire,
        accuracy: 255,
        pp: 25,
    }
}

pub fn tackle_data() -> MoveData {
    MoveData {
        id: MoveId::Tackle,
        effect: MoveEffect::NoAdditionalEffect,
        power: 35,
        move_type: PokemonType::Normal,
        accuracy: 255,
        pp: 35,
    }
}

pub fn vine_whip_data() -> MoveData {
    MoveData {
        id: MoveId::VineWhip,
        effect: MoveEffect::NoAdditionalEffect,
        power: 35,
        move_type: PokemonType::Grass,
        accuracy: 255,
        pp: 10,
    }
}

pub fn thunder_wave_data() -> MoveData {
    MoveData {
        id: MoveId::ThunderWave,
        effect: MoveEffect::ParalyzeEffect,
        power: 0,
        move_type: PokemonType::Electric,
        accuracy: 255,
        pp: 20,
    }
}

pub fn hit_randoms() -> MoveRandoms {
    MoveRandoms {
        confusion_roll: 255,
        paralysis_roll: 255,
        crit_roll: 255,
        accuracy_roll: 0,
        damage_roll: 255,
        effect_randoms: EffectRandoms {
            side_effect_roll: 255,
            duration_roll: 0,
            multi_hit_roll: 0,
        },
    }
}

pub fn hit_randoms_no_side_effect() -> MoveRandoms {
    MoveRandoms {
        confusion_roll: 255,
        paralysis_roll: 255,
        crit_roll: 255,
        accuracy_roll: 0,
        damage_roll: 255,
        effect_randoms: EffectRandoms {
            side_effect_roll: 255, // high = no side effect trigger
            duration_roll: 0,
            multi_hit_roll: 0,
        },
    }
}

pub fn turn_randoms_player_first() -> TurnRandoms {
    TurnRandoms {
        order_random: 0,
        first_mover: hit_randoms(),
        second_mover: hit_randoms(),
    }
}

pub fn turn_randoms_player_first_no_side() -> TurnRandoms {
    TurnRandoms {
        order_random: 0,
        first_mover: hit_randoms_no_side_effect(),
        second_mover: hit_randoms_no_side_effect(),
    }
}
