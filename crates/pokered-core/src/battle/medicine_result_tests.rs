//! ItemUseMedicine's party-owned successful result, Delay50 and ACK boundary.
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;
use crate::snapshot::BattleSnapshot;

fn battle(item: ItemId, target: usize) -> BattleScreen {
    let mon = |species, level| create_pokemon_with_moves(species, level, [0x9a, 0x78],
        [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None]).unwrap();
    let mut s = BattleScreen::from_parties(true,
        &[mon(Species::Bulbasaur, 20), mon(Species::Pikachu, 12)],
        &[mon(Species::Rattata, 5)], None);
    s.player_bag.add_item(item, 2).unwrap();
    s.bag_menu = Some(menu::BagMenuState::new(vec![(item, 2)]));
    s.phase = BattlePhase::ItemTargetSelect { item_id: item };
    s.party_cursor = target;
    s.sync_player_context();
    s
}

#[test]
fn medicine_result_heals_one_live_target_and_holds_through_delay50_and_no_input() {
    for target in [0, 1] {
        for ack_b in [false, true] {
            let mut s = battle(ItemId::Potion, target);
            s.battle_state.as_mut().unwrap().player.party[target].hp = 1;
            s.sync_display_from_state();
            let before = s.battle_state.clone().unwrap();
            s.update_frame(BattleInput { a: true, ..BattleInput::none() });
            assert_eq!(s.phase, BattlePhase::ItemMedicineResult { wait_frames: 50 });
            assert_eq!(s.party_cursor, target);
            assert_eq!(s.current_message.as_deref(), Some(if target == 0 { "BULBASAUR\nrecovered by 20!" } else { "PIKACHU\nrecovered by 20!" }));
            assert_eq!(s.player_bag.items(), &[(ItemId::Potion, 1)]);
            assert!(s.bag_menu.is_none());
            assert_eq!(s.take_item_sfx_pending(), Some(BattleItemSfx::HealHp));
            assert_eq!(s.take_item_sfx_pending(), None);
            let healed = s.battle_state.clone().unwrap();
            assert_eq!(healed.player.party[target].hp, 21);
            for (old, new) in before.player.party.iter().zip(&healed.player.party) {
                assert_eq!(old.pp, new.pp);
                assert_eq!(old.moves, new.moves);
                assert_eq!(old.ot_name, new.ot_name);
            }
            // No early ACK, even when the user presses throughout the delay.
            for remaining in (0..50).rev() {
                s.update_frame(BattleInput { a: true, b: true, down: true, ..BattleInput::none() });
                assert_eq!(s.phase, BattlePhase::ItemMedicineResult { wait_frames: remaining });
                assert_eq!(s.party_cursor, target);
            }
            for _ in 0..600 { s.update_frame(BattleInput::none()); }
            assert_eq!(serde_json::to_value(s.battle_state.as_ref().unwrap()).unwrap(), serde_json::to_value(&healed).unwrap(),
                "no extra healing, PP use, damage or opponent action while the party result is waiting");
            s.update_frame(BattleInput { a: !ack_b, b: ack_b, ..BattleInput::none() });
            assert_eq!(s.phase, BattlePhase::EnemyFreeTurnAfterItem);
            assert!(s.current_message.is_none());
            assert_eq!(s.player_bag.items(), &[(ItemId::Potion, 1)]);
        }
    }
}

#[test]
fn medicine_result_status_cure_and_revive_share_the_party_continuation() {
    for item in [ItemId::ParlyzHeal, ItemId::Revive, ItemId::MaxRevive] {
        let mut s = battle(item, 1);
        let mon = &mut s.battle_state.as_mut().unwrap().player.party[1];
        if item == ItemId::ParlyzHeal { mon.status = state::StatusCondition::Paralysis; }
        else { mon.hp = 0; }
        let max_hp = mon.max_hp;
        s.update_frame(BattleInput { a: true, ..BattleInput::none() });
        let mon = &s.battle_state.as_ref().unwrap().player.party[1];
        assert_eq!(mon.status, state::StatusCondition::None);
        assert_eq!(mon.hp, match item { ItemId::Revive => max_hp / 2, _ => max_hp });
        assert_eq!(s.phase, BattlePhase::ItemMedicineResult { wait_frames: 50 });
        assert_eq!(s.player_bag.items(), &[(item, 1)]);
        assert_eq!(s.take_item_sfx_pending(), Some(if item == ItemId::ParlyzHeal { BattleItemSfx::HealAilment } else { BattleItemSfx::HealHp }));
    }
}

