//! Pokémon Red `CheckForDisobedience` (`engine/battle/core.asm:3099, 3830-4007`)
//! ordering and Link Battle exemption regressions.

use super::*;
use crate::link::rng::LinkRng;
use crate::pokemon::stats::create_pokemon_with_moves;
use pokered_data::species::Species;

fn traded_screen(
    player_moves: [MoveId; 4],
    enemy_move: MoveId,
    player_speed: u16,
    enemy_speed: u16,
    stream: [u8; 10],
) -> BattleScreen {
    let mut player =
        create_pokemon_with_moves(Species::Charizard, 50, [0x9a, 0x78], player_moves).unwrap();
    player.ot_id = 9999;
    player.speed = player_speed;
    let mut enemy = create_pokemon_with_moves(
        Species::Snorlax,
        50,
        [0x9a, 0x78],
        [enemy_move, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    enemy.speed = enemy_speed;
    let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
    screen.player_id = 1234;
    screen.player_badges = 0; // threshold = 10 < level 50
    screen.link_enemy_move_override = Some((enemy_move, 0));
    screen.link_rng = Some(LinkRng::new(stream));
    screen.phase = BattlePhase::PlayerMenu;
    screen
}

fn turn_messages(screen: &BattleScreen) -> Vec<String> {
    match &screen.phase {
        BattlePhase::ShowingText { messages, .. } => messages.clone(),
        other => panic!("expected ShowingText, got {other:?}"),
    }
}

#[test]
fn link_battle_skips_disobedience_and_draws_no_obedience_bytes() {
    // Bytes [0xC1, 0x1C, 0x90, 0x19, ...] would force Nap(3) if CheckForDisobedience ran.
    let stream = [0xC1, 0x1C, 0x90, 0x19, 0, 0, 0, 0, 0, 0];
    let mut screen = traded_screen(
        [MoveId::Splash, MoveId::Growl, MoveId::None, MoveId::None],
        MoveId::Splash,
        200,
        50,
        stream,
    );
    screen.link_mode = true;
    screen.battle_state.as_mut().unwrap().link_battle = true;
    screen.execute_turn_with_move(0);

    let bs = screen.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.active_mon().status, StatusCondition::None);
    assert_eq!(
        screen.link_rng.as_ref().unwrap().consumed(),
        0,
        "Splash vs Splash in Link Battle draws 0 RNG bytes when obedience is skipped",
    );
    let msgs = turn_messages(&screen);
    assert!(msgs.iter().any(|m| m == "CHARIZARD used SPLASH!"), "{msgs:?}");
    assert!(!msgs.iter().any(|m| m.contains("nap") || m.contains("loafing")), "{msgs:?}");
}

#[test]
fn confusion_self_hit_precedes_disobedience_without_napping_or_drawing_obedience_rng() {
    // Byte 0: confusion self-hit roll = 0x80 (>= 128 => hurt itself!).
    // Remaining bytes [0xC1, 0x1C, 0x90, 0x19] would cause Nap(3) if disobedience ran.
    let mut screen = traded_screen(
        [MoveId::Splash, MoveId::Growl, MoveId::None, MoveId::None],
        MoveId::Splash,
        200,
        50,
        [0x80, 0xC1, 0x1C, 0x90, 0x19, 0, 0, 0, 0, 0],
    );
    {
        let p = &mut screen.battle_state.as_mut().unwrap().player;
        p.set_status1(state::status1::CONFUSED);
        p.confused_turns_left = 3;
    }
    let hp_before = screen.battle_state.as_ref().unwrap().player.active_mon().hp;
    let pp_before = screen.battle_state.as_ref().unwrap().player.active_mon().pp;
    screen.execute_turn_with_move(0);

    let bs = screen.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.confused_turns_left, 2, "confusion counter ticked");
    assert_eq!(bs.player.active_mon().status, StatusCondition::None, "must not nap");
    assert!(bs.player.active_mon().hp < hp_before, "took confusion self-hit damage");
    assert_eq!(bs.player.active_mon().pp, pp_before, "blocked turn spends no PP");
    assert_eq!(
        screen.link_rng.as_ref().unwrap().consumed(),
        1,
        "only the 1 confusion byte is consumed (Splash draws 0)",
    );
    let msgs = turn_messages(&screen);
    assert_eq!(msgs[0], "CHARIZARD hurt itself in\nconfusion!");
    assert!(!msgs.iter().any(|m| m.contains("nap") || m.contains("won't")), "{msgs:?}");
}

