//! A display name must not change move animation or residual HP publication.

use pokered_core::battle::{state, BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

#[derive(Debug, PartialEq, Eq)]
struct Trace {
    boundaries: Vec<(Option<(MoveId, bool)>, String, [u16; 2])>,
    final_hp: [u16; 2],
    simulated_hp: [u16; 2],
    after_eight_menu_frames: [u16; 2],
    draws: u64,
}

fn fixture(mask: u8, player_first: bool, condition: &str, affected_enemy: bool) -> BattleScreen {
    let mon = |species| {
        create_pokemon_with_moves(
            species,
            40,
            [0x9a, 0x78],
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    let mut player = mon(Species::Bulbasaur);
    let mut enemy = mon(Species::Rattata);
    if mask & 1 != 0 {
        player.set_nickname("SPROUT");
    }
    if mask & 2 != 0 {
        enemy.set_nickname("SQUEAK");
    }
    let affected = if affected_enemy {
        &mut enemy
    } else {
        &mut player
    };
    affected.status = match condition {
        "poison" => state::StatusCondition::Poison,
        "burn" => state::StatusCondition::Burn,
        "freeze" => state::StatusCondition::Freeze,
        _ => state::StatusCondition::None,
    };
    let mut b = BattleScreen::from_parties(true, &[player], &[enemy], None);
    b.link_enemy_move_override = Some((MoveId::Tackle, 0));
    b.link_rng = Some(LinkRng::new([255, 0, 255, 255, 0, 255, 255, 0, 255, 255]));
    let s = b.battle_state.as_mut().unwrap();
    s.player.active_mon_mut().speed = if player_first { 200 } else { 100 };
    s.enemy.active_mon_mut().speed = if player_first { 100 } else { 200 };
    b.set_presentation_enabled(true);
    b.phase = BattlePhase::PlayerMenu;
    b
}

fn turn(b: &mut BattleScreen) -> Trace {
    let mut a = BattleInput::none();
    a.a = true;
    b.update_frame(a);
    b.update_frame(a);
    let mut boundaries = Vec::new();
    for f in 0..4000 {
        if b.presentation.waiting {
            let text = b
                .current_message
                .as_deref()
                .unwrap_or("")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .replace("SPROUT", "BULBASAUR")
                .replace("SQUEAK", "RATTATA");
            boundaries.push((
                b.presentation
                    .current_move()
                    .map(|(_, id, player)| (id, player)),
                text,
                [b.player_hp, b.enemy_hp],
            ));
            b.complete_move_presentation();
        }
        if b.phase == BattlePhase::PlayerMenu {
            let final_hp = [b.player_hp, b.enemy_hp];
            let state = b.battle_state.as_ref().unwrap();
            let simulated_hp = [state.player.active_mon().hp, state.enemy.active_mon().hp];
            for _ in 0..8 {
                b.update_frame(BattleInput::none());
            }
            return Trace {
                boundaries,
                final_hp,
                simulated_hp,
                after_eight_menu_frames: [b.player_hp, b.enemy_hp],
                draws: b.link_rng.as_ref().map_or(0, |r| r.consumed()),
            };
        }
        b.update_frame(if f % 2 == 0 { a } else { BattleInput::none() });
    }
    panic!("unfinished {:?}", b.phase);
}

#[test]
fn nicknames_preserve_move_and_residual_hp_boundaries_for_both_sides() {
    let mut errors = Vec::new();
    for player_first in [false, true] {
        for condition in ["normal", "poison", "burn", "freeze"] {
            for affected_enemy in [false, true] {
                let control = turn(&mut fixture(0, player_first, condition, affected_enemy));
                assert_eq!(control.final_hp, control.simulated_hp);
                for mask in 1..=3 {
                    let named = turn(&mut fixture(mask, player_first, condition, affected_enemy));
                    if named != control {
                        errors.push(format!("player_first={player_first} {condition} affected_enemy={affected_enemy} mask={mask}: control={control:?}; named={named:?}"));
                    }
                }
            }
        }
    }
    assert!(
        errors.is_empty(),
        "{} mismatched traces:\n{}",
        errors.len(),
        errors.join("\n")
    );
}

#[test]
fn transform_preserves_animation_identity_before_and_after_copying_species() {
    for enemy_actor in [false, true] {
        for named in [false, true] {
            let mon = |species, mv| {
                create_pokemon_with_moves(
                    species,
                    40,
                    [0x9a, 0x78],
                    [mv, MoveId::None, MoveId::None, MoveId::None],
                )
                .unwrap()
            };
            let mut actor = mon(Species::Ditto, MoveId::Transform);
            let mut target = mon(Species::Tangela, MoveId::Tackle);
            if named {
                actor.set_nickname("COPY");
                target.set_nickname("VINE");
            }
            let (player, foe) = if enemy_actor {
                (target, actor)
            } else {
                (actor, target)
            };
            let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
            battle.rng = pokered_core::battle::pokered_rules::runtime::StdBattleRng::from_seed(42);
            battle.link_enemy_move_override = Some((
                if enemy_actor {
                    MoveId::Transform
                } else {
                    MoveId::Tackle
                },
                0,
            ));
            let state = battle.battle_state.as_mut().unwrap();
            state.player.active_mon_mut().speed = if enemy_actor { 10 } else { 200 };
            state.enemy.active_mon_mut().speed = if enemy_actor { 200 } else { 10 };
            battle.set_presentation_enabled(true);
            battle.phase = BattlePhase::PlayerMenu;
            let first = turn(&mut battle);
            let moves: Vec<_> = first.boundaries.iter().filter_map(|b| b.0).collect();
            assert_eq!(
                moves,
                [
                    (MoveId::Transform, !enemy_actor),
                    (MoveId::Tackle, enemy_actor)
                ],
                "enemy_actor={enemy_actor} named={named}: {first:?}"
            );
            let state = battle.battle_state.as_ref().unwrap();
            let actor = if enemy_actor {
                &state.enemy
            } else {
                &state.player
            };
            assert_eq!(actor.active_mon().species, Species::Tangela);
            battle.link_enemy_move_override = Some((MoveId::Tackle, 0));
            let second = turn(&mut battle);
            let moves: Vec<_> = second.boundaries.iter().filter_map(|b| b.0).collect();
            assert_eq!(
                moves.len(),
                2,
                "enemy_actor={enemy_actor} named={named}: {second:?}"
            );
            assert!(
                moves.contains(&(MoveId::Tackle, true)) && moves.contains(&(MoveId::Tackle, false))
            );
            assert_eq!(second.final_hp, second.simulated_hp);
        }
    }
}
