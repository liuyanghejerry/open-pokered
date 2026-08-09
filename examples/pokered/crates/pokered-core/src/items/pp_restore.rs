use crate::battle::state::Pokemon;
use crate::pokemon::move_learning::get_move_max_pp;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;

pub const MAX_PP_UPS_PER_MOVE: u8 = 3;
pub const PP_UP_BONUS_CAP: u8 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PpRestoreResult {
    Restored {
        move_index: usize,
        pp_restored: u8,
    },
    AllRestored {
        total_pp_restored: u16,
    },
    PpUpApplied {
        move_index: usize,
        new_pp_up_count: u8,
    },
    NoEffect,
    NotApplicable,
}

pub fn get_max_pp_with_ups(move_id: MoveId, pp_ups: u8) -> u8 {
    let base = get_move_max_pp(move_id);
    if base == 0 {
        return 0;
    }
    let bonus_per_up = (base / 5).min(PP_UP_BONUS_CAP);
    base + pp_ups * bonus_per_up
}

pub fn use_pp_restore(mon: &mut Pokemon, item: ItemId, move_index: usize) -> PpRestoreResult {
    match item {
        ItemId::Ether => use_ether(mon, move_index, false),
        ItemId::MaxEther => use_ether(mon, move_index, true),
        ItemId::Elixer => use_elixir(mon, false),
        ItemId::MaxElixer => use_elixir(mon, true),
        ItemId::PpUp => use_pp_up(mon, move_index),
        _ => PpRestoreResult::NotApplicable,
    }
}

fn use_ether(mon: &mut Pokemon, move_index: usize, full: bool) -> PpRestoreResult {
    if move_index >= 4 || mon.moves[move_index] == MoveId::None {
        return PpRestoreResult::NoEffect;
    }
    let max_pp = get_max_pp_with_ups(mon.moves[move_index], mon.pp_ups[move_index]);
    if mon.pp[move_index] >= max_pp {
        return PpRestoreResult::NoEffect;
    }
    let old_pp = mon.pp[move_index];
    mon.pp[move_index] = if full {
        max_pp
    } else {
        (mon.pp[move_index] + 10).min(max_pp)
    };
    PpRestoreResult::Restored {
        move_index,
        pp_restored: mon.pp[move_index] - old_pp,
    }
}

fn use_elixir(mon: &mut Pokemon, full: bool) -> PpRestoreResult {
    let mut total_restored: u16 = 0;
    for i in 0..4 {
        if mon.moves[i] == MoveId::None {
            continue;
        }
        let max_pp = get_max_pp_with_ups(mon.moves[i], mon.pp_ups[i]);
        if mon.pp[i] < max_pp {
            let old_pp = mon.pp[i];
            mon.pp[i] = if full {
                max_pp
            } else {
                (mon.pp[i] + 10).min(max_pp)
            };
            total_restored += (mon.pp[i] - old_pp) as u16;
        }
    }
    if total_restored == 0 {
        PpRestoreResult::NoEffect
    } else {
        PpRestoreResult::AllRestored {
            total_pp_restored: total_restored,
        }
    }
}

fn use_pp_up(mon: &mut Pokemon, move_index: usize) -> PpRestoreResult {
    if move_index >= 4 || mon.moves[move_index] == MoveId::None {
        return PpRestoreResult::NoEffect;
    }
    if mon.pp_ups[move_index] >= MAX_PP_UPS_PER_MOVE {
        return PpRestoreResult::NoEffect;
    }
    mon.pp_ups[move_index] += 1;
    let new_max = get_max_pp_with_ups(mon.moves[move_index], mon.pp_ups[move_index]);
    mon.pp[move_index] = new_max;
    PpRestoreResult::PpUpApplied {
        move_index,
        new_pp_up_count: mon.pp_ups[move_index],
    }
}
