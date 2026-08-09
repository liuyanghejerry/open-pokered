use super::state::{status2, status3, BattlerState, StatusCondition};

/// Result of applying residual damage after a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualResult {
    /// No residual damage applied.
    None,
    /// Damage was applied (burn, poison, toxic).
    Damaged(u16),
    /// Leech Seed drained HP and healed opponent.
    LeechSeedDrain { drained: u16, healed: u16 },
    /// Mon fainted from residual damage.
    Fainted,
}

/// Apply burn/poison/toxic residual damage to a battler.
///
/// ASM: HandleResidualDamage (after each side's move)
/// - Burn/Poison: floor(maxHP / 16), min 1
/// - Toxic: floor(maxHP / 16) * toxic_counter, toxic_counter increments each tick
///
/// Returns the damage dealt. If HP reaches 0, mon faints.
pub fn apply_residual_status_damage(battler: &mut BattlerState) -> ResidualResult {
    let mon = battler.active_mon();
    let status = mon.status;
    let max_hp = mon.max_hp;

    // Badly-poisoned (Toxic) takes the RAMP arm FIRST: a badly-poisoned mon's
    // non-volatile status IS `Poison`, so it would match the flat `Burn|Poison`
    // arm below and this arm would be dead code (the asm checks the
    // BADLY_POISONED bit inside the shared HP-decrease routine, core.asm:577-589).
    if battler.has_status3(status3::BADLY_POISONED) {
        battler.toxic_counter += 1;
        let base = (max_hp / 16).max(1);
        let dmg = base.saturating_mul(battler.toxic_counter as u16);
        return apply_hp_loss(battler, dmg);
    }

    match status {
        StatusCondition::Burn | StatusCondition::Poison => {
            let dmg = (max_hp / 16).max(1);
            apply_hp_loss(battler, dmg)
        }
        _ => ResidualResult::None,
    }
}

/// Apply Leech Seed drain: drain from target, heal on opponent.
///
/// ASM: HandleResidualDamage — Leech Seed section
/// - Drain: floor(maxHP / 16), min 1
/// - Heal opponent by same amount (capped at max_hp)
/// - Gen-1 TOXIC × LEECH SEED bug (reproduced): the drain runs through the SAME
///   HP-decrease routine as poison (HandlePoisonBurnLeechSeed_DecreaseOwnHP,
///   core.asm:550+), so a BADLY-poisoned seeded mon's Toxic counter increments a
///   SECOND time this turn and the drain scales with it.
///
/// Only applies if battler has SEEDED status2 flag.
pub fn apply_leech_seed(battler: &mut BattlerState, opponent: &mut BattlerState) -> ResidualResult {
    if !battler.has_status2(status2::SEEDED) {
        return ResidualResult::None;
    }

    let max_hp = battler.active_mon().max_hp;
    let base = (max_hp / 16).max(1);
    let drain = if battler.has_status3(status3::BADLY_POISONED) {
        battler.toxic_counter += 1;
        base.saturating_mul(battler.toxic_counter as u16)
    } else {
        base
    };

    let mon = battler.active_mon_mut();
    let actual_drain = drain.min(mon.hp);
    mon.hp = mon.hp.saturating_sub(drain);

    if mon.hp == 0 {
        return ResidualResult::Fainted;
    }

    let opp_mon = opponent.active_mon_mut();
    let before_heal = opp_mon.hp;
    // saturating_add: the Toxic-scaled drain (Gen-1 bug) can be large enough
    // that hp + drain overflows u16 before the max_hp clamp.
    opp_mon.hp = opp_mon.hp.saturating_add(actual_drain).min(opp_mon.max_hp);
    let healed = opp_mon.hp - before_heal;

    ResidualResult::LeechSeedDrain {
        drained: actual_drain,
        healed,
    }
}

/// Apply all residual effects for a battler after their move.
///
/// ASM order: status damage (burn/poison/toxic) first, then leech seed.
/// Each can independently kill.
pub fn apply_all_residual(
    battler: &mut BattlerState,
    opponent: &mut BattlerState,
) -> Vec<ResidualResult> {
    let mut results = Vec::new();

    let status_result = apply_residual_status_damage(battler);
    if status_result != ResidualResult::None {
        results.push(status_result);
    }
    if matches!(status_result, ResidualResult::Fainted) {
        return results;
    }

    let seed_result = apply_leech_seed(battler, opponent);
    if seed_result != ResidualResult::None {
        results.push(seed_result);
    }

    results
}

