//! `SwapMovesInMenu` (`engine/battle/core.asm:2618, 2762-2836`) regression tests.

use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;
use crate::snapshot::BattleSnapshot;

fn a() -> BattleInput {
    BattleInput {
        a: true,
        ..BattleInput::none()
    }
}
fn b() -> BattleInput {
    BattleInput {
        b: true,
        ..BattleInput::none()
    }
}
fn down() -> BattleInput {
    BattleInput {
        down: true,
        ..BattleInput::none()
    }
}
fn select() -> BattleInput {
    BattleInput {
        select: true,
        ..BattleInput::none()
    }
}

fn mon(species: Species, moves: [MoveId; 4]) -> state::Pokemon {
    create_pokemon_with_moves(species, 20, [0x9a, 0x78], moves).unwrap()
}

fn battle() -> BattleScreen {
    let mut player = mon(
        Species::Pikachu,
        [
            MoveId::Pound,
            MoveId::Tackle,
            MoveId::Growl,
            MoveId::Thundershock,
        ],
    );
    player.pp = [5, 6, 7, 8];
    player.pp_ups = [0, 1, 2, 3];
    let enemy = mon(
        Species::Weedle,
        [MoveId::PoisonSting, MoveId::StringShot, MoveId::None, MoveId::None],
    );
    let mut screen = BattleScreen::from_parties(false, &[player], &[enemy], None);
    screen.phase = BattlePhase::PlayerMenu;
    screen.rng = pokered_rules::runtime::StdBattleRng::from_seed(42);
    screen
}

#[test]
fn select_swap_swaps_moves_pp_pp_ups_disabled_slot_and_party_identity() {
    let mut screen = battle();
    {
        let p = &mut screen.battle_state.as_mut().unwrap().player;
        p.disabled_move = 1; // slot 1 (Pound) is Disabled
        p.disabled_turns_left = 4;
        p.preserve_original_identity();
    }
    let rng_before = serde_json::to_value(&screen.rng).unwrap();

    // Open FIGHT -> arm row 0 (Pound) -> move to row 2 (Growl) -> SELECT swap.
    screen.update_frame(a());
    assert_eq!(screen.phase, BattlePhase::MoveSelect);
    screen.update_frame(select());
    assert_eq!(screen.move_menu.as_ref().unwrap().swap_armed_slot(), Some(1));
    screen.update_frame(down());
    screen.update_frame(down());
    assert_eq!(screen.move_menu.as_ref().unwrap().cursor(), 2);
    screen.update_frame(select());

    assert_eq!(screen.move_menu.as_ref().unwrap().swap_armed_slot(), None);
    assert_eq!(screen.move_menu.as_ref().unwrap().cursor(), 0);
    assert_eq!(serde_json::to_value(&screen.rng).unwrap(), rng_before, "SELECT draws no RNG");

    let bs = screen.battle_state.as_ref().unwrap();
    let active = bs.player.active_mon();
    assert_eq!(
        active.moves,
        [
            MoveId::Growl,
            MoveId::Tackle,
            MoveId::Pound,
            MoveId::Thundershock
        ]
    );
    assert_eq!(active.pp, [7, 6, 5, 8]);
    assert_eq!(active.pp_ups, [2, 1, 0, 3]);
    assert_eq!(bs.player.disabled_move, 3, "Disabled slot tracked Pound from 1 -> 3");

    let (_, original) = bs.player.original_identity.as_ref().unwrap();
    assert_eq!(original.moves, active.moves, "party identity moves swapped too");
    assert_eq!(original.pp, active.pp);
    assert_eq!(original.pp_ups, active.pp_ups);

    // Rebuilt menu reflects the new disabled slot and max PP with PP Ups
    // (`GetMaxPP`/`AddBonusPP` caps each PP Up bonus at 7, so base 40 + 2 * 7 = 54).
    let slots = screen.move_menu.as_ref().unwrap().moves();
    assert!(!slots[0].is_disabled);
    assert!(slots[2].is_disabled);
    assert_eq!(slots[0].max_pp, 40 + 2 * 7);
}

