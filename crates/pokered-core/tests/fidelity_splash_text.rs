//! SplashEffect plays the move and then PrintNoEffectText. A blocked action
//! never enters that handler. Exercise both through public FIGHT input.

use pokered_core::battle::{state, BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

fn fixture(enemy: bool, condition: &str, both_splash: bool) -> BattleScreen {
    let mon = |species, mv| {
        create_pokemon_with_moves(
            species,
            40,
            [0x9a, 0x78],
            [mv, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    let mut actor = mon(Species::Magikarp, MoveId::Splash);
    actor.status = match condition {
        "sleep" => state::StatusCondition::Sleep(2),
        "freeze" => state::StatusCondition::Freeze,
        "paralyzed" | "paralysis_pass" => state::StatusCondition::Paralysis,
        _ => state::StatusCondition::None,
    };
    let reply = if both_splash {
        MoveId::Splash
    } else {
        MoveId::Growth
    };
    let target = mon(Species::Tangela, reply);
    let (player, foe) = if enemy {
        (target, actor)
    } else {
        (actor, target)
    };
    let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
    battle.link_enemy_move_override = Some((if enemy { MoveId::Splash } else { reply }, 0));
    battle.link_rng = Some(LinkRng::new(
        [if condition == "paralyzed" { 0 } else { 255 }; 10],
    ));
    let state = battle.battle_state.as_mut().unwrap();
    assert!(!state.link_battle && !battle.link_mode);
    let (actor, target) = if enemy {
        (&mut state.enemy, &mut state.player)
    } else {
        (&mut state.player, &mut state.enemy)
    };
    actor.active_mon_mut().speed = 200;
    target.active_mon_mut().speed = 10;
    if condition == "substitute" {
        target.set_status2(state::status2::HAS_SUBSTITUTE_UP);
        target.substitute_hp = 40;
    }
    battle.set_presentation_enabled(true);
    battle.phase = BattlePhase::PlayerMenu;
    battle
}

fn turn(battle: &mut BattleScreen) -> (Vec<String>, usize, u64) {
    let before = battle.battle_state.as_ref().unwrap();
    let hp = (before.player.active_mon().hp, before.enemy.active_mon().hp);
    let mut a = BattleInput::none();
    a.a = true;
    battle.update_frame(a);
    battle.update_frame(a);
    let mut messages = Vec::new();
    let mut previous = None;
    let mut splash_serials = Vec::new();
    for frame in 0..3000 {
        if battle.current_message != previous {
            if let Some(text) = &battle.current_message {
                messages.push(text.split_whitespace().collect::<Vec<_>>().join(" "));
            }
            previous = battle.current_message.clone();
        }
        if let Some((serial, MoveId::Splash, _)) = battle.presentation.current_move() {
            if !splash_serials.contains(&serial) {
                splash_serials.push(serial);
            }
        }
        if matches!(battle.phase, BattlePhase::PlayerMenu) {
            let after = battle.battle_state.as_ref().unwrap();
            assert_eq!(
                hp,
                (after.player.active_mon().hp, after.enemy.active_mon().hp)
            );
            return (
                messages,
                splash_serials.len(),
                battle.link_rng.as_ref().unwrap().consumed(),
            );
        }
        if battle.presentation.waiting {
            battle.complete_move_presentation();
        }
        battle.update_frame(if frame % 2 == 0 {
            a
        } else {
            BattleInput::none()
        });
    }
    panic!("Splash turn did not finish: {:?}", battle.phase);
}

#[test]
fn splash_result_follows_each_executed_move_but_never_a_blocked_action() {
    let mut errors = Vec::new();
    for enemy in [false, true] {
        for condition in [
            "normal",
            "substitute",
            "sleep",
            "freeze",
            "paralyzed",
            "paralysis_pass",
        ] {
            let mut battle = fixture(enemy, condition, false);
            let (messages, animations, draws) = turn(&mut battle);
            let blocked = match condition {
                "sleep" => Some("is fast asleep!"),
                "freeze" => Some("is frozen solid!"),
                "paralyzed" => Some("is fully paralyzed!"),
                _ => None,
            };
            let actor = if enemy { "Enemy MAGIKARP" } else { "MAGIKARP" };
            let announcement = format!("{actor} used SPLASH!");
            let count = messages
                .iter()
                .filter(|m| m.as_str() == "No effect!")
                .count();
            let expected_draws = u64::from(condition.starts_with("paraly"));
            let correct_text = if let Some(reason) = blocked {
                count == 0
                    && !messages.contains(&announcement)
                    && messages.contains(&format!("{actor} {reason}"))
            } else {
                count == 1
                    && messages
                        .windows(2)
                        .any(|m| m[0] == announcement && m[1] == "No effect!")
            };
            if !correct_text
                || animations != usize::from(blocked.is_none())
                || draws != expected_draws
            {
                errors.push(format!("enemy={enemy} {condition}: animations={animations}, draws={draws}, messages={messages:?}"));
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn both_sides_splash_then_repeat_without_losing_or_duplicating_result_pages() {
    let mut battle = fixture(false, "normal", true);
    for _ in 0..2 {
        let (messages, animations, draws) = turn(&mut battle);
        assert_eq!(draws, 0, "Splash skips accuracy and secondary RNG");
        assert_eq!(animations, 2);
        assert_eq!(
            messages,
            [
                "MAGIKARP used SPLASH!",
                "No effect!",
                "Enemy TANGELA used SPLASH!",
                "No effect!",
            ]
        );
    }
}
