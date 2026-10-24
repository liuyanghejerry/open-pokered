//! Status gate narration must describe the action when it was blocked, before
//! a later cure or new sleep changes the final battle state.

use pokered_core::battle::{state::StatusCondition as S, BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

fn fixture(status: S, reply: MoveId, enemy: bool, actor_first: bool) -> BattleScreen {
    let mon = |species, level, mv| {
        create_pokemon_with_moves(
            species,
            level,
            [0x9a, 0x78],
            [mv, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    let mut actor = mon(Species::Vaporeon, 100, MoveId::Splash);
    actor.status = status;
    let other = mon(Species::Mewtwo, 20, reply);
    let (player, foe) = if enemy {
        (other, actor)
    } else {
        (actor, other)
    };
    let mut b = BattleScreen::from_parties(true, &[player], &[foe], None);
    b.link_enemy_move_override = Some((if enemy { MoveId::Splash } else { reply }, 0));
    b.link_rng = Some(LinkRng::new(if reply == MoveId::Hypnosis {
        [0, 3, 255, 0, 255, 255, 0, 255, 255, 0]
    } else if status == S::Paralysis {
        [0, 255, 0, 255, 255, 0, 255, 255, 0, 255]
    } else {
        [255, 0, 255, 255, 0, 255, 255, 0, 255, 255]
    }));
    let state = b.battle_state.as_mut().unwrap();
    assert!(!state.link_battle && !b.link_mode);
    let (actor, other) = if enemy {
        (&mut state.enemy, &mut state.player)
    } else {
        (&mut state.player, &mut state.enemy)
    };
    actor.active_mon_mut().speed = if actor_first { 200 } else { 10 };
    other.active_mon_mut().speed = if actor_first { 10 } else { 200 };
    b.set_presentation_enabled(true);
    b.phase = BattlePhase::PlayerMenu;
    b
}

fn name(enemy: bool) -> &'static str {
    if enemy {
        "Enemy VAPOREON"
    } else {
        "VAPOREON"
    }
}

fn status(b: &BattleScreen, enemy: bool) -> S {
    let state = b.battle_state.as_ref().unwrap();
    if enemy {
        state.enemy.active_mon().status
    } else {
        state.player.active_mon().status
    }
}

fn turn(b: &mut BattleScreen) -> (Vec<String>, u64) {
    let initial_draws = b.link_rng.as_ref().unwrap().consumed();
    let mut a = BattleInput::none();
    a.a = true;
    b.update_frame(a);
    b.update_frame(a);
    let mut messages = Vec::new();
    let mut previous = None;
    for frame in 0..4000 {
        if b.current_message != previous {
            if let Some(text) = &b.current_message {
                messages.push(text.split_whitespace().collect::<Vec<_>>().join(" "));
            }
            previous = b.current_message.clone();
        }
        if b.phase == BattlePhase::PlayerMenu {
            return (
                messages,
                b.link_rng.as_ref().unwrap().consumed() - initial_draws,
            );
        }
        if b.presentation.waiting {
            b.complete_move_presentation();
        }
        b.update_frame(if frame % 2 == 0 {
            a
        } else {
            BattleInput::none()
        });
    }
    panic!("unfinished status turn: {:?}", b.phase);
}

fn reason(status: S) -> &'static str {
    match status {
        S::Sleep(1) => "woke up!",
        S::Sleep(_) => "is fast asleep!",
        S::Freeze => "is frozen solid!",
        S::Paralysis => "is fully paralyzed!",
        _ => unreachable!(),
    }
}

fn actor_lines(messages: &[String], enemy: bool) -> Vec<String> {
    messages
        .iter()
        .filter(|m| m.starts_with(&format!("{} ", name(enemy))))
        .cloned()
        .collect()
}

#[test]
fn gate_reason_survives_later_haze_and_each_fire_defrost() {
    let mut errors = Vec::new();
    let mut cases = 0;
    for enemy in [false, true] {
        for (initial, reply) in [
            (S::Sleep(2), MoveId::Splash),
            (S::Sleep(2), MoveId::Haze),
            (S::Sleep(1), MoveId::Splash),
            (S::Sleep(1), MoveId::Haze),
            (S::Freeze, MoveId::Splash),
            (S::Freeze, MoveId::Haze),
            (S::Freeze, MoveId::Ember),
            (S::Freeze, MoveId::FirePunch),
            (S::Freeze, MoveId::Flamethrower),
            (S::Freeze, MoveId::FireBlast),
            (S::Paralysis, MoveId::Splash),
            (S::Paralysis, MoveId::Haze),
        ] {
            let mut b = fixture(initial, reply, enemy, true);
            let (messages, draws) = turn(&mut b);
            let fire = !matches!(reply, MoveId::Splash | MoveId::Haze);
            let final_status = if reply != MoveId::Splash {
                S::None
            } else {
                match initial {
                    S::Sleep(2) => S::Sleep(1),
                    S::Sleep(1) => S::None,
                    s => s,
                }
            };
            let expected_draws = if fire {
                3
            } else {
                u64::from(initial == S::Paralysis)
            };
            if actor_lines(&messages, enemy) != [format!("{} {}", name(enemy), reason(initial))]
                || draws != expected_draws
                || status(&b, enemy) != final_status
                || messages
                    .iter()
                    .filter(|m| m.starts_with("Fire defrosted "))
                    .count()
                    != usize::from(fire)
            {
                errors.push(format!("enemy={enemy} {initial:?} {reply:?}: status={:?}, draws={draws}, messages={messages:?}", status(&b, enemy)));
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 24);
    assert!(
        errors.is_empty(),
        "{} wrong cases:\n{}",
        errors.len(),
        errors.join("\n")
    );
}

#[test]
fn an_earlier_fire_thaw_allows_the_actual_move_without_a_frozen_caption() {
    for enemy in [false, true] {
        for reply in [
            MoveId::Ember,
            MoveId::FirePunch,
            MoveId::Flamethrower,
            MoveId::FireBlast,
        ] {
            let mut b = fixture(S::Freeze, reply, enemy, false);
            let (messages, draws) = turn(&mut b);
            assert_eq!(draws, 3);
            assert_eq!(status(&b, enemy), S::None);
            assert_eq!(
                actor_lines(&messages, enemy),
                [format!("{} used SPLASH!", name(enemy))]
            );
            assert_eq!(
                messages
                    .iter()
                    .filter(|m| m.starts_with("Fire defrosted "))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn sleep_countdown_wake_and_next_turn_have_one_correct_action_caption() {
    for enemy in [false, true] {
        let mut b = fixture(S::Sleep(2), MoveId::Splash, enemy, true);
        for (expected, next_status) in [
            ("is fast asleep!", S::Sleep(1)),
            ("woke up!", S::None),
            ("used SPLASH!", S::None),
        ] {
            let (messages, draws) = turn(&mut b);
            assert_eq!(draws, 0);
            assert_eq!(status(&b, enemy), next_status);
            assert_eq!(
                actor_lines(&messages, enemy),
                [format!("{} {expected}", name(enemy))]
            );
        }
    }
}

#[test]
fn a_later_haze_does_not_leave_a_block_receipt_on_the_next_turn() {
    for enemy in [false, true] {
        for initial in [S::Sleep(2), S::Freeze, S::Paralysis] {
            let mut b = fixture(initial, MoveId::Haze, enemy, true);
            let (first, _) = turn(&mut b);
            assert_eq!(
                actor_lines(&first, enemy),
                [format!("{} {}", name(enemy), reason(initial))]
            );
            assert_eq!(status(&b, enemy), S::None);
            let state = b.battle_state.as_mut().unwrap();
            let other = if enemy {
                &mut state.player
            } else {
                &mut state.enemy
            };
            other.active_mon_mut().moves[0] = MoveId::Splash;
            other.active_mon_mut().pp[0] = 40;
            b.link_enemy_move_override = Some((MoveId::Splash, 0));
            let (next, draws) = turn(&mut b);
            assert_eq!(draws, 0);
            assert_eq!(
                actor_lines(&next, enemy),
                [format!("{} used SPLASH!", name(enemy))]
            );
            assert_eq!(
                next.iter().filter(|m| m.as_str() == "No effect!").count(),
                2
            );
        }
    }
}

#[test]
fn genuine_new_sleep_before_a_gate_or_after_waking_is_still_narrated_once() {
    for enemy in [false, true] {
        for wakes_first in [false, true] {
            let mut b = fixture(
                if wakes_first { S::Sleep(1) } else { S::None },
                MoveId::Hypnosis,
                enemy,
                wakes_first,
            );
            let (messages, draws) = turn(&mut b);
            assert_eq!(draws, 2, "{messages:?}");
            assert_eq!(status(&b, enemy), S::Sleep(if wakes_first { 3 } else { 2 }));
            let expected = if wakes_first {
                ["woke up!", "fell asleep!"]
            } else {
                ["fell asleep!", "is fast asleep!"]
            };
            assert_eq!(
                actor_lines(&messages, enemy),
                expected.map(|line| format!("{} {line}", name(enemy)))
            );
        }
    }
}
