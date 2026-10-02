//! Read-only, conditional direct-hit ranges for capture decisions. These are
//! NOT whole-turn predictions: the opponent may act first, a move may fail,
//! and secondary/residual effects may cause additional damage. No live RNG or
//! future enemy action is inspected. Unsupported move mechanics return None.

use super::{formula_damage_params, runtime, MoveData, MoveEffect, MoveId};
use crate::battle::damage::{calculate_damage, crit_chance, is_high_crit_move};
use crate::battle::state::{status1, status2, BattleState};
use dotzuki_engine::battle::BattlerRef;
use pokered_data::pokemon_data::get_base_stats;

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct DirectHitPreview {
    pub normal_damage: [u16; 2],
    pub critical_damage: [u16; 2],
    /// A crit byte is uniform 0..255 and is compared strictly below this value.
    pub critical_threshold: u8,
    pub target_hp: u16,
    pub direct_hit_can_ko: bool,
}

pub fn player_direct_hit(
    bs: &BattleState,
    move_id: MoveId,
    player_badges: u8,
) -> Option<DirectHitPreview> {
    let md = MoveData::get(move_id)?;
    // Explicit coverage: never apply a single ordinary hit's formula to fixed
    // damage, OHKO, multi-hit/trapping, charge/locked turns or called moves.
    if md.power == 0
        || !matches!(
            md.effect,
            MoveEffect::NoAdditionalEffect
                | MoveEffect::SwiftEffect
                | MoveEffect::DrainHpEffect
                | MoveEffect::PayDayEffect
                | MoveEffect::PoisonSideEffect1
                | MoveEffect::PoisonSideEffect2
                | MoveEffect::BurnSideEffect1
                | MoveEffect::BurnSideEffect2
                | MoveEffect::FreezeSideEffect1
                | MoveEffect::FreezeSideEffect2
                | MoveEffect::ParalyzeSideEffect1
                | MoveEffect::ParalyzeSideEffect2
                | MoveEffect::FlinchSideEffect1
                | MoveEffect::FlinchSideEffect2
                | MoveEffect::AttackDownSideEffect
                | MoveEffect::DefenseDownSideEffect
                | MoveEffect::SpeedDownSideEffect
                | MoveEffect::SpecialDownSideEffect
                | MoveEffect::ConfusionSideEffect
        )
        || bs.enemy.has_status2(status2::HAS_SUBSTITUTE_UP)
        || bs.enemy.has_status1(status1::INVULNERABLE)
    {
        return None;
    }
    // BattleScreen::sync_player_context runs at action execution, not when a
    // menu opens. Initial send-out and forced/Shift switches can therefore
    // expose None here; reading the raw adapter would underestimate damage.
    // Mirror that deterministic preparation on a COPY. The frontend badges
    // are authoritative before the first action (bs.player_badges may be 0).
    let mut prepared = bs.clone();
    prepared.player_badges = player_badges;
    crate::battle::badge_boosts::ensure_initialized(&mut prepared.player, player_badges);
    let (state, effects) = runtime::engine_state_from_legacy(&prepared);
    let player = &state.player_battlers[0];
    let enemy = &state.opponent_battlers[0];
    let damage = |critical, roll| {
        calculate_damage(&formula_damage_params(
            md,
            player,
            enemy,
            &effects,
            BattlerRef::PLAYER,
            BattlerRef::OPPONENT,
            critical,
            roll,
        ))
        .damage
    };
    let normal_damage = [damage(false, 217), damage(false, 255)];
    let critical_damage = [damage(true, 217), damage(true, 255)];
    let critical_threshold = crit_chance(
        get_base_stats(player.species)?.speed,
        is_high_crit_move(move_id),
        bs.player.has_status2(status2::GETTING_PUMPED),
    );
    let max_damage = if critical_threshold > 0 {
        normal_damage[1].max(critical_damage[1])
    } else {
        normal_damage[1]
    };
    Some(DirectHitPreview {
        normal_damage,
        critical_damage,
        critical_threshold,
        target_hp: enemy.hp,
        direct_hit_can_ko: max_damage >= enemy.hp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::*;
    use crate::battle::state::{new_battle_state, status3, BattleType, StatusCondition};
    use crate::pokemon::stats::create_pokemon;
    use pokered_data::{species::Species, types::PokemonType};

    fn player_direct_hit(bs: &BattleState, move_id: MoveId) -> Option<DirectHitPreview> {
        super::player_direct_hit(bs, move_id, bs.player_badges)
    }

    fn battle() -> BattleState {
        new_battle_state(
            BattleType::Wild,
            vec![create_pokemon(Species::Charizard, 74, [0x88, 0x88]).unwrap()],
            vec![create_pokemon(Species::Zapdos, 50, [0x88, 0x88]).unwrap()],
        )
    }

    #[test]
    fn capture_preview_compares_current_hp_and_critical_range_without_writes() {
        let mut bs = battle();
        let before = serde_json::to_value(&bs).unwrap();
        let cut = player_direct_hit(&bs, MoveId::Cut).unwrap();
        assert!(cut.normal_damage[0] > 0);
        assert!(cut.critical_damage[1] > cut.normal_damage[1]);
        assert!(!cut.direct_hit_can_ko, "{cut:?}");
        assert_eq!(cut.critical_threshold, 50);
        assert_eq!(serde_json::to_value(&bs).unwrap(), before);
        bs.enemy.active_mon_mut().hp = cut.critical_damage[1];
        assert!(
            player_direct_hit(&bs, MoveId::Cut)
                .unwrap()
                .direct_hit_can_ko
        );
        bs.player.set_status2(status2::GETTING_PUMPED);
        assert_eq!(
            player_direct_hit(&bs, MoveId::Cut)
                .unwrap()
                .critical_threshold,
            12
        );
        assert_eq!(
            player_direct_hit(&battle(), MoveId::Slash)
                .unwrap()
                .critical_threshold,
            255
        );
    }

    #[test]
    fn capture_preview_uses_live_badges_stages_burn_screens_and_conversion() {
        let mut bs = battle();
        let original = player_direct_hit(&bs, MoveId::Cut).unwrap();
        bs.player.badge_boosted_stats = Some([400, 200, 200, 200]);
        let boosted = player_direct_hit(&bs, MoveId::Cut).unwrap();
        assert!(boosted.normal_damage[1] > original.normal_damage[1]);
        bs.player.active_mon_mut().status = StatusCondition::Burn;
        bs.player.stat_stages.attack = -2;
        bs.enemy.stat_stages.defense = 2;
        bs.enemy.set_status3(status3::HAS_REFLECT_UP);
        let reduced = player_direct_hit(&bs, MoveId::Cut).unwrap();
        assert!(reduced.normal_damage[1] < boosted.normal_damage[1]);
        assert_eq!(reduced.critical_damage, boosted.critical_damage);
        bs.enemy.conversion_type1 = Some(PokemonType::Ghost);
        bs.enemy.conversion_type2 = Some(PokemonType::Ghost);
        let immune = player_direct_hit(&bs, MoveId::Cut).unwrap();
        assert_eq!(immune.normal_damage, [0, 0]);
        assert_eq!(immune.critical_damage, [0, 0]);
        assert!(!immune.direct_hit_can_ko);
    }

    #[test]
    fn capture_preview_abstains_for_unmodelled_mechanics() {
        let mut bs = battle();
        for mv in [
            MoveId::None,
            MoveId::Doubleslap,
            MoveId::Clamp,
            MoveId::Dig,
            MoveId::Sonicboom,
            MoveId::SuperFang,
            MoveId::Fissure,
            MoveId::Metronome,
            MoveId::Explosion,
            MoveId::Rage,
        ] {
            assert!(player_direct_hit(&bs, mv).is_none(), "{mv:?}");
        }
        bs.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
        assert!(player_direct_hit(&bs, MoveId::Cut).is_none());
        bs.enemy.clear_status2(status2::HAS_SUBSTITUTE_UP);
        bs.enemy.set_status1(status1::INVULNERABLE);
        assert!(player_direct_hit(&bs, MoveId::Cut).is_none());
    }

    #[test]
    fn capture_preview_after_forced_switch_includes_pending_send_out_boost() {
        let mut bs = battle();
        bs.player_badges = 1;
        bs.player.reset_volatile_status();
        assert!(bs.player.badge_boosted_stats.is_none());
        let raw = player_direct_hit(&bs, MoveId::Cut).unwrap();
        let mut initialized = bs.clone();
        crate::battle::badge_boosts::ensure_initialized(&mut initialized.player, bs.player_badges);
        let expected = player_direct_hit(&initialized, MoveId::Cut).unwrap();
        assert_eq!(
            raw, expected,
            "preview must include the boost execution applies lazily"
        );
        assert!(
            bs.player.badge_boosted_stats.is_none(),
            "preview must not mutate live state"
        );
    }

    #[test]
    fn capture_preview_ranges_match_production_stack_for_every_damage_roll() {
        use super::super::{
            clear_current_moves, install_canonical, set_current_move, PokeredRules,
        };
        use dotzuki_engine::battle::{rng::ScriptedRng, stack::StackDriver, BattleAction};
        install_canonical();
        for variant in 0..5 {
            let mut bs = battle();
            bs.enemy.active_mon_mut().hp = 10000;
            bs.enemy.active_mon_mut().max_hp = 10000;
            bs.player.active_mon_mut().speed = 999;
            bs.enemy.active_mon_mut().speed = 1;
            if variant >= 1 {
                bs.player.badge_boosted_stats = Some([400, 200, 999, 300]);
                bs.player.active_mon_mut().status = StatusCondition::Burn;
                bs.player.stat_stages.attack = -2;
                bs.enemy.stat_stages.defense = 2;
            }
            if variant >= 2 {
                bs.enemy
                    .set_status3(status3::HAS_REFLECT_UP | status3::HAS_LIGHT_SCREEN_UP);
                bs.player.stat_stages.special = 2;
                bs.enemy.stat_stages.special = -1;
            }
            if variant >= 3 {
                bs.enemy.conversion_type1 = Some(PokemonType::Ghost);
                bs.enemy.conversion_type2 = Some(PokemonType::Grass);
            }
            if variant >= 4 {
                bs.player.conversion_type1 = Some(PokemonType::Normal);
                bs.player.conversion_type2 = Some(PokemonType::Normal);
            }
            for mv in [MoveId::Cut, MoveId::Slash, MoveId::Flamethrower] {
                let preview = player_direct_hit(&bs, mv).unwrap();
                for critical in [false, true] {
                    let mut observed = [u16::MAX, 0];
                    for roll in 217..=255 {
                        let (mut state, mut effects) = runtime::engine_state_from_legacy(&bs);
                        clear_current_moves();
                        set_current_move(BattlerRef::PLAYER, *MoveData::get(mv).unwrap());
                        set_current_move(
                            BattlerRef::OPPONENT,
                            *MoveData::get(MoveId::Splash).unwrap(),
                        );
                        let mut rng = ScriptedRng::new(vec![
                            if critical { 0 } else { 255 },
                            0,
                            roll,
                            255,
                            255,
                        ]);
                        StackDriver::execute_turn(
                            &PokeredRules,
                            &mut state,
                            &mut effects,
                            [
                                BattleAction::Fight { move_: mv },
                                BattleAction::Fight {
                                    move_: MoveId::Splash,
                                },
                            ],
                            &mut rng,
                        );
                        let damage = 10000 - state.opponent_battlers[0].hp;
                        observed[0] = observed[0].min(damage);
                        observed[1] = observed[1].max(damage);
                    }
                    assert_eq!(
                        observed,
                        if critical {
                            preview.critical_damage
                        } else {
                            preview.normal_damage
                        },
                        "variant={variant} move={mv:?} critical={critical}"
                    );
                }
            }
        }
    }
}
