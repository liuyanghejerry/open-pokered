//! Original Counter/continuous-move behavior through the production StackDriver.
use dotzuki_engine::battle::rng::ScriptedRng;
use dotzuki_engine::battle::stack::{StackDriver, TurnEvent};
use dotzuki_engine::battle::{BattleAction, BattlerRef};
use pokered_core::battle::pokered_rules::{self as rules, PokeredRules};
use pokered_core::battle::state::*;
use pokered_data::{move_data::MoveData, moves::MoveId, species::Species, types::PokemonType};

fn mon(mv: MoveId) -> Pokemon {
    Pokemon {
        species: Species::Snorlax,
        nickname: [0x50; 11],
        level: 50,
        hp: 400,
        max_hp: 400,
        attack: 100,
        defense: 100,
        speed: 100,
        special: 100,
        type1: PokemonType::Normal,
        type2: PokemonType::Normal,
        moves: [mv, MoveId::None, MoveId::None, MoveId::None],
        pp: [20, 0, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xff; 2],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false,
        ot_id: 0,
        ot_name: [0x50; 11],
    }
}
fn state(player: MoveId, enemy: MoveId) -> BattleState {
    new_battle_state(BattleType::Wild, vec![mon(player)], vec![mon(enemy)])
}
fn turn(
    bs: &mut BattleState,
    player: MoveId,
    enemy: MoveId,
    enemy_acts: bool,
    bytes: Vec<u8>,
) -> (Vec<TurnEvent<PokeredRules>>, usize) {
    rules::install_canonical();
    rules::clear_current_moves();
    rules::clear_levels();
    rules::set_current_move(BattlerRef::PLAYER, *MoveData::get(player).unwrap());
    rules::set_current_move(BattlerRef::OPPONENT, *MoveData::get(enemy).unwrap());
    bs.player.selected_move = player;
    bs.enemy.selected_move = enemy;
    let (mut engine, mut effects) = rules::runtime::engine_state_from_legacy(bs);
    let mut rng = ScriptedRng::new(bytes);
    let (_, log) = StackDriver::execute_turn_logged(
        &PokeredRules,
        &mut engine,
        &mut effects,
        [
            BattleAction::Fight { move_: player },
            if enemy_acts {
                BattleAction::Fight { move_: enemy }
            } else {
                BattleAction::Nothing
            },
        ],
        &mut rng,
    );
    rules::runtime::apply_engine_to_legacy(bs, &engine, &effects);
    (log.events, rng.consumed())
}

#[test]
fn counter_hits_substitute_through_the_normal_damage_fold() {
    let mut bs = state(MoveId::Counter, MoveId::Tackle);
    bs.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
    bs.enemy.substitute_hp = 200;
    turn(
        &mut bs,
        MoveId::Counter,
        MoveId::Tackle,
        true,
        vec![255, 0, 255, 255, 0],
    );
    let incoming = 400 - bs.player.active_mon().hp;
    assert!(incoming > 0);
    assert_eq!(bs.enemy.active_mon().hp, 400);
    assert_eq!(bs.enemy.substitute_hp as u16, 200 - incoming * 2);
    assert_eq!(bs.damage, incoming * 2);
}

#[test]
fn counter_preserves_shared_previous_damage_and_uses_selected_move() {
    // core.asm 4590: shared damage can be from a previous turn or the user itself.
    let mut bs = state(MoveId::Counter, MoveId::Tackle);
    bs.damage = 37;
    let (_, draws) = turn(
        &mut bs,
        MoveId::Counter,
        MoveId::Tackle,
        false,
        vec![255, 0],
    );
    assert_eq!(bs.enemy.active_mon().hp, 326);
    assert_eq!(bs.damage, 74);
    assert_eq!(draws, 2); // crit + normal accuracy; no formula roll.
    let mut bs = state(MoveId::Counter, MoveId::Earthquake);
    bs.damage = 37;
    turn(
        &mut bs,
        MoveId::Counter,
        MoveId::Earthquake,
        false,
        vec![255],
    );
    assert_eq!(bs.enemy.active_mon().hp, 400);
    assert_eq!(bs.damage, 37);
}

#[test]
fn counter_rolls_accuracy_and_cannot_hit_dig_or_fly() {
    for invulnerable in [false, true] {
        let mut bs = state(MoveId::Counter, MoveId::Tackle);
        bs.damage = 37;
        if invulnerable {
            bs.enemy.set_status1(status1::INVULNERABLE);
        }
        let (events, _) = turn(
            &mut bs,
            MoveId::Counter,
            MoveId::Tackle,
            false,
            vec![255, 255],
        );
        assert_eq!(bs.enemy.active_mon().hp, 400);
        assert_eq!(bs.damage, 0); // MoveHitTest clears shared wDamage on either miss.
        assert!(events
            .iter()
            .any(|event| matches!(event,TurnEvent::Missed{actor} if *actor==BattlerRef::PLAYER)));
    }
}