#[test]
fn cancel_or_confirm_clears_armed_swap_marker() {
    let mut screen = battle();
    screen.update_frame(a());
    screen.update_frame(down());
    screen.update_frame(select());
    assert_eq!(screen.move_menu.as_ref().unwrap().swap_armed_slot(), Some(2));

    // B cancels MoveSelect; re-entering FIGHT starts unarmed.
    screen.update_frame(b());
    assert_eq!(screen.phase, BattlePhase::PlayerMenu);
    screen.update_frame(a());
    assert_eq!(screen.move_menu.as_ref().unwrap().swap_armed_slot(), None);
}

#[test]
fn select_swap_ignored_outside_regular_nonlink_fight_and_mixed_key_priority() {
    let mut ordinary = battle();
    ordinary.update_frame(a());

    let mut link = ordinary.clone();
    link.link_mode = true;
    let mut mimic = ordinary.clone();
    mimic.mimic_choice = Some((0, None));
    let mut ether = ordinary.clone();
    ether.phase = BattlePhase::ItemMoveSelect {
        item_id: ItemId::Ether,
        party_index: 0,
    };
    let mut forget = ordinary.clone();
    forget.phase = BattlePhase::LearnMoveChoose {
        party_index: 0,
        move_id: MoveId::Thunder,
        cursor: 0,
        resume: Box::new(BattlePhase::PlayerMenu),
    };
    for screen in [&mut link, &mut mimic, &mut ether, &mut forget] {
        let state = serde_json::to_value(BattleSnapshot::capture(screen)).unwrap();
        screen.update_frame(select());
        assert_eq!(serde_json::to_value(BattleSnapshot::capture(screen)).unwrap(), state);
    }

    // Exact SelectMenuItem order: Down beats SELECT and B; SELECT beats A.
    ordinary.update_frame(BattleInput {
        down: true,
        select: true,
        b: true,
        ..BattleInput::none()
    });
    assert_eq!(ordinary.move_menu.as_ref().unwrap().cursor(), 1);
    assert_eq!(ordinary.move_menu.as_ref().unwrap().swap_armed_slot(), None);

    ordinary.update_frame(BattleInput {
        select: true,
        a: true,
        ..BattleInput::none()
    });
    assert_eq!(ordinary.move_menu.as_ref().unwrap().swap_armed_slot(), Some(2));
    assert_eq!(ordinary.move_menu.as_ref().unwrap().cursor(), 0);
    assert_eq!(ordinary.phase, BattlePhase::MoveSelect);
}

#[test]
fn select_swap_snapshot_roundtrips_and_rejects_invalid_slots() {
    let mut screen = battle();
    screen.update_frame(a());
    for _ in 0..3 {
        screen.update_frame(down());
    }
    screen.update_frame(select());
    let encoded = serde_json::to_value(BattleSnapshot::capture(&screen)).unwrap();
    assert_eq!(encoded["move_menu"]["swap_armed"], 4);

    let snapshot: BattleSnapshot = serde_json::from_value(encoded.clone()).unwrap();
    let mut restored = BattleScreen::new(true);
    snapshot.restore_into(&mut restored);
    assert_eq!(restored.move_menu.as_ref().unwrap().swap_armed_slot(), Some(4));
    assert_eq!(restored.move_menu.as_ref().unwrap().cursor(), 0);

    restored.update_frame(select());
    assert_eq!(
        restored.battle_state.as_ref().unwrap().player.active_mon().moves,
        [
            MoveId::Thundershock,
            MoveId::Tackle,
            MoveId::Growl,
            MoveId::Pound
        ]
    );

    // Legacy snapshot without `swap_armed` deserializes cleanly as unarmed.
    let mut legacy = encoded.clone();
    legacy["move_menu"].as_object_mut().unwrap().remove("swap_armed");
    let legacy: BattleSnapshot = serde_json::from_value(legacy).unwrap();
    legacy.restore_into(&mut restored);
    assert_eq!(restored.move_menu.as_ref().unwrap().swap_armed_slot(), None);

    for invalid in [0, 5, 255] {
        let mut invalid_json = encoded.clone();
        invalid_json["move_menu"]["swap_armed"] = invalid.into();
        assert!(serde_json::from_value::<BattleSnapshot>(invalid_json).is_err());
    }
}
