//! Original StatModifierDownEffect/MoveMissed/CantLowerAnymore captions through
//! public FIGHT input, including action ordering and silent secondary failures.

use pokered_core::battle::{state, BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

const PRIMARY_MOVES: [MoveId; 9] = [
    MoveId::Growl,
    MoveId::TailWhip,
    MoveId::Leer,
    MoveId::StringShot,
    MoveId::SandAttack,
    MoveId::Smokescreen,
    MoveId::Kinesis,
    MoveId::Flash,
    MoveId::Screech,
];

fn prepared(move_id: MoveId, enemy: bool, condition: &str, reply: MoveId) -> BattleScreen {
    let mon = |species, level, mv| {
        create_pokemon_with_moves(
            species,
            level,
            [0x9a, 0x78],
            [mv, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    let attacker = mon(Species::Mewtwo, 20, move_id);
    let target = mon(
        if condition == "ghost_miss" {
            Species::Gastly
        } else {
            Species::Snorlax
        },
        100,
        reply,
    );
    let (player, foe) = if enemy {
        (target, attacker)
    } else {
        (attacker, target)
    };
    let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
    battle.link_enemy_move_override = Some((if enemy { move_id } else { reply }, 0));
    let mut bytes = [0, 0, 82, 73, 64, 55, 46, 37, 28, 19];
    if enemy && condition != "npc_pre_roll" {
        bytes[0] = 64;
    }
    if matches!(condition, "accuracy_miss" | "ghost_miss") {
        bytes[usize::from(enemy)] = 255;
    }
    battle.link_rng = Some(LinkRng::new(bytes));
    let state = battle.battle_state.as_mut().unwrap();
    assert!(!state.link_battle && !battle.link_mode);
    let (attacker, target) = if enemy {
        (&mut state.enemy, &mut state.player)
    } else {
        (&mut state.player, &mut state.enemy)
    };
    attacker.active_mon_mut().speed = 100;
    target.active_mon_mut().speed = 200;
    match condition {
        "mist" => target.set_status2(state::status2::PROTECTED_BY_MIST),
        "sub" => {
            target.set_status2(state::status2::HAS_SUBSTITUTE_UP);
            target.substitute_hp = 255;
        }
        "invulnerable" => target.set_status1(state::status1::INVULNERABLE),
        "min_stage" => {
            target.stat_stages.attack = -6;
            target.stat_stages.defense = -6;
            target.stat_stages.speed = -6;
            target.stat_stages.special = -6;
            target.stat_stages.accuracy = -6;
            target.stat_stages.evasion = -6;
        }
        _ => {}
    }
    battle.set_presentation_enabled(true);
    battle.phase = BattlePhase::PlayerMenu;
    battle
}

fn fight(battle: &mut BattleScreen) {
    assert!(matches!(battle.phase, BattlePhase::PlayerMenu));
    let mut a = BattleInput::none();
    a.a = true;
    battle.update_frame(a);
    battle.update_frame(a);
}

fn messages(battle: &mut BattleScreen) -> Vec<String> {
    let mut result = Vec::new();
    let mut previous = None;
    let mut a = BattleInput::none();
    a.a = true;
    for frame in 0..2500 {
        if battle.current_message != previous {
            if let Some(message) = &battle.current_message {
                result.push(message.split_whitespace().collect::<Vec<_>>().join(" "));
            }
            previous = battle.current_message.clone();
        }
        if matches!(battle.phase, BattlePhase::PlayerMenu) {
            return result;
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
    panic!("turn did not finish: {:?}", battle.phase);
}

fn stage(battle: &BattleScreen, enemy: bool, move_id: MoveId) -> i8 {
    let state = battle.battle_state.as_ref().unwrap();
    let target = if enemy { &state.player } else { &state.enemy };
    match move_id {
        MoveId::Growl => target.stat_stages.attack,
        MoveId::TailWhip | MoveId::Leer | MoveId::Screech => target.stat_stages.defense,
        MoveId::StringShot => target.stat_stages.speed,
        _ => target.stat_stages.accuracy,
    }
}

#[test]
fn nine_primary_moves_narrate_failure_and_floor_without_changing_outcomes_or_rng() {
    let mut errors = Vec::new();
    for move_id in PRIMARY_MOVES {
        for enemy in [false, true] {
            for condition in [
                "success",
                "accuracy_miss",
                "mist",
                "sub",
                "min_stage",
                "invulnerable",
                "npc_pre_roll",
                "ghost_miss",
            ] {
                let mut battle = prepared(move_id, enemy, condition, MoveId::Splash);
                fight(&mut battle);
                let failed = !matches!(condition, "success" | "min_stage")
                    && (condition != "npc_pre_roll" || enemy);
                let expected_stage = if condition == "min_stage" {
                    -6
                } else if failed {
                    0
                } else if move_id == MoveId::Screech {
                    -2
                } else {
                    -1
                };
                assert_eq!(stage(&battle, enemy, move_id), expected_stage);
                let draws = if matches!(condition, "mist" | "sub" | "invulnerable") {
                    u64::from(enemy)
                } else if condition == "npc_pre_roll" {
                    1
                } else {
                    1 + u64::from(enemy)
                };
                assert_eq!(battle.link_rng.as_ref().unwrap().consumed(), draws);
                let text = messages(&mut battle);
                let failure_count = text
                    .iter()
                    .filter(|line| *line == "But, it failed!")
                    .count();
                let floor_count = text
                    .iter()
                    .filter(|line| *line == "Nothing happened!")
                    .count();
                if failure_count != usize::from(failed)
                    || floor_count != usize::from(condition == "min_stage")
                    || text.iter().any(|line| {
                        line.contains("attack missed") || line.contains("doesn't affect")
                    })
                {
                    errors.push(format!("{move_id:?}, enemy={enemy}, {condition}: {text:?}"));
                }
            }
        }
    }
    assert!(
        errors.is_empty(),
        "{} wrong outcomes:\n{}",
        errors.len(),
        errors.join("\n")
    );
}

#[test]
fn floor_receipt_survives_later_haze_and_clears_before_next_turn() {
    for enemy in [false, true] {
        let mut battle = prepared(MoveId::Growl, enemy, "min_stage", MoveId::Haze);
        let state = battle.battle_state.as_mut().unwrap();
        // The attempted drop runs first; Haze then erases the floor state.
        state.player.active_mon_mut().speed = if enemy { 100 } else { 200 };
        state.enemy.active_mon_mut().speed = if enemy { 200 } else { 100 };
        fight(&mut battle);
        assert_eq!(stage(&battle, enemy, MoveId::Growl), 0);
        let first = messages(&mut battle);
        let floor = first.iter().position(|line| line == "Nothing happened!");
        let haze = first.iter().position(|line| line.contains("used HAZE!"));
        assert!(floor.is_some() && floor < haze, "{first:?}");
        battle.link_rng = Some(LinkRng::new([64, 0, 82, 73, 64, 55, 46, 37, 28, 19]));
        fight(&mut battle);
        let second = messages(&mut battle);
        assert!(
            second.iter().any(|line| line.contains("ATTACK fell!")),
            "{second:?}"
        );
        assert!(
            !second.iter().any(|line| line == "Nothing happened!"),
            "{second:?}"
        );
    }
}

#[test]
fn secondary_drop_failures_stay_silent_and_damaging_misses_keep_their_text() {
    for enemy in [false, true] {
        for move_id in [
            MoveId::Acid,
            MoveId::AuroraBeam,
            MoveId::Bubble,
            MoveId::Bubblebeam,
            MoveId::Constrict,
            MoveId::PsychicM,
        ] {
            for condition in ["min_stage", "sub"] {
                let mut battle = prepared(move_id, enemy, condition, MoveId::Splash);
                battle.link_rng = Some(LinkRng::new([255, 0, 255, 64, 0, 255, 0, 255, 0, 255]));
                fight(&mut battle);
                let text = messages(&mut battle);
                assert!(
                    !text.iter().any(|line| matches!(
                        line.as_str(),
                        "But, it failed!" | "Nothing happened!"
                    )),
                    "{text:?}"
                );
            }
        }
        let mut battle = prepared(MoveId::Tackle, enemy, "success", MoveId::Splash);
        battle.link_rng = Some(LinkRng::new([255, 255, 0, 0, 0, 0, 0, 0, 0, 0]));
        fight(&mut battle);
        let text = messages(&mut battle);
        assert!(
            text.iter().any(|line| line.contains("attack missed!")),
            "{text:?}"
        );
        assert!(
            !text.iter().any(|line| line == "But, it failed!"),
            "{text:?}"
        );
    }
}
