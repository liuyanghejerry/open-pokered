use super::*;
use crate::battle::state::{new_battle_state, BattleType, StatusCondition};
use crate::pokemon::stats::create_pokemon_with_moves;
use dotzuki_engine::battle::rng::ScriptedRng;
use dotzuki_engine::battle::stack::{MoveContext, StackDriver, TurnEvent};
use dotzuki_engine::battle::EnumMap;

fn accuracy(
    user: u8,
    target: u8,
    wild: bool,
    side: BattlerRef,
    move_: MoveId,
    bytes: Vec<u8>,
) -> (bool, usize, bool) {
    clear_current_moves();
    set_current_move(side, *MoveData::get(move_).unwrap());
    let mon = |level| {
        EngineBattler::<PokeredRules>::new(Species::Abra, 30, 30, EnumMap::new(), vec![move_])
            .with_level(level)
    };
    let (player, enemy) = if side == BattlerRef::PLAYER {
        (user, target)
    } else {
        (target, user)
    };
    let mut state = EngineState::new(vec![mon(player)], vec![mon(enemy)]);
    state.player_battlers[0]
        .resources
        .set(RES_WILD_BATTLE, wild as u16, 1);
    let mut effects = Vec::new();
    let mut mv = MoveContext::default();
    let mut rng = ScriptedRng::new(bytes);
    let result = pokered_accuracy(
        &mut BattleCtx {
            state: &mut state,
            effects: &mut effects,
            mv: &mut mv,
            rng: &mut rng,
        },
        RelayVar::Bool(true),
        if side == BattlerRef::PLAYER {
            BattlerRef::OPPONENT
        } else {
            BattlerRef::PLAYER
        },
        side,
        EffectId(0),
    );
    (
        !matches!(result, HandlerResult::Set(RelayVar::Bool(false))),
        rng.consumed(),
        escape_succeeded(&effects),
    )
}

#[test]
fn escape_move_rejects_out_of_range_bytes_and_preserves_exact_threshold() {
    for side in [BattlerRef::PLAYER, BattlerRef::OPPONENT] {
        for move_ in [MoveId::Teleport, MoveId::Roar, MoveId::Whirlwind] {
            assert_eq!(
                accuracy(5, 50, true, side, move_, vec![255, 56, 11]),
                (false, 3, false)
            );
            assert_eq!(
                accuracy(5, 50, true, side, move_, vec![255, 56, 12]),
                (true, 3, true)
            );
            assert_eq!(accuracy(50, 50, true, side, move_, vec![]), (true, 0, true));
            assert_eq!(
                accuracy(50, 5, false, side, move_, vec![]),
                (false, 0, false)
            );
        }
    }
}

