use crate::alloc_prelude::*;
use pokered_data::evos_moves::evos_moves_data;
use pokered_data::moves::MoveId;
use pokered_data::pokemon_data::get_base_stats;

use crate::battle::state::Pokemon;

use super::growth::level_from_exp;
use super::stats::calc_all_stats;

pub struct LevelUpResult {
    pub leveled_up: bool,
    pub old_level: u8,
    pub new_level: u8,
    pub learned_moves: Vec<MoveId>,
    /// Moves whose learn attempt hit a FULL move-slot set (learnmove.asm):
    /// the mon is "trying to learn" them and the game prompts the player to
    /// forget a move — NOT silently overwritten.
    pub blocked_moves: Vec<MoveId>,
}

pub fn process_level_up(mon: &mut Pokemon) -> LevelUpResult {
    let base = match get_base_stats(mon.species) {
        Some(b) => b,
        None => {
            return LevelUpResult {
                leveled_up: false,
                old_level: mon.level,
                new_level: mon.level,
                learned_moves: vec![],
                blocked_moves: vec![],
            }
        }
    };

    let new_level = level_from_exp(base.growth_rate, mon.total_exp);
    if new_level <= mon.level {
        return LevelUpResult {
            leveled_up: false,
            old_level: mon.level,
            new_level: mon.level,
            learned_moves: vec![],
            blocked_moves: vec![],
        };
    }

    let old_level = mon.level;
    let old_max_hp = mon.max_hp;

    let (new_hp, new_atk, new_def, new_spd, new_spc) =
        calc_all_stats(base, mon.dv_bytes, &mon.stat_exp, new_level);

    let hp_delta = new_hp.saturating_sub(old_max_hp);
    mon.hp = mon.hp.saturating_add(hp_delta);
    mon.max_hp = new_hp;
    mon.attack = new_atk;
    mon.defense = new_def;
    mon.speed = new_spd;
    mon.special = new_spc;
    mon.level = new_level;

    let mut learned = vec![];
    let mut blocked = vec![];
    for lv in (old_level + 1)..=new_level {
        match learn_move_at_level(mon, lv) {
            LearnMoveAtLevel::Learned(move_id) => learned.push(move_id),
            LearnMoveAtLevel::AlreadyKnown => {}
            LearnMoveAtLevel::SlotsFull(move_id) => blocked.push(move_id),
        }
    }

    LevelUpResult {
        leveled_up: true,
        old_level,
        new_level,
        learned_moves: learned,
        blocked_moves: blocked,
    }
}

/// Outcome of a level-up learn attempt (`learnmove.asm`): a free slot learns
/// immediately; a move already known is skipped; a FULL moveset leaves the
/// move "pending" for the forget/replace prompt.
enum LearnMoveAtLevel {
    Learned(MoveId),
    AlreadyKnown,
    SlotsFull(MoveId),
}

fn learn_move_at_level(mon: &mut Pokemon, level: u8) -> LearnMoveAtLevel {
    let all_data = evos_moves_data();
    let Some(entry) = all_data.iter().find(|e| e.species == mon.species) else {
        return LearnMoveAtLevel::AlreadyKnown;
    };
    let Some(move_to_learn) = entry.learnset.iter().find(|lm| lm.level == level) else {
        return LearnMoveAtLevel::AlreadyKnown;
    };

    let move_id = move_to_learn.move_id;

    if mon.moves.contains(&move_id) {
        return LearnMoveAtLevel::AlreadyKnown;
    }

    for i in 0..4 {
        if mon.moves[i] == MoveId::None {
            mon.moves[i] = move_id;
            mon.pp[i] = get_move_max_pp(move_id);
            return LearnMoveAtLevel::Learned(move_id);
        }
    }

    // All slots full — the game prompts the player to forget a move
    // (learnmove.asm); the mon does NOT silently lose its 4th move.
    LearnMoveAtLevel::SlotsFull(move_id)
}

fn get_move_max_pp(move_id: MoveId) -> u8 {
    use pokered_data::move_data::MOVES;
    MOVES
        .iter()
        .find(|m| m.id == move_id)
        .map(|m| m.pp)
        .unwrap_or(0)
}
