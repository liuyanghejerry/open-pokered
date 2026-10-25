//! Twineedle re-enters PoisonEffect only after both hits and its pre-roll guards.
use pokered_core::battle::{state, BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType};

fn fixture(enemy: bool, condition: &str, chance: u8) -> BattleScreen {
    let attacker = create_pokemon_with_moves(Species::Mewtwo, 20, [0x9a, 0x78],
        [MoveId::Twineedle, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let mut target = create_pokemon_with_moves(Species::Snorlax, 100, [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    if condition == "ko" { target.hp = 1; }
    target.status = match condition {
        "burned" => state::StatusCondition::Burn,
        "poisoned" => state::StatusCondition::Poison,
        "paralyzed" => state::StatusCondition::Paralysis,
        "asleep" => state::StatusCondition::Sleep(7),
        "frozen" => state::StatusCondition::Freeze,
        _ => state::StatusCondition::None,
    };
    let (player, foe) = if enemy { (target, attacker) } else { (attacker, target) };
    let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
    battle.link_enemy_move_override = Some((if enemy { MoveId::Twineedle } else { MoveId::Splash }, 0));
    battle.link_rng = Some(LinkRng::new(if condition == "paralyzed" {
        // The faster target's paralysis gate precedes crit, variance, accuracy.
        [255, 255, 255, 0, chance, 3, 73, 64, 55, 46]
    } else {
        [255, 255, 0, chance, 3, 82, 73, 64, 55, 46]
    }));
    let state = battle.battle_state.as_mut().unwrap();
    assert!(!state.link_battle && !battle.link_mode);
    let (attacker, target) = if enemy { (&mut state.enemy, &mut state.player) } else { (&mut state.player, &mut state.enemy) };
    attacker.active_mon_mut().speed = 100;
    target.active_mon_mut().speed = 999;
    if condition == "poison_type1" { target.conversion_type1 = Some(PokemonType::Poison); }
    if condition == "poison_type2" { target.conversion_type2 = Some(PokemonType::Poison); }
    if let Some(hp) = condition.strip_prefix("sub_") {
        target.set_status2(state::status2::HAS_SUBSTITUTE_UP);
        target.substitute_hp = hp.parse().unwrap();
    }
    battle.set_presentation_enabled(true);
    battle.phase = BattlePhase::PlayerMenu;
    battle
}

fn turn(mut battle: BattleScreen, enemy: bool) -> (u64, u16, state::StatusCondition, bool, Vec<String>) {
    let mut a = BattleInput::none(); a.a = true;
    battle.update_frame(a); battle.update_frame(a);
    let state = battle.battle_state.as_ref().unwrap();
    let target = if enemy { &state.player } else { &state.enemy };
    let result = (battle.link_rng.as_ref().unwrap().consumed(), target.active_mon().hp,
        target.active_mon().status, target.has_status2(state::status2::HAS_SUBSTITUTE_UP));
    let mut messages = Vec::new(); let mut previous = None;
    for frame in 0..6000 {
        if battle.current_message != previous {
            if let Some(message) = &battle.current_message {
                messages.push(message.split_whitespace().collect::<Vec<_>>().join(" "));
            }
            previous = battle.current_message.clone();
        }
        if matches!(battle.phase, BattlePhase::PlayerMenu | BattlePhase::BattleOver { .. }) {
            assert_eq!(battle.link_rng.as_ref().unwrap().consumed(), result.0);
            return (result.0, result.1, result.2, result.3, messages);
        }
        if battle.presentation.waiting { battle.complete_move_presentation(); }
        battle.update_frame(if frame % 2 == 0 { a } else { BattleInput::none() });
    }
    panic!("presentation did not finish: {:?}", battle.phase);
}

fn verify(conditions: &[&str]) {
    use state::StatusCondition as S;
    let mut errors = Vec::new();
    for enemy in [false, true] {
        for &condition in conditions {
            for chance in [51, 52] {
                let (draws, hp, status, sub_up, messages) = turn(fixture(enemy, condition, chance), enemy);
                let poisons = condition == "normal" && chance == 51;
                let expected_status = match condition {
                    "burned" => S::Burn, "poisoned" => S::Poison,
                    "paralyzed" => S::Paralysis, "asleep" => S::Sleep(6), "frozen" => S::Freeze,
                    _ if poisons => S::Poison, _ => S::None,
                };
                let expected_draws = 3 + u64::from(condition == "normal" || condition == "paralyzed");
                let expected_hp = match condition {
                    "ko" => 0,
                    "sub_60" | "sub_1" | "sub_3" => 450,
                    "burned" | "poisoned" => 416, // Faster target's 28 residual HP + 6 damage.
                    "poison_type1" | "poison_type2" => 438, // Gen-1 Bug is super effective against Poison.
                    _ => 444,
                };
                let poison_text = messages.iter().filter(|m| m.contains("was poisoned!")).count();
                let count_text = messages.iter().filter(|m| m.ends_with("2 times!")).count();
                let break_text = messages.iter().filter(|m| m.contains("SUBSTITUTE broke!")).count();
                if (draws, hp, status, sub_up) != (expected_draws, expected_hp, expected_status, condition == "sub_60")
                    || poison_text != usize::from(poisons)
                    || count_text != usize::from(!matches!(condition, "sub_1" | "ko"))
                    || break_text != usize::from(matches!(condition, "sub_1" | "sub_3")) {
                    errors.push(format!("enemy={enemy}, {condition}, chance={chance}: draws={draws}/{expected_draws}, hp={hp}/{expected_hp}, status={status:?}/{expected_status:?}, sub={sub_up}, messages={messages:?}"));
                }
            }
        }
    }
    assert!(errors.is_empty(), "{} wrong cases:\n{}", errors.len(), errors.join("\n"));
}

#[test]
fn eligible_target_keeps_the_original_52_of_256_poison_boundary() {
    verify(&["normal"]);
}

#[test]
fn status_poison_types_and_intact_substitute_skip_the_chance_draw() {
    verify(&["burned", "poisoned", "paralyzed", "asleep", "frozen", "poison_type1", "poison_type2", "sub_60"]);
}

#[test]
fn first_and_final_substitute_breaks_and_ko_cancel_poison() {
    verify(&["sub_1", "sub_3", "ko"]);
}
