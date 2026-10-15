//! MainInBattleLoop still consults TrainerAI after a consumed player action.
//! These exercise the production item phase and voluntary-switch entry points,
//! rather than the retired test-only enemy_ai_action_inner wrapper.
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

fn mon(species: Species, level: u8, move_: MoveId) -> state::Pokemon {
    create_pokemon_with_moves(species, level, [0x9a, 0x78],
        [move_, MoveId::None, MoveId::None, MoveId::None]).unwrap()
}

fn battle(class: TrainerClass) -> BattleScreen {
    let players = [mon(Species::Rattata, 50, MoveId::Tackle),
        mon(Species::Raticate, 50, MoveId::Tackle)];
    let mut enemy = mon(Species::Onix, 30, MoveId::Tackle);
    enemy.status = StatusCondition::Poison;
    let mut s = BattleScreen::from_parties(false, &players, &[enemy], Some(class));
    s.trainer_name = Some("BROCK".to_string());
    s.phase = BattlePhase::PlayerMenu;
    s.rng = pokered_rules::runtime::StdBattleRng::from_seed(42);
    s
}

fn press(s: &mut BattleScreen, a: bool, down: bool, right: bool) {
    s.update_frame(BattleInput { a, down, right, ..BattleInput::none() });
}

fn open_first_item(s: &mut BattleScreen, item: ItemId) {
    s.player_bag.add_item(item, 2).unwrap();
    press(s, false, true, false);
    press(s, true, false, false);
    assert_eq!(s.phase, BattlePhase::BagSelect);
    press(s, true, false, false);
}

fn finish_item_notice(s: &mut BattleScreen) {
    for _ in 0..600 {
        if matches!(s.phase, BattlePhase::EnemyFreeTurnAfterItem) {
            s.update_frame(BattleInput::none());
            return;
        }
        assert!(matches!(s.phase, BattlePhase::ShowingText { .. }), "{:?}", s.phase);
        press(s, true, false, false);
    }
    panic!("item notice did not resume its enemy turn");
}

fn messages(s: &BattleScreen) -> String {
    match &s.phase {
        BattlePhase::ShowingText { messages, .. } => messages.join("\n"),
        _ => s.current_message.clone().unwrap_or_default(),
    }
}

#[test]
fn trainer_free_turn_successful_x_item_consults_ai_once_instead_of_attacking() {
    let mut s = battle(TrainerClass::Brock);
    let start_hp = s.player_hp;
    let pp = s.battle_state.as_ref().unwrap().player.party[0].pp;
    open_first_item(&mut s, ItemId::XAccuracy);
    assert_eq!(s.enemy_ai_count, 5, "item notice precedes TrainerAI");
    finish_item_notice(&mut s);
    let bs = s.battle_state.as_ref().unwrap();
    assert!(bs.enemy.active_mon().status.is_none());
    assert_eq!(bs.player.active_mon().hp, start_hp);
    assert_eq!(bs.player.party[0].pp, pp);
    assert_eq!(bs.enemy.last_move_used, MoveId::None);
    assert_eq!(s.enemy_ai_count, 4);
    assert_eq!(s.player_bag.items(), &[(ItemId::XAccuracy, 1)]);
    assert!(messages(&s).contains("FULL HEAL"));
    assert!(!messages(&s).contains("used TACKLE"));
    for _ in 0..10 { s.update_frame(BattleInput::none()); }
    assert_eq!(s.enemy_ai_count, 4, "waiting for its notice cannot repeat the AI action");
}

#[test]
fn trainer_free_turn_successful_potion_has_the_same_ai_continuation() {
    let mut s = battle(TrainerClass::Brock);
    let bs = s.battle_state.as_mut().unwrap();
    bs.player.active_mon_mut().hp -= 30;
    s.sync_display_from_state();
    let before = s.player_hp;
    open_first_item(&mut s, ItemId::Potion);
    assert_eq!(s.phase, BattlePhase::ItemTargetSelect { item_id: ItemId::Potion });
    press(&mut s, true, false, false);
    // The real HP changes first; the existing item notice animates its bar.
    assert_eq!(s.battle_state.as_ref().unwrap().player.active_mon().hp, before + 20);
    finish_item_notice(&mut s);
    assert_eq!(s.battle_state.as_ref().unwrap().player.active_mon().hp, before + 20);
    assert!(s.battle_state.as_ref().unwrap().enemy.active_mon().status.is_none());
    assert_eq!(s.player_bag.items(), &[(ItemId::Potion, 1)]);
    assert_eq!(s.enemy_ai_count, 4);
}

