//! ItemUsePPRestore party caller, final PrintText ACK and original identity.
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

fn battle(item: ItemId, target: usize) -> BattleScreen {
    let mon = |species, level, moves| create_pokemon_with_moves(species, level, [0x9a, 0x78], moves).unwrap();
    let mut s = BattleScreen::from_parties(true,
        &[mon(Species::Bulbasaur, 20, [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None]),
          mon(Species::Pikachu, 12, [MoveId::Thundershock, MoveId::Growl, MoveId::None, MoveId::None])],
        &[mon(Species::Weedle, 5, [MoveId::PoisonSting, MoveId::StringShot, MoveId::None, MoveId::None])], None);
    s.player_bag.add_item(item, 2).unwrap();
    s.bag_menu = Some(menu::BagMenuState::new(vec![(item, 2)]));
    s.phase = BattlePhase::ItemTargetSelect { item_id: item };
    s.party_cursor = target;
    s.sync_player_context();
    s
}
fn select(s: &mut BattleScreen) { s.update_frame(BattleInput { a: true, ..BattleInput::none() }); }

#[test]
fn pp_success_changes_pp_now_but_spends_item_only_after_final_text_ack() {
    for item in [ItemId::Ether, ItemId::MaxEther, ItemId::Elixer, ItemId::MaxElixer] {
        for target in [0, 1] {
            for ack_b in [false, true] {
                let mut s = battle(item, target);
                s.battle_state.as_mut().unwrap().player.party[target].pp[0] = 0;
                select(&mut s);
                if matches!(s.phase, BattlePhase::ItemMoveSelect { .. }) { select(&mut s); }
                assert_eq!(s.player_bag.items(), &[(item, 2)], "PrintText precedes RemoveUsedItem");
                assert_eq!(s.take_item_sfx_pending(), Some(BattleItemSfx::HealAilment));
                let restored = serde_json::to_value(s.battle_state.as_ref().unwrap()).unwrap();
                assert!(s.battle_state.as_ref().unwrap().player.party[target].pp[0] > 0);
                for _ in 0..600 { s.update_frame(BattleInput::none()); }
                assert_eq!(serde_json::to_value(s.battle_state.as_ref().unwrap()).unwrap(), restored);
                s.update_frame(BattleInput { a: !ack_b, b: ack_b, ..BattleInput::none() });
                assert_eq!(s.phase, BattlePhase::EnemyFreeTurnAfterItem, "there is no Delay50");
                assert_eq!(s.player_bag.items(), &[(item, 1)]);
                s.post_text_transition();
                assert_eq!(s.player_bag.items(), &[(item, 1)]);
                assert!(s.bag_menu.is_none());
            }
        }
    }
}

fn transformed() -> BattleScreen {
    let mut s = battle(ItemId::Ether, 0);
    let ditto = create_pokemon_with_moves(Species::Ditto, 20, [0x9a,0x78],
        [MoveId::Transform,MoveId::None,MoveId::None,MoveId::None]).unwrap();
    s.battle_state.as_mut().unwrap().player.party[0] = ditto;
    s.sync_player_context();
    s.execute_turn_with_move(0);
    assert_eq!(s.battle_state.as_ref().unwrap().player.party_mon(0).pp, [9,0,0,0]);
    assert_eq!(s.battle_state.as_ref().unwrap().player.active_mon().moves[0], MoveId::PoisonSting);
    s.phase = BattlePhase::ItemTargetSelect { item_id: ItemId::Ether };
    s
}
#[test]
fn transformed_ether_chooser_uses_original_party_moves() {
    let mut s = transformed();
    select(&mut s);
    assert_eq!(s.move_menu.as_ref().unwrap().moves().iter().map(|m|m.move_id).collect::<Vec<_>>(), vec![MoveId::Transform]);
}
#[test]
fn transformed_ether_restores_original_pp_then_copies_all_pp_without_replacing_battle_identity() {
    let mut s = transformed();
    let working = s.battle_state.as_ref().unwrap().player.active_mon().clone();
    select(&mut s);select(&mut s);
    let p = &s.battle_state.as_ref().unwrap().player;
    assert_eq!(p.party_mon(0).pp, [10,0,0,0]);
    let mut expected = working;
    expected.pp = [10,0,0,0]; expected.pp_ups = [0;4];
    assert_eq!(serde_json::to_value(p.active_mon()).unwrap(), serde_json::to_value(expected).unwrap());
}