#[test]
fn passing_confusion_then_rolls_disobedience_in_source_order() {
    // Byte 0: confusion roll = 0x00 (< 128 => passes confusion!).
    // Bytes 1..=3: L50, 0 badges (c=10, d=50, b=60, diff=40):
    //   loop1: swap(0x13) = 0x31 = 49 (< 60, >= 10)
    //   loop2: 0x31 = 49 (< 60, >= 10)
    //   branch: swap(0x01) = 0x10 = 16 (< diff=40 => Nap!)
    //   nap turns: 0x19 => 2*0x19=0x32 => swap=0x23 & 7 = 3 turns.
    let mut screen = traded_screen(
        [MoveId::Splash, MoveId::Growl, MoveId::None, MoveId::None],
        MoveId::Splash,
        200,
        50,
        [0x00, 0x13, 0x31, 0x01, 0x19, 0, 0, 0, 0, 0],
    );
    {
        let p = &mut screen.battle_state.as_mut().unwrap().player;
        p.set_status1(state::status1::CONFUSED);
        p.confused_turns_left = 3;
    }
    let pp_before = screen.battle_state.as_ref().unwrap().player.active_mon().pp;
    screen.execute_turn_with_move(0);

    let bs = screen.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.confused_turns_left, 2);
    assert_eq!(bs.player.active_mon().status, StatusCondition::Sleep(3));
    assert_eq!(bs.player.active_mon().pp, pp_before);
    let msgs = turn_messages(&screen);
    assert_eq!(msgs[0], "CHARIZARD began\nto nap!");
    assert!(!msgs.iter().any(|m| m.contains("fell asleep!")), "Nap must not duplicate fell-asleep line: {msgs:?}");
}

#[test]
fn full_paralysis_precedes_disobedience_without_napping_or_drawing_obedience_rng() {
    // Byte 0: paralysis roll = 10 (< 63 => fully paralyzed!).
    let mut screen = traded_screen(
        [MoveId::Splash, MoveId::Growl, MoveId::None, MoveId::None],
        MoveId::Splash,
        200, // effective speed = 200 / 4 = 50 > enemy_speed 20
        20,
        [10, 0x13, 0x31, 0x01, 0x19, 0, 0, 0, 0, 0],
    );
    screen.battle_state.as_mut().unwrap().player.active_mon_mut().status =
        StatusCondition::Paralysis;
    screen.execute_turn_with_move(0);

    let bs = screen.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.active_mon().status, StatusCondition::Paralysis);
    assert_eq!(screen.link_rng.as_ref().unwrap().consumed(), 1);
    let msgs = turn_messages(&screen);
    assert_eq!(msgs[0], "CHARIZARD is fully\nparalyzed!");
}

#[test]
fn charging_strike_continuation_skips_disobedience() {
    // Turn 2 of Fly (`CHARGING_UP` already set): core.asm:3097 branches to
    // `PlayerCanExecuteChargingMove` BEFORE `CheckForDisobedience`.
    // Stream [0x13, 0x31, 0x01, 0x19, ...] would force Nap(3) if obedience ran,
    // whereas Fly's strike only draws crit + damage + accuracy.
    let mut screen = traded_screen(
        [MoveId::Fly, MoveId::Splash, MoveId::None, MoveId::None],
        MoveId::Splash,
        200,
        50,
        [0xFF, 0xFF, 0x00, 0x13, 0x31, 0x01, 0x19, 0, 0, 0],
    );
    {
        let p = &mut screen.battle_state.as_mut().unwrap().player;
        p.set_status1(state::status1::CHARGING_UP | state::status1::INVULNERABLE);
        p.selected_move = MoveId::Fly;
        p.selected_move_index = 0;
    }
    let enemy_hp_before = screen.battle_state.as_ref().unwrap().enemy.active_mon().hp;
    screen.execute_turn_with_move(0);

    let bs = screen.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.active_mon().status, StatusCondition::None);
    assert!(bs.enemy.active_mon().hp < enemy_hp_before, "Fly strike landed");
    assert_eq!(screen.link_rng.as_ref().unwrap().consumed(), 3, "crit + damage + accuracy only");
}

