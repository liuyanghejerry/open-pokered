//! Battle-item fidelity tests against the original disassembly
//! (`engine/items/item_effects.asm`): the trainer-blocked ball throw
//! (`ThrowBallAtTrainerMon`), the party+box-full throw refusal
//! (`BoxFullCannotThrowBall`), and in-battle PP restore
//! (`ItemUsePPRestore` — no `wIsInBattle` guard in the original).

use super::{BattleInput, BattlePhase, BattleScreen};
use crate::battle::BallAnimOutcome;
use crate::battle::BattleAnimEvent;
use crate::pokemon::stats::{create_pokemon, create_pokemon_with_moves};
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;

fn input(down: bool, a: bool) -> BattleInput {
    BattleInput {
        down,
        a,
        ..BattleInput::none()
    }
}

fn back_input() -> BattleInput {
    BattleInput {
        b: true,
        ..BattleInput::none()
    }
}

fn press_b(battle: &mut BattleScreen) {
    battle.update_frame(back_input());
}

/// Open the in-battle bag (main menu cursor Down → BAG → A) and select the
/// first (only) item with A.
fn use_first_bag_item(battle: &mut BattleScreen) {
    battle.update_frame(input(true, false)); // → BAG
    battle.update_frame(input(false, true)); // open bag
    battle.update_frame(input(false, true)); // select the item
}

fn bag_quantity(battle: &BattleScreen, item: ItemId) -> u32 {
    battle
        .player_bag
        .items()
        .iter()
        .find(|(id, _)| *id == item)
        .map(|(_, q)| *q)
        .unwrap_or(0)
}

fn current_message(battle: &BattleScreen) -> String {
    battle.current_message.clone().unwrap_or_default()
}

#[test]
fn potion_from_filtered_battle_bag_preserves_key_items() {
    for is_wild in [false, true] {
        let mut mon = create_pokemon(Species::Bulbasaur, 20, [0x9A, 0x78]).unwrap();
        mon.hp -= 10;
        let player = vec![mon];
        let enemy = vec![create_pokemon(Species::Rattata, 5, [0x9A, 0x78]).unwrap()];
        let mut battle = BattleScreen::from_parties(is_wild, &player, &enemy, None);
        battle.player_bag.add_item(ItemId::HelixFossil, 1).unwrap();
        battle.player_bag.add_item(ItemId::Hm01, 1).unwrap();
        battle.player_bag.add_item(ItemId::Potion, 2).unwrap();
        battle.phase = BattlePhase::PlayerMenu;
        use_first_bag_item(&mut battle);
        assert!(matches!(battle.phase, BattlePhase::ItemTargetSelect { item_id: ItemId::Potion }));
        battle.update_frame(input(false, true));
        assert_eq!(bag_quantity(&battle, ItemId::HelixFossil), 1);
        assert_eq!(bag_quantity(&battle, ItemId::Hm01), 1);
        assert_eq!(bag_quantity(&battle, ItemId::Potion), 1);
    }
}

// ── ThrowBallAtTrainerMon (item_effects.asm:2292-2306) ─────────────────────

/// In a trainer battle, throwing a ball plays the toss-only animation, prints
/// the two blocked texts, and CONSUMES the ball (`jr RemoveUsedItem`).
#[test]
fn trainer_ball_is_blocked_consumes_ball_and_animates() {
    let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(false, &player, &enemy, None);
    battle.player_bag.add_item(ItemId::HelixFossil, 1).unwrap();
    battle.player_bag.add_item(ItemId::PokeBall, 2).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    use_first_bag_item(&mut battle);

    // The ball is spent: 2 → 1.
    assert_eq!(bag_quantity(&battle, ItemId::PokeBall), 1);
    assert_eq!(bag_quantity(&battle, ItemId::HelixFossil), 1);
    // TOSS_ANIM: the $10 toss-only choreography, no shakes.
    assert!(matches!(
        battle.take_anim_event(),
        Some(BattleAnimEvent::Ball {
            outcome: BallAnimOutcome::Dodged,
            shakes: 0,
            ..
        })
    ));
    // _ThrowBallAtTrainerMonText1.
    assert!(
        current_message(&battle).contains("blocked the BALL"),
        "expected the trainer-blocked text, got {:?}",
        current_message(&battle)
    );
    // Nothing was caught and the battle continues.
    assert!(battle.captured_mon.is_none());
}

