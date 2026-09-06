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

// ── In-battle full-slot learn prompt (learnmove.asm) ────────────────────────


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

fn staged_messages(battle: &BattleScreen) -> Vec<String> {
    if let BattlePhase::ShowingText { messages, .. } = &battle.phase {
        messages.clone()
    } else {
        battle.current_message.clone().map(|m| vec![m]).unwrap_or_default()
    }
}

fn current_message(battle: &BattleScreen) -> String {
    battle.current_message.clone().unwrap_or_default()
}

// ── ThrowBallAtTrainerMon (item_effects.asm:2292-2306) ─────────────────────

/// In a trainer battle, throwing a ball plays the toss-only animation, prints
/// the two blocked texts, and CONSUMES the ball (`jr RemoveUsedItem`).
#[test]
fn trainer_ball_is_blocked_consumes_ball_and_animates() {
    let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(false, &player, &enemy, None);
    battle.player_bag.add_item(ItemId::PokeBall, 2).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    use_first_bag_item(&mut battle);

    // The ball is spent: 2 → 1.
    assert_eq!(bag_quantity(&battle, ItemId::PokeBall), 1);
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

// ── ItemUsePokeDoll (item_effects.asm:1597-1602) ───────────────────────────

/// The Poké Doll flees a WILD battle ("Got away safely!", doll consumed).
#[test]
fn poke_doll_flees_wild_battle() {
    let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.player_bag.add_item(ItemId::PokeDoll, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    use_first_bag_item(&mut battle);

    assert_eq!(bag_quantity(&battle, ItemId::PokeDoll), 0, "doll consumed");
    assert!(
        staged_messages(&battle).join(" ").contains("Got away safely"),
        "expected the flee text, got {:?}",
        staged_messages(&battle)
    );
    // Dismiss the flee text (press edges alternate with release frames); the
    // battle then ends as an escape.
    for _ in 0..8 {
        battle.update_frame(input(false, true));
        battle.update_frame(BattleInput::none());
    }
    assert!(
        matches!(battle.phase, BattlePhase::BattleOver { escaped: true, .. }),
        "expected a battle escape, got {:?}",
        battle.phase
    );
}

/// In a TRAINER battle the doll falls through to ItemUseNotTime: the OAK
/// refusal text shows and the doll is NOT consumed.
#[test]
fn poke_doll_refused_in_trainer_battle() {
    let player = vec![create_pokemon(Species::Rattata, 10, [0x9A, 0x78]).unwrap()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(false, &player, &enemy, None);
    battle.player_name = Some("RED".to_string());
    battle.player_bag.add_item(ItemId::PokeDoll, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;

    use_first_bag_item(&mut battle);

    assert_eq!(
        bag_quantity(&battle, ItemId::PokeDoll),
        1,
        "doll must NOT be consumed in a trainer battle"
    );
    assert!(
        staged_messages(&battle).join(" ").contains("the time to use"),
        "expected the OAK refusal, got {:?}",
        staged_messages(&battle)
    );
    // Dismiss the refusal with B (A would re-engage the main menu); play then
    // returns to the menu with the doll intact.
    for _ in 0..20 {
        press_b(&mut battle);
        battle.update_frame(BattleInput::none());
    }
    assert_eq!(battle.phase, BattlePhase::PlayerMenu);
}
/// wrap_learn_prompt turns a queued blocked move into the TryingToLearn text +
/// a LearnMoveAsk phase; YES → the forget list; A on slot 0 replaces it and
/// the chain resumes into the wrapped phase. The old move is NOT silently
/// lost without the prompt.
#[test]
fn learn_move_chain_replaces_a_chosen_move() {
    use super::BattlePhase;
    let player = vec![create_pokemon_with_moves(
        Species::Pikachu,
        20,
        [0x9A, 0x78],
        [MoveId::Thundershock, MoveId::Growl, MoveId::TailWhip, MoveId::QuickAttack],
    )
    .unwrap()];
    let enemy = vec![create_pokemon(Species::Pidgey, 5, [0x9A, 0x78]).unwrap()];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.pending_learn_moves = vec![(0, MoveId::Thunderbolt)];

    let mut msgs = vec!["PIKACHU grew to level 21!".to_string()];
    let next = battle.wrap_learn_prompt(&mut msgs, BattlePhase::PlayerMenu);
    assert!(
        matches!(next, BattlePhase::LearnMoveAsk { .. }),
        "expected the learn prompt phase, got {:?}",
        next
    );
    assert!(
        msgs.iter().any(|m| m.contains("trying to learn")),
        "TryingToLearn text appended: {:?}",
        msgs
    );
    battle.phase = next;
    battle.post_text_transition();

    // NO first (default) → the abandon confirm; YES there → did-not-learn text
    // and resume into PlayerMenu with the moveset unchanged.
    battle.update_frame(input(false, true)); // A on default NO
    assert!(
        matches!(battle.phase, BattlePhase::LearnMoveGiveUpConfirm { .. }),
        "NO goes to the abandon confirm, got {:?}",
        battle.phase
    );
    battle.update_frame(input(false, true)); // A on default NO → back to ask
    assert!(matches!(battle.phase, BattlePhase::LearnMoveAsk { .. }));
    battle.update_frame(input(true, false)); // UP toggles to YES
    battle.update_frame(input(false, true)); // A on YES → the forget list
    assert!(matches!(battle.phase, BattlePhase::LearnMoveChoose { .. }));

    // Choose slot 0 (ThunderShock): "1, 2 and... Poof!" texts, then resume.
    battle.update_frame(input(false, true));
    assert!(
        matches!(battle.phase, BattlePhase::ShowingText { .. }),
        "expected replacement texts, got {:?}",
        battle.phase
    );
    if let Some(bs) = battle.battle_state.as_ref() {
        assert_eq!(bs.player.party[0].moves[0], MoveId::Thunderbolt);
        assert_eq!(bs.player.party[0].moves[3], MoveId::QuickAttack);
    }
    // Dismiss the texts (4 pages × wait+press); the battle resumes at the
    // wrapped phase. Stop at the FIRST neutral frame — further A presses
    // would fight with the freshly learned move.
    for _ in 0..40 {
        if battle.phase == BattlePhase::PlayerMenu {
            return;
        }
        battle.update_frame(input(false, true));
        if battle.phase == BattlePhase::PlayerMenu {
            return;
        }
        battle.update_frame(super::BattleInput::none());
    }
    assert_eq!(battle.phase, BattlePhase::PlayerMenu);
}
