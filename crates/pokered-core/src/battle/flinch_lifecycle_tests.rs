//! MainInBattleLoop (core.asm:293–302) clears both late FLINCHED bits before
//! the next menu, except on the original Rage/recharge bypass. These tests
//! resolve real Headbutt effects through the canonical driver, write them back
//! through the production adapter, and resume through the public menu caller.
//! They are scoped core-caller tests, not a complete Game playthrough.
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;
use dotzuki_engine::battle::{
    rng::ScriptedRng,
    stack::{EffectState, StackDriver, TurnEvent, TurnLog},
    BattleAction, BattleState as EngineBattleState, BattlerRef,
};
use pokered_rules::{runtime, PokeVolatile, PokeredRules};

fn pokemon(species: Species, level: u8, moves: [MoveId; 4]) -> state::Pokemon {
    create_pokemon_with_moves(species, level, [0x88, 0x88], moves).unwrap()
}

fn screen(player: state::Pokemon, enemy: state::Pokemon) -> BattleScreen {
    let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
    screen.phase = BattlePhase::PlayerMenu;
    screen.rng = runtime::StdBattleRng::from_seed(42);
    screen
}

struct ResolvedTurn {
    log: TurnLog<PokeredRules>,
    state: EngineBattleState<PokeredRules>,
    effects: Vec<EffectState<PokeredRules>>,
    messages: Vec<String>,
    draws: usize,
}

// A scripted battle byte stream selects the real side-effect branch without
// searching seeds. It does not manufacture Flinched or a Blocked event.
fn canonical_turn(
    screen: &mut BattleScreen,
    player_move: MoveId,
    enemy_move: Option<MoveId>,
    bytes: Vec<u8>,
) -> ResolvedTurn {
    screen.sync_player_context();
    let bs = screen.battle_state.as_mut().unwrap();
    bs.player.selected_move = player_move;
    bs.player.selected_move_index = 0;
    if let Some(move_) = enemy_move {
        bs.enemy.selected_move = move_;
        bs.enemy.selected_move_index = 0;
    }
    let entry = (
        bs.player.battle_status1,
        bs.player.battle_status2,
        bs.player.has_status3(state::status3::TRANSFORMED),
        player_move,
    );
    let (mut state, mut effects) = runtime::engine_state_from_legacy(bs);
    pokered_rules::install_canonical();
    pokered_rules::clear_current_moves();
    pokered_rules::clear_last_move_live();
    pokered_rules::set_current_move(BattlerRef::PLAYER, *MoveData::get(player_move).unwrap());
    if let Some(move_) = enemy_move {
        pokered_rules::set_current_move(BattlerRef::OPPONENT, *MoveData::get(move_).unwrap());
    }
    let mut rng = ScriptedRng::new(bytes);
    let (_, log) = StackDriver::execute_turn_logged(
        &PokeredRules,
        &mut state,
        &mut effects,
        [
            BattleAction::Fight { move_: player_move },
            enemy_move.map_or(BattleAction::Nothing, |move_| BattleAction::Fight { move_ }),
        ],
        &mut rng,
    );
    let messages = runtime::translate_turn(&log, &state, &effects);
    runtime::apply_engine_to_legacy(bs, &state, &effects);
    BattleScreen::consume_player_pp_charge(bs, &mut state, &mut effects, entry, false);
    screen.sync_display_from_state();
    screen.show_text_then(messages.clone(), BattlePhase::PlayerMenu);
    ResolvedTurn {
        log,
        state,
        effects,
        messages,
        draws: rng.consumed(),
    }
}

fn finish_notice(screen: &mut BattleScreen) {
    for _ in 0..600 {
        if screen.phase == BattlePhase::PlayerMenu {
            return;
        }
        assert!(
            matches!(screen.phase, BattlePhase::ShowingText { .. }),
            "{:?}",
            screen.phase
        );
        screen.update_frame(BattleInput {
            a: true,
            ..BattleInput::none()
        });
    }
    panic!("turn narration did not return to the menu");
}

fn fight(screen: &mut BattleScreen, move_row: usize) {
    assert_eq!(screen.phase, BattlePhase::PlayerMenu);
    screen.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
    assert_eq!(screen.phase, BattlePhase::MoveSelect);
    for _ in 0..move_row {
        screen.update_frame(BattleInput {
            down: true,
            ..BattleInput::none()
        });
    }
    screen.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
    assert!(matches!(screen.phase, BattlePhase::ShowingText { .. }));
}

fn messages(screen: &BattleScreen) -> String {
    match &screen.phase {
        BattlePhase::ShowingText { messages, .. } => messages.join("\n"),
        phase => panic!("expected turn narration, got {phase:?}"),
    }
}

