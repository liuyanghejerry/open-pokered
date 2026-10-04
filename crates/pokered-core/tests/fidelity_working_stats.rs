//! Actual battle stat words: original burn/paralysis timing through production.
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
fn send_out_applies_burn_before_the_badge_integer_rounding() {
    let mut bs = state(MoveId::Splash, MoveId::Splash);
    bs.player.active_mon_mut().attack = 73;
    bs.player.active_mon_mut().status = StatusCondition::Burn;
    bs.player.refresh_unmodified_stats();
    bs.player_badges = 1;
    turn(
        &mut bs,
        MoveId::Splash,
        MoveId::Splash,
        false,
        vec![255; 20],
    );
    assert_eq!(bs.player.staged_badge_stats.unwrap()[0], 40); // floor(73/2)=36, then +floor(36/8)=4.
}

#[test]
fn agility_removes_own_paralysis_speed_penalty_and_keeps_the_status() {
    let mut bs = state(MoveId::Agility, MoveId::Splash);
    bs.player.active_mon_mut().status = StatusCondition::Paralysis;
    turn(
        &mut bs,
        MoveId::Agility,
        MoveId::Splash,
        false,
        vec![255; 20],
    );
    assert_eq!(bs.player.stat_stages.speed, 2);
    assert_eq!(bs.player.active_mon().status, StatusCondition::Paralysis);
    assert_eq!(bs.player.staged_badge_stats.unwrap()[2], 200);
    assert_eq!(
        pokered_core::battle::turn_order::effective_speed_for(&bs.player),
        200
    );
    turn(
        &mut bs,
        MoveId::Splash,
        MoveId::Splash,
        false,
        vec![255; 20],
    );
    assert_eq!(bs.player.staged_badge_stats.unwrap()[2], 200); // Cross-turn bridge does not re-penalize.
}

#[test]
fn self_stat_up_compounds_the_opposite_sides_status_penalty() {
    for (status, slot, first, second) in [
        (StatusCondition::Burn, 0, 25, 12),
        (StatusCondition::Paralysis, 2, 6, 1),
    ] {
        let mut bs = state(MoveId::DoubleTeam, MoveId::Splash);
        bs.enemy.active_mon_mut().status = status;
        turn(
            &mut bs,
            MoveId::DoubleTeam,
            MoveId::Splash,
            false,
            vec![255; 20],
        );
        assert_eq!(bs.enemy.staged_badge_stats.unwrap()[slot], first);
        turn(
            &mut bs,
            MoveId::DoubleTeam,
            MoveId::Splash,
            false,
            vec![255; 20],
        );
        assert_eq!(bs.enemy.staged_badge_stats.unwrap()[slot], second);
    }
}

#[test]
fn stat_down_reapplies_penalties_to_the_lowered_side() {
    let mut bs = state(MoveId::TailWhip, MoveId::Splash);
    bs.enemy.active_mon_mut().status = StatusCondition::Paralysis;
    turn(
        &mut bs,
        MoveId::TailWhip,
        MoveId::Splash,
        false,
        vec![0; 20],
    );
    assert_eq!(bs.enemy.stat_stages.defense, -1);
    assert_eq!(bs.enemy.staged_badge_stats.unwrap()[2], 6);
    let mut bs = state(MoveId::Growl, MoveId::Splash);
    bs.enemy.active_mon_mut().status = StatusCondition::Burn;
    turn(&mut bs, MoveId::Growl, MoveId::Splash, false, vec![0; 20]);
    assert_eq!(bs.enemy.staged_badge_stats.unwrap()[0], 33); // recompute 100*2/3, then burn /2.
}

#[test]
fn haze_restores_the_users_stat_words_without_curing_its_status() {
    for status in [StatusCondition::Burn, StatusCondition::Paralysis] {
        let mut bs = state(MoveId::Haze, MoveId::Splash);
        bs.player.active_mon_mut().status = status;
        turn(&mut bs, MoveId::Haze, MoveId::Splash, false, vec![255; 20]);
        assert_eq!(bs.player.staged_badge_stats, Some([100; 4]));
        assert_eq!(bs.player.active_mon().status, status);
        assert_eq!(
            pokered_core::battle::turn_order::effective_speed_for(&bs.player),
            100
        );
        turn(
            &mut bs,
            MoveId::Tackle,
            MoveId::Splash,
            false,
            if status == StatusCondition::Paralysis {
                vec![255, 255, 0, 255]
            } else {
                vec![255, 0, 255]
            },
        );
        assert_eq!(bs.enemy.active_mon().hp, 375); // full attack, not a second deferred burn penalty.
    }
}

#[test]
fn transform_copies_the_targets_already_penalized_stat_words() {
    let mut bs = state(MoveId::Transform, MoveId::Tackle);
    bs.enemy.active_mon_mut().status = StatusCondition::Burn;
    turn(
        &mut bs,
        MoveId::Transform,
        MoveId::Tackle,
        false,
        vec![255; 20],
    );
    assert_eq!(bs.player.staged_badge_stats.unwrap()[0], 50);
    assert_eq!(bs.player.active_mon().status, StatusCondition::None);
    // HandlePoisonBurnLeechSeed (core.asm:546-574): max HP 400 / 16 = 25.
    assert_eq!(bs.enemy.active_mon().hp, 375); // First turn's burn chip.
    turn(
        &mut bs,
        MoveId::Tackle,
        MoveId::Tackle,
        false,
        vec![255, 0, 255, 255],
    );
    assert_eq!(bs.player.staged_badge_stats.unwrap()[0], 50);
    assert_eq!(bs.damage, 13); // Tackle uses copied Attack 50 despite no user burn.
    assert_eq!(bs.enemy.active_mon().hp, 337); // 400 - 25 - 13 - 25: two burn ticks.
}

#[test]
fn transform_copies_target_speed_without_reapplying_the_users_paralysis() {
    let mut bs = state(MoveId::Transform, MoveId::Splash);
    bs.player.active_mon_mut().status = StatusCondition::Paralysis;
    turn(
        &mut bs,
        MoveId::Transform,
        MoveId::Splash,
        false,
        vec![255; 20],
    );
    assert_eq!(bs.player.active_mon().status, StatusCondition::Paralysis);
    assert_eq!(
        pokered_core::battle::turn_order::effective_speed_for(&bs.player),
        100
    );
}

#[test]
fn inflicted_paralysis_and_burn_update_the_stored_working_stat_once() {
    let mut bs = state(MoveId::ThunderWave, MoveId::Splash);
    turn(
        &mut bs,
        MoveId::ThunderWave,
        MoveId::Splash,
        false,
        vec![0; 20],
    );
    assert_eq!(bs.enemy.active_mon().status, StatusCondition::Paralysis);
    assert_eq!(bs.enemy.staged_badge_stats.unwrap()[2], 25);
    assert_eq!(
        pokered_core::battle::turn_order::effective_speed_for(&bs.enemy),
        25
    );
    let mut bs = state(MoveId::Ember, MoveId::Splash);
    turn(
        &mut bs,
        MoveId::Ember,
        MoveId::Splash,
        false,
        vec![255, 0, 255, 0],
    );
    assert_eq!(bs.enemy.active_mon().status, StatusCondition::Burn);
    assert_eq!(bs.enemy.staged_badge_stats.unwrap()[0], 50);
}
