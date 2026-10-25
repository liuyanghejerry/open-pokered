use pokered_core::battle::state::{status2, BattlerState, StatusCondition};
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType};

const TRAPS: [MoveId; 4] = [MoveId::Bind, MoveId::Wrap, MoveId::Clamp, MoveId::FireSpin];

fn seed(duration: u8) -> LinkRng {
    let mut bytes = [255; 10];
    bytes[0] = duration;
    let critical = if duration >= 2 {
        bytes[1] = duration;
        2
    } else {
        1
    };
    bytes[critical + 1] = 0;
    LinkRng::new(bytes)
}

fn actor(battle: &mut BattleScreen, enemy: bool) -> &mut BattlerState {
    let state = battle.battle_state.as_mut().unwrap();
    if enemy {
        &mut state.enemy
    } else {
        &mut state.player
    }
}

fn target(battle: &mut BattleScreen, enemy: bool) -> &mut BattlerState {
    let state = battle.battle_state.as_mut().unwrap();
    if enemy {
        &mut state.player
    } else {
        &mut state.enemy
    }
}

fn setup(id: MoveId, enemy: bool, duration: u8, immune_rage: bool) -> BattleScreen {
    let mut attacker = create_pokemon_with_moves(
        Species::Mewtwo,
        20,
        [0x9a, 0x78],
        [id, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    attacker.set_nickname("WRAPPER");
    let target_move = if immune_rage {
        MoveId::Rage
    } else {
        MoveId::Splash
    };
    let defender = create_pokemon_with_moves(
        Species::Snorlax,
        100,
        [0x9a, 0x78],
        [target_move, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let (player, foe) = if enemy {
        (defender, attacker)
    } else {
        (attacker, defender)
    };
    let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
    battle.link_enemy_move_override = Some((if enemy { id } else { target_move }, 0));
    battle.link_rng = Some(seed(duration));
    actor(&mut battle, enemy).active_mon_mut().speed = 200;
    let defender = target(&mut battle, enemy);
    defender.active_mon_mut().speed = 100;
    if immune_rage {
        defender.conversion_type1 = Some(PokemonType::Ghost);
        defender.conversion_type2 = Some(PokemonType::Ghost);
        defender.battle_status2 |= status2::USING_RAGE;
        defender.selected_move = MoveId::Rage;
    }
    battle.set_presentation_enabled(true);
    battle.phase = BattlePhase::PlayerMenu;
    battle
}

fn fight(battle: &mut BattleScreen) -> Vec<String> {
    let mut input = BattleInput::none();
    input.a = true;
    for _ in 0..4 {
        battle.update_frame(input);
        if let BattlePhase::ShowingText { messages, .. } = &battle.phase {
            return messages
                .iter()
                .map(|m| m.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect();
        }
    }
    panic!("FIGHT did not resolve: {:?}", battle.phase);
}

fn drain(battle: &mut BattleScreen) -> Vec<(MoveId, bool)> {
    let mut presentations = Vec::new();
    let mut previous = None;
    for frame in 0..6000 {
        if battle.presentation.waiting {
            if let Some(current) = battle.presentation.current_move() {
                if previous != Some(current) {
                    presentations.push((current.1, current.2));
                    previous = Some(current);
                }
            }
            battle.complete_move_presentation();
        }
        if matches!(
            battle.phase,
            BattlePhase::PlayerMenu | BattlePhase::BattleOver { .. }
        ) {
            return presentations;
        }
        let mut input = BattleInput::none();
        input.a = frame % 2 == 0;
        battle.update_frame(input);
    }
    panic!("presentation did not finish: {:?}", battle.phase);
}

fn continued(messages: &[String], enemy: bool) {
    let expected = if enemy {
        "Enemy WRAPPER's attack continues!"
    } else {
        "WRAPPER's attack continues!"
    };
    assert_eq!(
        messages.first().map(String::as_str),
        Some(expected),
        "{messages:?}"
    );
    assert_eq!(
        messages
            .iter()
            .filter(|m| m.contains("attack continues!"))
            .count(),
        1
    );
    assert!(
        messages.iter().all(|m| !m.contains("WRAPPER used")),
        "{messages:?}"
    );
}

#[test]
fn every_trapping_duration_keeps_its_caption_animation_pp_and_damage() {
    for id in TRAPS {
        for enemy in [false, true] {
            for duration in 0..=3 {
                let mut battle = setup(id, enemy, duration, false);
                let pp = actor(&mut battle, enemy).active_mon().pp[0];
                // These are ordinary battles: only the player spends PP.
                let pp_cost = u8::from(!enemy);
                let mut first_damage = 0;
                let mut first_draws = 0;
                for turn in 0..duration + 2 {
                    let hp = target(&mut battle, enemy).active_mon().hp;
                    let messages = fight(&mut battle);
                    let damage = hp - target(&mut battle, enemy).active_mon().hp;
                    if turn == 0 {
                        assert!(messages[0].contains("WRAPPER used"));
                        assert!(messages.iter().all(|m| !m.contains("attack continues!")));
                        first_damage = damage;
                        assert!(damage > 0);
                        first_draws = battle.link_rng.as_ref().unwrap().consumed();
                    } else {
                        continued(&messages, enemy);
                        assert_eq!(damage, first_damage);
                        assert_eq!(battle.link_rng.as_ref().unwrap().consumed(), first_draws);
                    }
                    assert_eq!(actor(&mut battle, enemy).active_mon().pp[0], pp - pp_cost);
                    assert_eq!(drain(&mut battle), vec![(id, !enemy)]);
                }
                // A new use after the final continuation must get its ordinary
                // announcement and a new animation, not a stale action receipt.
                battle.link_rng = Some(seed(0));
                let messages = fight(&mut battle);
                assert!(messages[0].contains("WRAPPER used"));
                assert!(messages.iter().all(|m| !m.contains("attack continues!")));
                assert_eq!(
                    actor(&mut battle, enemy).active_mon().pp[0],
                    pp - 2 * pp_cost
                );
                assert_eq!(drain(&mut battle), vec![(id, !enemy)]);
            }
        }
    }
}

#[test]
fn immune_continuations_announce_before_rage_and_keep_the_animation_handoff() {
    for id in [MoveId::Bind, MoveId::Wrap] {
        for enemy in [false, true] {
            for duration in [0, 3] {
                let mut battle = setup(id, enemy, duration, true);
                let hp = target(&mut battle, enemy).active_mon().hp;
                for turn in 0..duration + 2 {
                    let messages = fight(&mut battle);
                    if turn == 0 {
                        assert!(messages[0].contains("WRAPPER used"));
                        assert!(messages.iter().any(|m| m.contains("doesn't affect")));
                    } else {
                        continued(&messages, enemy);
                        assert!(messages[1].contains("RAGE is building!"), "{messages:?}");
                    }
                    assert_eq!(target(&mut battle, enemy).active_mon().hp, hp);
                    assert_eq!(target(&mut battle, enemy).stat_stages.attack, turn as i8);
                    assert_eq!(drain(&mut battle), vec![(id, !enemy)]);
                }
            }
        }
    }
}

#[test]
fn frozen_trappers_do_not_announce_or_start_a_continuation_animation() {
    for id in TRAPS {
        for enemy in [false, true] {
            let mut battle = setup(id, enemy, 0, false);
            fight(&mut battle);
            assert_eq!(drain(&mut battle), vec![(id, !enemy)]);
            actor(&mut battle, enemy).active_mon_mut().status = StatusCondition::Freeze;
            let messages = fight(&mut battle);
            assert!(
                messages.iter().any(|m| m.contains("frozen")),
                "{messages:?}"
            );
            assert!(messages
                .iter()
                .all(|m| !m.contains("attack continues!") && !m.contains("WRAPPER used")));
            assert!(drain(&mut battle).is_empty());
        }
    }
}

#[test]
fn continuation_caption_survives_the_trappers_later_poison_faint() {
    for id in TRAPS {
        for enemy in [false, true] {
            let mut battle = setup(id, enemy, 0, false);
            fight(&mut battle);
            assert_eq!(drain(&mut battle), vec![(id, !enemy)]);
            let source = actor(&mut battle, enemy).active_mon_mut();
            source.hp = 1;
            source.status = StatusCondition::Poison;
            let messages = fight(&mut battle);
            continued(&messages, enemy);
            assert_eq!(actor(&mut battle, enemy).active_mon().hp, 0);
            assert_eq!(drain(&mut battle), vec![(id, !enemy)]);
        }
    }
}