#[test]
fn medicine_result_snapshot_resumes_the_same_target_message_and_delay_once() {
    let mut s = battle(ItemId::Potion, 1);
    s.battle_state.as_mut().unwrap().player.party[1].hp = 1;
    s.update_frame(BattleInput { a: true, ..BattleInput::none() });
    for _ in 0..17 { s.update_frame(BattleInput::none()); }
    let snapshot = BattleSnapshot::capture(&s);
    let encoded = serde_json::to_vec(&snapshot).unwrap();
    let restored: BattleSnapshot = serde_json::from_slice(&encoded).unwrap();
    restored.restore_into(&mut s);
    assert_eq!(s.phase, BattlePhase::ItemMedicineResult { wait_frames: 33 });
    assert_eq!(s.party_cursor, 1);
    assert_eq!(s.current_message.as_deref(), Some("PIKACHU\nrecovered by 20!"));
    assert_eq!(s.battle_state.as_ref().unwrap().player.party[1].hp, 21);
    for _ in 0..33 { s.update_frame(BattleInput { b: true, ..BattleInput::none() }); }
    assert_eq!(s.phase, BattlePhase::ItemMedicineResult { wait_frames: 0 });
    s.update_frame(BattleInput { b: true, ..BattleInput::none() });
    assert_eq!(s.phase, BattlePhase::EnemyFreeTurnAfterItem);
    assert_eq!(s.player_bag.items(), &[(ItemId::Potion, 1)]);
    let fresh = battle(ItemId::Potion, 0);
    assert_eq!(fresh.phase, BattlePhase::ItemTargetSelect { item_id: ItemId::Potion });
    assert!(fresh.current_message.is_none());
}

#[test]
fn medicine_result_transformed_party_keeps_original_identity_and_pp_with_live_hp() {
    let mut s = battle(ItemId::Potion, 0);
    let player = &mut s.battle_state.as_mut().unwrap().player;
    player.active_mon_mut().hp = 1;
    player.preserve_original_identity();
    player.original_identity.as_mut().unwrap().1.set_nickname("LEAF");
    let original = player.original_identity.as_ref().unwrap().1.clone();
    player.active_mon_mut().species = Species::Mewtwo;
    player.active_mon_mut().moves = [MoveId::PsychicM, MoveId::None, MoveId::None, MoveId::None];
    player.active_mon_mut().pp = [5, 0, 0, 0];
    player.set_status3(state::status3::TRANSFORMED);
    s.sync_display_from_state();
    s.update_frame(BattleInput { a: true, ..BattleInput::none() });
    let player = &s.battle_state.as_ref().unwrap().player;
    let party = player.persistent_party();
    assert_eq!(party[0].species, original.species);
    assert_eq!(party[0].moves, original.moves);
    assert_eq!(party[0].pp, original.pp);
    assert_eq!(party[0].hp, 21);
    assert_eq!(s.current_message.as_deref(), Some("LEAF\nrecovered by 20!"));
    assert_eq!(player.active_mon().species, Species::Mewtwo);
    assert_eq!(player.active_mon().pp, [5, 0, 0, 0]);
}