#[test]
fn successful_teleport_stops_slower_foe_and_both_poison_and_burn_ticks() {
    install_canonical();
    let mut player = create_pokemon_with_moves(
        Species::Abra,
        50,
        [0xff, 0xff],
        [MoveId::Teleport, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let mut enemy = create_pokemon_with_moves(
        Species::Caterpie,
        5,
        [0xff, 0xff],
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    player.status = StatusCondition::Poison;
    enemy.status = StatusCondition::Burn;
    let mut legacy = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);
    legacy
        .player
        .set_status3(crate::battle::state::status3::BADLY_POISONED);
    legacy.player.toxic_counter = 15;
    legacy
        .player
        .set_status2(crate::battle::state::status2::SEEDED);
    legacy
        .enemy
        .set_status2(crate::battle::state::status2::SEEDED);
    let (mut state, mut effects) = runtime::engine_state_from_legacy(&legacy);
    let hp = [state.player_battlers[0].hp, state.opponent_battlers[0].hp];
    clear_current_moves();
    set_current_move(
        BattlerRef::PLAYER,
        *MoveData::get(MoveId::Teleport).unwrap(),
    );
    set_current_move(
        BattlerRef::OPPONENT,
        *MoveData::get(MoveId::Tackle).unwrap(),
    );
    let mut rng = ScriptedRng::new(vec![]);
    let (_, log) = StackDriver::execute_turn_logged(
        &PokeredRules,
        &mut state,
        &mut effects,
        [
            BattleAction::Fight {
                move_: MoveId::Teleport,
            },
            BattleAction::Fight {
                move_: MoveId::Tackle,
            },
        ],
        &mut rng,
    );
    assert!(escape_succeeded(&effects));
    assert_eq!(rng.consumed(), 0);
    assert_eq!(
        hp,
        [state.player_battlers[0].hp, state.opponent_battlers[0].hp]
    );
    assert!(!log.events.iter().any(
        |event| matches!(event,TurnEvent::MoveUsed{actor,..} if *actor==BattlerRef::OPPONENT)
    ));
    let text = runtime::translate_turn(&log, &state, &effects);
    assert!(text.iter().any(|text| text == "ABRA ran from battle!"));
    runtime::apply_engine_to_legacy(&mut legacy, &state, &effects);
    assert!(legacy.escaped);
    assert_eq!(legacy.player.toxic_counter, 15);
}

#[test]
fn both_sides_flee_moves_cancel_other_action_and_use_original_outcome_text() {
    for actor in [BattlerRef::PLAYER, BattlerRef::OPPONENT] {
        for move_ in [MoveId::Teleport, MoveId::Roar, MoveId::Whirlwind] {
            install_canonical();
            clear_current_moves();
            let mk = |sp, level, mv| {
                create_pokemon_with_moves(
                    sp,
                    level,
                    [0xff, 0xff],
                    [mv, MoveId::None, MoveId::None, MoveId::None],
                )
                .unwrap()
            };
            let fast = mk(Species::Abra, 50, move_);
            let slow = mk(Species::Caterpie, 5, MoveId::Tackle);
            let (player, enemy) = if actor == BattlerRef::PLAYER {
                (fast, slow)
            } else {
                (slow, fast)
            };
            let legacy = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);
            let (mut state, mut effects) = runtime::engine_state_from_legacy(&legacy);
            let mut actions = [
                BattleAction::Fight {
                    move_: MoveId::Tackle,
                },
                BattleAction::Fight {
                    move_: MoveId::Tackle,
                },
            ];
            actions[actor.side as usize] = BattleAction::Fight { move_ };
            for side in [BattlerRef::PLAYER, BattlerRef::OPPONENT] {
                set_current_move(
                    side,
                    *MoveData::get(if side == actor { move_ } else { MoveId::Tackle }).unwrap(),
                );
            }
            let mut rng = ScriptedRng::new(vec![]);
            let (_, log) = StackDriver::execute_turn_logged(
                &PokeredRules,
                &mut state,
                &mut effects,
                actions,
                &mut rng,
            );
            let used: Vec<_> = log
                .events
                .iter()
                .filter_map(|event| match event {
                    TurnEvent::MoveUsed { actor, .. } => Some(*actor),
                    _ => None,
                })
                .collect();
            assert_eq!(used, vec![actor]);
            assert_eq!(rng.consumed(), 0);
            let target = if actor == BattlerRef::PLAYER {
                BattlerRef::OPPONENT
            } else {
                BattlerRef::PLAYER
            };
            let expected = match move_ {
                MoveId::Teleport => {
                    format!("{} ran from battle!", runtime::display_name(&state, actor))
                }
                MoveId::Roar => {
                    format!("{} ran away scared!", runtime::display_name(&state, target))
                }
                _ => format!("{} was blown away!", runtime::display_name(&state, target)),
            };
            let texts = runtime::translate_turn(&log, &state, &effects);
            assert_eq!(texts.last(), Some(&expected));
            assert_eq!(texts.len(), 2);
        }
    }
}

#[test]
fn trainer_flee_move_failure_is_narrated_and_does_not_escape() {
    for move_ in [MoveId::Teleport, MoveId::Roar, MoveId::Whirlwind] {
        install_canonical();
        clear_current_moves();
        let mk = |sp, level, mv| {
            create_pokemon_with_moves(
                sp,
                level,
                [0xff, 0xff],
                [mv, MoveId::None, MoveId::None, MoveId::None],
            )
            .unwrap()
        };
        let legacy = new_battle_state(
            BattleType::Trainer,
            vec![mk(Species::Abra, 50, move_)],
            vec![mk(Species::Caterpie, 5, MoveId::Tackle)],
        );
        let (mut state, mut effects) = runtime::engine_state_from_legacy(&legacy);
        set_current_move(BattlerRef::PLAYER, *MoveData::get(move_).unwrap());
        let mut rng = ScriptedRng::new(vec![]);
        let (_, log) = StackDriver::execute_turn_logged(
            &PokeredRules,
            &mut state,
            &mut effects,
            [BattleAction::Fight { move_ }, BattleAction::Nothing],
            &mut rng,
        );
        assert!(!escape_succeeded(&effects));
        assert_eq!(rng.consumed(), 0);
        let texts = runtime::translate_turn(&log, &state, &effects);
        assert_eq!(
            texts.last().unwrap(),
            if move_ == MoveId::Teleport {
                "But it failed!"
            } else {
                "Enemy CATERPIE is unaffected!"
            }
        );
    }
}

#[test]
fn mirror_move_resolving_to_teleport_uses_the_same_escape_pipeline() {
    install_canonical();
    clear_current_moves();
    let mk = |sp, level, mv| {
        create_pokemon_with_moves(
            sp,
            level,
            [0xff, 0xff],
            [mv, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    let mut legacy = new_battle_state(
        BattleType::Wild,
        vec![mk(Species::Abra, 50, MoveId::MirrorMove)],
        vec![mk(Species::Caterpie, 5, MoveId::Tackle)],
    );
    legacy.enemy.last_move_used = MoveId::Teleport;
    let (mut state, mut effects) = runtime::engine_state_from_legacy(&legacy);
    set_current_move(
        BattlerRef::PLAYER,
        *MoveData::get(MoveId::MirrorMove).unwrap(),
    );
    set_current_move(
        BattlerRef::OPPONENT,
        *MoveData::get(MoveId::Tackle).unwrap(),
    );
    let mut rng = ScriptedRng::new(vec![]);
    let (_, log) = StackDriver::execute_turn_logged(
        &PokeredRules,
        &mut state,
        &mut effects,
        [
            BattleAction::Fight {
                move_: MoveId::MirrorMove,
            },
            BattleAction::Fight {
                move_: MoveId::Tackle,
            },
        ],
        &mut rng,
    );
    assert!(escape_succeeded(&effects));
    assert_eq!(rng.consumed(), 0);
    let used: Vec<_> = log
        .events
        .iter()
        .filter_map(|event| match event {
            TurnEvent::MoveUsed { actor, move_ } => Some((*actor, *move_)),
            _ => None,
        })
        .collect();
    assert_eq!(used, vec![(BattlerRef::PLAYER, MoveId::Teleport)]);
}