#[test]
fn slow_headbutt_on_an_already_acted_player_does_not_block_the_next_fight() {
    let player = pokemon(
        Species::Snorlax,
        50,
        [MoveId::Amnesia, MoveId::None, MoveId::None, MoveId::None],
    );
    let enemy = pokemon(
        Species::Slowpoke,
        33,
        [MoveId::Headbutt, MoveId::None, MoveId::None, MoveId::None],
    );
    assert!(player.speed > enemy.speed);
    let pp = player.pp[0];
    let mut screen = screen(player, enemy);
    // Amnesia draws nothing; Headbutt: noncrit255, hit0, damage255,
    // side-effect0. This really sets Flinched after the player has acted.
    let turn = canonical_turn(
        &mut screen,
        MoveId::Amnesia,
        Some(MoveId::Headbutt),
        vec![255, 255, 0, 0],
    );
    assert_eq!(turn.draws, 4);
    let used = turn
        .log
        .events
        .iter()
        .filter_map(|event| match event {
            TurnEvent::MoveUsed { actor, move_ } => Some((*actor, *move_)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        used,
        vec![
            (BattlerRef::PLAYER, MoveId::Amnesia),
            (BattlerRef::OPPONENT, MoveId::Headbutt)
        ]
    );
    let bs = screen.battle_state.as_ref().unwrap();
    assert!(bs.player.has_status1(state::status1::FLINCHED));
    assert_eq!(bs.player.active_mon().pp[0], pp - 1);
    assert!(!turn
        .log
        .events
        .iter()
        .any(|event| matches!(event, TurnEvent::Blocked { .. })));
    finish_notice(&mut screen);
    assert!(
        screen
            .battle_state
            .as_ref()
            .unwrap()
            .player
            .has_status1(state::status1::FLINCHED),
        "late flinch remains until the actual next menu update"
    );
    fight(&mut screen, 0);
    let bs = screen.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.last_move_used, MoveId::Amnesia);
    assert_eq!(
        bs.player.active_mon().pp[0],
        pp - 2,
        "the next accepted player action spends its own PP exactly once"
    );
    let text = messages(&screen);
    assert!(text.contains("SNORLAX used AMNESIA!"));
    assert!(!text.contains("hurt itself in confusion"));
    assert!(!text.contains("SNORLAX flinched!"));
}

#[test]
fn fast_headbutt_blocks_this_turn_with_flinch_text_and_does_not_block_the_next() {
    for status in [StatusCondition::None, StatusCondition::Paralysis] {
        let player = pokemon(
            Species::Tauros,
            50,
            [MoveId::Headbutt, MoveId::Growl, MoveId::None, MoveId::None],
        );
        let mut enemy = pokemon(
            Species::Snorlax,
            50,
            [MoveId::Growl, MoveId::None, MoveId::None, MoveId::None],
        );
        enemy.status = status;
        assert!(player.speed > enemy.speed);
        let mut screen = screen(player, enemy);
        let turn = canonical_turn(
            &mut screen,
            MoveId::Headbutt,
            Some(MoveId::Growl),
            vec![255, 255, 0, 0],
        );
        assert_eq!(turn.draws, 4, "flinch precedes any paralysis roll");
        assert!(turn.log.events.iter().any(|event| matches!(event,
            TurnEvent::Damaged { target, amount, .. } if *target == BattlerRef::OPPONENT && *amount > 0)));
        let blocked = turn
            .log
            .events
            .iter()
            .find(|event| {
                matches!(event,
            TurnEvent::Blocked { actor } if *actor == BattlerRef::OPPONENT)
            })
            .unwrap();
        assert!(turn
            .messages
            .iter()
            .any(|line| line == "Enemy SNORLAX flinched!"));
        assert_eq!(
            runtime::presentation_hp_message(blocked, &turn.state, &turn.effects).as_deref(),
            Some("Enemy SNORLAX flinched!")
        );
        assert!(!turn
            .messages
            .iter()
            .any(|line| line.contains("confusion") || line.contains("fully paralyzed")));
        assert_eq!(
            screen.battle_state.as_ref().unwrap().enemy.last_move_used,
            MoveId::None
        );
        assert!(
            !screen
                .battle_state
                .as_ref()
                .unwrap()
                .enemy
                .has_status1(state::status1::FLINCHED),
            "the real same-turn gate consumed this flinch"
        );
        // An unstatused enemy's next move must be accepted; the paralyzed
        // control above checks gate attribution without depending on a new roll.
        if status.is_none() {
            finish_notice(&mut screen);
            fight(&mut screen, 1); // Growl cannot cause a fresh flinch.
            let bs = screen.battle_state.as_ref().unwrap();
            assert_eq!(bs.player.last_move_used, MoveId::Growl);
            assert_eq!(bs.enemy.last_move_used, MoveId::Growl);
            assert!(messages(&screen).contains("Enemy SNORLAX used GROWL!"));
        }
    }
}

#[test]
fn actual_confusion_self_hit_keeps_its_gate_reason_even_when_paralyzed() {
    let mut player = pokemon(
        Species::Snorlax,
        50,
        [MoveId::Amnesia, MoveId::None, MoveId::None, MoveId::None],
    );
    player.status = StatusCondition::Paralysis;
    let hp = player.hp;
    let pp = player.pp[0];
    let enemy = pokemon(
        Species::Slowpoke,
        33,
        [MoveId::Growl, MoveId::None, MoveId::None, MoveId::None],
    );
    let mut screen = screen(player, enemy);
    let player = &mut screen.battle_state.as_mut().unwrap().player;
    player.set_status1(state::status1::CONFUSED);
    player.confused_turns_left = 3;
    // Original CP $80 / JR C: raw 128 selects the genuine self-hit branch.
    // Its narration receipt owns the cause before any paralysis roll is read.
    let turn = canonical_turn(&mut screen, MoveId::Amnesia, None, vec![128]);
    assert_eq!(
        turn.draws, 1,
        "self-hit blocks before the paralysis/attack rolls"
    );
    let blocked = turn
        .log
        .events
        .iter()
        .find(|event| {
            matches!(event,
        TurnEvent::Blocked { actor } if *actor == BattlerRef::PLAYER)
        })
        .unwrap();
    assert_eq!(
        turn.messages,
        vec!["SNORLAX hurt itself in confusion!".to_string()]
    );
    assert_eq!(
        runtime::presentation_hp_message(blocked, &turn.state, &turn.effects).as_deref(),
        Some("SNORLAX hurt itself in confusion!")
    );
    let bs = screen.battle_state.as_ref().unwrap();
    assert!(bs.player.active_mon().hp < hp);
    assert_eq!(bs.player.active_mon().pp[0], pp);
    assert_eq!(bs.player.last_move_used, MoveId::None);
    assert_eq!(bs.player.active_mon().status, StatusCondition::Paralysis);
    let (_, rebuilt) = runtime::engine_state_from_legacy(bs);
    assert!(
        !rebuilt
            .iter()
            .any(|entry| matches!(entry.kind, PokeVolatile::BlockedAction { .. })),
        "narration receipts are not carried into the next turn's arena"
    );
}

#[test]
fn next_menu_clears_both_flinches_but_preserves_original_rage_recharge_bypass() {
    for bypass in [
        0,
        state::status2::USING_RAGE,
        state::status2::NEEDS_TO_RECHARGE,
    ] {
        let mut screen = screen(
            pokemon(
                Species::Snorlax,
                50,
                [MoveId::Amnesia, MoveId::None, MoveId::None, MoveId::None],
            ),
            pokemon(
                Species::Slowpoke,
                33,
                [MoveId::Growl, MoveId::None, MoveId::None, MoveId::None],
            ),
        );
        let bs = screen.battle_state.as_mut().unwrap();
        bs.player
            .set_status1(state::status1::FLINCHED | state::status1::CONFUSED);
        bs.player.confused_turns_left = 3;
        bs.enemy.set_status1(state::status1::FLINCHED);
        bs.player.set_status2(bypass);
        let mut expected = bs.clone();
        if bypass == 0 {
            expected.player.clear_status1(state::status1::FLINCHED);
            expected.enemy.clear_status1(state::status1::FLINCHED);
        }
        screen.update_frame(BattleInput::none());
        assert_eq!(screen.phase, BattlePhase::PlayerMenu);
        assert_eq!(
            serde_json::to_value(screen.battle_state.as_ref().unwrap()).unwrap(),
            serde_json::to_value(expected).unwrap(),
            "bypass={bypass}"
        );
    }
}

#[test]
fn blocked_gate_receipt_survives_same_turn_trainer_ai_state_rebuild() {
    let player = pokemon(
        Species::Tauros,
        50,
        [MoveId::Rage, MoveId::None, MoveId::None, MoveId::None],
    );
    let mut enemy = pokemon(
        Species::Onix,
        30,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    );
    enemy.status = StatusCondition::Poison;
    let mut screen =
        BattleScreen::from_parties(false, &[player], &[enemy], Some(TrainerClass::Brock));
    screen.phase = BattlePhase::PlayerMenu;
    screen.trainer_name = Some("BROCK".to_string());
    screen.rng = runtime::StdBattleRng::from_seed(42);
    let bs = screen.battle_state.as_mut().unwrap();
    // A late flinch while locked into Rage takes the original MainInBattleLoop
    // bypass, reaches the next action gate, then the trainer cures its Onix.
    bs.player.set_status1(state::status1::FLINCHED);
    bs.player.set_status2(state::status2::USING_RAGE);
    bs.player.selected_move = MoveId::Rage;
    bs.player.selected_move_index = 0;
    let pp = bs.player.active_mon().pp[0];
    fight(&mut screen, 0);
    let bs = screen.battle_state.as_ref().unwrap();
    assert!(
        bs.enemy.active_mon().status.is_none(),
        "the real trainer AI rebuilt the arena"
    );
    assert_eq!(screen.enemy_ai_count, 4);
    assert_eq!(bs.player.active_mon().pp[0], pp);
    assert_eq!(bs.player.last_move_used, MoveId::None);
    let text = messages(&screen);
    assert!(text.contains("TAUROS flinched!"));
    assert!(text.contains("FULL HEAL"));
    assert!(!text.contains("TAUROS can't move"));
    assert!(!text.contains("hurt itself in confusion"));
}
