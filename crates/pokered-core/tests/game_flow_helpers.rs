#![allow(dead_code)]

use pokered_core::battle::effects::EffectRandoms;
use pokered_core::battle::move_execution::MoveRandoms;
use pokered_core::battle::state::*;
use pokered_core::battle::turn::TurnRandoms;
use pokered_core::game_state::*;
use pokered_core::pokemon::party::Party;
use pokered_core::pokemon::pokedex::Pokedex;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;
use pokered_data::wild_data::GameVersion;

pub fn new_game_state_red() -> GameState {
    GameState::new(GameVersion::Red)
}

pub fn new_game_state_blue() -> GameState {
    GameState::new(GameVersion::Blue)
}

pub fn new_game_config_red() -> GameConfig {
    GameConfig::new(GameVersion::Red)
}

pub fn make_starter_bulbasaur() -> Pokemon {
    Pokemon {
        species: Species::Bulbasaur,
        nickname: [0x50; 11],
        level: 5,
        hp: 20,
        max_hp: 20,
        attack: 11,
        defense: 11,
        speed: 11,
        special: 13,
        type1: PokemonType::Grass,
        type2: PokemonType::Poison,
        moves: [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
        pp: [35, 40, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

pub fn make_starter_charmander() -> Pokemon {
    Pokemon {
        species: Species::Charmander,
        nickname: [0x50; 11],
        level: 5,
        hp: 19,
        max_hp: 19,
        attack: 12,
        defense: 11,
        speed: 13,
        special: 11,
        type1: PokemonType::Fire,
        type2: PokemonType::Fire,
        moves: [MoveId::Scratch, MoveId::Growl, MoveId::None, MoveId::None],
        pp: [35, 40, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

pub fn make_starter_squirtle() -> Pokemon {
    Pokemon {
        species: Species::Squirtle,
        nickname: [0x50; 11],
        level: 5,
        hp: 20,
        max_hp: 20,
        attack: 11,
        defense: 13,
        speed: 11,
        special: 11,
        type1: PokemonType::Water,
        type2: PokemonType::Water,
        moves: [MoveId::Tackle, MoveId::TailWhip, MoveId::None, MoveId::None],
        pp: [35, 30, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

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

pub fn make_brock_geodude() -> Pokemon {
    make_pokemon(
        Species::Geodude,
        12,
        33,
        26,
        30,
        10,
        12,
        PokemonType::Rock,
        PokemonType::Ground,
        [
            MoveId::Tackle,
            MoveId::DefenseCurl,
            MoveId::None,
            MoveId::None,
        ],
        [35, 40, 0, 0],
    )
}

pub fn make_brock_onix() -> Pokemon {
    make_pokemon(
        Species::Onix,
        14,
        33,
        16,
        50,
        18,
        12,
        PokemonType::Rock,
        PokemonType::Ground,
        [MoveId::Tackle, MoveId::Screech, MoveId::Bide, MoveId::None],
        [35, 40, 10, 0],
    )
}

pub fn make_misty_staryu() -> Pokemon {
    make_pokemon(
        Species::Staryu,
        18,
        44,
        28,
        28,
        38,
        35,
        PokemonType::Water,
        PokemonType::Water,
        [MoveId::Tackle, MoveId::WaterGun, MoveId::None, MoveId::None],
        [35, 25, 0, 0],
    )
}

pub fn make_misty_starmie() -> Pokemon {
    make_pokemon(
        Species::Starmie,
        21,
        55,
        37,
        38,
        49,
        46,
        PokemonType::Water,
        PokemonType::Psychic,
        [
            MoveId::Tackle,
            MoveId::WaterGun,
            MoveId::Bubblebeam,
            MoveId::None,
        ],
        [35, 25, 20, 0],
    )
}

pub fn make_strong_player_pokemon(species: Species, level: u8) -> Pokemon {
    make_pokemon(
        species,
        level,
        200,
        100,
        100,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        [MoveId::Strength, MoveId::Tackle, MoveId::None, MoveId::None],
        [15, 35, 0, 0],
    )
}

pub fn make_champion_pokemon(
    species: Species,
    level: u8,
    type1: PokemonType,
    type2: PokemonType,
    moves: [MoveId; 4],
    pp: [u8; 4],
) -> Pokemon {
    make_pokemon(
        species, level, 250, 150, 120, 130, 140, type1, type2, moves, pp,
    )
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
            side_effect_roll: 255,
            duration_roll: 0,
            multi_hit_roll: 0,
        },
    }
}

pub fn turn_randoms_player_first() -> TurnRandoms {
    TurnRandoms {
        order_random: 0,
        first_mover: hit_randoms_no_side_effect(),
        second_mover: hit_randoms_no_side_effect(),
    }
}

pub const BOULDER_BADGE: u8 = 1 << 0;
pub const CASCADE_BADGE: u8 = 1 << 1;
pub const THUNDER_BADGE: u8 = 1 << 2;
pub const RAINBOW_BADGE: u8 = 1 << 3;
pub const SOUL_BADGE: u8 = 1 << 4;
pub const MARSH_BADGE: u8 = 1 << 5;
pub const VOLCANO_BADGE: u8 = 1 << 6;
pub const EARTH_BADGE: u8 = 1 << 7;

pub const ALL_BADGES: u8 = 0xFF;

pub fn badge_count(badges: u8) -> u8 {
    badges.count_ones() as u8
}

pub fn party_at_start(starter: Pokemon) -> Party {
    Party::from_pokemon(vec![starter]).unwrap()
}

pub fn party_mid_game() -> Party {
    Party::from_pokemon(vec![
        make_pokemon(
            Species::Charmeleon,
            32,
            90,
            52,
            43,
            65,
            50,
            PokemonType::Fire,
            PokemonType::Fire,
            [MoveId::Slash, MoveId::Ember, MoveId::None, MoveId::None],
            [20, 25, 0, 0],
        ),
        make_pokemon(
            Species::Pikachu,
            28,
            55,
            45,
            30,
            80,
            45,
            PokemonType::Electric,
            PokemonType::Electric,
            [
                MoveId::Thunderbolt,
                MoveId::QuickAttack,
                MoveId::None,
                MoveId::None,
            ],
            [15, 30, 0, 0],
        ),
        make_pokemon(
            Species::Kadabra,
            30,
            50,
            25,
            20,
            90,
            100,
            PokemonType::Psychic,
            PokemonType::Psychic,
            [
                MoveId::Psybeam,
                MoveId::Confusion,
                MoveId::None,
                MoveId::None,
            ],
            [20, 25, 0, 0],
        ),
    ])
    .unwrap()
}

pub fn party_elite_four() -> Party {
    Party::from_pokemon(vec![
        make_pokemon(
            Species::Charizard,
            63,
            180,
            120,
            95,
            120,
            105,
            PokemonType::Fire,
            PokemonType::Flying,
            [
                MoveId::Slash,
                MoveId::FireBlast,
                MoveId::Earthquake,
                MoveId::Strength,
            ],
            [20, 5, 10, 15],
        ),
        make_pokemon(
            Species::Alakazam,
            60,
            140,
            50,
            45,
            145,
            155,
            PokemonType::Psychic,
            PokemonType::Psychic,
            [
                MoveId::PsychicM,
                MoveId::Recover,
                MoveId::ThunderWave,
                MoveId::None,
            ],
            [10, 20, 20, 0],
        ),
        make_pokemon(
            Species::Lapras,
            58,
            220,
            95,
            90,
            70,
            105,
            PokemonType::Water,
            PokemonType::Ice,
            [
                MoveId::Surf,
                MoveId::IceBeam,
                MoveId::Thunderbolt,
                MoveId::None,
            ],
            [15, 10, 15, 0],
        ),
        make_pokemon(
            Species::Snorlax,
            60,
            280,
            130,
            80,
            40,
            80,
            PokemonType::Normal,
            PokemonType::Normal,
            [
                MoveId::BodySlam,
                MoveId::Earthquake,
                MoveId::Rest,
                MoveId::None,
            ],
            [15, 10, 10, 0],
        ),
        make_pokemon(
            Species::Jolteon,
            57,
            145,
            75,
            70,
            150,
            130,
            PokemonType::Electric,
            PokemonType::Electric,
            [
                MoveId::Thunderbolt,
                MoveId::ThunderWave,
                MoveId::DoubleKick,
                MoveId::None,
            ],
            [15, 20, 30, 0],
        ),
        make_pokemon(
            Species::Rhydon,
            58,
            190,
            150,
            140,
            50,
            55,
            PokemonType::Ground,
            PokemonType::Rock,
            [
                MoveId::Earthquake,
                MoveId::RockSlide,
                MoveId::Strength,
                MoveId::None,
            ],
            [10, 10, 15, 0],
        ),
    ])
    .unwrap()
}

pub fn pokedex_early_game() -> Pokedex {
    let mut dex = Pokedex::new();
    dex.set_owned(Species::Charmander);
    dex.set_seen(Species::Pidgey);
    dex.set_seen(Species::Rattata);
    dex.set_owned(Species::Pidgey);
    dex
}

pub fn pokedex_mid_game() -> Pokedex {
    let mut dex = Pokedex::new();
    let seen_species = [
        Species::Bulbasaur,
        Species::Charmander,
        Species::Squirtle,
        Species::Pidgey,
        Species::Pidgeotto,
        Species::Rattata,
        Species::Raticate,
        Species::Spearow,
        Species::Ekans,
        Species::Pikachu,
        Species::NidoranF,
        Species::NidoranM,
        Species::Clefairy,
        Species::Vulpix,
        Species::Jigglypuff,
        Species::Zubat,
        Species::Golbat,
        Species::Oddish,
        Species::Paras,
        Species::Diglett,
        Species::Meowth,
        Species::Psyduck,
        Species::Mankey,
        Species::Growlithe,
        Species::Poliwag,
        Species::Abra,
        Species::Machop,
        Species::Bellsprout,
        Species::Tentacool,
        Species::Geodude,
        Species::Ponyta,
        Species::Slowpoke,
        Species::Magnemite,
        Species::Doduo,
        Species::Onix,
        Species::Voltorb,
        Species::Gastly,
        Species::Drowzee,
        Species::Krabby,
        Species::Exeggcute,
    ];
    for &s in &seen_species {
        dex.set_seen(s);
    }
    let owned_species = [
        Species::Charmander,
        Species::Charmeleon,
        Species::Pidgey,
        Species::Pidgeotto,
        Species::Rattata,
        Species::Pikachu,
        Species::NidoranM,
        Species::Nidorino,
        Species::Zubat,
        Species::Oddish,
        Species::Diglett,
        Species::Geodude,
        Species::Abra,
        Species::Kadabra,
        Species::Machop,
        Species::Bellsprout,
        Species::Gastly,
        Species::Drowzee,
    ];
    for &s in &owned_species {
        dex.set_owned(s);
    }
    dex
}