#[test]
fn trainer_free_turn_failed_medicine_does_not_consult_ai_or_spend_turn() {
    let mut s = battle(TrainerClass::Brock);
    open_first_item(&mut s, ItemId::Potion);
    press(&mut s, true, false, false);
    for _ in 0..600 {
        if s.phase == BattlePhase::BagSelect { break; }
        press(&mut s, true, false, false);
    }
    assert_eq!(s.phase, BattlePhase::BagSelect);
    assert_eq!(s.enemy_ai_count, 5);
    assert_eq!(s.player_bag.items(), &[(ItemId::Potion, 2)]);
    assert_eq!(s.battle_state.as_ref().unwrap().enemy.active_mon().status, StatusCondition::Poison);
}

#[test]
fn trainer_free_turn_voluntary_party_switch_still_allows_enemy_full_heal() {
    let mut s = battle(TrainerClass::Brock);
    let before = s.battle_state.as_ref().unwrap().player.party[1].hp;
    press(&mut s, false, false, true); // POKeMON
    press(&mut s, true, false, false);
    assert_eq!(s.phase, BattlePhase::PartySelect);
    press(&mut s, false, true, false);
    press(&mut s, true, false, false);
    assert!(matches!(s.phase, BattlePhase::PartySubMenu { selected_index: 1 }));
    press(&mut s, true, false, false);
    let bs = s.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.active_pokemon_index, 1);
    assert_eq!(bs.player.active_mon().hp, before);
    assert!(bs.enemy.active_mon().status.is_none());
    assert_eq!(s.enemy_ai_count, 4);
    let m = messages(&s);
    assert!(m.find("Go!").unwrap() < m.find("FULL HEAL").unwrap());
}

#[test]
fn trainer_free_turn_juggler_switch_emits_real_load_and_resets_participants() {
    for seed in 0..64 {
        let mut s = battle(TrainerClass::Juggler);
        s.trainer_name = Some("JUGGLER".to_string());
        let bs = s.battle_state.as_mut().unwrap();
        bs.enemy.active_mon_mut().status = StatusCondition::None;
        bs.enemy.party.push(mon(Species::Hypno, 34, MoveId::Pound));
        bs.party_gain_exp_flags[1] = true;
        bs.party_fought_flags[1] = true;
        s.loaded_enemy_slots = 0; // first load already consumed by frontend
        s.rng = pokered_rules::runtime::StdBattleRng::from_seed(seed);
        let hp = s.player_hp;
        open_first_item(&mut s, ItemId::XAccuracy);
        finish_item_notice(&mut s);
        let bs = s.battle_state.as_ref().unwrap();
        if bs.enemy.active_pokemon_index == 1 {
            assert_eq!(s.loaded_enemy_slots, 1 << 1);
            assert_eq!(bs.player.active_mon().hp, hp, "switch replaces the attack");
            assert_eq!(bs.enemy.last_move_used, MoveId::None);
            assert_eq!(bs.party_gain_exp_flags, [true, false, false, false, false, false]);
            assert_eq!(bs.party_fought_flags, bs.party_gain_exp_flags);
            assert_eq!(s.enemy_ai_count, 3, "incoming enemy reseeds AI count");
            assert!(messages(&s).contains("withdrew"));
            assert!(messages(&s).contains("sent out"));
            return;
        }
    }
    panic!("no deterministic Juggler switch across seeded ordinary item turns");
}

