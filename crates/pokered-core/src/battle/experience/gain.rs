use crate::alloc_prelude::*;
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
    /// Actual displayed awards, in the original's participant / EXP ALL order.
    pub exp_gains: Vec<(usize, u32)>,
    /// Gained / level / learned notices are emitted together for each award.
    pub notices: Vec<ExperienceNotice>,
    pub leveled_up: Vec<usize>,
    pub new_moves: Vec<(usize, pokered_data::moves::MoveId)>,
    /// (party index, move) pairs whose learn attempt hit a FULL moveset — the
    /// battle prompts the player to forget a move (learnmove.asm) instead of
    /// silently overwriting.
    pub blocked_moves: Vec<(usize, pokered_data::moves::MoveId)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExperienceNotice {
    Gained { party_index: usize, amount: u32 },
    Leveled { party_index: usize, level: u8 },
    Learned { party_index: usize, move_id: pokered_data::moves::MoveId },
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
                exp_gains: vec![],
                notices: vec![],
                leveled_up: vec![],
                new_moves: vec![],
                blocked_moves: vec![],
            }
        }
    };

    let is_trainer = state.battle_type == BattleType::Trainer;

    let num_gainers = state.party_gain_exp_flags.iter().filter(|&&f| f).count() as u32;
    if num_gainers == 0 && !has_exp_all {
        return GainExpResult {
            exp_gains: vec![],
            notices: vec![],
            leveled_up: vec![],
            new_moves: vec![],
            blocked_moves: vec![],
        };
    }

    let mut leveled_up = vec![];
    let mut new_moves = vec![];
    let mut blocked_moves = vec![];
    let mut exp_gains = vec![];
    let mut notices = vec![];

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
        if let Some(exp) = gain_one(
            state, i, &data, defeated_level, is_trainer, &mut leveled_up, &mut new_moves,
            &mut blocked_moves, &mut notices,
        ) { exp_gains.push((i, exp)); }
    }

    // Pass 2 (EXP ALL only): every party member gets a share of the (already
    // halved and participant-divided) data, divided by the party count.
    if has_exp_all {
        let party_count = state.player.party.len() as u32;
        if party_count >= 2 {
            data = divide_base(&data, party_count);
        }
        for i in 0..state.player.party.len() {
            if let Some(exp) = gain_one(
                state, i, &data, defeated_level, is_trainer, &mut leveled_up, &mut new_moves,
                &mut blocked_moves, &mut notices,
            ) { exp_gains.push((i, exp)); }
        }
    }

    // GainExperience.done clears the flags after each defeated enemy and
    // seeds only the mon that is still out for the next enemy.
    state.party_gain_exp_flags.fill(false);
    state.party_gain_exp_flags[state.player.active_pokemon_index] = true;
    GainExpResult {
        exp_gains,
        notices,
        leveled_up,
        new_moves,
        blocked_moves,
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
    blocked_moves: &mut Vec<(usize, pokered_data::moves::MoveId)>,
    notices: &mut Vec<ExperienceNotice>,
) -> Option<u32> {
    // Experience uses the party identity, even while the battle copy has
    // Transform/Mimic's temporary species and moves (experience.asm LoadMonData).
    let battle_copy = state.player.original_identity.as_ref()
        .filter(|(index, _)| *index == i).map(|_| state.player.party[i].clone());
    if battle_copy.is_some() { state.player.party[i] = state.player.persistent_party()[i].clone(); }
    let mon = &mut state.player.party[i];
    if mon.hp == 0 {
        if let Some(battle_copy) = battle_copy { state.player.party[i] = battle_copy; }
        return None; // fainted mons gain no EXP (experience.asm:9-11)
    }
    add_stat_exp(mon, data);
    let exp = calc_exp_gain(data.base_exp, defeated_level, mon.is_traded, is_trainer);
    let growth_rate = get_base_stats(mon.species).map(|b| b.growth_rate).unwrap();
    let max = max_exp(growth_rate);
    mon.total_exp = (mon.total_exp + exp).min(max);

    let result = process_level_up(mon);
    notices.push(ExperienceNotice::Gained { party_index: i, amount: exp });
    if result.leveled_up {
        notices.push(ExperienceNotice::Leveled { party_index: i, level: result.new_level });
    }
    if result.leveled_up && !leveled_up.contains(&i) {
        leveled_up.push(i);
    }
    let learned = !result.learned_moves.is_empty();
    for m in result.learned_moves {
        notices.push(ExperienceNotice::Learned { party_index: i, move_id: m });
        new_moves.push((i, m));
    }
    for m in result.blocked_moves {
        blocked_moves.push((i, m));
    }
    if let Some(mut battle_copy) = battle_copy {
        let original = state.player.party[i].clone();
        battle_copy.hp = original.hp;
        battle_copy.level = original.level;
        battle_copy.total_exp = original.total_exp;
        battle_copy.stat_exp = original.stat_exp;
        if result.leveled_up {
            battle_copy.max_hp = original.max_hp;
            battle_copy.attack = original.attack; battle_copy.defense = original.defense;
            battle_copy.speed = original.speed; battle_copy.special = original.special;
        }
        // LearnMove copies all four party move/PP slots back to the active
        // battle mon, including a transformed mon (learn_move.asm:61-73).
        if learned { battle_copy.moves = original.moves; battle_copy.pp = original.pp; }
        state.player.original_identity = Some((i, original));
        state.player.party[i] = battle_copy;
    }
    Some(exp)
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
