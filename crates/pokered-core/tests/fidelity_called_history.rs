//! Mirror Move uses live UsedMove text bytes; called moves resolve after status gates.
use dotzuki_engine::battle::rng::BattleRng;
use pokered_core::battle::menu::{MoveMenuState, MoveSlot};
use pokered_core::battle::pokered_rules::runtime::StdBattleRng;
use pokered_core::battle::state::*;
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_data::{
    moves::MoveId, species::Species, trainer_data::TrainerClass, types::PokemonType,
};

fn mon(mv: MoveId, speed: u16) -> Pokemon {
    Pokemon {
        species: Species::Snorlax,
        nickname: [0x50; 11],
        level: 50,
        hp: 400,
        max_hp: 400,
        attack: 100,
        defense: 100,
        speed,
        special: 100,
        type1: PokemonType::Normal,
        type2: PokemonType::Normal,
        moves: [mv, MoveId::None, MoveId::None, MoveId::None],
        pp: [20, 0, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xff; 2],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false,
        ot_id: 0,
        ot_name: [0x50; 11],
    }
}
fn screen(player_move: MoveId, enemy_move: MoveId, enemy_first: bool) -> BattleScreen {
    let (ps, es) = if enemy_first { (10, 200) } else { (200, 10) };
    let mut s = BattleScreen::from_parties(
        false,
        &[mon(player_move, ps)],
        &[mon(enemy_move, es)],
        Some(TrainerClass::Brock),
    );
    s.rng = StdBattleRng::from_seed(42);
    s
}
fn act(s: &mut BattleScreen, mv: MoveId) {
    s.phase = BattlePhase::MoveSelect;
    s.move_menu = Some(MoveMenuState::new(vec![MoveSlot {
        move_id: mv,
        current_pp: 20,
        max_pp: 20,
        is_disabled: false,
    }]));
    s.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
}
fn messages(s: &BattleScreen) -> &[String] {
    match &s.phase {
        BattlePhase::ShowingText { messages, .. } => messages,
        phase => panic!("unexpected phase {phase:?}"),
    }
}


#[test]
fn slower_mirror_move_copies_the_faster_actors_current_turn_on_both_sides() {
    for enemy_first in [false, true] {
        let (p, e) = if enemy_first {
            (MoveId::MirrorMove, MoveId::SeismicToss)
        } else {
            (MoveId::SeismicToss, MoveId::MirrorMove)
        };
        let mut s = screen(p, e, enemy_first);
        {
            let bs = s.battle_state.as_mut().unwrap();
            bs.player.set_status2(status2::USING_X_ACCURACY);
            bs.enemy.set_status2(status2::USING_X_ACCURACY);
        }
        act(&mut s, p);
        let bs = s.battle_state.as_ref().unwrap();
        assert_eq!(bs.player.active_mon().hp, 350, "enemy_first={enemy_first}");
        assert_eq!(bs.enemy.active_mon().hp, 350, "enemy_first={enemy_first}");
        assert_eq!(bs.player.last_move_used, MoveId::SeismicToss);
        assert_eq!(bs.enemy.last_move_used, MoveId::SeismicToss);
    }
}

#[test]
fn sleep_and_freeze_clear_used_move_before_a_slower_mirror_move() {
    for status in [StatusCondition::Sleep(2), StatusCondition::Freeze] {
        let mut s = screen(MoveId::SeismicToss, MoveId::MirrorMove, false);
        let bs = s.battle_state.as_mut().unwrap();
        bs.player.active_mon_mut().status = status;
        bs.player.last_move_used = MoveId::Tackle;
        bs.enemy.set_status2(status2::USING_X_ACCURACY);
        act(&mut s, MoveId::SeismicToss);
        let bs = s.battle_state.as_ref().unwrap();
        assert_eq!(bs.player.active_mon().hp, 400);
        assert_eq!(bs.enemy.active_mon().hp, 400);
        assert_eq!(bs.player.last_move_used, MoveId::None);
        assert!(
            messages(&s)
                .iter()
                .any(|m| m.to_ascii_lowercase().contains("failed")),
            "{:?}",
            messages(&s)
        );
    }
}

#[test]
fn mirror_move_cannot_copy_another_failed_mirror_move() {
    let mut s = screen(MoveId::MirrorMove, MoveId::Splash, false);
    s.battle_state.as_mut().unwrap().enemy.last_move_used = MoveId::MirrorMove;
    act(&mut s, MoveId::MirrorMove);
    assert!(
        messages(&s)
            .iter()
            .any(|m| m.to_ascii_lowercase().contains("failed")),
        "{:?}",
        messages(&s)
    );
}

#[test]
fn a_blocked_metronome_does_not_consume_its_move_selection_randomness() {
    for status in [StatusCondition::Sleep(2), StatusCondition::Freeze] {
        let mut called = screen(MoveId::Metronome, MoveId::Splash, false);
        let mut ordinary = screen(MoveId::Tackle, MoveId::Splash, false);
        called
            .battle_state
            .as_mut()
            .unwrap()
            .player
            .active_mon_mut()
            .status = status;
        ordinary
            .battle_state
            .as_mut()
            .unwrap()
            .player
            .active_mon_mut()
            .status = status;
        act(&mut called, MoveId::Metronome);
        act(&mut ordinary, MoveId::Tackle);
        assert_eq!(called.rng.next_u8(), ordinary.rng.next_u8());
        assert_eq!(
            called.battle_state.as_ref().unwrap().player.active_mon().pp[0],
            20
        );
    }
}

#[test]
fn metronome_quick_attack_keeps_the_selected_moves_original_turn_priority() {
    use dotzuki_engine::battle::rng::ScriptedRng;
    use dotzuki_engine::battle::stack::{StackDriver, TurnEvent};
    use dotzuki_engine::battle::{BattleAction, BattlerRef};
    use pokered_core::battle::pokered_rules::{self as rules, PokeredRules};
    use pokered_data::move_data::MoveData;
    let mut bs = new_battle_state(
        BattleType::Wild,
        vec![mon(MoveId::Metronome, 10)],
        vec![mon(MoveId::Tackle, 200)],
    );
    rules::install_canonical();
    rules::clear_current_moves();
    rules::set_current_move(
        BattlerRef::PLAYER,
        *MoveData::get(MoveId::Metronome).unwrap(),
    );
    rules::set_current_move(
        BattlerRef::OPPONENT,
        *MoveData::get(MoveId::Tackle).unwrap(),
    );
    let (mut state, mut effects) = rules::runtime::engine_state_from_legacy(&bs);
    let (_, log) = StackDriver::execute_turn_logged(
        &PokeredRules,
        &mut state,
        &mut effects,
        [
            BattleAction::Fight {
                move_: MoveId::Metronome,
            },
            BattleAction::Fight {
                move_: MoveId::Tackle,
            },
        ],
        &mut ScriptedRng::new(vec![255, 0, 255, MoveId::QuickAttack as u8, 255, 0, 255]),
    );
    let used = log
        .events
        .iter()
        .filter_map(|event| match event {
            TurnEvent::MoveUsed { actor, move_ } => Some((*actor, *move_)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        used,
        vec![
            (BattlerRef::OPPONENT, MoveId::Tackle),
            (BattlerRef::PLAYER, MoveId::QuickAttack)
        ]
    );
    rules::runtime::apply_engine_to_legacy(&mut bs, &state, &effects);
    assert_eq!(bs.player.last_move_used, MoveId::QuickAttack);
}
