//! Original core.asm checks AnyPartyAlive before awarding a simultaneous KO.
//! Exercise real Selfdestruct/recoil turns, not fabricated terminal phases.
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

fn mon(species: Species, level: u8, move_id: MoveId) -> state::Pokemon {
    create_pokemon_with_moves(
        species,
        level,
        [0x9a, 0x78],
        [move_id, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap()
}

// reserve: None = one mon; Some(false) = a fainted reserve; Some(true) = alive.
fn battle(
    wild: bool,
    player_attacks: bool,
    move_id: MoveId,
    reserve: Option<bool>,
    enemy_reserve: bool,
) -> BattleScreen {
    let mut attacker = mon(Species::Electrode, 60, move_id);
    let mut target = mon(Species::Magikarp, 5, MoveId::Splash);
    // A connecting hit is lethal and Double-Edge's minimum recoil is lethal.
    attacker.hp = 1;
    target.hp = 1;
    let (player, enemy) = if player_attacks {
        (attacker, target)
    } else {
        (target, attacker)
    };
    let mut players = vec![player];
    if let Some(alive) = reserve {
        let mut reserve = mon(Species::Squirtle, 20, MoveId::Tackle);
        if !alive {
            reserve.hp = 0;
        }
        players.push(reserve);
    }
    let mut enemies = vec![enemy];
    if enemy_reserve {
        enemies.push(mon(Species::Caterpie, 5, MoveId::Tackle));
    }
    let mut screen = BattleScreen::from_parties(
        wild,
        &players,
        &enemies,
        (!wild).then_some(TrainerClass::Youngster),
    );
    screen.player_badges = 0xff;
    screen.player_money = 1000;
    screen.rng = pokered_rules::runtime::StdBattleRng::from_seed(42);
    screen
}

fn attack(screen: &mut BattleScreen) {
    screen.execute_turn_with_move(0);
    let bs = screen.battle_state.as_ref().unwrap();
    assert_eq!(
        bs.player.active_mon().hp,
        0,
        "real move must faint the player"
    );
    assert_eq!(
        bs.enemy.active_mon().hp,
        0,
        "real move must faint the enemy"
    );
}

fn continuation(screen: &BattleScreen) -> &BattlePhase {
    match &screen.phase {
        BattlePhase::ShowingText { next_phase, .. } => next_phase,
        phase => phase,
    }
}

fn assert_terminal(screen: &BattleScreen, won: bool) {
    match continuation(screen) {
        BattlePhase::TrainerVictory { player_won, .. } => assert_eq!(*player_won, won),
        BattlePhase::BattleOver {
            won: actual,
            escaped,
            ..
        } => {
            assert_eq!(*actual, won);
            assert!(!escaped);
        }
        phase => panic!("expected terminal outcome, got {phase:?}"),
    }
}

fn finish_text(screen: &mut BattleScreen) {
    for _ in 0..1000 {
        if !matches!(screen.phase, BattlePhase::ShowingText { .. }) {
            return;
        }
        screen.update_frame(BattleInput {
            a: true,
            ..BattleInput::none()
        });
    }
    panic!("text did not return: {:?}", screen.phase);
}

fn finish_battle(screen: &mut BattleScreen) {
    for _ in 0..2000 {
        if matches!(
            screen.update_frame(BattleInput {
                a: true,
                ..BattleInput::none()
            }),
            ScreenAction::Transition(GameScreen::Overworld)
        ) {
            return;
        }
    }
    panic!("battle did not return: {:?}", screen.phase);
}

fn terminal_matrix(move_id: MoveId) {
    for wild in [true, false] {
        for player_attacks in [true, false] {
            for reserve in [None, Some(false), Some(true)] {
                let mut screen = battle(wild, player_attacks, move_id, reserve, false);
                attack(&mut screen);
                let won = reserve == Some(true);
                assert_terminal(&screen, won);
                finish_battle(&mut screen);
                let settlement = screen.settlement.as_ref().unwrap();
                assert_eq!(
                    settlement.outcome,
                    if won {
                        BattleOutcome::Win
                    } else {
                        BattleOutcome::Loss
                    }
                );
                if !won {
                    assert_eq!(settlement.money_gained, 0);
                    assert_eq!(settlement.money_lost, 500);
                    assert!(settlement.exp_entries.is_empty());
                }
            }
        }
    }
}

#[test]
fn selfdestruct_final_double_ko_respects_whole_party_survival() {
    terminal_matrix(MoveId::Selfdestruct);
}

#[test]
fn recoil_final_double_ko_respects_whole_party_survival() {
    terminal_matrix(MoveId::DoubleEdge);
}

#[test]
fn enemy_free_selfdestruct_uses_the_same_loss_and_win_priority() {
    for wild in [true, false] {
        for reserve in [None, Some(false), Some(true)] {
            let mut screen = battle(wild, false, MoveId::Selfdestruct, reserve, false);
            screen.run_enemy_free_turn_stack(Vec::new(), Some((MoveId::Selfdestruct, 0)));
            let bs = screen.battle_state.as_ref().unwrap();
            assert_eq!(bs.player.active_mon().hp, 0);
            assert_eq!(bs.enemy.active_mon().hp, 0);
            assert_terminal(&screen, reserve == Some(true));
        }
    }
}

#[test]
fn all_dead_cannot_enter_shift_or_send_another_enemy() {
    for style in [BattleStyle::Shift, BattleStyle::Set] {
        for reserve in [None, Some(false)] {
            for player_attacks in [true, false] {
                let mut screen = battle(false, player_attacks, MoveId::Selfdestruct, reserve, true);
                screen.battle_style = style;
                attack(&mut screen);
                assert_terminal(&screen, false);
                finish_battle(&mut screen);
                assert_eq!(
                    screen
                        .battle_state
                        .as_ref()
                        .unwrap()
                        .enemy
                        .active_pokemon_index,
                    0
                );
                assert_eq!(
                    screen.settlement.as_ref().unwrap().outcome,
                    BattleOutcome::Loss
                );
            }
        }
    }
}

#[test]
fn both_reserves_force_own_replacement_before_enemy_send_out() {
    for style in [BattleStyle::Shift, BattleStyle::Set] {
        for move_id in [MoveId::Selfdestruct, MoveId::DoubleEdge] {
            for player_attacks in [true, false] {
                let mut screen = battle(false, player_attacks, move_id, Some(true), true);
                screen.battle_style = style;
                attack(&mut screen);
                assert_eq!(continuation(&screen), &BattlePhase::PlayerFaintSwitch);
                finish_text(&mut screen);
                assert_eq!(screen.phase, BattlePhase::PlayerFaintSwitch);
                let pp = screen.battle_state.as_ref().unwrap().player.party[1].pp;
                let enemy_pp = screen.battle_state.as_ref().unwrap().enemy.party[1].pp;
                let exp_after_turn: Vec<_> = screen
                    .battle_state
                    .as_ref()
                    .unwrap()
                    .player
                    .party
                    .iter()
                    .map(|mon| mon.total_exp)
                    .collect();
                screen.update_frame(BattleInput {
                    b: true,
                    ..BattleInput::none()
                });
                assert_eq!(
                    screen.phase,
                    BattlePhase::PlayerFaintSwitch,
                    "B cannot cancel"
                );
                screen.update_frame(BattleInput {
                    a: true,
                    ..BattleInput::none()
                });
                assert_eq!(
                    continuation(&screen),
                    &BattlePhase::PlayerFaintSwitch,
                    "dead lead rejected"
                );
                finish_text(&mut screen);
                screen.update_frame(BattleInput {
                    down: true,
                    ..BattleInput::none()
                });
                screen.update_frame(BattleInput {
                    a: true,
                    ..BattleInput::none()
                });
                let bs = screen.battle_state.as_ref().unwrap();
                assert_eq!(
                    (
                        bs.player.active_pokemon_index,
                        bs.enemy.active_pokemon_index
                    ),
                    (1, 0)
                );
                let expected = if style == BattleStyle::Shift {
                    BattlePhase::ShiftPrompt
                } else {
                    BattlePhase::EnemySendingNext { wait_frames: 30 }
                };
                assert_eq!(continuation(&screen), &expected);
                finish_text(&mut screen);
                assert_eq!(screen.phase, expected);
                if style == BattleStyle::Shift {
                    let bs = screen.battle_state.as_ref().unwrap();
                    assert_eq!(bs.player.party.iter().filter(|mon| mon.hp > 0).count(), 1,
                        "only the current replacement is alive; original SHIFT still counts the dead lead");
                    assert_eq!(
                        bs.enemy.active_pokemon_index, 0,
                        "the old enemy remains until the optional answer"
                    );
                    assert!(!screen.shift_prompt_yes, "original defaults to NO");
                    assert!(screen
                        .current_message
                        .as_ref()
                        .unwrap()
                        .contains("is about to use"));
                    // Exercise both original NO routes without another switch.
                    let no = if player_attacks {
                        BattleInput {
                            b: true,
                            ..BattleInput::none()
                        }
                    } else {
                        BattleInput {
                            a: true,
                            ..BattleInput::none()
                        }
                    };
                    screen.update_frame(no);
                    assert_eq!(
                        screen.phase,
                        BattlePhase::EnemySendingNext { wait_frames: 30 }
                    );
                }
                let bs = screen.battle_state.as_ref().unwrap();
                assert_eq!(
                    (
                        bs.player.active_pokemon_index,
                        bs.enemy.active_pokemon_index
                    ),
                    (1, 1)
                );
                // A second send_next_enemy would reset this live volatile.
                // Waiting for the send-out cannot reset or act on the mon again.
                let enemy = &mut screen.battle_state.as_mut().unwrap().enemy;
                enemy.set_status1(state::status1::CONFUSED);
                enemy.confused_turns_left = 2;
                for _ in 0..31 {
                    screen.update_frame(BattleInput {
                        b: true,
                        ..BattleInput::none()
                    });
                }
                assert_eq!(screen.phase, BattlePhase::PlayerMenu);
                let bs = screen.battle_state.as_ref().unwrap();
                assert!(bs.player.active_mon().hp > 0 && bs.enemy.active_mon().hp > 0);
                assert!(bs.enemy.has_status1(state::status1::CONFUSED));
                assert_eq!(
                    bs.enemy.confused_turns_left, 2,
                    "enemy sent exactly once, no turn taken"
                );
                assert_eq!(
                    bs.player
                        .party
                        .iter()
                        .map(|mon| mon.total_exp)
                        .collect::<Vec<_>>(),
                    exp_after_turn,
                    "continuations cannot distribute EXP a second time"
                );
                assert_eq!(bs.player.active_mon().pp, pp, "replacement is not a move");
                assert_eq!(bs.enemy.active_mon().pp, enemy_pp, "no enemy free attack");
                assert_eq!(
                    bs.party_gain_exp_flags,
                    [false, true, false, false, false, false]
                );
                assert_eq!(
                    bs.party_fought_flags,
                    [false, true, false, false, false, false]
                );
                assert!(screen.pending_shift_switch.is_none());
            }
        }
    }
}

#[test]
fn ordinary_faint_replacement_keeps_the_living_enemy_and_returns_to_menu() {
    let mut screen = battle(false, true, MoveId::Selfdestruct, Some(true), true);
    screen.battle_state.as_mut().unwrap().player.party[0].hp = 0;
    screen.phase = screen.check_faint_after_turn();
    assert_eq!(screen.phase, BattlePhase::PlayerFaintSwitch);
    screen.update_frame(BattleInput {
        down: true,
        ..BattleInput::none()
    });
    screen.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
    assert_eq!(continuation(&screen), &BattlePhase::PlayerMenu);
    finish_text(&mut screen);
    assert_eq!(screen.phase, BattlePhase::PlayerMenu);
    assert_eq!(
        screen
            .battle_state
            .as_ref()
            .unwrap()
            .enemy
            .active_pokemon_index,
        0
    );
}

#[test]
fn living_active_with_only_dead_reserve_retains_original_shift_count_prompt() {
    let mut screen = battle(false, true, MoveId::Tackle, Some(false), true);
    screen.battle_state.as_mut().unwrap().enemy.party[0].hp = 0;
    assert_eq!(
        screen.check_faint_after_turn(),
        BattlePhase::ShiftPrompt,
        "original ReplaceFaintedEnemyMon checks party count, not healthy reserve count"
    );
}

#[test]
fn real_double_ko_loss_writes_blackout_without_trainer_victory() {
    use crate::overworld::OverworldScreen;
    use crate::save::SaveData;
    use pokered_data::impl_traits::PokemonRedData;
    use pokered_data::maps::MapId;
    for wild in [true, false] {
        let mut screen = battle(wild, true, MoveId::Selfdestruct, Some(false), false);
        screen.map_id = MapId::Route1 as u8;
        let mut save = SaveData::new();
        save.game_data.player_money = 1000;
        let mut overworld = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
        attack(&mut screen);
        finish_battle(&mut screen);
        let result = settlement::settle_battle_into_save(&mut screen, &mut save, &mut overworld);
        assert_eq!(result.outcome, Some("lose"));
        assert_eq!(save.game_data.player_money, 500);
        assert!(overworld.heal_requested);
        assert!(overworld.pending_warp.is_some());
        assert!(save.party.iter().all(|mon| mon.hp == 0));
    }
}

// Narration observes the same actual-turn fixtures as the outcome tests above.
// The core harness acknowledges ONLY the existing frontend presentation API;
// it does not fabricate an HP-zero log, handler selector or reserve receipt.
fn narration(screen: &mut BattleScreen) -> (Vec<FaintCue>, Vec<String>) {
    let mut cues = Vec::new();
    let mut captions = Vec::new();
    let mut serial = None;
    for _ in 0..2500 {
        if !matches!(screen.phase, BattlePhase::ShowingText { .. }) {
            return (cues, captions);
        }
        if let Some((id, cue)) = screen.presentation.current_faint() {
            if serial != Some(id) {
                cues.push(cue);
                serial = Some(id);
            }
        }
        if screen.presentation.waiting {
            // Even A/B cannot acknowledge an unfinished structural action.
            let before = screen.current_message.clone();
            screen.update_frame(BattleInput {
                a: true,
                b: true,
                ..BattleInput::none()
            });
            assert_eq!(screen.current_message, before);
            screen.complete_move_presentation();
        }
        if let Some(message) = screen
            .current_message
            .as_ref()
            .filter(|m| m.ends_with("fainted!"))
        {
            if captions.last() != Some(message) {
                captions.push(message.clone());
            }
        }
        // A hidden cue must return without consuming a text acknowledgement.
        let silent = screen
            .presentation
            .current_faint()
            .is_some_and(|(_, cue)| cue.text.is_none());
        screen.update_frame(if silent {
            BattleInput::none()
        } else {
            BattleInput {
                a: true,
                ..BattleInput::none()
            }
        });
    }
    panic!("faint presentation did not return: {:?}", screen.phase);
}

fn narration_matrix(move_id: MoveId) {
    for wild in [false, true] {
        for player_attacks in [true, false] {
            for reserve in [None, Some(false), Some(true)] {
                for actor_first in [false, true] {
                    let mut screen = battle(wild, player_attacks, move_id, reserve, false);
                    let bs = screen.battle_state.as_mut().unwrap();
                    let player_fast = player_attacks == actor_first;
                    bs.player.active_mon_mut().speed = if player_fast { 200 } else { 1 };
                    bs.enemy.active_mon_mut().speed = if player_fast { 1 } else { 200 };
                    screen.set_presentation_enabled(true);
                    attack(&mut screen);
                    let pp = screen.battle_state.as_ref().unwrap().player.active_mon().pp;
                    let outcome = continuation(&screen).clone();
                    let (cues, captions) = narration(&mut screen);
                    let alive = reserve == Some(true);
                    let enemy_sound = if wild {
                        FaintSound::WildVictory
                    } else {
                        FaintSound::EnemyFall
                    };
                    let expected = if player_attacks {
                        vec![
                            FaintCue {
                                side: FaintSide::Enemy,
                                sound: Some(enemy_sound),
                                text: None,
                            },
                            FaintCue {
                                side: FaintSide::Player,
                                sound: None,
                                text: alive.then_some(FaintSide::Enemy),
                            },
                        ]
                    } else {
                        let mut cues = vec![FaintCue {
                            side: FaintSide::Player,
                            sound: Some(FaintSound::OwnCry),
                            text: Some(FaintSide::Player),
                        }];
                        if alive {
                            cues.push(FaintCue {
                                side: FaintSide::Enemy,
                                sound: Some(enemy_sound),
                                text: Some(FaintSide::Enemy),
                            });
                        }
                        cues
                    };
                    assert_eq!(cues, expected, "actor={player_attacks} first={actor_first} reserve={reserve:?} move={move_id:?}");
                    let expected_captions: Vec<_> =
                        expected.iter().filter_map(|cue| cue.text).collect();
                    assert_eq!(captions.len(), expected_captions.len());
                    for (caption, side) in captions.iter().zip(expected_captions) {
                        assert_eq!(caption.starts_with("Enemy "), side == FaintSide::Enemy);
                    }
                    assert_eq!(
                        screen.battle_state.as_ref().unwrap().player.active_mon().pp,
                        pp
                    );
                    assert_eq!(screen.phase, outcome);
                }
            }
        }
    }
}

#[test]
fn actual_selfdestruct_and_explosion_faint_narration_uses_handler_entry_in_both_speed_slots() {
    narration_matrix(MoveId::Selfdestruct);
    narration_matrix(MoveId::Explosion);
}

#[test]
fn actual_lethal_recoil_uses_the_same_handler_narration_and_structural_slides() {
    narration_matrix(MoveId::DoubleEdge);
}

#[test]
fn enemy_free_turn_faint_narration_retains_own_before_enemy_and_blackout_short_circuit() {
    for reserve in [None, Some(false), Some(true)] {
        let mut screen = battle(false, false, MoveId::Selfdestruct, reserve, false);
        screen.set_presentation_enabled(true);
        screen.run_enemy_free_turn_stack(Vec::new(), Some((MoveId::Selfdestruct, 0)));
        let (cues, captions) = narration(&mut screen);
        assert_eq!(
            cues[0],
            FaintCue {
                side: FaintSide::Player,
                sound: Some(FaintSound::OwnCry),
                text: Some(FaintSide::Player)
            }
        );
        assert_eq!(cues.len(), if reserve == Some(true) { 2 } else { 1 });
        assert_eq!(captions.len(), cues.len());
    }
}

#[test]
fn single_user_recoil_selfdestruct_and_residual_faints_keep_their_real_cause_and_own_cry() {
    for move_id in [MoveId::Selfdestruct, MoveId::DoubleEdge] {
        let mut screen = battle(false, true, move_id, Some(true), false);
        // The actual target survives: player move identity must not suppress
        // this ordinary HandlePlayerMonFainted path.
        screen.battle_state.as_mut().unwrap().enemy.party[0] =
            mon(Species::Snorlax, 100, MoveId::Splash);
        screen.set_presentation_enabled(true);
        screen.execute_turn_with_move(0);
        assert!(screen.battle_state.as_ref().unwrap().enemy.active_mon().hp > 0);
        let (cues, captions) = narration(&mut screen);
        assert_eq!(
            cues,
            vec![FaintCue {
                side: FaintSide::Player,
                sound: Some(FaintSound::OwnCry),
                text: Some(FaintSide::Player)
            }]
        );
        assert_eq!(captions.len(), 1);
    }
    let mut screen = battle(false, true, MoveId::Splash, Some(true), false);
    screen
        .battle_state
        .as_mut()
        .unwrap()
        .player
        .active_mon_mut()
        .status = state::StatusCondition::Poison;
    screen
        .battle_state
        .as_mut()
        .unwrap()
        .enemy
        .active_mon_mut()
        .hp = 100;
    screen
        .battle_state
        .as_mut()
        .unwrap()
        .enemy
        .active_mon_mut()
        .max_hp = 100;
    screen.set_presentation_enabled(true);
    screen.execute_turn_with_move(0);
    let messages = match &screen.phase {
        BattlePhase::ShowingText { messages, .. } => messages,
        _ => panic!(),
    };
    assert!(messages.iter().any(|message| message.contains("POISON")));
    let (cues, captions) = narration(&mut screen);
    assert_eq!(
        cues,
        vec![FaintCue {
            side: FaintSide::Player,
            sound: Some(FaintSound::OwnCry),
            text: Some(FaintSide::Player)
        }]
    );
    assert_eq!(captions.len(), 1);
}

#[test]
fn silent_faint_page_snapshot_keeps_typed_actions_and_needs_no_new_ack() {
    use crate::snapshot::BattleSnapshot;
    let mut screen = battle(false, true, MoveId::Selfdestruct, None, false);
    screen.set_presentation_enabled(true);
    attack(&mut screen);
    for _ in 0..1000 {
        if screen.presentation.current_faint().is_some() {
            break;
        }
        if screen.presentation.waiting {
            screen.complete_move_presentation();
        }
        screen.update_frame(BattleInput {
            a: true,
            ..BattleInput::none()
        });
    }
    let expected = screen.presentation.current_faint().unwrap();
    assert_eq!(expected.1.text, None);
    let snapshot = BattleSnapshot::capture(&screen);
    let saved = serde_json::to_vec(&snapshot).unwrap();
    let snapshot: BattleSnapshot = serde_json::from_slice(&saved).unwrap();
    let mut restored = BattleScreen::new(false);
    snapshot.restore_into(&mut restored);
    assert_eq!(restored.presentation.current_faint(), Some(expected));
    assert!(restored.presentation.waiting);
    let (cues, captions) = narration(&mut restored);
    assert_eq!(cues.len(), 2);
    assert!(captions.is_empty());
    assert_terminal(&restored, false);
}

#[test]
fn link_turn_retains_its_existing_faint_text_and_does_not_enter_the_new_presentation_policy() {
    let mut screen = battle(false, true, MoveId::Selfdestruct, Some(true), false);
    screen.link_mode = true;
    screen.execute_turn_with_move(0);
    assert!(screen.presentation.current_faint().is_none());
    let messages = match &screen.phase {
        BattlePhase::ShowingText { messages, .. } => messages,
        _ => panic!(),
    };
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.ends_with("fainted!"))
            .count(),
        2
    );
}