#[test]
fn medicine_cancel_and_no_effect_keep_the_bag_and_party_without_result_or_enemy_turn() {
    for item in [ItemId::Potion, ItemId::ParlyzHeal, ItemId::Revive] {
        let mut s = battle(item, 1);
        let original = serde_json::to_value(s.battle_state.as_ref().unwrap()).unwrap();
        s.update_frame(BattleInput { b: true, ..BattleInput::none() });
        assert_eq!(s.phase, BattlePhase::BagSelect);
        assert_eq!(s.player_bag.items(), &[(item, 2)]);
        s.phase = BattlePhase::ItemTargetSelect { item_id: item };
        s.update_frame(BattleInput { a: true, ..BattleInput::none() });
        assert!(matches!(&s.phase, BattlePhase::ShowingText { next_phase, .. } if **next_phase == BattlePhase::BagSelect));
        assert_eq!(s.take_item_sfx_pending(), None);
        for _ in 0..100 {
            if s.phase == BattlePhase::BagSelect { break; }
            s.update_frame(BattleInput { b: true, ..BattleInput::none() });
        }
        assert_eq!(s.phase, BattlePhase::BagSelect);
        assert_eq!(s.player_bag.items(), &[(item, 2)]);
        assert_eq!(serde_json::to_value(s.battle_state.as_ref().unwrap()).unwrap(), original);
    }
}

#[test]
fn medicine_result_names_the_selected_member_for_every_source_cure_and_full_restore_branch() {
    for (item, status, hp, suffix) in [
        (ItemId::Antidote, state::StatusCondition::Poison, None, " was\ncured of poison!"),
        (ItemId::BurnHeal, state::StatusCondition::Burn, None, "'s\nburn was healed!"),
        (ItemId::IceHeal, state::StatusCondition::Freeze, None, " was\ndefrosted!"),
        (ItemId::Awakening, state::StatusCondition::Sleep(3), None, "\nwoke up!"),
        (ItemId::ParlyzHeal, state::StatusCondition::Paralysis, None, "'s\nrid of paralysis!"),
        (ItemId::FullHeal, state::StatusCondition::Poison, None, "'s\nhealth returned!"),
        (ItemId::FullRestore, state::StatusCondition::Poison, None, "'s\nhealth returned!"),
        (ItemId::FullRestore, state::StatusCondition::Poison, Some(1), "\nrecovered by 31!"),
        (ItemId::Revive, state::StatusCondition::None, Some(0), "\nis revitalized!"),
    ] {
        let mut s = battle(item, 1);
        let mon = &mut s.battle_state.as_mut().unwrap().player.party[1];
        mon.set_nickname("SPARKY"); mon.status = status;
        if let Some(hp) = hp { mon.hp = hp; }
        s.update_frame(BattleInput { a: true, ..BattleInput::none() });
        assert_eq!(s.current_message.as_deref(), Some(format!("SPARKY{suffix}").as_str()), "{item:?}");
        assert_eq!(s.phase, BattlePhase::ItemMedicineResult { wait_frames: 50 });
        assert_eq!(s.player_bag.items(), &[(item, 1)]);
    }
}

#[test]
fn medicine_no_effect_preserves_party_bag_ai_and_returns_on_generic_ack() {
    for item in [ItemId::Potion, ItemId::ParlyzHeal, ItemId::Revive] {
        for target in [0, 1] {
            for ack_b in [false, true] {
                let mut s = battle(item, target);
                s.enemy_ai_count = 3;
                let original = serde_json::to_value(s.battle_state.as_ref().unwrap()).unwrap();
                s.update_frame(BattleInput { a: true, ..BattleInput::none() });
                assert!(matches!(&s.phase, BattlePhase::ShowingText {
                    wait_frames: BATTLE_TEXT_PAGE_WAIT_FRAMES,
                    backdrop: BattleTextBackdrop::MedicineParty, next_phase, ..
                } if **next_phase == BattlePhase::BagSelect));
                assert_eq!(s.current_message.as_deref(), Some("It won't have any effect."));
                assert_eq!(s.take_item_sfx_pending(), None);
                for _ in 0..600 {
                    s.update_frame(BattleInput { down: true, ..BattleInput::none() });
                    assert!(s.phase.uses_medicine_party_backdrop());
                    assert_eq!(s.party_cursor, target);
                }
                assert_eq!(serde_json::to_value(s.battle_state.as_ref().unwrap()).unwrap(), original);
                assert_eq!(s.enemy_ai_count, 3);
                assert_eq!(s.player_bag.items(), &[(item, 2)]);
                assert!(s.bag_menu.is_some());
                s.update_frame(BattleInput { a: !ack_b, b: ack_b, ..BattleInput::none() });
                assert_eq!(s.phase, BattlePhase::BagSelect);
                assert!(!s.phase.uses_medicine_party_backdrop());
                assert!(s.current_message.is_none());
                for _ in 0..60 { s.update_frame(BattleInput::none()); }
                assert_eq!(s.enemy_ai_count, 3);
                assert_eq!(s.player_bag.items(), &[(item, 2)]);
                assert_eq!(serde_json::to_value(s.battle_state.as_ref().unwrap()).unwrap(), original);
                s.update_frame(BattleInput { b: true, ..BattleInput::none() });
                assert_eq!(s.phase, BattlePhase::PlayerMenu);
            }
        }
    }
}

