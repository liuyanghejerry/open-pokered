//! The expected flags were observed by executing original CriticalHitTest.
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::battle::state::StatusCondition;
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

fn setup(raw: u8, enemy: bool) -> BattleScreen {
    let attacker = create_pokemon_with_moves(Species::Mewtwo, 20, [0x9a, 0x78],
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let mut target = create_pokemon_with_moves(Species::Snorlax, 100, [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    target.status = StatusCondition::Sleep(7);
    let (p, e) = if enemy { (target, attacker) } else { (attacker, target) };
    let mut b = BattleScreen::from_parties(true, &[p], &[e], None);
    b.link_enemy_move_override = Some((if enemy { MoveId::Tackle } else { MoveId::Splash }, 0));
    b.link_rng = Some(LinkRng::new([raw, 0, 255, 255, 255, 255, 255, 255, 255, 255]));
    let s = b.battle_state.as_mut().unwrap();
    let (actor, defender) = if enemy { (&mut s.enemy, &mut s.player) } else { (&mut s.player, &mut s.enemy) };
    actor.active_mon_mut().speed = 100; defender.active_mon_mut().speed = 200;
    b.set_presentation_enabled(true); b.phase = BattlePhase::PlayerMenu; b
}

fn fight(b: &mut BattleScreen) -> bool {
    let mut a = BattleInput::none(); a.a = true;
    for _ in 0..4 {
        b.update_frame(a);
        if let BattlePhase::ShowingText { messages, .. } = &b.phase {
            return messages.iter().any(|s| s == "Critical hit!");
        }
    }
    panic!("FIGHT did not resolve: {:?}", b.phase);
}

fn drain(b: &mut BattleScreen) {
    for frame in 0..4000 {
        if b.presentation.waiting { b.complete_move_presentation(); }
        if matches!(b.phase, BattlePhase::PlayerMenu) { return; }
        let mut a = BattleInput::none(); a.a = frame % 2 == 0; b.update_frame(a);
    }
    panic!("presentation did not finish: {:?}", b.phase);
}

#[test]
fn original_256_byte_critical_oracle_matches_both_sides_without_extra_rng() {
    // Real original-ROM results: Mewtwo base Speed130, Tackle, all256 raw bytes.
    let golden = concat!(
        "11111111100000000000000000000000",
        "11111111000000000000000000000000",
        "11111111000000000000000000000000",
        "11111111000000000000000000000000",
        "11111111000000000000000000000000",
        "11111111000000000000000000000000",
        "11111111000000000000000000000000",
        "11111111000000000000000000000000",
    ).as_bytes();
    assert_eq!(golden.len(), 256);
    for enemy in [false, true] {
        let mut control = setup(255, enemy);
        assert!(!fight(&mut control));
        let draws = control.link_rng.as_ref().unwrap().consumed(); drain(&mut control);
        for raw in 0..=255u8 {
            let mut b = setup(raw, enemy);
            assert_eq!(fight(&mut b), golden[usize::from(raw)] == b'1', "raw={raw} enemy={enemy}");
            assert_eq!(b.link_rng.as_ref().unwrap().consumed(), draws);
            drain(&mut b);
        }
    }
}
