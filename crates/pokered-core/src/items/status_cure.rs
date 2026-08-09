use crate::battle::state::{Pokemon, StatusCondition};
use pokered_data::items::ItemId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusCureResult {
    Cured,
    NoEffect,
    NotApplicable,
}

pub fn use_status_cure(mon: &mut Pokemon, item: ItemId) -> StatusCureResult {
    match item {
        ItemId::Antidote => cure_specific(mon, |s| matches!(s, StatusCondition::Poison)),
        ItemId::BurnHeal => cure_specific(mon, |s| matches!(s, StatusCondition::Burn)),
        ItemId::IceHeal => cure_specific(mon, |s| matches!(s, StatusCondition::Freeze)),
        ItemId::Awakening => cure_specific(mon, |s| matches!(s, StatusCondition::Sleep(_))),
        ItemId::ParlyzHeal => cure_specific(mon, |s| matches!(s, StatusCondition::Paralysis)),
        ItemId::FullHeal => cure_all(mon),
        _ => StatusCureResult::NotApplicable,
    }
}

fn cure_specific(
    mon: &mut Pokemon,
    matches: impl Fn(&StatusCondition) -> bool,
) -> StatusCureResult {
    if mon.hp == 0 {
        return StatusCureResult::NoEffect;
    }
    if matches(&mon.status) {
        mon.status = StatusCondition::None;
        StatusCureResult::Cured
    } else {
        StatusCureResult::NoEffect
    }
}

fn cure_all(mon: &mut Pokemon) -> StatusCureResult {
    if mon.hp == 0 {
        return StatusCureResult::NoEffect;
    }
    if mon.status == StatusCondition::None {
        return StatusCureResult::NoEffect;
    }
    mon.status = StatusCondition::None;
    StatusCureResult::Cured
}