fn apply_hp_loss(battler: &mut BattlerState, damage: u16) -> ResidualResult {
    let mon = battler.active_mon_mut();
    mon.hp = mon.hp.saturating_sub(damage);
    if mon.hp == 0 {
        ResidualResult::Fainted
    } else {
        ResidualResult::Damaged(damage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::state::*;
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    fn make_pokemon(hp: u16, max_hp: u16, status: StatusCondition) -> Pokemon {
        Pokemon {
            species: Species::Pikachu,
            nickname: None,
            level: 50,
            hp,
            max_hp,
            attack: 80,
            defense: 60,
            speed: 90,
            special: 70,
            type1: PokemonType::Electric,
            type2: PokemonType::Electric,
            moves: [
                MoveId::Thundershock,
                MoveId::None,
                MoveId::None,
                MoveId::None,
            ],
            pp: [30, 0, 0, 0],
            pp_ups: [0; 4],
            status,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: None,
        }
    }

    fn make_battler(hp: u16, max_hp: u16, status: StatusCondition) -> BattlerState {
        new_battler_state(vec![make_pokemon(hp, max_hp, status)])
    }

    #[test]
    fn burn_deals_one_sixteenth() {
        let mut b = make_battler(100, 160, StatusCondition::Burn);
        let result = apply_residual_status_damage(&mut b);
        // floor(160/16) = 10
        assert_eq!(result, ResidualResult::Damaged(10));
        assert_eq!(b.active_mon().hp, 90);
    }

    #[test]
    fn poison_deals_one_sixteenth() {
        let mut b = make_battler(100, 160, StatusCondition::Poison);
        let result = apply_residual_status_damage(&mut b);
        assert_eq!(result, ResidualResult::Damaged(10));
        assert_eq!(b.active_mon().hp, 90);
    }

    #[test]
    fn burn_min_one_damage() {
        // maxHP=10 → floor(10/16) = 0 → clamped to 1
        let mut b = make_battler(5, 10, StatusCondition::Burn);
        let result = apply_residual_status_damage(&mut b);
        assert_eq!(result, ResidualResult::Damaged(1));
        assert_eq!(b.active_mon().hp, 4);
    }

    #[test]
    fn toxic_increments_counter() {
        let mut b = make_battler(200, 160, StatusCondition::None);
        b.set_status3(status3::BADLY_POISONED);
        b.toxic_counter = 0;

        // Tick 1: counter becomes 1, damage = floor(160/16)*1 = 10
        let r1 = apply_residual_status_damage(&mut b);
        assert_eq!(r1, ResidualResult::Damaged(10));
        assert_eq!(b.toxic_counter, 1);
        assert_eq!(b.active_mon().hp, 190);

        // Tick 2: counter becomes 2, damage = 10*2 = 20
        let r2 = apply_residual_status_damage(&mut b);
        assert_eq!(r2, ResidualResult::Damaged(20));
        assert_eq!(b.toxic_counter, 2);
        assert_eq!(b.active_mon().hp, 170);

        // Tick 3: counter becomes 3, damage = 10*3 = 30
        let r3 = apply_residual_status_damage(&mut b);
        assert_eq!(r3, ResidualResult::Damaged(30));
        assert_eq!(b.toxic_counter, 3);
        assert_eq!(b.active_mon().hp, 140);
    }

    #[test]
    fn burn_kills() {
        let mut b = make_battler(5, 160, StatusCondition::Burn);
        let result = apply_residual_status_damage(&mut b);
        // floor(160/16) = 10 > 5hp
        assert_eq!(result, ResidualResult::Fainted);
        assert_eq!(b.active_mon().hp, 0);
    }

    #[test]
    fn leech_seed_drains_and_heals() {
        let mut target = make_battler(100, 160, StatusCondition::None);
        target.set_status2(status2::SEEDED);
        let mut opponent = make_battler(50, 100, StatusCondition::None);

        let result = apply_leech_seed(&mut target, &mut opponent);
        // drain = floor(160/16) = 10
        assert_eq!(
            result,
            ResidualResult::LeechSeedDrain {
                drained: 10,
                healed: 10
            }
        );
        assert_eq!(target.active_mon().hp, 90);
        assert_eq!(opponent.active_mon().hp, 60);
    }

    #[test]
    fn leech_seed_heal_capped_at_max() {
        let mut target = make_battler(100, 160, StatusCondition::None);
        target.set_status2(status2::SEEDED);
        let mut opponent = make_battler(95, 100, StatusCondition::None);

        let result = apply_leech_seed(&mut target, &mut opponent);
        // drain=10, but opponent only heals 5 (95+10=105, capped at 100)
        assert_eq!(
            result,
            ResidualResult::LeechSeedDrain {
                drained: 10,
                healed: 5
            }
        );
        assert_eq!(opponent.active_mon().hp, 100);
    }

    #[test]
    fn leech_seed_kills_target() {
        let mut target = make_battler(5, 160, StatusCondition::None);
        target.set_status2(status2::SEEDED);
        let mut opponent = make_battler(50, 100, StatusCondition::None);

        let result = apply_leech_seed(&mut target, &mut opponent);
        // drain=10, target has 5hp → dies
        assert_eq!(result, ResidualResult::Fainted);
        assert_eq!(target.active_mon().hp, 0);
    }

    #[test]
    fn no_seed_no_effect() {
        let mut target = make_battler(100, 160, StatusCondition::None);
        let mut opponent = make_battler(50, 100, StatusCondition::None);
        let result = apply_leech_seed(&mut target, &mut opponent);
        assert_eq!(result, ResidualResult::None);
    }

    #[test]
    fn toxic_leech_seed_interaction_scales_drain() {
        // Gen-1 bug (reproduced): Leech Seed's drain runs through the same
        // HP-decrease routine as poison, so a badly-poisoned seeded mon's Toxic
        // counter increments TWICE per turn and the drain scales with it
        // (core.asm HandlePoisonBurnLeechSeed_DecreaseOwnHP).
        let mut target = make_battler(200, 160, StatusCondition::Poison);
        target.set_status2(status2::SEEDED);
        target.set_status3(status3::BADLY_POISONED);
        target.toxic_counter = 0;
        let mut opponent = make_battler(10, 100, StatusCondition::None);

        let results = apply_all_residual(&mut target, &mut opponent);
        // Poison tick: counter 0→1, damage 10×1 = 10.
        // Leech tick: counter 1→2, drain 10×2 = 20, heal 20.
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], ResidualResult::Damaged(10));
        assert_eq!(
            results[1],
            ResidualResult::LeechSeedDrain {
                drained: 20,
                healed: 20
            }
        );
        assert_eq!(target.toxic_counter, 2, "counter incremented twice in one turn");
        assert_eq!(target.active_mon().hp, 170);
        assert_eq!(opponent.active_mon().hp, 30);
    }

    #[test]
    fn no_status_no_residual() {
        let mut b = make_battler(100, 160, StatusCondition::None);
        let result = apply_residual_status_damage(&mut b);
        assert_eq!(result, ResidualResult::None);
        assert_eq!(b.active_mon().hp, 100);
    }

    #[test]
    fn badly_poisoned_with_poison_status_ramps_not_flat() {
        // D7: a Toxic'd mon's non-volatile status IS Poison — it must take the
        // toxic RAMP, not the flat Burn|Poison chip (previously the flat arm
        // matched first and the ramp arm was dead code).
        let mut b = make_battler(200, 160, StatusCondition::Poison);
        b.set_status3(status3::BADLY_POISONED);
        b.toxic_counter = 0;

        let r1 = apply_residual_status_damage(&mut b);
        assert_eq!(r1, ResidualResult::Damaged(10), "tick 1: 10×1");
        assert_eq!(b.toxic_counter, 1);

        let r2 = apply_residual_status_damage(&mut b);
        assert_eq!(r2, ResidualResult::Damaged(20), "tick 2: 10×2 (ramped, NOT flat 10)");
        assert_eq!(b.toxic_counter, 2);
        assert_eq!(b.active_mon().hp, 170);
    }

    #[test]
    fn apply_all_burn_then_seed() {
        let mut target = make_battler(100, 160, StatusCondition::Burn);
        target.set_status2(status2::SEEDED);
        let mut opponent = make_battler(50, 100, StatusCondition::None);

        let results = apply_all_residual(&mut target, &mut opponent);
        // Burn: 10 damage (100→90), then Leech Seed: 10 drain (90→80), heal opponent 10 (50→60)
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], ResidualResult::Damaged(10));
        assert_eq!(
            results[1],
            ResidualResult::LeechSeedDrain {
                drained: 10,
                healed: 10
            }
        );
        assert_eq!(target.active_mon().hp, 80);
        assert_eq!(opponent.active_mon().hp, 60);
    }

    #[test]
    fn apply_all_burn_kills_skips_seed() {
        let mut target = make_battler(5, 160, StatusCondition::Burn);
        target.set_status2(status2::SEEDED);
        let mut opponent = make_battler(50, 100, StatusCondition::None);

        let results = apply_all_residual(&mut target, &mut opponent);
        // Burn kills (5→0), leech seed should NOT apply
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], ResidualResult::Fainted);
        assert_eq!(target.active_mon().hp, 0);
        assert_eq!(opponent.active_mon().hp, 50); // no heal
    }
}
