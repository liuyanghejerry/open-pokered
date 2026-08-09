use pokered_data::pokemon_data::{get_base_stats, BaseStats};
use pokered_data::species::Species;

use crate::battle::state::{BattleState, BattleType, Pokemon};

use super::growth::max_exp;
use super::level_up::process_level_up;

pub fn calc_exp_gain(base_exp: u8, enemy_level: u8, is_traded: bool, is_trainer: bool) -> u32 {
    let raw = (base_exp as u32 * enemy_level as u32) / 7;
    let mut exp = raw;
    if is_traded {
        exp = (exp * 3) / 2;
    }
    if is_trainer {
        exp = (exp * 3) / 2;
    }
    exp
}

pub fn add_stat_exp(mon: &mut Pokemon, enemy_base: &BaseStats) {
    mon.stat_exp[0] = mon.stat_exp[0].saturating_add(enemy_base.hp as u16);
    mon.stat_exp[1] = mon.stat_exp[1].saturating_add(enemy_base.attack as u16);
    mon.stat_exp[2] = mon.stat_exp[2].saturating_add(enemy_base.defense as u16);
    mon.stat_exp[3] = mon.stat_exp[3].saturating_add(enemy_base.speed as u16);
    mon.stat_exp[4] = mon.stat_exp[4].saturating_add(enemy_base.special as u16);
}

pub struct GainExpResult {
    pub leveled_up: Vec<usize>,
    pub new_moves: Vec<(usize, pokered_data::moves::MoveId)>,
}

pub fn gain_experience(
    state: &mut BattleState,
    defeated_species: Species,
    defeated_level: u8,
    has_exp_all: bool,
) -> GainExpResult {
    let enemy_base = match get_base_stats(defeated_species) {
        Some(b) => b,
        None => {
            return GainExpResult {
                leveled_up: vec![],
                new_moves: vec![],
            }
        }
    };

    let is_trainer = state.battle_type == BattleType::Trainer;

    let num_gainers = state.party_gain_exp_flags.iter().filter(|&&f| f).count() as u32;
    if num_gainers == 0 && !has_exp_all {
        return GainExpResult {
            leveled_up: vec![],
            new_moves: vec![],
        };
    }

    let mut leveled_up = vec![];
    let mut new_moves = vec![];

    // Original structure (core.asm:818-857 + experience.asm
    // DivideExpDataByNumMonsGainingExp): the enemy's base stats AND base exp are
    // pre-divided by the number of mons gaining exp (the stat EXP is divided
    // too). With EXP ALL the data is HALVED first, then GainExperience runs
    // TWICE — pass 1 over the battle participants, pass 2 over the whole party
    // — and pass 2's division applies to the ALREADY pass-1-divided data (the
    // original mutates wEnemyMonBaseStats in place, quirk included).
    let mut data = enemy_base.clone();
    if has_exp_all {
        data = divide_base(&data, 2);
    }

    // Pass 1: the mons that actually fought (flagged). Fainted mons (HP 0) are
    // skipped — experience.asm:9-11 — but still COUNT toward the division.
    if num_gainers >= 2 {
        data = divide_base(&data, num_gainers);
    }
    for i in 0..state.player.party.len() {
        if !state.party_gain_exp_flags[i] {
            continue;
        }
        gain_one(
            state, i, &data, defeated_level, is_trainer, &mut leveled_up, &mut new_moves,
        );
    }

    // Pass 2 (EXP ALL only): every party member gets a share of the (already
    // halved and participant-divided) data, divided by the party count.
    if has_exp_all {
        let party_count = state.player.party.len() as u32;
        if party_count >= 2 {
            data = divide_base(&data, party_count);
        }
        for i in 0..state.player.party.len() {
            gain_one(
                state, i, &data, defeated_level, is_trainer, &mut leveled_up, &mut new_moves,
            );
        }
    }

    GainExpResult {
        leveled_up,
        new_moves,
    }
}

/// Award one party mon its share: stat EXP + EXP (traded ×1.5 / trainer ×1.5
/// boosts), capped at max-level exp, then process any level-up. Fainted mons
/// (HP 0) gain NOTHING (experience.asm:9-11 skips them).
#[allow(clippy::too_many_arguments)]
fn gain_one(
    state: &mut BattleState,
    i: usize,
    data: &BaseStats,
    defeated_level: u8,
    is_trainer: bool,
    leveled_up: &mut Vec<usize>,
    new_moves: &mut Vec<(usize, pokered_data::moves::MoveId)>,
) {
    let mon = &mut state.player.party[i];
    if mon.hp == 0 {
        return; // fainted mons gain no EXP (experience.asm:9-11)
    }
    add_stat_exp(mon, data);
    let exp = calc_exp_gain(data.base_exp, defeated_level, mon.is_traded, is_trainer);
    let growth_rate = get_base_stats(mon.species).map(|b| b.growth_rate).unwrap();
    let max = max_exp(growth_rate);
    mon.total_exp = (mon.total_exp + exp).min(max);

    let result = process_level_up(mon);
    if result.leveled_up && !leveled_up.contains(&i) {
        leveled_up.push(i);
    }
    for m in result.learned_moves {
        new_moves.push((i, m));
    }
}

/// Divide the enemy base stats + base exp by `n` — the asm's
/// DivideExpDataByNumMonsGainingExp, which divides each data byte in place
/// (integer division). Catch rate is divided too in the original but is unused
/// for EXP gain here.
fn divide_base(base: &BaseStats, n: u32) -> BaseStats {
    let d = |x: u8| (x as u32 / n) as u8;
    BaseStats {
        species: base.species,
        hp: d(base.hp),
        attack: d(base.attack),
        defense: d(base.defense),
        speed: d(base.speed),
        special: d(base.special),
        type1: base.type1,
        type2: base.type2,
        catch_rate: d(base.catch_rate),
        base_exp: d(base.base_exp),
        initial_moves: base.initial_moves,
        growth_rate: base.growth_rate,
        tm_hm_flags: base.tm_hm_flags,
    }
}
