//! ItemUseBall uses the original encounter's actual catch rate, not the saved
//! catch byte copied by Transform (original wram.asm:1196-1199,
//! core.asm:6107-6121, transform.asm:57-71, item_effects.asm:300-303,329).
//! These are canonical-effect + public Bag caller tests, not a Game playthrough.
use super::*;
use capture::{try_capture_with_rng, CaptureContext, CaptureResult};
use crate::pokemon::stats::{create_pokemon, create_pokemon_with_moves};
use dotzuki_engine::battle::{
    rng::{BattleRng, ScriptedRng},
    stack::StackDriver,
    BattleAction, BattlerRef,
};
use pokered_rules::{runtime, PokeredRules};

fn a() -> BattleInput {
    BattleInput { a: true, ..BattleInput::none() }
}

fn mon(species: Species, move_: MoveId) -> state::Pokemon {
    create_pokemon_with_moves(
        species, 30, [0x99, 0x88],
        [move_, MoveId::None, MoveId::None, MoveId::None],
    ).unwrap()
}

fn screen(player: state::Pokemon, enemy: state::Pokemon) -> BattleScreen {
    let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
    // These unit fixtures begin at the menu; intro rendering is outside scope.
    screen.phase = BattlePhase::PlayerMenu;
    screen
}

/// Resolve actual moves through the production canonical handlers/adapter.
/// Scripted zero bytes make the accuracy checks connect without seed searches;
/// neither TRANSFORMED nor transform_catch_rate is manufactured by the fixture.
fn canonical_turn(screen: &mut BattleScreen, player_move: MoveId, enemy_move: MoveId) {
    screen.sync_player_context();
    let bs = screen.battle_state.as_mut().unwrap();
    bs.player.selected_move = player_move;
    bs.player.selected_move_index = 0;
    bs.enemy.selected_move = enemy_move;
    bs.enemy.selected_move_index = 0;
    let entry = (
        bs.player.battle_status1, bs.player.battle_status2,
        bs.player.has_status3(state::status3::TRANSFORMED), player_move,
    );
    let (mut engine, mut effects) = runtime::engine_state_from_legacy(bs);
    pokered_rules::install_canonical();
    pokered_rules::clear_current_moves();
    pokered_rules::clear_last_move_live();
    pokered_rules::set_current_move(BattlerRef::PLAYER, *MoveData::get(player_move).unwrap());
    pokered_rules::set_current_move(BattlerRef::OPPONENT, *MoveData::get(enemy_move).unwrap());
    let (_, log) = StackDriver::execute_turn_logged(
        &PokeredRules, &mut engine, &mut effects,
        [BattleAction::Fight { move_: player_move }, BattleAction::Fight { move_: enemy_move }],
        &mut ScriptedRng::new(vec![0; 64]),
    );
    let messages = runtime::translate_turn(&log, &engine, &effects);
    runtime::apply_engine_to_legacy(bs, &engine, &effects);
    BattleScreen::consume_player_pp_charge(bs, &mut engine, &mut effects, entry, false);
    screen.sync_display_from_state();
    screen.show_text_then(messages, BattlePhase::PlayerMenu);
    for _ in 0..600 {
        if screen.phase == BattlePhase::PlayerMenu {
            // The unit fixture has no renderer consuming the turn's animations.
            while screen.take_anim_event().is_some() {}
            return;
        }
        assert!(matches!(screen.phase, BattlePhase::ShowingText { .. }));
        screen.update_frame(a());
    }
    panic!("canonical turn narration did not finish");
}

fn rng_tail(rng: &runtime::StdBattleRng) -> [u8; 8] {
    let mut rng = rng.clone();
    core::array::from_fn(|_| rng.next_u8())
}