fn faster_player(bs: &mut BattleState) {
    bs.player.active_mon_mut().speed = 200;
    bs.enemy.active_mon_mut().speed = 50;
}

#[test]
fn wrap_continuations_repeat_damage_without_random_draws_and_hold_the_final_turn() {
    let mut bs = state(MoveId::Wrap, MoveId::Tackle);
    faster_player(&mut bs);
    let (_, draws) = turn(
        &mut bs,
        MoveId::Wrap,
        MoveId::Tackle,
        true,
        vec![0, 255, 0, 255],
    );
    let damage = 400 - bs.enemy.active_mon().hp;
    assert!(damage > 0);
    assert_eq!(draws, 4); // duration, crit, accuracy, formula.
    assert_eq!(bs.player.active_mon().hp, 400);
    assert_eq!(bs.player.num_attacks_left, 1);
    assert!(bs.player.has_status1(status1::USING_TRAPPING_MOVE));
    // Evasion and invulnerability cannot make continuation damage miss.
    bs.enemy.set_status1(status1::INVULNERABLE);
    bs.enemy.stat_stages.evasion = 6;
    let (_, draws) = turn(&mut bs, MoveId::Wrap, MoveId::Tackle, true, vec![255]);
    assert_eq!(draws, 0);
    assert_eq!(bs.enemy.active_mon().hp, 400 - damage * 2);
    assert_eq!(bs.player.active_mon().hp, 400); // foe held on final tick too.
    assert!(!bs.player.has_status1(status1::USING_TRAPPING_MOVE));
    assert_eq!(bs.player.num_attacks_left, 0);
    bs.enemy.clear_status1(status1::INVULNERABLE);
    turn(
        &mut bs,
        MoveId::Splash,
        MoveId::Tackle,
        true,
        vec![255, 0, 255],
    );
    assert!(bs.player.active_mon().hp < 400); // foe released next turn.
}

#[test]
fn wrap_initial_miss_releases_foe_and_clears_recharge_before_hit_test() {
    let mut bs = state(MoveId::Wrap, MoveId::Tackle);
    faster_player(&mut bs);
    bs.enemy.set_status2(status2::NEEDS_TO_RECHARGE);
    // Conditional second duration byte is consumed before the crit/accuracy bytes.
    let (_, draws) = turn(
        &mut bs,
        MoveId::Wrap,
        MoveId::Tackle,
        true,
        vec![2, 3, 255, 255, 255, 0, 255],
    );
    assert_eq!(draws, 7);
    assert!(!bs.player.has_status1(status1::USING_TRAPPING_MOVE));
    assert!(!bs.enemy.has_status2(status2::NEEDS_TO_RECHARGE));
    assert_eq!(bs.enemy.active_mon().hp, 400);
    assert!(bs.player.active_mon().hp < 400); // cleared recharge lets it act after miss.
}

#[test]
fn wrap_duration_preserves_conditional_original_draw_order() {
    for (bytes, remaining, expected_draws) in [
        (vec![0, 255, 0, 255], 1, 4),
        (vec![1, 255, 0, 255], 2, 4),
        (vec![2, 2, 255, 0, 255], 3, 5),
        (vec![3, 3, 255, 0, 255], 4, 5),
    ] {
        let mut bs = state(MoveId::Wrap, MoveId::Splash);
        faster_player(&mut bs);
        let (_, draws) = turn(&mut bs, MoveId::Wrap, MoveId::Splash, false, bytes);
        assert_eq!(bs.player.num_attacks_left, remaining);
        assert_eq!(draws, expected_draws);
    }
}

#[test]
fn thrash_misses_still_start_and_tick_the_rampage() {
    let mut bs = state(MoveId::Thrash, MoveId::Splash);
    faster_player(&mut bs);
    let (_, draws) = turn(
        &mut bs,
        MoveId::Thrash,
        MoveId::Splash,
        false,
        vec![0, 255, 255],
    );
    assert_eq!(draws, 3);
    assert_eq!(bs.player.num_attacks_left, 2);
    assert!(bs.player.has_status1(status1::THRASHING_ABOUT));
    let (_, draws) = turn(
        &mut bs,
        MoveId::Thrash,
        MoveId::Splash,
        false,
        vec![255, 255],
    );
    assert_eq!(draws, 2);
    assert_eq!(bs.player.num_attacks_left, 1);
    assert_eq!(bs.enemy.active_mon().hp, 400);
    let (_, draws) = turn(
        &mut bs,
        MoveId::Thrash,
        MoveId::Splash,
        false,
        vec![3, 255, 255],
    );
    assert_eq!(draws, 3); // fatigue first, then crit and missed accuracy.
    assert!(!bs.player.has_status1(status1::THRASHING_ABOUT));
    assert_eq!(bs.player.num_attacks_left, 0);
    assert!(bs.player.has_status1(status1::CONFUSED));
    assert_eq!(bs.player.confused_turns_left, 5);
    assert_eq!(bs.player.active_mon().hp, 400); // fatigue gate starts next turn.
}

