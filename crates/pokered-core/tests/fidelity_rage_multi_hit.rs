//! HandleBuildingRage runs after every survived strike (core.asm:3243,5643).
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::battle::state::{status2, BattlerState, StatusCondition};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType};

const MOVES: [MoveId; 10] = [MoveId::Doubleslap, MoveId::CometPunch, MoveId::FuryAttack,
    MoveId::FurySwipes, MoveId::Barrage, MoveId::PinMissile, MoveId::SpikeCannon,
    MoveId::DoubleKick, MoveId::Bonemerang, MoveId::Twineedle];

fn side(b: &mut BattleScreen, enemy: bool) -> &mut BattlerState {
    let s = b.battle_state.as_mut().unwrap();
    if enemy { &mut s.enemy } else { &mut s.player }
}

fn setup(id: MoveId, enemy: bool, count: u8, initial_stage: i8) -> BattleScreen {
    setup_target(id, enemy, count, initial_stage, Species::Snorlax)
}

fn setup_target(id: MoveId, enemy: bool, count: u8, initial_stage: i8, species: Species) -> BattleScreen {
    let a = create_pokemon_with_moves(Species::Mewtwo, 20, [0x9a, 0x78],
        [id, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let mut t = create_pokemon_with_moves(species, 100, [0x9a, 0x78],
        [MoveId::Rage, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    t.status = StatusCondition::Sleep(2);
    let (p, e) = if enemy { (t, a) } else { (a, t) };
    let mut b = BattleScreen::from_parties(true, &[p], &[e], None);
    b.link_enemy_move_override = Some((if enemy { id } else { MoveId::Rage }, 0));
    b.link_rng = Some(LinkRng::new([255, 255, 0, 255, count - 2, 82, 73, 64, 55, 46]));
    side(&mut b, enemy).active_mon_mut().speed = 100;
    let target = side(&mut b, !enemy);
    target.active_mon_mut().speed = 200;
    target.battle_status2 |= status2::USING_RAGE;
    target.selected_move = MoveId::Rage;
    target.stat_stages.attack = initial_stage;
    b.set_presentation_enabled(true);
    b.phase = BattlePhase::PlayerMenu;
    b
}

fn fight(b: &mut BattleScreen) -> Vec<String> {
    let mut a = BattleInput::none(); a.a = true;
    for _ in 0..4 {
        b.update_frame(a);
        if let BattlePhase::ShowingText { messages, .. } = &b.phase { return messages.clone(); }
    }
    panic!("FIGHT did not resolve: {:?}", b.phase);
}

fn check(b: &mut BattleScreen, enemy: bool, before: i8, hits: u8, messages: &[String]) {
    let gains = hits.min((6 - before) as u8);
    assert_eq!(side(b, !enemy).stat_stages.attack, before + gains as i8, "{messages:?}");
    let normalized: Vec<String> = messages.iter().map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
    assert_eq!(normalized.iter().filter(|s| s.contains("RAGE is building!")).count(), gains as usize, "{messages:?}");
    assert_eq!(normalized.iter().filter(|s| s.contains("ATTACK rose!")).count(), gains as usize, "{messages:?}");
    assert!(!normalized.iter().any(|s| s.contains("ATTACK greatly rose")));
}

#[test]
fn ten_multi_hit_moves_raise_rage_for_each_survived_hit_until_the_stage_cap() {
    for (index, id) in MOVES.into_iter().enumerate() {
        for enemy in [false, true] {
            for count in 2..=if index < 7 { 5 } else { 2 } {
                for initial in [0, 4, 5, 6] {
                    let mut b = setup(id, enemy, count, initial);
                    let messages = fight(&mut b);
                    check(&mut b, enemy, initial, count, &messages);
                    let count_at = messages.iter().position(|s| s.starts_with("Hit ")).unwrap();
                    assert!(messages[count_at + 1..].iter().all(|s| !s.contains("RAGE") && !s.contains("ATTACK")));
                }
            }
        }
    }
}

#[test]
fn a_knockout_hit_cannot_build_rage_but_earlier_survived_hits_do() {
    for (index, id) in MOVES.into_iter().enumerate() {
        let count = if index < 7 { 5 } else { 2 };
        for enemy in [false, true] {
            let mut probe = setup(id, enemy, count, 0);
            let initial_hp = side(&mut probe, !enemy).active_mon().hp;
            fight(&mut probe);
            let per_hit = (initial_hp - side(&mut probe, !enemy).active_mon().hp) / u16::from(count);
            assert!(per_hit > 0);
            for knockout_hit in 1..=count {
                let mut b = setup(id, enemy, count, 0);
                side(&mut b, !enemy).active_mon_mut().hp = per_hit * u16::from(knockout_hit);
                let messages = fight(&mut b);
                assert_eq!(side(&mut b, !enemy).active_mon().hp, 0);
                check(&mut b, enemy, 0, knockout_hit - 1, &messages);
            }
        }
    }
}

#[test]
fn substitute_hits_build_rage_and_first_hit_break_only_gives_one_gain() {
    for (id, count) in [(MoveId::DoubleKick, 2), (MoveId::FuryAttack, 5)] {
        for enemy in [false, true] {
            let mut probe = setup(id, enemy, count, 0);
            let initial_hp = side(&mut probe, !enemy).active_mon().hp;
            fight(&mut probe);
            let per_hit = (initial_hp - side(&mut probe, !enemy).active_mon().hp) / u16::from(count);
            for (sub_hp, gains) in [(1, 1), (per_hit + 1, count), (200, count)] {
                let mut b = setup(id, enemy, count, 0);
                let t = side(&mut b, !enemy);
                t.battle_status2 |= status2::HAS_SUBSTITUTE_UP;
                t.substitute_hp = sub_hp as u8;
                let messages = fight(&mut b);
                check(&mut b, enemy, 0, gains, &messages);
            }
        }
    }
}

#[test]
fn missed_or_immune_multi_hit_moves_never_build_rage() {
    for enemy in [false, true] {
        let mut b = setup(MoveId::DoubleKick, enemy, 2, 0);
        b.link_rng = Some(LinkRng::new([255; 10]));
        let messages = fight(&mut b);
        check(&mut b, enemy, 0, 0, &messages);
        for (id, immune) in [(MoveId::FuryAttack, PokemonType::Ghost),
            (MoveId::DoubleKick, PokemonType::Ghost), (MoveId::Bonemerang, PokemonType::Flying)] {
            let mut b = setup(id, enemy, 5, 0);
            side(&mut b, !enemy).conversion_type1 = Some(immune);
            side(&mut b, !enemy).conversion_type2 = Some(immune);
            let messages = fight(&mut b);
            check(&mut b, enemy, 0, 0, &messages);
        }
    }
}

#[test]
fn each_actual_gain_reapplies_the_original_attackers_burn_penalty() {
    for enemy in [false, true] {
        for initial in [0, 5, 6] {
            let mut control = setup(MoveId::FuryAttack, enemy, 5, initial);
            side(&mut control, !enemy).battle_status2 &= !status2::USING_RAGE;
            side(&mut control, enemy).active_mon_mut().status = StatusCondition::Burn;
            fight(&mut control);
            let baseline = side(&mut control, enemy).staged_badge_stats.unwrap()[0];
            let draws = control.link_rng.as_ref().unwrap().consumed();
            let mut b = setup(MoveId::FuryAttack, enemy, 5, initial);
            side(&mut b, enemy).active_mon_mut().status = StatusCondition::Burn;
            let messages = fight(&mut b);
            check(&mut b, enemy, initial, 5, &messages);
            let gains = 5.min((6 - initial) as u8);
            let expected = (0..gains).fold(baseline, |value, _| (value / 2).max(1));
            assert_eq!(side(&mut b, enemy).staged_badge_stats.unwrap()[0], expected);
            assert_eq!(b.link_rng.as_ref().unwrap().consumed(), draws);
        }
    }
}

#[test]
fn a_missed_next_turn_does_not_reuse_the_previous_rage_receipt() {
    for enemy in [false, true] {
        for (id, count) in [(MoveId::DoubleKick, 2), (MoveId::FuryAttack, 5)] {
            let mut b = setup(id, enemy, count, 0);
            let messages = fight(&mut b);
            check(&mut b, enemy, 0, count, &messages);
            for frame in 0..4000 {
                if b.presentation.waiting { b.complete_move_presentation(); }
                if matches!(b.phase, BattlePhase::PlayerMenu) { break; }
                let mut a = BattleInput::none(); a.a = frame % 2 == 0;
                b.update_frame(a);
            }
            assert!(matches!(b.phase, BattlePhase::PlayerMenu));
            b.link_rng = Some(LinkRng::new([255; 10]));
            let messages = fight(&mut b);
            check(&mut b, enemy, count as i8, 0, &messages);
        }
    }
}

#[test]
fn attack_999_restores_the_stage_and_reports_failed_attempts_without_penalties() {
    for enemy in [false, true] {
        for (initial, gains) in [(3, 2), (4, 1), (5, 0), (6, 0)] {
            for burned in [false, true] {
                let make = || setup_target(MoveId::FuryAttack, enemy, 5, initial, Species::Dragonite);
                let mut control = make();
                side(&mut control, !enemy).battle_status2 &= !status2::USING_RAGE;
                if burned { side(&mut control, enemy).active_mon_mut().status = StatusCondition::Burn; }
                fight(&mut control);
                let before = side(&mut control, enemy).staged_badge_stats.unwrap()[0];
                let mut b = make();
                if burned { side(&mut b, enemy).active_mon_mut().status = StatusCondition::Burn; }
                let messages = fight(&mut b);
                assert_eq!(side(&mut b, !enemy).stat_stages.attack, initial + gains);
                assert_eq!(side(&mut b, !enemy).staged_badge_stats.unwrap()[0], 999);
                let normalized: Vec<String> = messages.iter().map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
                let attempts = if initial == 6 { 0 } else { 5 };
                assert_eq!(normalized.iter().filter(|s| s.contains("RAGE is building!")).count(), attempts, "{messages:?}");
                assert_eq!(normalized.iter().filter(|s| s.contains("ATTACK rose!")).count(), gains as usize, "{messages:?}");
                assert_eq!(normalized.iter().filter(|s| s.as_str() == "Nothing happened!").count(), attempts - gains as usize, "{messages:?}");
                let expected = if burned { (0..gains).fold(before, |n, _| (n / 2).max(1)) } else { before };
                assert_eq!(side(&mut b, enemy).staged_badge_stats.unwrap()[0], expected);
                assert_eq!(b.link_rng.as_ref().unwrap().consumed(), control.link_rng.as_ref().unwrap().consumed());
            }
        }
    }
}
