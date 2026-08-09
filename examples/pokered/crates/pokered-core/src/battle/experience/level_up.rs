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
    for lv in (old_level + 1)..=new_level {
        if let Some(move_id) = learn_move_at_level(mon, lv) {
            learned.push(move_id);
        }
    }

    LevelUpResult {
        leveled_up: true,
        old_level,
        new_level,
        learned_moves: learned,
    }
}

fn learn_move_at_level(mon: &mut Pokemon, level: u8) -> Option<MoveId> {
    let all_data = evos_moves_data();
    let entry = all_data.iter().find(|e| e.species == mon.species)?;
    let move_to_learn = entry.learnset.iter().find(|lm| lm.level == level)?;

    let move_id = move_to_learn.move_id;

    if mon.moves.contains(&move_id) {
        return None;
    }

    for i in 0..4 {
        if mon.moves[i] == MoveId::None {
            mon.moves[i] = move_id;
            mon.pp[i] = get_move_max_pp(move_id);
            return Some(move_id);
        }
    }

    // All slots full — replace last slot (real game prompts player)
    mon.moves[3] = move_id;
    mon.pp[3] = get_move_max_pp(move_id);
    Some(move_id)
}

fn get_move_max_pp(move_id: MoveId) -> u8 {
    use pokered_data::move_data::MOVES;
    MOVES
        .iter()
        .find(|m| m.id == move_id)
        .map(|m| m.pp)
        .unwrap_or(0)
}