#[test]
fn trainer_free_turn_ai_potion_precedes_residual_and_suppresses_attack() {
    let mut s = battle(TrainerClass::CooltrainerF);
    let mon = s.battle_state.as_mut().unwrap().enemy.active_mon_mut();
    mon.max_hp = 160;
    mon.hp = 1; // CooltrainerFAI deterministically uses Hyper Potion below max/10
    let hp = s.player_hp;
    open_first_item(&mut s, ItemId::XAccuracy);
    finish_item_notice(&mut s);
    let bs = s.battle_state.as_ref().unwrap();
    assert_eq!(bs.enemy.active_mon().hp, 150, "heal to160, then poison subtracts10");
    assert_eq!(bs.player.active_mon().hp, hp);
    let m = messages(&s);
    assert!(m.find("HYPER POTION").unwrap() < m.find("POISON").unwrap());
}

#[test]
fn trainer_free_turn_ai_item_does_not_spend_recharge_or_charge_continuation() {
    for charging in [false, true] {
        let mut s = battle(TrainerClass::Brock);
        let bs = s.battle_state.as_mut().unwrap();
        if charging {
            bs.enemy.selected_move = MoveId::Dig;
            bs.enemy.set_status1(state::status1::CHARGING_UP);
        } else {
            bs.enemy.set_status2(state::status2::NEEDS_TO_RECHARGE);
        }
        open_first_item(&mut s, ItemId::XAccuracy);
        finish_item_notice(&mut s);
        let enemy = &s.battle_state.as_ref().unwrap().enemy;
        assert!(enemy.active_mon().status.is_none());
        assert_eq!(s.enemy_ai_count, 4);
        if charging { assert!(enemy.has_status1(state::status1::CHARGING_UP)); }
        else { assert!(enemy.has_status2(state::status2::NEEDS_TO_RECHARGE)); }
        assert!(!messages(&s).contains("must recharge"));
        assert_eq!(enemy.last_move_used, MoveId::None);
    }
}

#[test]
fn trainer_free_turn_wild_link_and_exhausted_ai_do_not_use_trainer_items() {
    for kind in 0..3 {
        let mut s = battle(TrainerClass::Brock);
        if kind == 0 { s.is_wild = true; }
        if kind == 1 { s.link_mode = true; }
        if kind == 2 { s.enemy_ai_count = 0; }
        let count = s.enemy_ai_count;
        s.run_enemy_free_turn_stack(Vec::new(), if kind == 1 { Some((MoveId::Tackle, 0)) } else { None });
        assert_eq!(s.battle_state.as_ref().unwrap().enemy.active_mon().status, StatusCondition::Poison);
        assert_eq!(s.enemy_ai_count, count);
        assert!(!messages(&s).contains("FULL HEAL"));
    }
}

#[test]
fn trainer_free_turn_ai_replacement_does_not_resolve_or_announce_metronome() {
    let mut s = battle(TrainerClass::Brock);
    s.battle_state.as_mut().unwrap().enemy.active_mon_mut().moves =
        [MoveId::Metronome, MoveId::None, MoveId::None, MoveId::None];
    open_first_item(&mut s, ItemId::XAccuracy);
    finish_item_notice(&mut s);
    assert!(messages(&s).contains("FULL HEAL"));
    assert!(!messages(&s).contains("METRONOME"));
    assert_eq!(s.battle_state.as_ref().unwrap().enemy.last_move_used, MoveId::None);
}

#[test]
fn trainer_free_turn_unsuppressed_mirror_move_uses_canonical_called_move() {
    let mut s = battle(TrainerClass::Youngster);
    let bs = s.battle_state.as_mut().unwrap();
    bs.enemy.active_mon_mut().status = StatusCondition::None;
    bs.enemy.active_mon_mut().moves = [MoveId::MirrorMove, MoveId::None, MoveId::None, MoveId::None];
    bs.player.last_move_used = MoveId::Growl;
    s.run_enemy_free_turn_stack(Vec::new(), None);
    assert_eq!(s.battle_state.as_ref().unwrap().enemy.last_move_used, MoveId::Growl);
    assert!(messages(&s).contains("MIRROR MOVE"));
    assert!(messages(&s).contains("GROWL"));
}
