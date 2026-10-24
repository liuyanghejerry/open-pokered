//! An immune attack returns before the ROM's multi-hit effect dispatch.
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

fn setup(id: MoveId, enemy: bool, count: u8, doll: bool, raging: bool) -> BattleScreen {
    let a = create_pokemon_with_moves(Species::Mewtwo, 20, [0x9a, 0x78],
        [id, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let mut t = create_pokemon_with_moves(Species::Snorlax, 100, [0x9a, 0x78],
        [MoveId::Rage, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    t.status = StatusCondition::Sleep(7);
    let (p, e) = if enemy { (t, a) } else { (a, t) };
    let mut b = BattleScreen::from_parties(true, &[p], &[e], None);
    b.link_enemy_move_override = Some((if enemy { id } else { MoveId::Rage }, 0));
    reset_rng(&mut b, count);
    side(&mut b, enemy).active_mon_mut().speed = 100;
    let target = side(&mut b, !enemy);
    target.active_mon_mut().speed = 200;
    target.selected_move = MoveId::Rage;
    if raging { target.battle_status2 |= status2::USING_RAGE; }
    if doll { target.battle_status2 |= status2::HAS_SUBSTITUTE_UP; target.substitute_hp = 200; }
    b.set_presentation_enabled(true);
    b.phase = BattlePhase::PlayerMenu;
    b
}

fn reset_rng(b: &mut BattleScreen, count: u8) {
    b.link_rng = Some(LinkRng::new([255, 0, 255, 255, count - 2, 82, 73, 64, 55, 46]));
}

fn make_immune(b: &mut BattleScreen, enemy: bool, kind: PokemonType) {
    let target = side(b, !enemy);
    target.conversion_type1 = Some(kind);
    target.conversion_type2 = Some(kind);
}

fn fight(b: &mut BattleScreen) -> Vec<String> {
    let mut a = BattleInput::none(); a.a = true;
    for _ in 0..4 {
        b.update_frame(a);
        if let BattlePhase::ShowingText { messages, .. } = &b.phase { return messages.clone(); }
    }
    panic!("FIGHT did not resolve: {:?}", b.phase);
}

fn drain(b: &mut BattleScreen, id: MoveId, enemy: bool) -> usize {
    let mut serials = Vec::new();
    for frame in 0..4000 {
        if let Some((serial, actual, player)) = b.presentation.current_move() {
            if actual == id && player != enemy && !serials.contains(&serial) { serials.push(serial); }
        }
        if b.presentation.waiting { b.complete_move_presentation(); }
        if matches!(b.phase, BattlePhase::PlayerMenu) { return serials.len(); }
        let mut a = BattleInput::none(); a.a = frame % 2 == 0;
        b.update_frame(a);
    }
    panic!("presentation did not finish: {:?}", b.phase);
}

#[test]
fn immune_sequences_do_not_draw_hit_counts_or_repeat_the_failed_action() {
    for id in MOVES.into_iter().filter(|id| !matches!(id, MoveId::PinMissile | MoveId::Twineedle)) {
        let (kind, single) = match id {
            MoveId::Bonemerang => (PokemonType::Flying, MoveId::Earthquake),
            MoveId::DoubleKick => (PokemonType::Ghost, MoveId::Submission),
            _ => (PokemonType::Ghost, MoveId::Tackle),
        };
        for enemy in [false, true] {
            for doll in [false, true] {
                for raging in [false, true] {
                    let mut control = setup(single, enemy, 5, doll, false);
                    make_immune(&mut control, enemy, kind);
                    fight(&mut control);
                    let control_draws = control.link_rng.as_ref().unwrap().consumed();
                    assert_eq!(drain(&mut control, single, enemy), 1);
                    let mut b = setup(id, enemy, 5, doll, raging);
                    make_immune(&mut b, enemy, kind);
                    let hp = side(&mut b, !enemy).active_mon().hp;
                    let messages = fight(&mut b);
                    assert_eq!(side(&mut b, !enemy).active_mon().hp, hp);
                    assert_eq!(side(&mut b, !enemy).substitute_hp, if doll { 200 } else { 0 });
                    assert_eq!(side(&mut b, !enemy).stat_stages.attack, 0);
                    assert!(messages.iter().any(|s| s.contains("doesn't affect")), "{messages:?}");
                    assert!(messages.iter().all(|s| !s.starts_with("Hit ") && !s.contains("RAGE is building")));
                    // Compare with an equally immune single-hit move. The overall
                    // damage/accuracy RNG pipeline is a separate fidelity issue.
                    assert_eq!(b.link_rng.as_ref().unwrap().consumed(), control_draws,
                        "{id:?} enemy={enemy} doll={doll} raging={raging}");
                    assert_eq!(drain(&mut b, id, enemy), 1, "{id:?} enemy={enemy}");
                }
            }
        }
    }
}

#[test]
fn ordinary_misses_still_have_one_failed_action_and_no_hit_count() {
    for id in MOVES {
        for enemy in [false, true] {
            for doll in [false, true] {
                let mut b = setup(id, enemy, 5, doll, true);
                b.link_rng = Some(LinkRng::new([255; 10]));
                let hp = side(&mut b, !enemy).active_mon().hp;
                let messages = fight(&mut b);
                assert_eq!(side(&mut b, !enemy).active_mon().hp, hp);
                assert_eq!(side(&mut b, !enemy).stat_stages.attack, 0);
                assert!(messages.iter().any(|s| s.contains("missed")), "{messages:?}");
                assert!(messages.iter().all(|s| !s.starts_with("Hit ") && !s.contains("RAGE is building")));
                assert_eq!(drain(&mut b, id, enemy), 1);
            }
        }
    }
}

#[test]
fn an_immune_next_turn_cannot_reuse_the_previous_hit_sequence() {
    for enemy in [false, true] {
        for doll in [false, true] {
            let mut b = setup(MoveId::FuryAttack, enemy, 5, doll, false);
            fight(&mut b);
            assert_eq!(drain(&mut b, MoveId::FuryAttack, enemy), 5);
            make_immune(&mut b, enemy, PokemonType::Ghost);
            reset_rng(&mut b, 5);
            let hp = side(&mut b, !enemy).active_mon().hp;
            let messages = fight(&mut b);
            assert_eq!(side(&mut b, !enemy).active_mon().hp, hp);
            assert!(messages.iter().all(|s| !s.starts_with("Hit ")));
            assert_eq!(drain(&mut b, MoveId::FuryAttack, enemy), 1);
        }
    }
}

#[test]
fn successful_multi_hit_moves_still_present_every_strike() {
    for (index, id) in MOVES.into_iter().enumerate() {
        for enemy in [false, true] {
            for count in 2..=if index < 7 { 5 } else { 2 } {
                let mut b = setup(id, enemy, count, false, false);
                let messages = fight(&mut b);
                assert!(messages.iter().any(|s| s.starts_with("Hit ")), "{messages:?}");
                assert_eq!(drain(&mut b, id, enemy), usize::from(count));
            }
        }
    }
}
