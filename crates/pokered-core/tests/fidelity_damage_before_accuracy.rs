//! Observed original calculation slices run variance before MoveHitTest.
use pokered_core::battle::state::{status1, status2, StatusCondition};
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType};

fn observe(enemy: bool, case: &str) -> (u16, u64, bool) {
    let id = if case.starts_with("swift") {
        MoveId::Swift
    } else if case.starts_with("dream_") {
        MoveId::DreamEater
    } else if case.starts_with("fly_") {
        MoveId::Fly
    } else if case.starts_with("dig_") {
        MoveId::Dig
    } else {
        MoveId::Tackle
    };
    let attacker = create_pokemon_with_moves(
        Species::Mewtwo,
        100,
        [0x9a, 0x78],
        [id, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let mut defender = create_pokemon_with_moves(
        Species::Snorlax,
        100,
        [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    defender.hp = 999;
    defender.max_hp = 999;
    defender.status = if case == "dream_awake" {
        StatusCondition::None
    } else {
        StatusCondition::Sleep(7)
    };
    let (player, foe) = if enemy {
        (defender, attacker)
    } else {
        (attacker, defender)
    };
    let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
    battle.link_enemy_move_override = Some((if enemy { id } else { MoveId::Splash }, 0));
    let seeds = match case {
        "variance_then_miss" | "fly_miss" | "dig_miss" => {
            [255, 179, 255, 0, 255, 255, 255, 255, 255, 255]
        }
        "maximum_then_hit" => [255, 255, 0, 255, 255, 255, 255, 255, 255, 255],
        _ => [255, 179, 0, 255, 255, 255, 255, 255, 255, 255],
    };
    battle.link_rng = Some(LinkRng::new(seeds));
    let state = battle.battle_state.as_mut().unwrap();
    state.damage = 73; // A hit-test miss must clear the previous action's damage.
    let (actor, target) = if enemy {
        (&mut state.enemy, &mut state.player)
    } else {
        (&mut state.player, &mut state.enemy)
    };
    if case.starts_with("fly_") || case.starts_with("dig_") {
        actor.selected_move = id;
        actor.battle_status1 |= status1::CHARGING_UP | status1::INVULNERABLE;
    }
    actor.active_mon_mut().attack = 203;
    actor.active_mon_mut().special = 100;
    target.active_mon_mut().special = 100;
    actor.active_mon_mut().speed = 100;
    target.active_mon_mut().defense = 30;
    target.active_mon_mut().speed = 200;
    if case.contains("immune") {
        target.conversion_type1 = Some(PokemonType::Ghost);
        target.conversion_type2 = Some(PokemonType::Ghost);
    }
    if case.contains("invulnerable") {
        target.battle_status1 |= status1::INVULNERABLE;
    }
    if case.contains("x_accuracy") {
        actor.battle_status2 |= status2::USING_X_ACCURACY;
    }
    battle.set_presentation_enabled(true);
    battle.phase = BattlePhase::PlayerMenu;
    let mut input = BattleInput::none();
    input.a = true;
    for _ in 0..4 {
        battle.update_frame(input);
        if matches!(battle.phase, BattlePhase::ShowingText { .. }) {
            break;
        }
    }
    let BattlePhase::ShowingText { messages, .. } = &battle.phase else {
        panic!("FIGHT did not resolve");
    };
    assert!(messages.iter().all(|m| !m.contains("Critical hit!")));
    let missed = messages.iter().any(|m| {
        m.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .contains("attack missed!")
    });
    let state = battle.battle_state.as_ref().unwrap();
    let actor = if enemy { &state.enemy } else { &state.player };
    if case.starts_with("fly_") || case.starts_with("dig_") {
        assert_eq!(
            actor.battle_status1 & (status1::CHARGING_UP | status1::INVULNERABLE),
            0
        );
    }
    if missed {
        assert_eq!(state.damage, 0, "miss clears shared damage");
    }
    let target = if enemy { &state.player } else { &state.enemy };
    let result = (
        999 - target.active_mon().hp,
        battle.link_rng.as_ref().unwrap().consumed(),
        missed,
    );
    for frame in 0..6000 {
        if battle.presentation.waiting {
            battle.complete_move_presentation();
        }
        if matches!(battle.phase, BattlePhase::PlayerMenu) {
            assert_eq!(battle.link_rng.as_ref().unwrap().consumed(), result.1);
            return result;
        }
        let mut input = BattleInput::none();
        input.a = frame % 2 == 0;
        battle.update_frame(input);
    }
    panic!("presentation did not finish: {:?}", battle.phase);
}

#[test]
fn ordinary_attacks_use_variance_before_accuracy() {
    for enemy in [false, true] {
        for (case, expected) in [
            ("variance_then_miss", (0, 3, true)),
            ("maximum_then_hit", (200, 3, false)),
            ("minimum_then_hit", (170, 3, false)),
        ] {
            assert_eq!(observe(enemy, case), expected, "enemy={enemy} case={case}");
        }
    }
}

#[test]
fn invulnerability_vetoes_accuracy_after_variance_has_been_drawn() {
    for enemy in [false, true] {
        for case in ["invulnerable_target", "x_accuracy_invulnerable"] {
            assert_eq!(
                observe(enemy, case),
                (0, 2, true),
                "enemy={enemy} case={case}"
            );
        }
    }
}

#[test]
fn immunity_and_accuracy_exemptions_keep_their_original_results() {
    for enemy in [false, true] {
        for (case, expected) in [
            ("immune_control", (0, 2, false)),
            ("x_accuracy", (170, 2, false)),
            ("swift", (291, 2, false)),
            ("swift_invulnerable", (291, 2, false)),
            ("immune_x_accuracy", (0, 1, false)),
        ] {
            assert_eq!(observe(enemy, case), expected, "enemy={enemy} case={case}");
        }
    }
}

#[test]
fn charge_strikes_use_variance_before_accuracy_and_end_invulnerability() {
    for enemy in [false, true] {
        for (case, expected) in [
            ("fly_hit", (339, 3, false)),
            ("fly_miss", (0, 3, true)),
            ("dig_hit", (485, 3, false)),
            ("dig_miss", (0, 3, true)),
        ] {
            assert_eq!(observe(enemy, case), expected, "enemy={enemy} case={case}");
        }
    }
}

#[test]
fn dream_eater_checks_sleep_after_variance_but_before_accuracy() {
    for enemy in [false, true] {
        for (case, expected) in [
            ("dream_awake", (0, 2, true)),
            ("dream_asleep", (109, 3, false)),
        ] {
            assert_eq!(observe(enemy, case), expected, "enemy={enemy} case={case}");
        }
    }
}