/// The literal rate is an independent original-ASM expectation (Ditto35,
/// Pidgeot45, Rattata255), never a call to the new production lookup/getter.
/// Every seed in the declared matrix runs; there is no seed-fitting break.
fn check_capture_matrix(base: &BattleScreen, literal_rate: u8, copied_rate: Option<u8>) {
    let original = *base.battle_state.as_ref().unwrap().enemy.party_mon(0);
    let transformed = base.battle_state.as_ref().unwrap().enemy.has_status3(state::status3::TRANSFORMED);
    let mut old_rate_differs = false;
    for ball in [ItemId::PokeBall, ItemId::GreatBall, ItemId::UltraBall, ItemId::MasterBall] {
        let mut seen = [false; 2];
        for low_hp in [false, true] {
            for seed in 0..256 {
                let mut actual = base.clone();
                actual.rng = runtime::StdBattleRng::from_seed(seed);
                let enemy = actual.battle_state.as_mut().unwrap().enemy.active_mon_mut();
                enemy.hp = if low_hp { 1 } else { enemy.max_hp };
                let (hp, max_hp, status) = (enemy.hp, enemy.max_hp, enemy.status);
                let context = CaptureContext {
                    ball, wild_max_hp: max_hp, wild_current_hp: hp,
                    wild_catch_rate: literal_rate, wild_status: status,
                };
                let mut expected_rng = actual.rng.clone();
                let expected = try_capture_with_rng(&context, &mut || expected_rng.next_u8());
                if let Some(copied_rate) = copied_rate {
                    let mut old_rng = actual.rng.clone();
                    let old = try_capture_with_rng(
                        &CaptureContext { wild_catch_rate: copied_rate, ..context },
                        &mut || old_rng.next_u8(),
                    );
                    old_rate_differs |= old != expected || rng_tail(&old_rng) != rng_tail(&expected_rng);
                }
                actual.player_bag.add_item(ball, 2).unwrap();
                assert_eq!(actual.phase, BattlePhase::PlayerMenu);
                actual.update_frame(BattleInput { down: true, ..BattleInput::none() });
                actual.update_frame(a());
                assert_eq!(actual.phase, BattlePhase::BagSelect);
                actual.update_frame(a()); // actual public Bag → ItemUseBall caller
                let caught = expected == CaptureResult::Captured;
                seen[usize::from(caught)] = true;
                assert_eq!(actual.player_bag.items(), &[(ball, 1)], "ball={ball:?}, seed={seed}");
                let (outcome, shakes) = match expected {
                    CaptureResult::Captured => (BallAnimOutcome::Caught, 3),
                    CaptureResult::Failed { shakes } => (BallAnimOutcome::BrokeFree, shakes),
                };
                assert_eq!(actual.take_anim_event(), Some(BattleAnimEvent::Ball { ball, outcome, shakes }),
                    "ball={ball:?}, seed={seed}, low_hp={low_hp}");
                assert!(actual.take_anim_event().is_none());
                let expected_mon = caught.then(|| {
                    // Preserve the ROM bug: ANY transformed wild is rebuilt as
                    // Ditto after capture, including a Mirror Move user.
                    let mut mon = if transformed {
                        create_pokemon(Species::Ditto, original.level, original.dv_bytes).unwrap()
                    } else { original };
                    mon.hp = hp;
                    mon.status = status;
                    mon.ot_id = actual.player_id;
                    mon
                });
                assert_eq!(actual.captured_mon, expected_mon, "ball={ball:?}, seed={seed}");
                assert!(matches!(&actual.phase, BattlePhase::ShowingText { next_phase, .. }
                    if matches!(**next_phase, BattlePhase::BattleOver { .. }) == caught));
                if !caught {
                    assert!(matches!(&actual.phase, BattlePhase::ShowingText { next_phase, .. }
                        if **next_phase == BattlePhase::EnemyFreeTurnAfterItem));
                }
                assert_eq!(rng_tail(&actual.rng), rng_tail(&expected_rng),
                    "capture RNG tail: ball={ball:?}, seed={seed}, low_hp={low_hp}");
                assert_eq!(serde_json::to_value(&actual.rng).unwrap(), serde_json::to_value(&expected_rng).unwrap());
                let bs = actual.battle_state.as_ref().unwrap();
                assert_eq!(bs.enemy.party_mon(0).species, original.species);
                if let Some(copied_rate) = copied_rate {
                    assert_eq!(bs.enemy.transform_catch_rate, Some(copied_rate), "working copy is retained");
                }
            }
        }
        assert!(seen[1], "matrix must include catches for {ball:?}");
        if ball != ItemId::MasterBall { assert!(seen[0], "matrix must include escapes for {ball:?}"); }
    }
    if copied_rate.is_some_and(|rate| rate != literal_rate) {
        assert!(old_rate_differs, "the full matrix must distinguish the old copied-rate caller");
    }
}

#[test]
fn transformed_ditto_keeps_actual_rate_when_copying_high_or_low_saved_catch_bytes() {
    let mut gyarados = mon(Species::Magikarp, MoveId::Splash);
    crate::battle::settlement::evolution::apply_evolution(&mut gyarados, Species::Gyarados);
    assert_eq!(gyarados.catch_rate, Some(255), "natural evolution retains Magikarp's saved byte");
    let snorlax = mon(Species::Snorlax, MoveId::Splash);
    assert_eq!(snorlax.catch_rate, Some(25));
    for (player, copied_rate) in [(gyarados, 255), (snorlax, 25)] {
        let ditto = mon(Species::Ditto, MoveId::Transform);
        let mut battle = screen(player, ditto);
        canonical_turn(&mut battle, MoveId::Splash, MoveId::Transform);
        let bs = battle.battle_state.as_ref().unwrap();
        assert!(bs.enemy.has_status3(state::status3::TRANSFORMED));
        assert_eq!(bs.enemy.active_mon().species, player.species);
        assert_eq!(bs.enemy.party_mon(0).species, Species::Ditto);
        assert_eq!(bs.enemy.transform_catch_rate, Some(copied_rate));
        check_capture_matrix(&battle, 35, Some(copied_rate));
    }
}

#[test]
fn untransformed_capture_keeps_the_original_encounter_rate() {
    let battle = screen(mon(Species::Magikarp, MoveId::Splash), mon(Species::Rattata, MoveId::Tackle));
    assert!(!battle.battle_state.as_ref().unwrap().enemy.has_status3(state::status3::TRANSFORMED));
    check_capture_matrix(&battle, 255, None);
}

#[test]
fn mirror_move_transform_uses_original_non_ditto_rate_but_still_captures_as_ditto() {
    let mut player = mon(Species::Ditto, MoveId::Transform);
    let enemy = mon(Species::Pidgeot, MoveId::MirrorMove);
    // Explicit unit speed fixture: Transform must execute before Mirror Move
    // so the latter copies this actual move, without seeding last-move state.
    player.speed = enemy.speed + 1;
    let mut battle = screen(player, enemy);
    canonical_turn(&mut battle, MoveId::Transform, MoveId::MirrorMove);
    let bs = battle.battle_state.as_ref().unwrap();
    assert!(bs.player.has_status3(state::status3::TRANSFORMED));
    assert!(bs.enemy.has_status3(state::status3::TRANSFORMED));
    assert_eq!(bs.enemy.party_mon(0).species, Species::Pidgeot);
    assert_eq!(bs.enemy.transform_catch_rate, Some(45));
    check_capture_matrix(&battle, 45, Some(45));
}