#[test]
fn wont_obey_self_hit_emits_both_lines_and_damages_player() {
    // L50, 0 badges (c=10, d=50, b=60, diff=40):
    //   loop1: swap(0x13) = 49 (< 60, >= 10)
    //   loop2: 0x31 = 49 (< 60, >= 10)
    //   branch: swap(0x83) = 0x38 = 56 (>= diff=40, and 56 - 40 = 16 < 40 => WontObeySelfHit!)
    let mut screen = traded_screen(
        [MoveId::Splash, MoveId::Growl, MoveId::None, MoveId::None],
        MoveId::Splash,
        200,
        50,
        [0x13, 0x31, 0x83, 0, 0, 0, 0, 0, 0, 0],
    );
    let hp_before = screen.battle_state.as_ref().unwrap().player.active_mon().hp;
    screen.execute_turn_with_move(0);

    let bs = screen.battle_state.as_ref().unwrap();
    assert!(bs.player.active_mon().hp < hp_before);
    let msgs = turn_messages(&screen);
    assert_eq!(msgs[0], "CHARIZARD won't\nobey!");
    assert_eq!(msgs[1], "It hurt itself in\nits confusion!");
}

#[test]
fn faster_enemy_ko_skips_player_disobedience_and_leaves_fainted_status_clean() {
    // Faster enemy uses Pound to KO a 1-HP traded player mon.
    // Even with Nap bytes queued after the enemy's crit/damage/acc draws, the
    // fainted player never reaches ExecutePlayerMove / CheckForDisobedience.
    let mut screen = traded_screen(
        [MoveId::Splash, MoveId::Growl, MoveId::None, MoveId::None],
        MoveId::Pound,
        30,
        200,
        [0xFF, 0xFF, 0x00, 0x13, 0x31, 0x01, 0x19, 0, 0, 0],
    );
    screen.battle_state.as_mut().unwrap().player.active_mon_mut().hp = 1;
    screen.execute_turn_with_move(0);

    let bs = screen.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.active_mon().hp, 0);
    assert_eq!(
        bs.player.active_mon().status,
        StatusCondition::None,
        "KO'd player mon must not be put to sleep by pre-turn disobedience",
    );
    assert_eq!(
        screen.link_rng.as_ref().unwrap().consumed(),
        3,
        "only the faster enemy's crit + damage + accuracy bytes were consumed",
    );
    let msgs = turn_messages(&screen);
    assert!(!msgs.iter().any(|m| m.contains("nap") || m.contains("loafing")), "{msgs:?}");
}

#[test]
fn disobedience_random_move_spends_replacement_slot_pp_and_preserves_selected_priority() {
    // Player selects QuickAttack (slot 0, priority +1) with speed 30 vs enemy speed 200.
    // Turn order is decided BEFORE ExecutePlayerMove, so player still acts FIRST even
    // when CheckForDisobedience `.useRandomMove` switches to Splash (slot 1)!
    // L50, 0 badges (c=10, b=60):
    //   loop1: swap(0x13) = 49 (< 60, >= 10)
    //   loop2: 0x05 (< 10 => .useRandomMove!)
    //   chooseMove: 0x01 & 3 = 1 (slot 1 = Splash!)
    let mut screen = traded_screen(
        [MoveId::QuickAttack, MoveId::Splash, MoveId::None, MoveId::None],
        MoveId::Splash,
        30,
        200,
        [0x13, 0x05, 0x01, 0, 0, 0, 0, 0, 0, 0],
    );
    let pp_before = screen.battle_state.as_ref().unwrap().player.active_mon().pp;
    screen.execute_turn_with_move(0);

    let bs = screen.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.selected_move_index, 1);
    assert_eq!(bs.player.active_mon().pp[0], pp_before[0], "original slot PP untouched");
    assert_eq!(bs.player.active_mon().pp[1], pp_before[1] - 1, "replacement slot PP spent");
    let msgs = turn_messages(&screen);
    assert_eq!(msgs[0], "CHARIZARD used SPLASH!", "player acted first via QuickAttack priority, then used replacement Splash: {msgs:?}");
}