#[test]
fn thrash_fatigue_is_two_to_five_turns() {
    for byte in 0..4 {
        let mut bs = state(MoveId::Thrash, MoveId::Splash);
        faster_player(&mut bs);
        bs.player.set_status1(status1::THRASHING_ABOUT);
        bs.player.num_attacks_left = 1;
        turn(
            &mut bs,
            MoveId::Thrash,
            MoveId::Splash,
            false,
            vec![byte, 255, 255],
        );
        assert_eq!(bs.player.confused_turns_left, byte + 2);
    }
}

#[test]
fn shared_damage_survives_a_failed_self_effect_but_not_type_immunity() {
    let mut bs = state(MoveId::Counter, MoveId::Recover);
    bs.damage = 37;
    turn(&mut bs, MoveId::Counter, MoveId::Recover, true, vec![255]);
    assert_eq!(bs.damage, 37); // failed HealEffect never calls MoveHitTest.
    let mut bs = state(MoveId::Counter, MoveId::Tackle);
    bs.damage = 37;
    bs.player.active_mon_mut().type1 = PokemonType::Ghost;
    bs.player.active_mon_mut().type2 = PokemonType::Ghost;
    turn(
        &mut bs,
        MoveId::Counter,
        MoveId::Tackle,
        true,
        vec![255, 0, 255, 255],
    );
    assert_eq!(bs.damage, 0);
    assert_eq!(bs.enemy.active_mon().hp, 400);
}

#[test]
fn trapped_enemy_entry_skips_status_checks_but_player_sleep_still_ticks() {
    let mut bs = state(MoveId::Wrap, MoveId::Tackle);
    faster_player(&mut bs);
    bs.enemy.active_mon_mut().status = StatusCondition::Sleep(3);
    turn(
        &mut bs,
        MoveId::Wrap,
        MoveId::Tackle,
        true,
        vec![0, 255, 0, 255],
    );
    assert_eq!(bs.enemy.active_mon().status, StatusCondition::Sleep(2));
    turn(&mut bs, MoveId::Wrap, MoveId::Tackle, true, vec![255]);
    assert_eq!(bs.enemy.active_mon().status, StatusCondition::Sleep(2));
    let mut bs = state(MoveId::Splash, MoveId::Wrap);
    bs.player.active_mon_mut().speed = 50;
    bs.enemy.active_mon_mut().speed = 200;
    bs.player.active_mon_mut().status = StatusCondition::Sleep(3);
    turn(
        &mut bs,
        MoveId::Splash,
        MoveId::Wrap,
        true,
        vec![0, 255, 0, 255],
    );
    assert_eq!(bs.player.active_mon().status, StatusCondition::Sleep(2));
    turn(&mut bs, MoveId::Splash, MoveId::Wrap, true, vec![255]);
    assert_eq!(bs.player.active_mon().status, StatusCondition::Sleep(1));
}

#[test]
fn bide_natural_expiry_resets_the_counter_scalar() {
    let mut bs = state(MoveId::Bide, MoveId::Splash);
    faster_player(&mut bs);
    bs.player.set_status1(status1::STORING_ENERGY);
    bs.player.num_attacks_left = 1;
    bs.player.bide_accumulated_damage = 7;
    turn(&mut bs, MoveId::Bide, MoveId::Splash, false, vec![255]);
    assert!(!bs.player.has_status1(status1::STORING_ENERGY));
    assert_eq!(bs.player.num_attacks_left, 0);
    assert_eq!(bs.player.bide_accumulated_damage, 0);
}

