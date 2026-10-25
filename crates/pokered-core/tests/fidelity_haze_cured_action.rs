//! Haze silently forfeits only the action whose sleep/freeze it cured first.
use pokered_core::battle::{state::StatusCondition, BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

fn fixture(enemy: bool, first: bool, status: StatusCondition, presentation: bool) -> BattleScreen {
    let make = |species, mv| {
        create_pokemon_with_moves(
            species,
            100,
            [0x9a, 0x78],
            [mv, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    let mut target = make(Species::Chansey, MoveId::Splash);
    target.set_nickname("WARD");
    target.status = status;
    let mut user = make(Species::Snorlax, MoveId::Haze);
    user.set_nickname("TARGET");
    let (player, foe) = if enemy {
        (user, target)
    } else {
        (target, user)
    };
    let mut b = BattleScreen::from_parties(true, &[player], &[foe], None);
    b.link_rng = Some(LinkRng::new([255, 255, 0, 255, 3, 82, 73, 64, 55, 46]));
    b.link_enemy_move_override = Some((foe.moves[0], 0));
    let state = b.battle_state.as_mut().unwrap();
    let (target, user) = if enemy {
        (&mut state.enemy, &mut state.player)
    } else {
        (&mut state.player, &mut state.enemy)
    };
    target.active_mon_mut().speed = if first { 800 } else { 100 };
    user.active_mon_mut().speed = if first { 100 } else { 200 };
    b.set_presentation_enabled(presentation);
    b.phase = BattlePhase::PlayerMenu;
    b
}

fn target(b: &BattleScreen, enemy: bool) -> &pokered_core::battle::state::BattlerState {
    let state = b.battle_state.as_ref().unwrap();
    if enemy {
        &state.enemy
    } else {
        &state.player
    }
}

fn turn(b: &mut BattleScreen, enemy: bool) -> (String, Vec<bool>) {
    let a = BattleInput {
        a: true,
        ..BattleInput::none()
    };
    b.update_frame(a);
    b.update_frame(a);
    let mut lines = Vec::new();
    let mut previous = None;
    let mut serial = None;
    let mut animations = Vec::new();
    for frame in 0..5000 {
        if b.current_message != previous {
            if let Some(message) = &b.current_message {
                lines.push(message.split_whitespace().collect::<Vec<_>>().join(" "));
            }
            previous = b.current_message.clone();
        }
        if let Some((id, MoveId::Splash, player)) = b.presentation.current_move() {
            if serial != Some(id) && player != enemy {
                animations.push(b.presentation.current_move_missed());
                serial = Some(id);
            }
        }
        if b.phase == BattlePhase::PlayerMenu {
            return (lines.join(" "), animations);
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
    panic!("turn did not drain: {:?}", b.phase);
}

#[test]
fn faster_haze_silently_forfeits_the_cured_action_and_next_turn_recovers() {
    let mut errors = Vec::new();
    for presentation in [false, true] {
        for enemy in [false, true] {
            for status in [StatusCondition::Sleep(2), StatusCondition::Freeze] {
                let mut b = fixture(enemy, false, status, presentation);
                let old = *target(&b, enemy).active_mon();
                let (text, animations) = turn(&mut b, enemy);
                let mon = target(&b, enemy).active_mon();
                assert_eq!((mon.hp, mon.pp[0]), (old.hp, old.pp[0]));
                assert_eq!(mon.status, StatusCondition::None);
                assert_eq!(b.link_rng.as_ref().unwrap().consumed(), 0);
                assert!(animations.is_empty());
                let user = if enemy { "TARGET" } else { "Enemy TARGET" };
                let expected = format!("{user} used HAZE! All STATUS changes are eliminated!");
                if text != expected {
                    errors.push(format!(
                        "presentation={presentation} enemy={enemy} status={status:?}: {text}"
                    ));
                }
                let state = b.battle_state.as_mut().unwrap();
                state.player.active_mon_mut().moves[0] = MoveId::Splash;
                state.enemy.active_mon_mut().moves[0] = MoveId::Splash;
                b.link_enemy_move_override = Some((MoveId::Splash, 0));
                let (next, animations) = turn(&mut b, enemy);
                let name = if enemy { "Enemy WARD" } else { "WARD" };
                assert!(next.contains(&format!("{name} used SPLASH!")), "{next}");
                assert!(!next.contains("can't move") && !next.contains("used HAZE"));
                assert_eq!(animations, if presentation { vec![false] } else { vec![] });
                assert_eq!(
                    target(&b, enemy).active_mon().pp[0],
                    old.pp[0] - u8::from(!enemy)
                );
                assert_eq!(b.link_rng.as_ref().unwrap().consumed(), 0);
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn slower_haze_keeps_the_already_blocked_sleep_freeze_and_wakeup_messages() {
    for presentation in [false, true] {
        for enemy in [false, true] {
            for (status, reason) in [
                (StatusCondition::Sleep(2), "is fast asleep!"),
                (StatusCondition::Freeze, "is frozen solid!"),
                (StatusCondition::Sleep(1), "woke up!"),
            ] {
                let mut b = fixture(enemy, true, status, presentation);
                let old = *target(&b, enemy).active_mon();
                let (text, animations) = turn(&mut b, enemy);
                let name = if enemy { "Enemy WARD" } else { "WARD" };
                let blocked = format!("{name} {reason}");
                assert_eq!(text.matches(&blocked).count(), 1, "{text}");
                assert!(text.find(&blocked).unwrap() < text.find("used HAZE!").unwrap());
                assert!(!text.contains("can't move!"));
                assert!(animations.is_empty());
                let mon = target(&b, enemy).active_mon();
                assert_eq!((mon.hp, mon.pp[0]), (old.hp, old.pp[0]));
                assert_eq!(mon.status, StatusCondition::None);
                assert_eq!(b.link_rng.as_ref().unwrap().consumed(), 0);
            }
        }
    }
}

#[test]
fn other_haze_cures_do_not_forfeit_the_targets_move() {
    for presentation in [false, true] {
        for enemy in [false, true] {
            for status in [
                StatusCondition::None,
                StatusCondition::Poison,
                StatusCondition::Burn,
                StatusCondition::Paralysis,
            ] {
                let mut b = fixture(enemy, false, status, presentation);
                let old = *target(&b, enemy).active_mon();
                let (text, animations) = turn(&mut b, enemy);
                let name = if enemy { "Enemy WARD" } else { "WARD" };
                let used = format!("{name} used SPLASH!");
                assert_eq!(text.matches(&used).count(), 1, "{text}");
                assert!(text.find("used HAZE!").unwrap() < text.find(&used).unwrap());
                assert!(!text.contains("can't move!"));
                assert_eq!(animations, if presentation { vec![false] } else { vec![] });
                let mon = target(&b, enemy).active_mon();
                assert_eq!(mon.hp, old.hp);
                assert_eq!(mon.pp[0], old.pp[0] - u8::from(!enemy));
                assert_eq!(mon.status, StatusCondition::None);
                assert_eq!(b.link_rng.as_ref().unwrap().consumed(), 0);
            }
        }
    }
}