#[test]
fn pp_results_retain_typed_party_and_ether_cursor_without_retaining_move_menu_copy() {
    for item in [ItemId::Ether, ItemId::MaxEther, ItemId::Elixer, ItemId::MaxElixer] {
        let mut s = battle(item, 1);
        s.battle_state.as_mut().unwrap().player.party[1].pp[1] = 0;
        select(&mut s);
        let row = if matches!(item, ItemId::Ether | ItemId::MaxEther) {
            s.update_frame(BattleInput { down: true, ..BattleInput::none() });
            select(&mut s); Some(1)
        } else { None };
        assert!(matches!(s.phase, BattlePhase::ShowingText {
            backdrop: BattleTextBackdrop::PpRestoreParty { party_index: 1, move_row, consume_on_ack: Some(id) }, ..
        } if move_row == row && id == item));
        assert!(s.move_menu.is_none(), "the result borrows moves from BattleState");
        assert!(s.phase.uses_item_party_backdrop());
    }
}

#[test]
fn pp_cancel_and_no_effect_keep_party_and_bag_and_never_give_a_free_enemy_turn() {
    for item in [ItemId::Ether, ItemId::MaxEther, ItemId::Elixer, ItemId::MaxElixer] {
        let mut s = battle(item, 1);
        let protected = serde_json::to_value(&s.battle_state).unwrap();
        select(&mut s);
        if matches!(s.phase, BattlePhase::ItemMoveSelect { .. }) {
            s.update_frame(BattleInput { b: true, ..BattleInput::none() });
            assert_eq!(s.phase, BattlePhase::ItemTargetSelect { item_id: item });
            select(&mut s); select(&mut s);
        }
        assert!(matches!(s.phase, BattlePhase::ShowingText {
            backdrop: BattleTextBackdrop::PpRestoreParty { consume_on_ack: None, .. }, .. }));
        assert_eq!(s.current_message.as_deref(), Some("It won't have any effect."));
        assert_eq!(s.take_item_sfx_pending(), None);
        for _ in 0..600 { s.update_frame(BattleInput::none()); }
        select(&mut s);
        assert_eq!(s.phase, BattlePhase::BagSelect);
        assert_eq!(s.player_bag.items(), &[(item, 2)]);
        assert_eq!(serde_json::to_value(&s.battle_state).unwrap(), protected);
    }
}

#[test]
fn pp_restore_on_fainted_bench_keeps_hp_status_and_other_party_fields() {
    let mut s = battle(ItemId::MaxElixer, 1);
    let mon = &mut s.battle_state.as_mut().unwrap().player.party[1];
    mon.hp = 0; mon.status = state::StatusCondition::Poison; mon.pp = [1,2,0,0];
    let mut expected = mon.clone(); expected.pp = [30,40,0,0];
    select(&mut s);
    let actual = &s.battle_state.as_ref().unwrap().player.party[1];
    assert_eq!(serde_json::to_value(actual).unwrap(), serde_json::to_value(expected).unwrap());
}

