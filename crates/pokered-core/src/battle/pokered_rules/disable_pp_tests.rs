//! Primary effects.asm:1286–1360 and move_effects/transform.asm:103–119.
//! Real constructors -> production adapter -> canonical StackDriver. Expected
//! slot/draw/PP values below are literal source contracts, never legacy-oracle output.
use super::*;
use crate::battle::state::{new_battle_state, BattleState as GameBattleState, BattleType};
use crate::battle::{BattleInput, BattlePhase, BattleScreen};
use crate::pokemon::stats::create_pokemon_with_moves;
use dotzuki_engine::battle::rng::ScriptedRng;
use dotzuki_engine::battle::stack::StackDriver;

fn pokemon(species: Species, level: u8, moves: [MoveId; 4]) -> crate::battle::state::Pokemon {
    create_pokemon_with_moves(species, level, [0x9a, 0x78], moves).unwrap()
}

struct Outcome {
    disabled: Option<(u8, u8)>,
    draws: usize,
    packed_at_entry: [u16; 4],
}

fn disable_case(
    target_player: bool,
    link: bool,
    pp: [u8; 4],
    ups: [u8; 4],
    bytes: Vec<u8>,
    already_disabled: bool,
) -> Outcome {
    let mut target = pokemon(
        Species::Bulbasaur,
        20,
        [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
    );
    target.pp = pp;
    target.pp_ups = ups;
    let user = pokemon(
        Species::Drowzee,
        20,
        [
            MoveId::Disable,
            MoveId::Confusion,
            MoveId::None,
            MoveId::None,
        ],
    );
    let (player, enemy) = if target_player {
        (target, user)
    } else {
        (user, target)
    };
    let mut game = new_battle_state(BattleType::Trainer, vec![player], vec![enemy]);
    game.link_battle = link;
    if already_disabled {
        let party = if target_player {
            &mut game.player
        } else {
            &mut game.enemy
        };
        party.disabled_move = 1;
        party.disabled_turns_left = 5;
    }
    let before = game.clone();
    let (mut state, mut effects) = runtime::engine_state_from_legacy(&game);
    let target_ref = if target_player {
        BattlerRef::PLAYER
    } else {
        BattlerRef::OPPONENT
    };
    let battler = if target_player {
        &state.player_battlers[0]
    } else {
        &state.opponent_battlers[0]
    };
    let packed_at_entry =
        core::array::from_fn(|i| battler.resources.current(RES_PP_BASE + i as u16).unwrap());
    install_canonical();
    clear_current_moves();
    clear_last_move_live();
    let disable_side = if target_player {
        BattlerRef::OPPONENT
    } else {
        BattlerRef::PLAYER
    };
    set_current_move(disable_side, *MoveData::get(MoveId::Disable).unwrap());
    let actions = if target_player {
        [
            BattleAction::Nothing,
            BattleAction::Fight {
                move_: MoveId::Disable,
            },
        ]
    } else {
        [
            BattleAction::Fight {
                move_: MoveId::Disable,
            },
            BattleAction::Nothing,
        ]
    };
    let mut rng = ScriptedRng::new(bytes);
    StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
    let disabled = effects.iter().find_map(|e| match e.kind {
        PokeVolatile::Disable { slot, turns } if e.host == target_ref => Some((slot, turns)),
        _ => None,
    });
    runtime::apply_engine_to_legacy(&mut game, &state, &effects);
    // This direct StackDriver diagnostic does not consume the frontend's charge
    // receipt. Disable itself cannot alter HP/status, moves or PP metadata.
    assert_eq!(game.player.active_mon(), before.player.active_mon());
    assert_eq!(game.enemy.active_mon(), before.enemy.active_mon());
    Outcome {
        disabled,
        draws: rng.consumed(),
        packed_at_entry,
    }
}

#[test]
fn disable_packed_zero_current_pp_up_slot_is_eligible_for_player_and_link_target() {
    for (target_player, link) in [(true, false), (false, true)] {
        // Accuracy, known Tackle ($40), duration: $40 is accepted, not rejected.
        let got = disable_case(
            target_player,
            link,
            [0, 5, 0, 0],
            [1, 0, 0, 0],
            vec![0, 0, 1, 6],
            false,
        );
        assert_eq!(got.packed_at_entry, [0x40, 5, 0, 0]);
        assert_eq!(got.disabled, Some((1, 2)));
        assert_eq!(got.draws, 3);
    }
}

#[test]
fn disable_all_current_pp_zero_still_draws_and_rejects_blank_before_failing() {
    // Accuracy, empty slot3, known slot0. PP-Up flags do not defeat OR(PP)&$3f.
    let got = disable_case(true, false, [0; 4], [1, 3, 0, 0], vec![0, 3, 0, 7], false);
    assert_eq!(got.packed_at_entry, [0x40, 0xc0, 0, 0]);
    assert_eq!(got.disabled, None);
    assert_eq!(
        got.draws, 3,
        "no duration draw after the all-current-PP failure"
    );
}

#[test]
fn disable_rejects_empty_move_and_whole_zero_pp_byte_then_draws_duration() {
    // Accuracy, blank3, known but zero byte0, accepted Growl1, duration7.
    let got = disable_case(
        true,
        false,
        [0, 5, 0, 0],
        [0; 4],
        vec![0, 3, 0, 1, 7],
        false,
    );
    assert_eq!(got.disabled, Some((2, 8)));
    assert_eq!(got.draws, 5);
}

#[test]
fn disable_nonlink_enemy_ignores_pp_but_link_enemy_checks_after_known_draw() {
    for (link, expected, draws) in [(false, Some((1, 8)), 3), (true, None, 2)] {
        let got = disable_case(false, link, [0; 4], [0; 4], vec![0, 0, 7], false);
        assert_eq!(got.disabled, expected);
        assert_eq!(got.draws, draws);
    }
}

#[test]
fn disable_already_disabled_draws_accuracy_without_slot_or_duration_draw() {
    let got = disable_case(true, false, [0, 5, 0, 0], [1, 0, 0, 0], vec![0, 0, 7], true);
    assert_eq!(got.disabled, Some((1, 5)));
    assert_eq!(got.draws, 1);
}

fn transform_state() -> GameBattleState {
    let mut ditto = pokemon(
        Species::Ditto,
        50,
        [MoveId::Transform, MoveId::None, MoveId::None, MoveId::None],
    );
    // Deliberately depleted input isolates Transform's new working PP view;
    // the public FIGHT tests below exercise the frontend's actual PP charging.
    ditto.pp = [0; 4];
    ditto.pp_ups = [1, 0, 0, 0];
    let foe = pokemon(
        Species::Drowzee,
        20,
        [MoveId::Disable, MoveId::Pound, MoveId::None, MoveId::None],
    );
    new_battle_state(BattleType::Trainer, vec![ditto], vec![foe])
}

#[test]
fn disable_same_turn_transform_reads_new_working_pp_and_preserves_original_identity() {
    let mut game = transform_state();
    let before = game.clone();
    let (mut state, mut effects) = runtime::engine_state_from_legacy(&game);
    install_canonical();
    clear_current_moves();
    clear_last_move_live();
    set_current_move(
        BattlerRef::PLAYER,
        *MoveData::get(MoveId::Transform).unwrap(),
    );
    set_current_move(
        BattlerRef::OPPONENT,
        *MoveData::get(MoveId::Disable).unwrap(),
    );
    // Transform has no accuracy draw; enemy accuracy0, copied Pound slot1,
    // duration6. Extra bytes let the old stale-view implementation finish.
    let mut rng = ScriptedRng::new(vec![0, 1, 6, 0, 1]);
    StackDriver::execute_turn_logged(
        &PokeredRules,
        &mut state,
        &mut effects,
        [
            BattleAction::Fight {
                move_: MoveId::Transform,
            },
            BattleAction::Fight {
                move_: MoveId::Disable,
            },
        ],
        &mut rng,
    );
    let packed: [u16; 4] = core::array::from_fn(|i| {
        state.player_battlers[0]
            .resources
            .current(RES_PP_BASE + i as u16)
            .unwrap()
    });
    assert_eq!(packed, [5, 5, 0, 0]);
    assert_eq!(rng.consumed(), 3);
    assert!(effects.iter().any(|e| e.host == BattlerRef::PLAYER
        && matches!(e.kind, PokeVolatile::Disable { slot: 2, turns: 7 })));
    runtime::apply_engine_to_legacy(&mut game, &state, &effects);
    assert_eq!(
        game.player.active_mon().moves,
        [MoveId::Disable, MoveId::Pound, MoveId::None, MoveId::None]
    );
    assert_eq!(game.player.active_mon().pp, [5, 5, 0, 0]);
    assert_eq!(game.player.active_mon().pp_ups, [0; 4]);
    assert_eq!(game.player.active_mon().hp, before.player.active_mon().hp);
    assert_eq!(
        game.player.active_mon().status,
        before.player.active_mon().status
    );
    assert_eq!(game.enemy.active_mon(), before.enemy.active_mon());
    assert_eq!(game.player.party_mon(0), before.player.active_mon());
    game.player.restore_original_identity();
    assert_eq!(game.player.active_mon(), before.player.active_mon());
}

#[test]
fn transform_working_pp_zeroes_tail_after_first_none_without_reordering() {
    // Explicit raw engine inputs isolate Transform's copy loop. The existing
    // adapter's None compaction and ordinary menu behavior belong to984, not this test.
    for (raw, expected) in [
        (
            [MoveId::None, MoveId::Disable, MoveId::Pound, MoveId::None],
            [0, 0, 0, 0],
        ),
        (
            [MoveId::Disable, MoveId::None, MoveId::Pound, MoveId::None],
            [5, 0, 0, 0],
        ),
        (
            [MoveId::Disable, MoveId::Pound, MoveId::None, MoveId::None],
            [5, 5, 0, 0],
        ),
    ] {
        let mut game = transform_state();
        let original = game.player.active_mon().clone();
        let (mut state, mut effects) = runtime::engine_state_from_legacy(&game);
        state.opponent_battlers[0].moves = raw.to_vec();
        install_canonical();
        clear_current_moves();
        clear_last_move_live();
        set_current_move(
            BattlerRef::PLAYER,
            *MoveData::get(MoveId::Transform).unwrap(),
        );
        let mut rng = ScriptedRng::new(vec![]);
        StackDriver::execute_turn_logged(
            &PokeredRules,
            &mut state,
            &mut effects,
            [
                BattleAction::Fight {
                    move_: MoveId::Transform,
                },
                BattleAction::Nothing,
            ],
            &mut rng,
        );
        let packed: [u16; 4] = core::array::from_fn(|i| {
            state.player_battlers[0]
                .resources
                .current(RES_PP_BASE + i as u16)
                .unwrap()
        });
        assert_eq!(packed, expected);
        assert_eq!(rng.consumed(), 0);
        runtime::apply_engine_to_legacy(&mut game, &state, &effects);
        assert_eq!(game.player.active_mon().moves, raw);
        assert_eq!(game.player.active_mon().pp, expected.map(|pp| pp as u8));
        assert_eq!(game.player.active_mon().pp_ups, [0; 4]);
        assert_eq!(game.player.party_mon(0), &original);
    }
}

fn fight_screen(
    player: crate::battle::state::Pokemon,
    enemy: crate::battle::state::Pokemon,
) -> BattleScreen {
    let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
    screen.phase = BattlePhase::PlayerMenu;
    screen.rng = runtime::StdBattleRng::from_seed(42);
    screen
}

fn fight_first_move(screen: &mut BattleScreen) {
    let confirm = BattleInput {
        a: true,
        ..BattleInput::none()
    };
    screen.update_frame(confirm);
    assert_eq!(screen.phase, BattlePhase::MoveSelect);
    screen.update_frame(confirm);
    assert!(matches!(screen.phase, BattlePhase::ShowingText { .. }));
}

#[test]
fn fight_last_pp_is_charged_before_slower_disable_with_same_seed_positive_control() {
    for (initial_pp, expected_slot) in [(1, 0), (2, 1)] {
        let mut player = pokemon(
            Species::Pikachu,
            50,
            [MoveId::Growl, MoveId::None, MoveId::None, MoveId::None],
        );
        player.pp = [initial_pp, 0, 0, 0];
        let enemy = pokemon(
            Species::Drowzee,
            20,
            [MoveId::Disable, MoveId::None, MoveId::None, MoveId::None],
        );
        let player_before = player.clone();
        let enemy_before = enemy.clone();
        let mut screen = fight_screen(player, enemy);
        fight_first_move(&mut screen);
        let bs = screen.battle_state.as_ref().unwrap();
        assert_eq!(bs.player.last_move_used, MoveId::Growl);
        assert_eq!(bs.enemy.last_move_used, MoveId::Disable);
        assert_eq!(bs.player.disabled_move, expected_slot);
        if expected_slot != 0 {
            assert!(
                bs.player.disabled_turns_left > 0,
                "same RNG must actually land Disable"
            );
        }
        let mut expected = player_before;
        expected.pp[0] = initial_pp - 1;
        assert_eq!(bs.player.active_mon(), &expected);
        // Non-link enemies retain their original infinite-PP policy.
        assert_eq!(bs.enemy.active_mon(), &enemy_before);
        for _ in 0..20 {
            screen.update_frame(BattleInput::none());
        }
        assert_eq!(
            screen.battle_state.as_ref().unwrap().player.active_mon().pp[0],
            initial_pp - 1
        );
    }
}

#[test]
fn fight_pp_receipt_does_not_charge_blocked_or_recharging_actions() {
    for (status, recharging) in [
        (LegacyStatus::Freeze, false),
        (LegacyStatus::Sleep(2), false),
        (LegacyStatus::None, true),
    ] {
        let mut player = pokemon(
            Species::Pikachu,
            50,
            [MoveId::Growl, MoveId::None, MoveId::None, MoveId::None],
        );
        player.pp[0] = 2;
        player.status = status;
        let enemy = pokemon(
            Species::Magikarp,
            20,
            [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
        );
        let mut screen = fight_screen(player, enemy);
        if recharging {
            screen
                .battle_state
                .as_mut()
                .unwrap()
                .player
                .set_status2(crate::battle::state::status2::NEEDS_TO_RECHARGE);
        }
        fight_first_move(&mut screen);
        let bs = screen.battle_state.as_ref().unwrap();
        assert_eq!(bs.player.active_mon().pp[0], 2);
        assert_eq!(bs.player.last_move_used, MoveId::None);
    }
}

#[test]
fn fight_pp_charge_covers_second_actor_and_first_actor_terminal_without_double_charge() {
    // Splash is slower, so its receipt is consumed only after the driver returns.
    let mut player = pokemon(
        Species::Magikarp,
        20,
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
    );
    player.pp[0] = 2;
    let enemy = pokemon(
        Species::Pikachu,
        50,
        [MoveId::Growl, MoveId::None, MoveId::None, MoveId::None],
    );
    let mut screen = fight_screen(player, enemy);
    fight_first_move(&mut screen);
    assert_eq!(
        screen.battle_state.as_ref().unwrap().player.active_mon().pp[0],
        1
    );

    // Swift cannot miss; a KO cancels the opponent callback, so final cleanup
    // still has to pay exactly once for the accepted first actor's move.
    let mut player = pokemon(
        Species::Pikachu,
        50,
        [MoveId::Swift, MoveId::None, MoveId::None, MoveId::None],
    );
    player.pp[0] = 2;
    let mut enemy = pokemon(
        Species::Drowzee,
        20,
        [MoveId::Disable, MoveId::None, MoveId::None, MoveId::None],
    );
    enemy.hp = 1;
    let mut screen = fight_screen(player, enemy);
    fight_first_move(&mut screen);
    let bs = screen.battle_state.as_ref().unwrap();
    assert_eq!(bs.enemy.active_mon().hp, 0);
    assert_eq!(bs.enemy.last_move_used, MoveId::None);
    assert_eq!(bs.player.active_mon().pp[0], 1);
}

#[test]
fn accepted_pp_receipt_is_consumed_once_and_not_recreated_by_state_rebuild() {
    let player = pokemon(
        Species::Pikachu,
        50,
        [MoveId::Growl, MoveId::None, MoveId::None, MoveId::None],
    );
    let enemy = pokemon(
        Species::Magikarp,
        20,
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
    );
    let mut game = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);
    let pp = game.player.active_mon().pp[0];
    game.player.selected_move_index = 0;
    let (mut state, mut effects) = runtime::engine_state_from_legacy(&game);
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, *MoveData::get(MoveId::Growl).unwrap());
    let mut rng = ScriptedRng::new(vec![0]);
    StackDriver::execute_turn_logged(
        &PokeredRules,
        &mut state,
        &mut effects,
        [
            BattleAction::Fight {
                move_: MoveId::Growl,
            },
            BattleAction::Nothing,
        ],
        &mut rng,
    );
    runtime::apply_engine_to_legacy(&mut game, &state, &effects);
    let entry = (0, 0, false, MoveId::Growl);
    BattleScreen::consume_player_pp_charge(&mut game, &mut state, &mut effects, entry, false);
    assert_eq!(game.player.active_mon().pp[0], pp - 1);
    assert_eq!(
        state.player_battlers[0].resources.current(RES_PP_BASE),
        Some((pp - 1) as u16)
    );
    BattleScreen::consume_player_pp_charge(&mut game, &mut state, &mut effects, entry, false);
    assert_eq!(game.player.active_mon().pp[0], pp - 1);
    let (mut state, mut effects) = runtime::engine_state_from_legacy(&game);
    assert!(!effects
        .iter()
        .any(|entry| matches!(entry.kind, PokeVolatile::PlayerPpChargePending)));
    BattleScreen::consume_player_pp_charge(&mut game, &mut state, &mut effects, entry, false);
    assert_eq!(game.player.active_mon().pp[0], pp - 1);
}
