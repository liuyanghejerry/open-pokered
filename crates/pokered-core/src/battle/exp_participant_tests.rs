//! Participant-mask boundaries from core.asm and ItemUseMedicine. These test
//! the production BattleScreen entry points; GainExperience's separate test
//! for a still-flagged HP-zero mon deliberately retains the source divisor.
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

fn battle() -> BattleScreen {
    let mon = |species, level, moves| {
        create_pokemon_with_moves(species, level, [0x9a, 0x78], moves).unwrap()
    };
    let moves = [MoveId::Cut, MoveId::None, MoveId::None, MoveId::None];
    let players = vec![
        mon(Species::Bulbasaur, 20, moves),
        mon(Species::Bulbasaur, 20, moves),
    ];
    let enemy_moves = [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None];
    let enemies = vec![
        mon(Species::Weedle, 6, enemy_moves),
        mon(Species::Caterpie, 6, enemy_moves),
    ];
    BattleScreen::from_parties(false, &players, &enemies, Some(TrainerClass::BugCatcher))
}

fn masks(s: &BattleScreen) -> ([bool; 6], [bool; 6]) {
    let bs = s.battle_state.as_ref().unwrap();
    (bs.party_gain_exp_flags, bs.party_fought_flags)
}

const FIRST: [bool; 6] = [true, false, false, false, false, false];
const SECOND: [bool; 6] = [false, true, false, false, false, false];
const BOTH: [bool; 6] = [true, true, false, false, false, false];

#[test]
fn exp_participant_initial_send_out_and_switch_set_both_masks() {
    let mut s = battle();
    assert_eq!(masks(&s), (FIRST, FIRST));
    s.apply_player_switch(1);
    assert_eq!(masks(&s), (BOTH, BOTH));
}

#[test]
fn exp_participant_faint_and_forced_switch_leave_survivor_full_share() {
    let mut s = battle();
    let initial = s.battle_state.as_ref().unwrap().player.party[1].total_exp;
    s.battle_state.as_mut().unwrap().player.party[0].hp = 0;
    assert_eq!(s.check_faint_after_turn(), BattlePhase::PlayerFaintSwitch);
    assert_eq!(masks(&s), ([false; 6], FIRST));
    s.force_switch_player(1);
    assert_eq!(masks(&s), (SECOND, BOTH));
    s.battle_state.as_mut().unwrap().enemy.party[0].hp = 0;
    s.process_exp_gain();
    let bs = s.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.party[1].total_exp - initial, 66);
    assert_eq!(bs.player.party[1].stat_exp, [40, 35, 30, 50, 20]);
    assert_eq!(masks(&s), (SECOND, SECOND));
}

#[test]
fn exp_participant_simultaneous_faint_clears_before_distribution() {
    let mut s = battle();
    s.apply_player_switch(1);
    let initial = s.battle_state.as_ref().unwrap().player.party[0].total_exp;
    let bs = s.battle_state.as_mut().unwrap();
    bs.player.party[1].hp = 0;
    bs.enemy.party[0].hp = 0;
    s.check_faint_after_turn();
    assert_eq!(masks(&s), (FIRST, BOTH));
    s.process_exp_gain();
    assert_eq!(
        s.battle_state.as_ref().unwrap().player.party[0].total_exp - initial,
        66
    );
}

fn revive_from_bag(s: &mut BattleScreen, item: ItemId) {
    s.player_bag.add_item(item, 2).unwrap();
    s.phase = BattlePhase::PlayerMenu;
    s.update_frame(BattleInput {
        down: true,
        ..BattleInput::none()
    });
    s.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
    s.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
    assert_eq!(s.phase, BattlePhase::ItemTargetSelect { item_id: item });
    s.party_cursor = 0;
    s.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
    assert_eq!(s.player_bag.items(), &[(item, 1)]);
    assert!(
        matches!(&s.phase, BattlePhase::ShowingText { next_phase, .. }
        if **next_phase == BattlePhase::EnemyFreeTurnAfterItem)
    );
}

#[test]
fn exp_participant_same_enemy_revive_restores_fainted_participant_share() {
    for item in [ItemId::Revive, ItemId::MaxRevive] {
        let mut s = battle();
        s.battle_state.as_mut().unwrap().player.party[0].hp = 0;
        s.check_faint_after_turn();
        s.force_switch_player(1);
        revive_from_bag(&mut s, item);
        assert!(s.battle_state.as_ref().unwrap().player.party[0].hp > 0);
        assert_eq!(masks(&s), (BOTH, BOTH));
        let initial = s.battle_state.as_ref().unwrap().player.party[1].total_exp;
        s.battle_state.as_mut().unwrap().enemy.party[0].hp = 0;
        s.process_exp_gain();
        assert_eq!(
            s.battle_state.as_ref().unwrap().player.party[1].total_exp - initial,
            33
        );
    }
}

#[test]
fn exp_participant_revive_does_not_add_mon_that_never_fought_current_enemy() {
    let mut s = battle();
    s.apply_player_switch(1);
    s.battle_state.as_mut().unwrap().player.party[0].hp = 0;
    s.battle_state.as_mut().unwrap().enemy.party[0].hp = 0;
    s.send_next_enemy();
    assert_eq!(masks(&s), (SECOND, SECOND));
    revive_from_bag(&mut s, ItemId::Revive);
    assert_eq!(masks(&s), (SECOND, SECOND));
}

#[test]
fn exp_participant_new_enemy_and_shift_switch_exclude_recalled_mon() {
    let mut s = battle();
    s.battle_state.as_mut().unwrap().enemy.party[0].hp = 0;
    s.send_next_enemy();
    s.apply_shift_switch(1);
    assert_eq!(masks(&s), (SECOND, SECOND));
}

#[test]
fn exp_participant_trainer_ai_switch_starts_new_participant_history() {
    let mut s = battle();
    s.apply_player_switch(1);
    let mut count = 1;
    BattleScreen::apply_enemy_ai_action_to_state(
        s.battle_state.as_mut().unwrap(),
        AiAction::SwitchPokemon,
        "BUG CATCHER",
        1,
        &mut count,
        &mut Vec::new(),
    );
    assert_eq!(masks(&s), (SECOND, SECOND));
}

#[test]
fn exp_participant_failed_revive_changes_neither_masks_nor_quantity() {
    let mut s = battle();
    s.player_bag.add_item(ItemId::Revive, 2).unwrap();
    s.apply_item_to_pokemon(ItemId::Revive, 1);
    assert_eq!(masks(&s), (FIRST, FIRST));
    assert_eq!(s.player_bag.items(), &[(ItemId::Revive, 2)]);
}

#[test]
fn exp_participant_no_eligible_gainers_still_reset_fought_history() {
    let mut s = battle();
    s.apply_player_switch(1);
    let bs = s.battle_state.as_mut().unwrap();
    bs.party_gain_exp_flags.fill(false);
    bs.enemy.party[0].hp = 0;
    let before: Vec<_> = bs.player.party.iter().map(|p| p.total_exp).collect();
    assert!(s.process_exp_gain().is_empty());
    assert_eq!(masks(&s), (SECOND, SECOND));
    let after: Vec<_> = s
        .battle_state
        .as_ref()
        .unwrap()
        .player
        .party
        .iter()
        .map(|p| p.total_exp)
        .collect();
    assert_eq!(before, after);
}
