//! Integration tests for the capture → party flow. A successful ball throw
//! must record the caught wild Pokémon in `BattleScreen::captured_mon` so the
//! app layer can move it into the party (or a PC box) after the battle.

use super::{BallAnimOutcome, BattleInput, BattlePhase, BattleScreen};
use crate::battle::BattleAnimEvent;
use crate::pokemon::stats::create_pokemon;
use pokered_data::items::ItemId;
use pokered_data::species::Species;

fn input(down: bool, a: bool) -> BattleInput {
    BattleInput {
        down,
        a,
        ..BattleInput::none()
    }
}

/// A Master Ball always captures, so this is deterministic: navigate the battle
/// menu to the bag, throw the ball, and confirm the wild mon is recorded.
#[test]
fn master_ball_throw_records_captured_mon() {
    let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.player_bag.add_item(ItemId::MasterBall, 1).unwrap();

    // Skip the intro animation; the main menu starts on FIGHT (row 0, col 0).
    battle.phase = BattlePhase::PlayerMenu;

    // Down -> BAG (row 1, col 0); A -> open the bag; A -> select + throw the
    // (only) Master Ball.
    battle.update_frame(input(true, false));
    battle.update_frame(input(false, true));
    assert_eq!(
        format!("{:?}", battle.phase).split(' ').next(),
        Some("BagSelect"),
        "A on BAG should open the in-battle bag"
    );
    battle.update_frame(input(false, true));

    let caught = battle
        .captured_mon
        .as_ref()
        .expect("a Master Ball throw must record the caught Pokémon");
    assert_eq!(caught.species, Species::Pidgey);
}

