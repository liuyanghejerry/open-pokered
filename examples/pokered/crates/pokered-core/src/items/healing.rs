use crate::battle::state::{Pokemon, StatusCondition};
use pokered_data::items::ItemId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealResult {
    Healed { hp_restored: u16 },
    Revived { hp_restored: u16 },
    AlreadyFullHp,
    NotFainted,
    NotApplicable,
}

pub fn heal_amount_for_item(item: ItemId) -> Option<HealAmount> {
    match item {
        ItemId::Potion => Some(HealAmount::Fixed(20)),
        ItemId::SuperPotion => Some(HealAmount::Fixed(50)),
        ItemId::HyperPotion => Some(HealAmount::Fixed(200)),
        ItemId::MaxPotion => Some(HealAmount::Full),
        ItemId::FullRestore => Some(HealAmount::FullRestore),
        ItemId::FreshWater => Some(HealAmount::Fixed(50)),
        ItemId::SodaPop => Some(HealAmount::Fixed(60)),
        ItemId::Lemonade => Some(HealAmount::Fixed(80)),
        ItemId::Revive => Some(HealAmount::Revive),
        ItemId::MaxRevive => Some(HealAmount::MaxRevive),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealAmount {
    Fixed(u16),
    Full,
    FullRestore,
    Revive,
    MaxRevive,
}

pub fn use_healing_item(mon: &mut Pokemon, item: ItemId) -> HealResult {
    let amount = match heal_amount_for_item(item) {
        Some(a) => a,
        None => return HealResult::NotApplicable,
    };

    match amount {
        HealAmount::Revive => use_revive(mon, false),
        HealAmount::MaxRevive => use_revive(mon, true),
        HealAmount::FullRestore => use_full_restore(mon),
        HealAmount::Full => use_max_potion(mon),
        HealAmount::Fixed(hp) => use_potion(mon, hp),
    }
}

fn use_potion(mon: &mut Pokemon, amount: u16) -> HealResult {
    if mon.hp == 0 {
        return HealResult::NotFainted;
    }
    if mon.hp >= mon.max_hp {
        return HealResult::AlreadyFullHp;
    }
    let old_hp = mon.hp;
    mon.hp = mon.hp.saturating_add(amount).min(mon.max_hp);
    HealResult::Healed {
        hp_restored: mon.hp - old_hp,
    }
}

fn use_max_potion(mon: &mut Pokemon) -> HealResult {
    if mon.hp == 0 {
        return HealResult::NotFainted;
    }
    if mon.hp >= mon.max_hp {
        return HealResult::AlreadyFullHp;
    }
    let old_hp = mon.hp;
    mon.hp = mon.max_hp;
    HealResult::Healed {
        hp_restored: mon.hp - old_hp,
    }
}

fn use_full_restore(mon: &mut Pokemon) -> HealResult {
    if mon.hp == 0 {
        return HealResult::NotFainted;
    }
    if mon.hp >= mon.max_hp && mon.status == StatusCondition::None {
        return HealResult::AlreadyFullHp;
    }
    let old_hp = mon.hp;
    mon.hp = mon.max_hp;
    mon.status = StatusCondition::None;
    HealResult::Healed {
        hp_restored: mon.hp - old_hp,
    }
}

fn use_revive(mon: &mut Pokemon, full: bool) -> HealResult {
    if mon.hp > 0 {
        return HealResult::NotFainted;
    }
    let restored = if full { mon.max_hp } else { mon.max_hp / 2 };
    mon.hp = restored.max(1);
    HealResult::Revived {
        hp_restored: mon.hp,
    }
}
