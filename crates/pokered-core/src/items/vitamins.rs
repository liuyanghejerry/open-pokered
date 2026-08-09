use crate::battle::state::Pokemon;
use crate::pokemon::move_learning::{process_level_up_moves, LevelUpMovesResult};
use crate::pokemon::stats::recalculate_stats;
use pokered_data::items::ItemId;
use pokered_data::pokemon_data::get_base_stats;

pub const VITAMIN_STAT_EXP_GAIN: u16 = 2560;
pub const VITAMIN_STAT_EXP_CAP: u16 = 25600;
pub const MAX_LEVEL: u8 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VitaminResult {
    Applied {
        stat_index: usize,
        new_stat_exp: u16,
    },
    NoEffect,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RareCandyResult {
    pub new_level: u8,
    pub move_result: LevelUpMovesResult,
}

pub fn use_vitamin(mon: &mut Pokemon, item: ItemId) -> VitaminResult {
    let stat_index = match item {
        ItemId::HpUp => 0,
        ItemId::Protein => 1,
        ItemId::Iron => 2,
        ItemId::Carbos => 3,
        ItemId::Calcium => 4,
        _ => return VitaminResult::NotApplicable,
    };

    if mon.hp == 0 {
        return VitaminResult::NoEffect;
    }
    if mon.stat_exp[stat_index] >= VITAMIN_STAT_EXP_CAP {
        return VitaminResult::NoEffect;
    }

    mon.stat_exp[stat_index] =
        (mon.stat_exp[stat_index] + VITAMIN_STAT_EXP_GAIN).min(VITAMIN_STAT_EXP_CAP);
    recalculate_stats(mon);

    VitaminResult::Applied {
        stat_index,
        new_stat_exp: mon.stat_exp[stat_index],
    }
}

/// Rare Candy (`ItemUseRareCandy`, engine/items/item_effects.asm): level +1,
/// stats recomputed, level-up moves learned. The evolution check
/// (`TryEvolvingMon` with `wForceEvolution = 0`, item_effects.asm:1409-1411)
/// is deliberately NOT done here — the caller runs it through the evolution
/// cutscene (`crate::evolution_screen`), which handles the B-cancel.
pub fn use_rare_candy(mon: &mut Pokemon) -> Option<RareCandyResult> {
    if mon.hp == 0 || mon.level >= MAX_LEVEL {
        return None;
    }

    let old_level = mon.level;
    mon.level += 1;

    let base = get_base_stats(mon.species)?;
    mon.total_exp = crate::battle::experience::growth::exp_for_level(base.growth_rate, mon.level);

    recalculate_stats(mon);

    let move_result = process_level_up_moves(mon, old_level, mon.level);

    Some(RareCandyResult {
        new_level: mon.level,
        move_result,
    })
}
