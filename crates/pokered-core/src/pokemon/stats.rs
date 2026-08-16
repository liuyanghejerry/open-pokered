use pokered_data::moves::MoveId;
use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::Species;

use crate::battle::experience::stats::{
    calc_all_stats, extract_atk_iv, extract_def_iv, extract_hp_iv, extract_spc_iv, extract_spd_iv,
};
use crate::battle::state::{Pokemon, StatusCondition};

pub use crate::battle::experience::stats::{
    calc_stat as raw_calc_stat, extract_atk_iv as atk_iv, extract_def_iv as def_iv,
    extract_hp_iv as hp_iv, extract_spc_iv as spc_iv, extract_spd_iv as spd_iv,
};

pub fn recalculate_stats(mon: &mut Pokemon) {
    let base = match get_base_stats(mon.species) {
        Some(b) => b,
        None => return,
    };

    let (hp, atk, def, spd, spc) = calc_all_stats(base, mon.dv_bytes, &mon.stat_exp, mon.level);

    let old_max_hp = mon.max_hp;
    mon.max_hp = hp;
    mon.attack = atk;
    mon.defense = def;
    mon.speed = spd;
    mon.special = spc;

    if old_max_hp > 0 {
        let hp_delta = hp.saturating_sub(old_max_hp);
        mon.hp = mon.hp.saturating_add(hp_delta);
    } else {
        mon.hp = hp;
    }
}

/// DV bytes for enemy trainer Pokémon — ATKDEFDV_TRAINER / SPDSPCDV_TRAINER
/// (constants/battle_constants.asm:72-73, applied in add_mon.asm:78-79).
pub const TRAINER_DV_BYTES: [u8; 2] = [0x98, 0x88];

/// Roll the two random DV bytes used by every non-trainer `AddPartyMon` path:
/// wild encounters (`BattleRandom` ×2, core.asm:6012-6019) and gifted/starter
/// mons (`Random` ×2, add_mon.asm:95-101).
pub fn roll_random_dvs() -> [u8; 2] {
    [rand::random::<u8>(), rand::random::<u8>()]
}

pub fn create_pokemon(species: Species, level: u8, dv_bytes: [u8; 2]) -> Option<Pokemon> {
    let base = get_base_stats(species)?;

    let (hp, atk, def, spd, spc) = calc_all_stats(base, dv_bytes, &[0u16; 5], level);

    // Start from the L1 initial moves, then apply level-up learnset entries
    // (Gen-1 WriteMonMoves via AddPartyMon, evos_moves.asm:383-470).
    let mut moves = base.initial_moves;
    super::move_learning::write_mon_moves(species, level, &mut moves);

    Some(Pokemon {
        species,
        nickname: [0x50; 11],
        level,
        hp,
        max_hp: hp,
        attack: atk,
        defense: def,
        speed: spd,
        special: spc,
        type1: base.type1,
        type2: base.type2,
        moves,
        pp: [
            super::move_learning::get_move_max_pp(moves[0]),
            super::move_learning::get_move_max_pp(moves[1]),
            super::move_learning::get_move_max_pp(moves[2]),
            super::move_learning::get_move_max_pp(moves[3]),
        ],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes,
        stat_exp: [0; 5],
        total_exp: crate::battle::experience::growth::exp_for_level(base.growth_rate, level),
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    })
}

pub fn create_pokemon_with_moves(
    species: Species,
    level: u8,
    dv_bytes: [u8; 2],
    moves: [MoveId; 4],
) -> Option<Pokemon> {
    let mut mon = create_pokemon(species, level, dv_bytes)?;
    mon.moves = moves;
    mon.pp = [
        super::move_learning::get_move_max_pp(moves[0]),
        super::move_learning::get_move_max_pp(moves[1]),
        super::move_learning::get_move_max_pp(moves[2]),
        super::move_learning::get_move_max_pp(moves[3]),
    ];
    Some(mon)
}

pub fn stat_summary(mon: &Pokemon) -> StatSummary {
    StatSummary {
        hp: mon.max_hp,
        attack: mon.attack,
        defense: mon.defense,
        speed: mon.speed,
        special: mon.special,
        hp_iv: extract_hp_iv(mon.dv_bytes),
        atk_iv: extract_atk_iv(mon.dv_bytes),
        def_iv: extract_def_iv(mon.dv_bytes),
        spd_iv: extract_spd_iv(mon.dv_bytes),
        spc_iv: extract_spc_iv(mon.dv_bytes),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatSummary {
    pub hp: u16,
    pub attack: u16,
    pub defense: u16,
    pub speed: u16,
    pub special: u16,
    pub hp_iv: u8,
    pub atk_iv: u8,
    pub def_iv: u8,
    pub spd_iv: u8,
    pub spc_iv: u8,
}
