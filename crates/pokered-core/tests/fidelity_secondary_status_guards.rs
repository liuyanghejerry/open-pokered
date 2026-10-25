//! Original PoisonEffect / FreezeBurnParalyzeEffect pre-roll guards and
//! CheckDefrost dispatch, exercised through ordinary public FIGHT input.

use pokered_core::battle::{state, BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{
    move_data::MoveData,
    moves::{MoveEffect, MoveId},
    species::Species,
    types::PokemonType,
};

fn same_type_species(move_type: PokemonType) -> Species {
    match move_type {
        PokemonType::Fire => Species::Charizard,
        PokemonType::Ice => Species::Lapras,
        PokemonType::Electric => Species::Pikachu,
        PokemonType::Normal => Species::Snorlax,
        PokemonType::Ghost => Species::Gengar,
        PokemonType::Poison => Species::Weezing,
        _ => unreachable!(),
    }
}

fn fixture(move_id: MoveId, enemy: bool, condition: &str, chance: u8) -> BattleScreen {
    let md = MoveData::get(move_id).unwrap();
    let mon = |species, level, mv| {
        create_pokemon_with_moves(
            species,
            level,
            [0x9a, 0x78],
            [mv, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    // Keep typed damage above 1 so these draw counts isolate the secondary
    // guard. Damage 0/1 bypasses variance and has its own exhaustive fixtures.
    let attacker = mon(Species::Mewtwo, 40, move_id);
    let mut target = mon(
        if condition == "same_type" {
            same_type_species(md.move_type)
        } else {
            Species::Vaporeon
        },
        100,
        MoveId::Splash,
    );
    if condition == "burned" {
        target.status = state::StatusCondition::Burn;
    }
    if condition.starts_with("frozen") {
        target.status = state::StatusCondition::Freeze;
    }
    if condition.ends_with("ko") {
        target.hp = 1;
    }
    let (player, foe) = if enemy {
        (target, attacker)
    } else {
        (attacker, target)
    };
    let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
    battle.link_enemy_move_override = Some((if enemy { move_id } else { MoveId::Splash }, 0));
    battle.link_rng = Some(LinkRng::new([255, 255, 0, chance, 82, 73, 64, 55, 46, 37]));
    let state = battle.battle_state.as_mut().unwrap();
    assert!(!state.link_battle && !battle.link_mode);
    let (attacker, target) = if enemy {
        (&mut state.enemy, &mut state.player)
    } else {
        (&mut state.player, &mut state.enemy)
    };
    attacker.active_mon_mut().speed = 100;
    target.active_mon_mut().speed = 200;
    if condition.starts_with("frozen_sub") {
        target.set_status2(state::status2::HAS_SUBSTITUTE_UP);
        target.substitute_hp = if condition == "frozen_sub_break" {
            1
        } else {
            255
        };
    }
    battle.set_presentation_enabled(true);
    battle.phase = BattlePhase::PlayerMenu;
    battle
}

fn turn(battle: &mut BattleScreen, enemy: bool) -> (u64, state::StatusCondition, Vec<String>) {
    let mut a = BattleInput::none();
    a.a = true;
    battle.update_frame(a);
    battle.update_frame(a);
    let state = battle.battle_state.as_ref().unwrap();
    let status = if enemy {
        state.player.active_mon().status
    } else {
        state.enemy.active_mon().status
    };
    let draws = battle.link_rng.as_ref().unwrap().consumed();
    let mut messages = Vec::new();
    let mut previous = None;
    for frame in 0..3500 {
        if battle.current_message != previous {
            if let Some(text) = &battle.current_message {
                messages.push(text.split_whitespace().collect::<Vec<_>>().join(" "));
            }
            previous = battle.current_message.clone();
        }
        if matches!(
            battle.phase,
            BattlePhase::PlayerMenu | BattlePhase::BattleOver { .. }
        ) {
            return (draws, status, messages);
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

fn inflicted(effect: MoveEffect) -> Option<state::StatusCondition> {
    use state::StatusCondition as S;
    Some(match effect {
        MoveEffect::PoisonSideEffect1 | MoveEffect::PoisonSideEffect2 => S::Poison,
        MoveEffect::BurnSideEffect1 | MoveEffect::BurnSideEffect2 => S::Burn,
        MoveEffect::FreezeSideEffect1 | MoveEffect::FreezeSideEffect2 => S::Freeze,
        MoveEffect::ParalyzeSideEffect1 | MoveEffect::ParalyzeSideEffect2 => S::Paralysis,
        _ => return None,
    })
}

#[test]
fn sixteen_moves_check_status_type_substitute_and_ko_before_chance_or_defrost() {
    use state::StatusCondition as S;
    let mut errors = Vec::new();
    let mut cases = 0;
    for id in 1..=165 {
        let move_id = MoveId::from_id(id);
        let Some(md) = MoveData::get(move_id) else {
            continue;
        };
        let Some(secondary) = inflicted(md.effect) else {
            continue;
        };
        for enemy in [false, true] {
            for condition in [
                "normal",
                "burned",
                "same_type",
                "frozen",
                "frozen_sub",
                "frozen_sub_break",
                "frozen_ko",
                "ko",
            ] {
                for chance in [0, 255] {
                    let mut battle = fixture(move_id, enemy, condition, chance);
                    let (draws, status, messages) = turn(&mut battle, enemy);
                    let thawed = condition == "frozen" && md.move_type == PokemonType::Fire;
                    let expected_status = if condition == "burned" {
                        S::Burn
                    } else if condition.starts_with("frozen") && !thawed {
                        S::Freeze
                    } else if condition == "normal" && chance == 0 {
                        secondary
                    } else {
                        S::None
                    };
                    let expected_draws = if condition == "normal" { 4 } else { 3 };
                    let defrosted = messages
                        .iter()
                        .filter(|m| m.starts_with("Fire defrosted "))
                        .count();
                    if draws != expected_draws
                        || status != expected_status
                        || defrosted != usize::from(thawed)
                    {
                        errors.push(format!(
                            "{move_id:?}, enemy={enemy}, {condition}, chance={chance}: \
                            draws={draws}/{expected_draws}, status={status:?}/{expected_status:?}, \
                            defrosted={defrosted}/{thawed}, messages={messages:?}"
                        ));
                    }
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 512);
    assert!(
        errors.is_empty(),
        "{} wrong cases:\n{}",
        errors.len(),
        errors.join("\n")
    );
}

#[test]
fn secondary_type_precheck_uses_current_conversion_types() {
    let mut errors = Vec::new();
    for move_id in [MoveId::Ember, MoveId::Thundershock] {
        for enemy in [false, true] {
            for immune in [false, true] {
                let mut battle = fixture(
                    move_id,
                    enemy,
                    if immune { "normal" } else { "same_type" },
                    0,
                );
                let state = battle.battle_state.as_mut().unwrap();
                let target = if enemy {
                    &mut state.player
                } else {
                    &mut state.enemy
                };
                let md = MoveData::get(move_id).unwrap();
                let converted = if immune {
                    md.move_type
                } else {
                    PokemonType::Water
                };
                target.conversion_type1 = Some(converted);
                target.conversion_type2 = Some(converted);
                let (draws, status, _) = turn(&mut battle, enemy);
                let expected_status = if immune {
                    state::StatusCondition::None
                } else {
                    inflicted(md.effect).unwrap()
                };
                if draws != if immune { 3 } else { 4 } || status != expected_status {
                    errors.push(format!("{move_id:?}, enemy={enemy}, immune={immune}: draws={draws}, status={status:?}"));
                }
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