#[test]
fn medicine_no_effect_has_no_success_delay50_and_backdrop_survives_pagination() {
    let mut s = battle(ItemId::Potion, 1);
    s.update_frame(BattleInput { a: true, ..BattleInput::none() });
    for remaining in (0..BATTLE_TEXT_PAGE_WAIT_FRAMES).rev() {
        s.update_frame(BattleInput { a: true, ..BattleInput::none() });
        assert!(matches!(s.phase, BattlePhase::ShowingText {
            wait_frames, backdrop: BattleTextBackdrop::MedicineParty, ..
        } if wait_frames == remaining));
    }
    s.update_frame(BattleInput { a: true, ..BattleInput::none() });
    assert_eq!(s.phase, BattlePhase::BagSelect, "ordinary page wait, no success Delay50");
    s.show_medicine_no_effect("First line\nSecond line\nThird line".into());
    assert!(matches!(&s.phase, BattlePhase::ShowingText { messages, .. } if messages.len() > 1));
    for _ in 0..BATTLE_TEXT_PAGE_WAIT_FRAMES { s.update_frame(BattleInput::none()); }
    s.update_frame(BattleInput { b: true, ..BattleInput::none() });
    assert!(matches!(s.phase, BattlePhase::ShowingText {
        current: 1, backdrop: BattleTextBackdrop::MedicineParty, ..
    }));
    // Equal wording from another caller must not select a medicine view.
    s.show_text_then(vec!["It won't have any effect.".into()], BattlePhase::PlayerMenu);
    assert!(matches!(s.phase, BattlePhase::ShowingText { backdrop: BattleTextBackdrop::Battle, .. }));
    assert!(!s.phase.uses_medicine_party_backdrop());
}

#[test]
fn medicine_no_effect_snapshot_owns_backdrop_and_legacy_text_defaults_to_battle() {
    let mut s = battle(ItemId::Revive, 1);
    s.update_frame(BattleInput { a: true, ..BattleInput::none() });
    for _ in 0..3 { s.update_frame(BattleInput::none()); }
    let snapshot = BattleSnapshot::capture(&s);
    let encoded = serde_json::to_vec(&snapshot).unwrap();
    let restored: BattleSnapshot = serde_json::from_slice(&encoded).unwrap();
    restored.restore_into(&mut s);
    assert!(matches!(&s.phase, BattlePhase::ShowingText {
        wait_frames: 7, backdrop: BattleTextBackdrop::MedicineParty, next_phase, ..
    } if **next_phase == BattlePhase::BagSelect));
    assert_eq!(s.party_cursor, 1);
    assert_eq!(s.current_message.as_deref(), Some("It won't have any effect."));
    let mut legacy = serde_json::to_value(&snapshot).unwrap();
    legacy["phase"]["ShowingText"].as_object_mut().unwrap().remove("backdrop");
    let old: BattleSnapshot = serde_json::from_value(legacy).unwrap();
    old.restore_into(&mut s);
    assert!(matches!(s.phase, BattlePhase::ShowingText { backdrop: BattleTextBackdrop::Battle, .. }));
    restored.restore_into(&mut s);
    for _ in 0..7 { s.update_frame(BattleInput::none()); }
    s.update_frame(BattleInput { a: true, ..BattleInput::none() });
    assert_eq!(s.phase, BattlePhase::BagSelect);
    assert_eq!(s.player_bag.items(), &[(ItemId::Revive, 2)]);
    assert!(!BattleScreen::new(true).phase.uses_medicine_party_backdrop());
}