#[test]
fn bide_initial_counter_does_not_tick_and_storage_reads_shared_old_damage() {
    for byte in [0, 1] {
        let mut bs = state(MoveId::Bide, MoveId::Splash);
        faster_player(&mut bs);
        let (_, draws) = turn(&mut bs, MoveId::Bide, MoveId::Splash, false, vec![byte]);
        assert_eq!(draws, 1);
        assert_eq!(bs.player.num_attacks_left, byte + 2);
        assert_eq!(bs.player.bide_accumulated_damage, 0);
    }
    let mut bs = state(MoveId::Bide, MoveId::Splash);
    faster_player(&mut bs);
    bs.damage = 37;
    turn(&mut bs, MoveId::Bide, MoveId::Splash, false, vec![0]);
    assert_eq!(bs.player.num_attacks_left, 2);
    assert_eq!(bs.damage, 0); // initial GetDamageVars clears the old shared byte.
    bs.damage = 37; // old damage retained from an earlier action while storing.
    let (_, draws) = turn(&mut bs, MoveId::Bide, MoveId::Splash, false, vec![255]);
    assert_eq!(draws, 0);
    assert_eq!(bs.player.num_attacks_left, 1);
    assert_eq!(bs.player.bide_accumulated_damage, 37);
    assert_eq!(bs.enemy.active_mon().hp, 400);
    turn(&mut bs, MoveId::Bide, MoveId::Splash, false, vec![255]);
    assert_eq!(bs.enemy.active_mon().hp, 252); // shared old37 accumulated twice, doubled.
    assert_eq!(bs.player.num_attacks_left, 0);
    assert_eq!(bs.player.bide_accumulated_damage, 0);
}

#[test]
fn bide_release_uses_substitute_and_ignores_accuracy_type_and_invulnerability() {
    let mut bs = state(MoveId::Bide, MoveId::Splash);
    faster_player(&mut bs);
    bs.player.set_status1(status1::STORING_ENERGY);
    bs.player.num_attacks_left = 1;
    bs.player.bide_accumulated_damage = 7;
    bs.damage = 5;
    bs.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
    bs.enemy.substitute_hp = 80;
    bs.enemy.set_status1(status1::INVULNERABLE);
    bs.enemy.stat_stages.evasion = 6;
    bs.enemy.active_mon_mut().type1 = PokemonType::Ghost;
    bs.enemy.active_mon_mut().type2 = PokemonType::Ghost;
    let (_, draws) = turn(&mut bs, MoveId::Bide, MoveId::Splash, false, vec![255]);
    assert_eq!(draws, 0);
    assert_eq!(bs.enemy.active_mon().hp, 400);
    assert_eq!(bs.enemy.substitute_hp, 56);
    assert_eq!(bs.damage, 24);
    assert_eq!(bs.player.bide_accumulated_damage, 0);
}

#[test]
fn bide_storage_waits_for_the_status_gates_and_wraps_sixteen_bit_arithmetic() {
    let mut bs = state(MoveId::Bide, MoveId::Splash);
    faster_player(&mut bs);
    bs.player.set_status1(status1::STORING_ENERGY);
    bs.player.num_attacks_left = 2;
    bs.player.bide_accumulated_damage = 7;
    bs.damage = 30;
    bs.player.active_mon_mut().status = StatusCondition::Sleep(2);
    turn(&mut bs, MoveId::Bide, MoveId::Splash, false, vec![255]);
    assert_eq!(bs.player.num_attacks_left, 2);
    assert_eq!(bs.player.bide_accumulated_damage, 7);
    bs.player.active_mon_mut().status = StatusCondition::None;
    bs.player.num_attacks_left = 1;
    bs.player.bide_accumulated_damage = 65530;
    bs.damage = 10;
    turn(&mut bs, MoveId::Bide, MoveId::Splash, false, vec![255]);
    assert_eq!(bs.enemy.active_mon().hp, 392);
    assert_eq!(bs.damage, 8);
}

#[test]
fn bide_zero_release_misses_without_hit_test_draws() {
    for stored in [0, 32768] {
        let mut bs = state(MoveId::Bide, MoveId::Splash);
        faster_player(&mut bs);
        bs.player.set_status1(status1::STORING_ENERGY);
        bs.player.num_attacks_left = 1;
        bs.player.bide_accumulated_damage = stored;
        let (events, draws) = turn(&mut bs, MoveId::Bide, MoveId::Splash, false, vec![255]);
        assert_eq!(draws, 0);
        assert_eq!(bs.enemy.active_mon().hp, 400);
        assert!(events
            .iter()
            .any(|event| matches!(event,TurnEvent::Missed{actor}
            if *actor==BattlerRef::PLAYER)));
        assert_eq!(bs.player.bide_accumulated_damage, 0);
        assert_eq!(bs.player.num_attacks_left, 0);
    }
}

#[test]
fn counter_can_reflect_the_live_power_byte_of_a_bide_release() {
    let mut bs = state(MoveId::Bide, MoveId::Counter);
    faster_player(&mut bs);
    bs.player.set_status1(status1::STORING_ENERGY);
    bs.player.num_attacks_left = 1;
    bs.player.bide_accumulated_damage = 7;
    let (_, draws) = turn(&mut bs, MoveId::Bide, MoveId::Counter, true, vec![255, 0]);
    assert_eq!(draws, 2); // Counter's own crit + accuracy, Bide release draws none.
    assert_eq!(bs.enemy.active_mon().hp, 386);
    assert_eq!(bs.player.active_mon().hp, 372);
}
