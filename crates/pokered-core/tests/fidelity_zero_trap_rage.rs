//! Trapping continuations skip the hit test, even when the stored damage is zero.
use pokered_core::battle::state::{status2, BattlerState};
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType};

fn target(battle: &mut BattleScreen, enemy: bool) -> &mut BattlerState {
    let state = battle.battle_state.as_mut().unwrap();
    if enemy {
        &mut state.player
    } else {
        &mut state.enemy
    }
}

fn setup(id: MoveId, enemy: bool, immune: bool, initial: i8, species: Species) -> BattleScreen {
    let actor = create_pokemon_with_moves(
        Species::Mewtwo,
        20,
        [0x9a, 0x78],
        [id, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let defender = create_pokemon_with_moves(
        species,
        100,
        [0x9a, 0x78],
        [MoveId::Rage, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let (player, foe) = if enemy {
        (defender, actor)
    } else {
        (actor, defender)
    };
    let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
    // Two-turn trap; non-critical first hit; accuracy succeeds; maximum damage.
    battle.link_rng = Some(LinkRng::new(if immune {
        [0, 255, 0, 255, 255, 255, 255, 255, 255, 255]
    } else {
        [0, 255, 255, 0, 255, 255, 255, 255, 255, 255]
    }));
    let state = battle.battle_state.as_mut().unwrap();
    let (actor, defender) = if enemy {
        (&mut state.enemy, &mut state.player)
    } else {
        (&mut state.player, &mut state.enemy)
    };
    actor.active_mon_mut().speed = 200;
    defender.active_mon_mut().speed = 100;
    defender.battle_status2 |= status2::USING_RAGE;
    defender.selected_move = MoveId::Rage;
    defender.stat_stages.attack = initial;
    if immune {
        defender.conversion_type1 = Some(PokemonType::Ghost);
        defender.conversion_type2 = Some(PokemonType::Ghost);
    }
    battle.set_presentation_enabled(true);
    battle.phase = BattlePhase::PlayerMenu;
    battle
}

fn fight(battle: &mut BattleScreen, id: MoveId, enemy: bool) -> Vec<String> {
    battle.link_enemy_move_override = Some((if enemy { id } else { MoveId::Rage }, 0));
    let mut input = BattleInput::none();
    input.a = true;
    for _ in 0..4 {
        battle.update_frame(input);
        if let BattlePhase::ShowingText { messages, .. } = &battle.phase {
            return messages
                .iter()
                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect();
        }
    }
    panic!("FIGHT did not resolve: {:?}", battle.phase);
}

fn drain(battle: &mut BattleScreen) {
    for frame in 0..4000 {
        if battle.presentation.waiting {
            battle.complete_move_presentation();
        }
        if matches!(battle.phase, BattlePhase::PlayerMenu) {
            return;
        }
        let mut input = BattleInput::none();
        input.a = frame % 2 == 0;
        battle.update_frame(input);
    }
    panic!("presentation did not finish: {:?}", battle.phase);
}

fn building(messages: &[String]) -> usize {
    messages
        .iter()
        .filter(|s| s.contains("RAGE is building!"))
        .count()
}

#[test]
fn immune_first_hit_does_not_build_rage_but_its_zero_continuation_does() {
    for id in [MoveId::Bind, MoveId::Wrap] {
        for enemy in [false, true] {
            for initial in [0, 5, 6] {
                for one_hp in [false, true] {
                    let mut battle = setup(id, enemy, true, initial, Species::Snorlax);
                    if one_hp {
                        target(&mut battle, enemy).active_mon_mut().hp = 1;
                    }
                    let hp = target(&mut battle, enemy).active_mon().hp;
                    let first = fight(&mut battle, id, enemy);
                    assert!(first.iter().any(|s| s.contains("doesn't affect")));
                    assert_eq!(target(&mut battle, enemy).active_mon().hp, hp);
                    assert_eq!(target(&mut battle, enemy).stat_stages.attack, initial);
                    assert_eq!(building(&first), 0);
                    drain(&mut battle);
                    let draws = battle.link_rng.as_ref().unwrap().consumed();
                    let second = fight(&mut battle, id, enemy);
                    assert_eq!(target(&mut battle, enemy).active_mon().hp, hp);
                    let gain = initial < 6;
                    assert_eq!(
                        target(&mut battle, enemy).stat_stages.attack,
                        initial + i8::from(gain),
                        "{id:?} enemy={enemy} {second:?}"
                    );
                    assert_eq!(building(&second), usize::from(gain));
                    assert_eq!(
                        second.iter().filter(|s| s.contains("ATTACK rose!")).count(),
                        usize::from(gain)
                    );
                    assert!(second.iter().all(|s| !s.contains("doesn't affect")));
                    assert_eq!(battle.link_rng.as_ref().unwrap().consumed(), draws);
                    drain(&mut battle);
                }
            }
        }
    }
}

#[test]
fn damaging_trapping_hits_and_continuations_keep_their_stage_caps() {
    for id in [MoveId::Bind, MoveId::Wrap] {
        for enemy in [false, true] {
            for initial in [0, 5, 6] {
                let mut battle = setup(id, enemy, false, initial, Species::Snorlax);
                let mut stage = initial;
                for turn in 0..2 {
                    let hp = target(&mut battle, enemy).active_mon().hp;
                    let draws = battle.link_rng.as_ref().unwrap().consumed();
                    let messages = fight(&mut battle, id, enemy);
                    assert!(target(&mut battle, enemy).active_mon().hp < hp);
                    let gain = stage < 6;
                    stage += i8::from(gain);
                    assert_eq!(target(&mut battle, enemy).stat_stages.attack, stage);
                    assert_eq!(building(&messages), usize::from(gain));
                    if turn == 1 {
                        assert_eq!(battle.link_rng.as_ref().unwrap().consumed(), draws);
                    }
                    drain(&mut battle);
                }
            }
        }
    }
}

#[test]
fn zero_and_nonzero_continuations_obey_actual_attack_999() {
    for id in [MoveId::Bind, MoveId::Wrap] {
        for enemy in [false, true] {
            for immune in [false, true] {
                let mut battle = setup(id, enemy, immune, 5, Species::Dragonite);
                for turn in 0..2 {
                    let messages = fight(&mut battle, id, enemy);
                    let attempted = turn == 1 || !immune;
                    assert_eq!(
                        target(&mut battle, enemy).stat_stages.attack,
                        5,
                        "{messages:?}"
                    );
                    assert_eq!(
                        target(&mut battle, enemy).staged_badge_stats.unwrap()[0],
                        999
                    );
                    assert_eq!(building(&messages), usize::from(attempted));
                    assert_eq!(
                        messages
                            .iter()
                            .filter(|s| *s == "Nothing happened!")
                            .count(),
                        usize::from(attempted)
                    );
                    assert!(messages.iter().all(|s| !s.contains("ATTACK rose!")));
                    drain(&mut battle);
                }
            }
        }
    }
}