#[test]
fn max_pp_restore_preserves_original_full_packed_pp_up_bug() {
    for item in [ItemId::Ether, ItemId::MaxEther, ItemId::Elixer, ItemId::MaxElixer] {
        let mut s = battle(item, 0);
        let mon = &mut s.battle_state.as_mut().unwrap().player.party[0];
        mon.pp_ups[0] = 1; mon.pp[0] = 42; // Tackle35 + one PP Up7, already full.
        select(&mut s);
        if matches!(s.phase, BattlePhase::ItemMoveSelect { .. }) { select(&mut s); }
        let is_max = matches!(item, ItemId::MaxEther | ItemId::MaxElixer);
        assert_eq!(s.take_item_sfx_pending().is_some(), is_max);
        for _ in 0..20 { s.update_frame(BattleInput::none()); }
        select(&mut s);
        assert_eq!(s.player_bag.items(), &[(item, if is_max { 1 } else { 2 })]);
        assert_eq!(s.battle_state.as_ref().unwrap().player.party[0].pp[0], 42);
        assert_eq!(s.battle_state.as_ref().unwrap().player.party[0].pp_ups[0], 1);
    }
}

#[test]
fn pp_snapshot_preserves_final_ack_owner_and_legacy_text_does_not_consume_again() {
    use crate::snapshot::BattleSnapshot;
    let mut s = battle(ItemId::Ether, 1);
    s.battle_state.as_mut().unwrap().player.party[1].pp[0] = 0;
    select(&mut s);select(&mut s);
    for _ in 0..20 { s.update_frame(BattleInput::none()); }
    let encoded = serde_json::to_vec(&BattleSnapshot::capture(&s)).unwrap();
    let snapshot: BattleSnapshot = serde_json::from_slice(&encoded).unwrap();
    let mut resumed = battle(ItemId::Ether, 0); snapshot.restore_into(&mut resumed);
    assert_eq!(resumed.phase, s.phase);
    assert_eq!(resumed.party_cursor, 1);
    select(&mut resumed);
    assert_eq!(resumed.player_bag.items(), &[(ItemId::Ether, 1)]);
    assert_eq!(resumed.phase, BattlePhase::EnemyFreeTurnAfterItem);
    // An old ShowingText snapshot already spent its item before printing.
    let mut legacy = serde_json::to_value(&s.phase).unwrap();
    legacy["ShowingText"].as_object_mut().unwrap().remove("backdrop");
    s.phase = serde_json::from_value(legacy).unwrap();
    s.player_bag.remove_item(ItemId::Ether, 1).unwrap();
    select(&mut s);
    assert_eq!(s.player_bag.items(), &[(ItemId::Ether, 1)]);
}

#[test]
fn pp_consume_waits_for_last_page_and_new_battle_discards_pending_result() {
    let mut s = battle(ItemId::Elixer, 1);
    s.battle_state.as_mut().unwrap().player.party[1].pp[0] = 0;
    select(&mut s);
    if let BattlePhase::ShowingText { messages, .. } = &mut s.phase { messages.push("Second page".into()); }
    for _ in 0..20 { s.update_frame(BattleInput::none()); }
    select(&mut s);
    assert_eq!(s.player_bag.items(), &[(ItemId::Elixer, 2)]);
    assert!(matches!(s.phase, BattlePhase::ShowingText { current: 1, .. }));
    for _ in 0..20 { s.update_frame(BattleInput::none()); }
    select(&mut s);
    assert_eq!(s.player_bag.items(), &[(ItemId::Elixer, 1)]);
    let fresh = battle(ItemId::Elixer, 1);
    assert!(matches!(fresh.phase, BattlePhase::ItemTargetSelect { .. }));
    assert_eq!(fresh.player_bag.items(), &[(ItemId::Elixer, 2)]);
}

#[test]
fn transformed_no_effect_does_not_copy_pp_back_to_working_moves() {
    let mut s = transformed();
    s.battle_state.as_mut().unwrap().player.party_mon_mut(0).pp[0] = 10;
    let protected = serde_json::to_value(&s.battle_state).unwrap();
    select(&mut s);select(&mut s);
    assert_eq!(s.current_message.as_deref(),Some("It won't have any effect."));
    assert_eq!(serde_json::to_value(&s.battle_state).unwrap(),protected);
    assert_eq!(s.player_bag.items(),&[(ItemId::Ether,2)]);
    assert_eq!(s.take_item_sfx_pending(),None);
}