// ── BoxFullCannotThrowBall (item_effects.asm:118-137) ──────────────────────

/// A full party + full box refuses the throw BEFORE anything is consumed —
/// previously the port let the throw happen and the settlement silently
/// discarded the caught mon on a full box.
#[test]
fn full_party_and_box_refuses_ball_without_consuming() {
    let player: Vec<_> = (0..6)
        .map(|_| create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap())
        .collect();
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.player_box_full = true;
    battle.player_bag.add_item(ItemId::MasterBall, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    use_first_bag_item(&mut battle);

    assert!(
        current_message(&battle).contains("#MON BOX"),
        "expected the box-full refusal, got {:?}",
        current_message(&battle)
    );
    assert_eq!(bag_quantity(&battle, ItemId::MasterBall), 1, "ball not consumed");
    assert!(battle.captured_mon.is_none(), "no capture on a full box");
}

/// The guard needs BOTH conditions: with party room, a full box must not
/// block the catch (the mon joins the party).
#[test]
fn full_box_alone_does_not_block_ball() {
    let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.player_box_full = true;
    battle.player_bag.add_item(ItemId::MasterBall, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    use_first_bag_item(&mut battle);

    assert!(
        battle.captured_mon.is_some(),
        "a Master Ball with party room must still capture"
    );
}

/// The check runs before the Safari branch in the original, so a full party +
/// box refuses even a Safari Ball.
#[test]
fn safari_ball_refused_when_party_and_box_full() {
    use crate::battle::menu::SafariMenuAction;
    use crate::battle::safari::SafariState;

    let player: Vec<_> = (0..6)
        .map(|_| create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap())
        .collect();
    let enemy = vec![create_pokemon(Species::NidoranF, 20, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.is_safari = true;
    battle.safari = Some(SafariState::new(150, 30));
    battle.player_box_full = true;
    let balls_before = battle.safari.as_ref().unwrap().balls;

    battle.resolve_safari_action(SafariMenuAction::Ball);

    assert!(
        current_message(&battle).contains("#MON BOX"),
        "expected the box-full refusal, got {:?}",
        current_message(&battle)
    );
    assert_eq!(
        battle.safari.as_ref().unwrap().balls,
        balls_before,
        "a refused Safari Ball is not spent"
    );
}

// ── ItemUsePPRestore in battle (item_effects.asm:1954-2117) ────────────────

fn pp_mon() -> crate::battle::state::Pokemon {
    create_pokemon_with_moves(
        Species::Rattata,
        20,
        [0x98, 0x88],
        [MoveId::Tackle, MoveId::TailWhip, MoveId::None, MoveId::None],
    )
    .unwrap()
}

/// (Max) Ether: party pick → move menu → +10 PP (capped at max) on the chosen
/// move, item consumed, "PP was restored."
#[test]
fn ether_in_battle_restores_selected_move() {
    let player = vec![pp_mon()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.battle_state.as_mut().unwrap().player.party[0].pp[0] = 1;
    battle.player_bag.add_item(ItemId::Ether, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    use_first_bag_item(&mut battle); // → ItemTargetSelect on the Ether
    assert!(matches!(battle.phase, BattlePhase::ItemTargetSelect { .. }));
    battle.update_frame(input(false, true)); // A on party member 0
    assert!(
        matches!(battle.phase, BattlePhase::ItemMoveSelect { .. }),
        "an Ether must enter the per-move selection, got {:?}",
        battle.phase
    );
    battle.update_frame(input(false, true)); // A on move row 0

    assert_eq!(
        battle.battle_state.as_ref().unwrap().player.party[0].pp[0],
        11,
        "Ether restores +10 PP (1 → 11)"
    );
    assert_eq!(bag_quantity(&battle, ItemId::Ether), 0, "the Ether is consumed");
    assert!(
        current_message(&battle).contains("PP was restored"),
        "expected _PPRestoredText, got {:?}",
        current_message(&battle)
    );
}

/// A 0-PP move row is selectable in the item menu (no FIGHT-menu NoPP veto) —
/// that is exactly the row that wants restoring.
#[test]
fn ether_in_battle_can_target_zero_pp_move() {
    let player = vec![pp_mon()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.battle_state.as_mut().unwrap().player.party[0].pp[0] = 0;
    battle.player_bag.add_item(ItemId::Ether, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    use_first_bag_item(&mut battle);
    battle.update_frame(input(false, true)); // party member 0
    battle.update_frame(input(false, true)); // move row 0 (0 PP)

    assert_eq!(
        battle.battle_state.as_ref().unwrap().player.party[0].pp[0],
        10,
        "a 0-PP move is restored to 10, not vetoed"
    );
}

/// A full-PP target prints "It won't have any effect." and keeps the item
/// (`.noEffect` → `ItemUseNoEffect`, no `RemoveUsedItem`).
#[test]
fn ether_no_effect_on_full_pp_keeps_item() {
    let player = vec![pp_mon()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.player_bag.add_item(ItemId::Ether, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    use_first_bag_item(&mut battle);
    battle.update_frame(input(false, true)); // party member 0
    battle.update_frame(input(false, true)); // move row 0 (full PP)

    assert_eq!(bag_quantity(&battle, ItemId::Ether), 1, "item kept on no effect");
    assert!(
        current_message(&battle).contains("won't have any"),
        "expected ItemUseNoEffect text, got {:?}",
        current_message(&battle)
    );
}

/// Elixirs skip MoveSelectionMenu entirely (`.useElixir`) and restore every
/// move at once.
#[test]
fn elixir_in_battle_restores_all_moves_without_move_menu() {
    let player = vec![pp_mon()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    {
        let mon = &mut battle.battle_state.as_mut().unwrap().player.party[0];
        mon.pp[0] = 1;
        mon.pp[1] = 2;
    }
    battle.player_bag.add_item(ItemId::Elixer, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    use_first_bag_item(&mut battle);
    battle.update_frame(input(false, true)); // A on party member 0

    // Straight to the result text — never the per-move menu.
    assert!(
        !matches!(battle.phase, BattlePhase::ItemMoveSelect { .. }),
        "an Elixir must skip the move menu, got {:?}",
        battle.phase
    );
    let mon = &battle.battle_state.as_ref().unwrap().player.party[0];
    assert_eq!(mon.pp[0], 11, "move 0 restored +10");
    assert_eq!(mon.pp[1], 12, "move 1 restored +10");
    assert_eq!(bag_quantity(&battle, ItemId::Elixer), 0);
}

/// B in the Ether move menu loops back to the party pick (`.chooseMon`), and
/// B there backs out to the main menu.
#[test]
fn ether_move_menu_b_returns_to_party_select() {
    let player = vec![pp_mon()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.battle_state.as_mut().unwrap().player.party[0].pp[0] = 1;
    battle.player_bag.add_item(ItemId::Ether, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    use_first_bag_item(&mut battle);
    battle.update_frame(input(false, true)); // party member 0 → ItemMoveSelect
    press_b(&mut battle);
    assert!(
        matches!(battle.phase, BattlePhase::ItemTargetSelect { .. }),
        "B in the move menu returns to the party pick, got {:?}",
        battle.phase
    );
    assert_eq!(bag_quantity(&battle, ItemId::Ether), 1, "item untouched");
    press_b(&mut battle);
    assert_eq!(battle.phase, BattlePhase::PlayerMenu);
}
