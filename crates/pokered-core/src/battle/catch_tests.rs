//! Integration tests for the capture → party flow. A successful ball throw
//! must record the caught wild Pokémon in `BattleScreen::captured_mon` so the
//! app layer can move it into the party (or a PC box) after the battle.

use super::{BallAnimOutcome, BattleInput, BattlePhase, BattleScreen};
use crate::battle::BattleAnimEvent;
use crate::battle::settlement::BattleOutcome;
use crate::game_state::{GameScreen, ScreenAction};
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

/// Follow the actual BAG/ball/text/terminal path, not a manually supplied
/// Captured settlement. The old BattleOver phase misclassified this as Win,
/// so the existing writeback/script tests never exercised the faulty handoff.
#[test]
fn master_ball_capture_finishes_with_captured_settlement() {
    let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
    let enemy = vec![create_pokemon(Species::Snorlax, 30, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    let before = battle.battle_state.as_ref().unwrap().player.party.clone();
    battle.player_bag.add_item(ItemId::MasterBall, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;
    battle.update_frame(input(true, false));
    battle.update_frame(input(false, true));
    battle.update_frame(input(false, true));
    assert_eq!(battle.captured_mon.as_ref().unwrap().species, Species::Snorlax);
    assert!(!battle.player_bag.has_item(ItemId::MasterBall, 1));
    let mut transitioned = false;
    for frame in 0..2000 {
        if matches!(battle.update_frame(input(false, frame % 2 == 0)),
            ScreenAction::Transition(GameScreen::Overworld)) {
            transitioned = true;
            break;
        }
    }
    assert!(transitioned, "the actual capture text must finish within its fixed budget");
    let settlement = battle.settlement.as_ref().unwrap();
    assert_eq!(settlement.outcome, BattleOutcome::Captured);
    assert_eq!(settlement.money_gained, 0);
    assert_eq!(settlement.money_lost, 0);
    assert!(settlement.evolutions.is_empty());
    for (before, after) in before.iter().zip(&battle.battle_state.as_ref().unwrap().player.party) {
        assert_eq!(before.species, after.species);
        assert_eq!(before.level, after.level);
        assert_eq!(before.total_exp, after.total_exp);
        assert_eq!(before.hp, after.hp);
        assert_eq!(before.pp, after.pp);
    }
}

#[test]
fn uncaught_battle_terminal_outcomes_stay_distinct() {
    for (won, escaped, expected) in [
        (true, false, BattleOutcome::Win),
        (false, true, BattleOutcome::Escaped),
        (false, false, BattleOutcome::Loss),
    ] {
        let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
        let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
        let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
        assert!(battle.captured_mon.is_none());
        battle.phase = BattlePhase::BattleOver { won, escaped, wait_frames: 0 };
        assert!(matches!(battle.update_frame(BattleInput::none()),
            ScreenAction::Transition(GameScreen::Overworld)));
        assert_eq!(battle.settlement.as_ref().unwrap().outcome, expected);
    }
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
        Some(1),
        "the ghost-dodged ball is not consumed"
    );
    assert!(battle.captured_mon.is_none());
}

#[test]
fn revealed_restless_soul_dodges_every_ball_but_normal_marowak_is_catchable() {
    for spirit in [true, false] {
        for ball in [ItemId::PokeBall, ItemId::GreatBall, ItemId::UltraBall, ItemId::MasterBall] {
            let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
            let enemy = vec![create_pokemon(Species::Marowak, 30, [0x9A, 0x78]).unwrap()];
            let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
            battle.ghost_marowak_reveal = spirit;
            battle.ghost_marowak_unveiled = spirit;
            battle.player_bag.add_item(ball, 1).unwrap();
            battle.phase = BattlePhase::PlayerMenu;
            battle.update_frame(input(true, false));
            battle.update_frame(input(false, true));
            battle.update_frame(input(false, true));
            if spirit {
                assert_eq!(battle.take_anim_event(), Some(BattleAnimEvent::Ball {
                    ball, shakes: 0, outcome: BallAnimOutcome::Dodged,
                }));
                assert!(battle.captured_mon.is_none());
                assert!(battle.player_bag.has_item(ball, 1));
                assert!(!battle.is_ghost, "revealed spirit remains attackable");
            } else if ball == ItemId::MasterBall {
                assert_eq!(battle.captured_mon.as_ref().unwrap().species, Species::Marowak);
            }
        }
    }
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
