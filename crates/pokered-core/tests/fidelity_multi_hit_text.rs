//! Completed-attack captions: core.asm:3238-3267,5638-5667; text_2.asm.
//! Prepared parties/RNG, then public FIGHT input and presentation completion.
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::battle::state::{status2, StatusCondition};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

const MOVES: [MoveId; 10] = [MoveId::Doubleslap, MoveId::CometPunch, MoveId::FuryAttack,
    MoveId::FurySwipes, MoveId::Barrage, MoveId::PinMissile, MoveId::SpikeCannon,
    MoveId::DoubleKick, MoveId::Bonemerang, MoveId::Twineedle];

fn setup(id: MoveId, enemy: bool, count: u8) -> BattleScreen {
    let a = create_pokemon_with_moves(Species::Mewtwo, 20, [0x9a, 0x78],
        [id, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let b = create_pokemon_with_moves(Species::Snorlax, 100, [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let (p, e) = if enemy { (b, a) } else { (a, b) };
    let mut battle = BattleScreen::from_parties(true, &[p], &[e], None);
    battle.link_enemy_move_override = Some((if enemy { id } else { MoveId::Splash }, 0));
    battle.link_rng = Some(LinkRng::new([255, 255, 0, 255, count - 2, 82, 73, 64, 55, 46]));
    let s = battle.battle_state.as_mut().unwrap();
    s.player.active_mon_mut().speed = if enemy { 200 } else { 100 };
    s.enemy.active_mon_mut().speed = if enemy { 100 } else { 200 };
    battle.set_presentation_enabled(true);
    battle.phase = BattlePhase::PlayerMenu;
    battle
}

fn target(b: &mut BattleScreen, enemy: bool) -> &mut pokered_core::battle::state::BattlerState {
    let s = b.battle_state.as_mut().unwrap();
    if enemy { &mut s.player } else { &mut s.enemy }
}

fn fight(b: &mut BattleScreen) -> Vec<String> {
    let mut a = BattleInput::none(); a.a = true;
    for _ in 0..4 {
        b.update_frame(a);
        if let BattlePhase::ShowingText { messages, .. } = &b.phase { return messages.clone(); }
    }
    panic!("FIGHT did not produce text: {:?}", b.phase);
}

fn counts(messages: &[String]) -> Vec<&str> {
    messages.iter().filter(|s| s.starts_with("Hit ")).map(String::as_str).collect()
}

fn caption(enemy: bool, count: u8) -> String {
    if enemy { format!("Hit {count} times!") } else { format!("Hit the enemy\n{count} times!") }
}

fn finish(b: &mut BattleScreen, id: MoveId, enemy: bool, expected: usize) {
    let mut serials = Vec::new();
    for frame in 0..3500 {
        if let Some((serial, actual, player)) = b.presentation.current_move() {
            if actual == id && player != enemy && !serials.contains(&serial) { serials.push(serial); }
        }
        if b.current_message.as_ref().is_some_and(|s| s.starts_with("Hit ")) {
            assert_eq!(serials.len(), expected, "count appeared before all hits completed");
            assert!(!b.presentation.waiting, "count page must not replay the move");
        }
        if b.presentation.waiting { b.complete_move_presentation(); }
        if matches!(b.phase, BattlePhase::PlayerMenu | BattlePhase::BattleOver { .. }) {
            assert_eq!(serials.len(), expected); return;
        }
        let mut input = BattleInput::none(); input.a = frame % 2 == 0;
        b.update_frame(input);
    }
    panic!("presentation did not end: {:?}", b.phase);
}

#[test]
fn every_completed_multi_hit_move_prints_its_actual_count_on_both_sides() {
    for (index, id) in MOVES.into_iter().enumerate() {
        for enemy in [false, true] {
            for count in 2..=if index < 7 { 5 } else { 2 } {
                let mut b = setup(id, enemy, count);
                let messages = fight(&mut b);
                assert_eq!(counts(&messages), [caption(enemy, count)], "{id:?}, enemy={enemy}");
                finish(&mut b, id, enemy, count as usize);
            }
        }
    }
}

#[test]
fn knockout_on_any_hit_suppresses_the_completed_count() {
    for (index, id) in MOVES.into_iter().enumerate() {
        let count = if index < 7 { 5 } else { 2 };
        for enemy in [false, true] {
            let mut probe = setup(id, enemy, count);
            let initial = target(&mut probe, enemy).active_mon().hp;
            fight(&mut probe);
            let damage = initial - target(&mut probe, enemy).active_mon().hp;
            let per_hit = damage / u16::from(count);
            assert!(per_hit > 0 && damage % u16::from(count) == 0);
            for hit in 1..=count {
                let mut b = setup(id, enemy, count);
                target(&mut b, enemy).active_mon_mut().hp = per_hit * u16::from(hit);
                let messages = fight(&mut b);
                assert!(counts(&messages).is_empty(), "{id:?}, enemy={enemy}, KO={hit}");
                finish(&mut b, id, enemy, hit as usize);
            }
        }
    }
}

#[test]
fn substitute_first_hit_cancellation_differs_from_later_break_or_intact_doll() {
    for id in [MoveId::DoubleKick, MoveId::FuryAttack] {
        let count = if id == MoveId::DoubleKick { 2 } else { 5 };
        for enemy in [false, true] {
            let mut probe = setup(id, enemy, count);
            let initial = target(&mut probe, enemy).active_mon().hp;
            fight(&mut probe);
            let per_hit = (initial - target(&mut probe, enemy).active_mon().hp) / u16::from(count);
            for (sub_hp, expected) in [(1, 1), (per_hit + 1, count), (200, count)] {
                let mut b = setup(id, enemy, count);
                let t = target(&mut b, enemy);
                t.battle_status2 |= status2::HAS_SUBSTITUTE_UP;
                t.substitute_hp = sub_hp as u8;
                let messages = fight(&mut b);
                if expected == 1 { assert!(counts(&messages).is_empty()); }
                else { assert_eq!(counts(&messages), [caption(enemy, count)]); }
                finish(&mut b, id, enemy, expected as usize);
            }
        }
    }
}

#[test]
fn completed_count_survives_a_later_self_destruct_and_precedes_its_announcement() {
    for enemy in [false, true] {
        let mut b = setup(MoveId::DoubleKick, enemy, 2);
        let s = b.battle_state.as_mut().unwrap();
        let (a, t) = if enemy { (&mut s.enemy, &mut s.player) } else { (&mut s.player, &mut s.enemy) };
        a.active_mon_mut().speed = 300; t.active_mon_mut().speed = 100;
        t.active_mon_mut().moves[0] = MoveId::Selfdestruct;
        b.link_enemy_move_override = Some((if enemy { MoveId::DoubleKick } else { MoveId::Selfdestruct }, 0));
        b.link_rng = Some(LinkRng::new([255, 255, 0, 255, 255, 0, 255, 255, 0, 255]));
        let messages = fight(&mut b);
        assert_eq!(target(&mut b, enemy).active_mon().hp, 0);
        assert_eq!(counts(&messages), [caption(enemy, 2)]);
        let count = messages.iter().position(|s| s.starts_with("Hit ")).unwrap();
        let later = messages.iter().position(|s| s.contains("SELFDESTRUCT")).unwrap_or_else(|| panic!("{messages:?}"));
        assert!(count < later, "{messages:?}");
    }
}

#[test]
fn count_precedes_twineedle_poison_and_follows_existing_rage_stat_narration() {
    for enemy in [false, true] {
        for raging in [false, true] {
            let mut b = setup(MoveId::Twineedle, enemy, 2);
            let s = b.battle_state.as_mut().unwrap();
            let (a, t) = if enemy { (&mut s.enemy, &mut s.player) } else { (&mut s.player, &mut s.enemy) };
            a.active_mon_mut().speed = 300; t.active_mon_mut().speed = 100;
            if raging {
                t.battle_status2 |= status2::USING_RAGE;
                t.selected_move = MoveId::Rage;
                t.active_mon_mut().moves[0] = MoveId::Rage;
            }
            b.link_rng = Some(LinkRng::new([255, 255, 0, 0, 255, 255, 0, 255, 255, 255]));
            let messages = fight(&mut b);
            let count = messages.iter().position(|s| s.starts_with("Hit ")).expect("count");
            let poison = messages.iter().position(|s| s.contains("was poisoned")).expect("poison");
            assert!(count < poison, "{messages:?}");
            if raging {
                let rage = messages.iter().position(|s| s.contains("ATTACK") && s.contains("rose")).expect("Rage stat caption");
                assert!(rage < count, "{messages:?}");
            }
        }
    }
}

#[test]
fn previous_count_does_not_leak_into_a_miss_or_blocked_next_turn() {
    for enemy in [false, true] {
        for blocked in [false, true] {
            let mut b = setup(MoveId::DoubleKick, enemy, 2);
            assert_eq!(counts(&fight(&mut b)), [caption(enemy, 2)]);
            finish(&mut b, MoveId::DoubleKick, enemy, 2);
            if blocked {
                let s = b.battle_state.as_mut().unwrap();
                (if enemy { &mut s.enemy } else { &mut s.player }).active_mon_mut().status = StatusCondition::Freeze;
            }
            b.link_rng = Some(LinkRng::new([255; 10]));
            let messages = fight(&mut b);
            assert!(counts(&messages).is_empty(), "{messages:?}");
            finish(&mut b, MoveId::DoubleKick, enemy, if blocked { 0 } else { 1 });
        }
    }
}