/// A thrown ball queues the non-move animation event carrying the ball id,
/// the shake count and the outcome (`wPokeBallAnimData` in the original), so
/// the frontend can stage the toss → poof → shakes choreography.
#[test]
fn ball_throw_queues_ball_anim_event() {
    let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.player_bag.add_item(ItemId::MasterBall, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    battle.update_frame(input(true, false)); // -> BAG
    battle.update_frame(input(false, true)); // A: open bag
    battle.update_frame(input(false, true)); // A: throw the Master Ball

    // A catch always shows 3 shakes ($43 in the original).
    let event = battle
        .take_anim_event()
        .expect("a ball throw must queue a Ball anim event");
    assert_eq!(
        event,
        BattleAnimEvent::Ball {
            ball: ItemId::MasterBall,
            shakes: 3,
            outcome: BallAnimOutcome::Caught,
        }
    );
    assert!(battle.take_anim_event().is_none(), "exactly one event");

    // The "used BALL!" line precedes the result text (ItemUseText00).
    let msg = battle.current_message.clone().unwrap_or_default();
    assert!(
        msg.contains(" used") && msg.contains("MASTER BALL!"),
        "first message should be the item-use line, got {msg:?}"
    );

    // A successful catch ends the battle (won) once the text finishes.
    match battle.phase {
        BattlePhase::ShowingText { ref next_phase, .. } => assert!(
            matches!(**next_phase, BattlePhase::BattleOver { won: true, .. }),
            "a catch must end the battle after the text, got {next_phase:?}"
        ),
        other => panic!("expected ShowingText after a catch, got {other:?}"),
    }
}

/// A ball thrown at an unidentified ghost is dodged ($10): toss-only event,
/// ball NOT consumed, battle continues.
#[test]
fn ghost_ball_throw_queues_dodged_event() {
    let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.is_ghost = true;
    battle.player_bag.add_item(ItemId::PokeBall, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    battle.update_frame(input(true, false)); // -> BAG
    battle.update_frame(input(false, true)); // A: open bag
    battle.update_frame(input(false, true)); // A: throw the ball

    let event = battle
        .take_anim_event()
        .expect("a ghost ball throw must queue a Ball anim event");
    assert_eq!(
        event,
        BattleAnimEvent::Ball {
            ball: ItemId::PokeBall,
            shakes: 0,
            outcome: BallAnimOutcome::Dodged,
        }
    );
    assert_eq!(
        battle
            .player_bag
            .items()
            .iter()
            .find(|(id, _)| *id == ItemId::PokeBall)
            .map(|(_, q)| *q),
        None,
        "the ghost-dodged ball is consumed"
    );
    assert!(battle.captured_mon.is_none());
}

/// A successful X-stat item queues the XSTATITEM_ANIM event
/// (ItemUseXStat → StatModifierUpEffect in the original).
#[test]
fn x_stat_item_queues_xstat_anim_event() {
    let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.player_bag.add_item(ItemId::XAttack, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    battle.update_frame(input(true, false)); // -> BAG
    battle.update_frame(input(false, true)); // A: open bag
    battle.update_frame(input(false, true)); // A: use X Attack

    let event = battle
        .take_anim_event()
        .expect("an X-stat use must queue an XStatItem anim event");
    assert_eq!(event, BattleAnimEvent::XStatItem);
    assert!(battle.take_anim_event().is_none(), "exactly one event");
}

/// The bag must be usable: a battle with no balls can't capture. (Guards the
/// `player_bag` population fix — an empty bag shows "No items!".)
#[test]
fn empty_bag_yields_no_capture() {
    let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.phase = BattlePhase::PlayerMenu;

    battle.update_frame(input(true, false)); // -> BAG
    battle.update_frame(input(false, true)); // A: bag is empty -> "No items!"
    battle.update_frame(input(false, true));

    assert!(battle.captured_mon.is_none());
}

#[test]
fn revealed_restless_soul_dodges_master_ball_but_regular_marowak_is_catchable() {
    for tower in [true, false] {
        let player = vec![create_pokemon(Species::Rattata, 30, [0x9A, 0x78]).unwrap()];
        let enemy = vec![create_pokemon(Species::Marowak, 30, [0x9A, 0x78]).unwrap()];
        let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
        battle.map_id = if tower { pokered_data::maps::MapId::PokemonTower6F as u8 }
            else { pokered_data::maps::MapId::Route10 as u8 };
        battle.player_bag.add_item(ItemId::MasterBall, 2).unwrap();
        battle.phase = BattlePhase::PlayerMenu;
        battle.update_frame(input(true, false));
        battle.update_frame(input(false, true));
        battle.update_frame(input(false, true));
        assert_eq!(battle.captured_mon.is_none(), tower);
        if tower {
            assert!(matches!(battle.take_anim_event(), Some(BattleAnimEvent::Ball {
                outcome: BallAnimOutcome::Dodged, .. })));
            assert!(matches!(&battle.phase, BattlePhase::ShowingText { next_phase, .. }
                if **next_phase == BattlePhase::EnemyFreeTurnAfterItem));
        }
        assert_eq!(battle.player_bag.items(), &[(ItemId::MasterBall, 1)]);
    }
}

fn capture_rng_open_bag(battle: &mut BattleScreen) {
    battle.phase = BattlePhase::PlayerMenu;
    battle.update_frame(input(true, false));
    battle.update_frame(input(false, true));
    assert_eq!(battle.phase, BattlePhase::BagSelect);
}

#[test]
fn capture_rng_owned_normal_ball_matches_stream_and_replays_snapshot() {
    use crate::battle::pokered_rules::runtime::StdBattleRng;
    use crate::snapshot::BattleSnapshot;
    use dotzuki_engine::battle::rng::BattleRng;
    let player = create_pokemon(Species::Rattata, 12, [0x9a, 0x78]).unwrap();
    let enemy = create_pokemon(Species::Rattata, 3, [0x9a, 0x78]).unwrap();
    let w = (enemy.max_hp as u32 * 255 / 12) / u32::from((enemy.hp / 4).max(1));
    assert!(w <= 255);
    let mut observed = [false; 2];
    for seed in 0..16 {
        let mut battle = BattleScreen::from_parties(true, &[player], &[enemy], None);
        battle.rng = StdBattleRng::from_seed(seed);
        let mut expected_rng = battle.rng.clone();
        let _rand1 = expected_rng.next_u8(); // Rattata catch-rate255 accepts every byte.
        let caught = u32::from(expected_rng.next_u8()) <= w;
        observed[usize::from(caught)] = true;
        battle.player_bag.add_item(ItemId::PokeBall, 2).unwrap();
        capture_rng_open_bag(&mut battle);
        let before = BattleSnapshot::capture(&battle);
        let mut expected = None;
        for _ in 0..4 {
            before.restore_into(&mut battle);
            battle.update_frame(input(false, true));
            assert_eq!(
                serde_json::to_value(&battle.rng).unwrap(),
                serde_json::to_value(&expected_rng).unwrap()
            );
            assert_eq!(battle.captured_mon.is_some(), caught);
            assert_eq!(battle.player_bag.items(), &[(ItemId::PokeBall, 1)]);
            assert!(
                matches!(&battle.phase, BattlePhase::ShowingText { next_phase, .. }
                if matches!(**next_phase, BattlePhase::BattleOver { .. }) == caught)
            );
            if !caught {
                assert!(
                    matches!(&battle.phase, BattlePhase::ShowingText { next_phase, .. }
                    if **next_phase == BattlePhase::EnemyFreeTurnAfterItem)
                );
            }
            let result = serde_json::to_value(BattleSnapshot::capture(&battle)).unwrap();
            if let Some(ref expected) = expected {
                assert_eq!(&result, expected);
            } else {
                expected = Some(result);
            }
        }
    }
    assert_eq!(
        observed,
        [true, true],
        "cover both successful and failed throws"
    );
}

#[test]
fn capture_rng_owned_master_ball_advances_once_and_consumes_once() {
    use crate::battle::pokered_rules::runtime::StdBattleRng;
    use dotzuki_engine::battle::rng::BattleRng;
    let player = create_pokemon(Species::Rattata, 12, [0x9a, 0x78]).unwrap();
    let enemy = create_pokemon(Species::Pidgey, 3, [0x9a, 0x78]).unwrap();
    let mut battle = BattleScreen::from_parties(true, &[player], &[enemy], None);
    battle.rng = StdBattleRng::from_seed(19);
    let mut expected = battle.rng.clone();
    expected.next_u8();
    battle.player_bag.add_item(ItemId::MasterBall, 2).unwrap();
    capture_rng_open_bag(&mut battle);
    battle.update_frame(input(false, true));
    assert_eq!(
        serde_json::to_value(&battle.rng).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    assert!(battle.captured_mon.is_some());
    assert_eq!(battle.player_bag.items(), &[(ItemId::MasterBall, 1)]);
}

#[test]
fn capture_rng_owned_refusal_and_dodge_do_not_draw() {
    use crate::battle::pokered_rules::runtime::StdBattleRng;
    for case in ["trainer", "full-box", "ghost", "restless-soul"] {
        let player = create_pokemon(Species::Rattata, 12, [0x9a, 0x78]).unwrap();
        let party = vec![player; if case == "full-box" { 6 } else { 1 }];
        let enemy = create_pokemon(Species::Marowak, 30, [0x9a, 0x78]).unwrap();
        let mut battle = BattleScreen::from_parties(case != "trainer", &party, &[enemy], None);
        battle.rng = StdBattleRng::from_seed(19);
        battle.player_box_full = case == "full-box";
        battle.is_ghost = case == "ghost";
        if case == "restless-soul" {
            battle.map_id = pokered_data::maps::MapId::PokemonTower6F as u8;
        }
        let expected = serde_json::to_value(&battle.rng).unwrap();
        battle.player_bag.add_item(ItemId::MasterBall, 2).unwrap();
        capture_rng_open_bag(&mut battle);
        battle.update_frame(input(false, true));
        assert_eq!(
            serde_json::to_value(&battle.rng).unwrap(),
            expected,
            "{case}"
        );
        assert!(battle.captured_mon.is_none(), "{case}");
        let qty = if case == "full-box" { 2 } else { 1 };
        assert_eq!(
            battle.player_bag.items(),
            &[(ItemId::MasterBall, qty)],
            "{case}"
        );
    }
}
