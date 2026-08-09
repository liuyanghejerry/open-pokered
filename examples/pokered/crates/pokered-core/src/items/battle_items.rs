use crate::battle::stat_stages::StatIndex;
use crate::battle::state::{status2, BattlerState};
use pokered_data::items::ItemId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BattleItemResult {
    StatBoosted { stat: StatIndex },
    FlagSet,
    Escaped,
    NoEffect,
    NotApplicable,
}

pub fn use_battle_item(battler: &mut BattlerState, item: ItemId) -> BattleItemResult {
    match item {
        ItemId::XAttack => use_x_stat(battler, StatIndex::Attack),
        ItemId::XDefend => use_x_stat(battler, StatIndex::Defense),
        ItemId::XSpeed => use_x_stat(battler, StatIndex::Speed),
        ItemId::XSpecial => use_x_stat(battler, StatIndex::Special),
        ItemId::XAccuracy => {
            if battler.has_status2(status2::USING_X_ACCURACY) {
                return BattleItemResult::NoEffect;
            }
            battler.set_status2(status2::USING_X_ACCURACY);
            BattleItemResult::FlagSet
        }
        ItemId::GuardSpec => {
            if battler.has_status2(status2::PROTECTED_BY_MIST) {
                return BattleItemResult::NoEffect;
            }
            battler.set_status2(status2::PROTECTED_BY_MIST);
            BattleItemResult::FlagSet
        }
        ItemId::DireHit => {
            if battler.has_status2(status2::GETTING_PUMPED) {
                return BattleItemResult::NoEffect;
            }
            battler.set_status2(status2::GETTING_PUMPED);
            BattleItemResult::FlagSet
        }
        _ => BattleItemResult::NotApplicable,
    }
}

pub fn use_poke_doll() -> BattleItemResult {
    BattleItemResult::Escaped
}

fn use_x_stat(battler: &mut BattlerState, stat: StatIndex) -> BattleItemResult {
    let changed = battler.stat_stages.modify(stat, 1);
    if changed {
        BattleItemResult::StatBoosted { stat }
    } else {
        BattleItemResult::NoEffect
    }
}
