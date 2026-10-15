use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;
use crate::snapshot::BattleSnapshot;

fn screen(player_speed: u16, enemy_speed: u16, seed: u64) -> BattleScreen {
    let mut player = create_pokemon_with_moves(
        Species::Rattata,
        12,
        [0x9a, 0x78],
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let mut enemy = player.clone();
    player.speed = player_speed;
    enemy.speed = enemy_speed;
    let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
    screen.rng = pokered_rules::runtime::StdBattleRng::from_seed(seed);
    screen.phase = BattlePhase::PlayerMenu;
    screen
}

#[test]
fn run_rng_owned_probabilistic_success_advances_one_byte() {
    // 127 < 128, so this is a probabilistic attempt with odds 127. The
    // seeded first byte is at that inclusive boundary, not a speed shortcut.
    let mut screen = screen(127, 128, 0);
    let mut expected = screen.rng.clone();
    assert_eq!(expected.next_u8(), 127);
    screen.handle_run();
    let bs = screen.battle_state.as_ref().unwrap();
    assert!(bs.escaped);
    assert_eq!(bs.num_run_attempts, 1);
    assert_eq!(
        serde_json::to_value(&screen.rng).unwrap(),
        serde_json::to_value(&expected).unwrap(),
        "RUN must advance its saved RNG once"
    );
    assert_eq!(
        bs.enemy.active_mon().pp[0],
        35,
        "a successful run gives no enemy turn"
    );
}

#[test]
fn run_rng_owned_snapshot_replays_failed_attempt_and_one_enemy_turn() {
    let mut screen = screen(50, 200, 19);
    let saved = BattleSnapshot::capture(&screen);
    let mut first = screen.rng.clone();
    assert!(first.next_u8() > 32, "seeded first attempt must fail");
    screen.handle_run();
    let bs = screen.battle_state.as_ref().unwrap();
    assert!(!bs.escaped);
    assert_eq!(bs.num_run_attempts, 1);
    assert_eq!(bs.enemy.last_move_used, MoveId::Tackle);
    let BattlePhase::ShowingText { ref messages, .. } = screen.phase else {
        panic!("failed RUN must narrate");
    };
    assert_eq!(
        messages
            .iter()
            .filter(|m| m.contains("used TACKLE"))
            .count(),
        1,
        "one failed RUN must execute one enemy move"
    );
    let expected = serde_json::to_value(BattleSnapshot::capture(&screen)).unwrap();
    for _ in 0..8 {
        saved.restore_into(&mut screen);
        assert_eq!(screen.battle_state.as_ref().unwrap().num_run_attempts, 0);
        screen.handle_run();
        assert_eq!(
            serde_json::to_value(BattleSnapshot::capture(&screen)).unwrap(),
            expected
        );
    }
}

#[test]
fn run_rng_owned_early_exits_leave_stream_and_attempt_rules_unchanged() {
    for kind in ["trainer", "ghost", "link", "faster"] {
        let mut screen = screen(50, 200, 19);
        match kind {
            "trainer" => {
                screen.is_wild = false;
                screen.battle_state.as_mut().unwrap().battle_type = BattleType::Trainer;
            }
            "ghost" => screen.is_ghost = true,
            "link" => screen.link_mode = true,
            "faster" => screen = self::screen(200, 50, 19),
            _ => unreachable!(),
        }
        let rng = serde_json::to_value(&screen.rng).unwrap();
        screen.handle_run();
        assert_eq!(serde_json::to_value(&screen.rng).unwrap(), rng, "{kind}");
        assert_eq!(
            screen.battle_state.as_ref().unwrap().num_run_attempts,
            if kind == "faster" { 1 } else { 0 },
            "{kind}"
        );
        if kind == "link" {
            assert!(matches!(screen.phase, BattlePhase::LinkWaiting));
            assert_eq!(
                screen.link_pending_local_action,
                Some(crate::link::protocol::LinkAction::Run)
            );
        }
    }
}
