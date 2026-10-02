//! TrainerAI precedes ExecuteEnemyMove and per-actor residual in original core.asm.
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

fn has_message(s: &BattleScreen, text: &str) -> bool {
    messages(s).iter().any(|message| {
        message
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .contains(text)
    })
}

#[test]
fn brock_full_heal_replaces_each_forced_turn_without_consuming_its_lock() {
    // core.asm:416/454 calls TrainerAI before any ExecuteEnemyMove status gates.
    // AIUseFullHeal replaces that call, then HandlePoisonBurnLeechSeed runs.
    for enemy_first in [false, true] {
        for (mv, flags1, flags2) in [
            (MoveId::Fly, status1::CHARGING_UP | status1::INVULNERABLE, 0),
            (MoveId::Thrash, status1::THRASHING_ABOUT, 0),
            (MoveId::Bide, status1::STORING_ENERGY, 0),
            (MoveId::HyperBeam, 0, status2::NEEDS_TO_RECHARGE),
            (MoveId::Rage, 0, status2::USING_RAGE),
            (MoveId::Wrap, status1::USING_TRAPPING_MOVE, 0),
        ] {
            let mut s = screen(MoveId::Splash, mv, enemy_first);
            let bs = s.battle_state.as_mut().unwrap();
            bs.enemy.active_mon_mut().status = StatusCondition::Poison;
            bs.enemy.selected_move = mv;
            bs.enemy.set_status1(flags1);
            bs.enemy.set_status2(flags2);
            bs.enemy.num_attacks_left = 3;
            bs.enemy.bide_accumulated_damage = 17;
            bs.damage = 37;
            act(&mut s, MoveId::Splash);
            let bs = s.battle_state.as_ref().unwrap();
            assert!(
                bs.enemy.active_mon().status.is_none(),
                "{mv:?}/{enemy_first}"
            );
            assert_eq!(
                bs.enemy.active_mon().hp,
                400,
                "cure precedes poison residual: {mv:?}/{enemy_first}"
            );
            assert_eq!(
                bs.player.active_mon().hp,
                400,
                "item replaces forced strike: {mv:?}/{enemy_first}"
            );
            assert_eq!(bs.enemy.battle_status1 & flags1, flags1);
            assert_eq!(bs.enemy.battle_status2 & flags2, flags2);
            assert_eq!(bs.enemy.num_attacks_left, 3);
            assert_eq!(bs.enemy.bide_accumulated_damage, 17);
            assert_eq!(bs.enemy.last_move_used, MoveId::None);
            assert_eq!(s.enemy_ai_count, 4);
            assert!(
                has_message(&s, "FULL HEAL"),
                "actual item pages: {:?}",
                messages(&s)
            );
            assert!(!has_message(&s, "must recharge") && !has_message(&s, "hurt by POISON"));
            if mv == MoveId::HyperBeam {
                act(&mut s, MoveId::Splash);
                assert!(!s
                    .battle_state
                    .as_ref()
                    .unwrap()
                    .enemy
                    .has_status2(status2::NEEDS_TO_RECHARGE));
                assert_eq!(s.enemy_ai_count, 4);
                assert!(has_message(&s, "must recharge"), "{:?}", messages(&s));
            }
        }
    }
}

#[test]
fn a_faster_player_can_trigger_brocks_full_heal_in_the_same_turn() {
    let mut s = screen(MoveId::Poisonpowder, MoveId::Tackle, false);
    s.battle_state
        .as_mut()
        .unwrap()
        .player
        .set_status2(status2::USING_X_ACCURACY);
    act(&mut s, MoveId::Poisonpowder);
    let bs = s.battle_state.as_ref().unwrap();
    assert!(bs.enemy.active_mon().status.is_none());
    assert_eq!(bs.enemy.active_mon().hp, 400);
    assert_eq!(bs.player.active_mon().hp, 400);
    assert_eq!(s.enemy_ai_count, 4);
    assert!(
        has_message(&s, "FULL HEAL"),
        "actual item pages: {:?}",
        messages(&s)
    );
}

#[test]
fn a_player_first_ko_cancels_ai_consultation_and_its_random_draw() {
    let make = |count| {
        let mut s = screen(MoveId::SeismicToss, MoveId::Tackle, false);
        s.battle_state.as_mut().unwrap().enemy.active_mon_mut().hp = 20;
        s.battle_state
            .as_mut()
            .unwrap()
            .enemy
            .active_mon_mut()
            .status = StatusCondition::Poison;
        s.battle_state
            .as_mut()
            .unwrap()
            .player
            .set_status2(status2::USING_X_ACCURACY);
        s.enemy_ai_count = count;
        s
    };
    let mut enabled = make(5);
    let mut exhausted = make(0);
    act(&mut enabled, MoveId::SeismicToss);
    act(&mut exhausted, MoveId::SeismicToss);
    assert_eq!(
        enabled.battle_state.as_ref().unwrap().enemy.active_mon().hp,
        0
    );
    assert_eq!(enabled.enemy_ai_count, 5);
    assert_eq!(enabled.rng.next_u8(), exhausted.rng.next_u8());
    assert!(!has_message(&enabled, "FULL HEAL"));
}

#[test]
fn ai_full_heal_clears_badly_poisoned_before_leech_seed_residual() {
    for enemy_first in [false, true] {
        let mut s = screen(MoveId::Splash, MoveId::Bide, enemy_first);
        let bs = s.battle_state.as_mut().unwrap();
        bs.player.active_mon_mut().hp = 200;
        bs.enemy.active_mon_mut().status = StatusCondition::Poison;
        bs.enemy.set_status3(status3::BADLY_POISONED);
        bs.enemy.set_status2(status2::SEEDED);
        bs.enemy.set_status1(status1::STORING_ENERGY);
        bs.enemy.selected_move = MoveId::Bide;
        bs.enemy.num_attacks_left = 3;
        bs.enemy.toxic_counter = 2;
        act(&mut s, MoveId::Splash);
        let bs = s.battle_state.as_ref().unwrap();
        assert!(bs.enemy.active_mon().status.is_none());
        // AICureStatus clears BADLY_POISONED before residual; seed drains 25.
        // The old counter byte remains unchanged, but no longer scales seed.
        assert_eq!(bs.enemy.active_mon().hp, 375);
        assert_eq!(bs.player.active_mon().hp, 225);
        assert_eq!(bs.enemy.toxic_counter, 2);
        assert!(!bs.enemy.has_status3(status3::BADLY_POISONED));
        assert_eq!(bs.enemy.num_attacks_left, 3);
    }
}

#[test]
fn player_first_transform_pays_original_pp_before_installing_the_copied_five_pp() {
    let mut s = screen(MoveId::Transform, MoveId::Tackle, false);
    s.battle_state
        .as_mut()
        .unwrap()
        .enemy
        .active_mon_mut()
        .status = StatusCondition::Poison;
    act(&mut s, MoveId::Transform);
    let bs = s.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.active_mon().moves[0], MoveId::Tackle);
    assert_eq!(bs.player.active_mon().pp[0], 5);
    assert_eq!(bs.player.original_identity.as_ref().unwrap().1.pp[0], 19);
    assert_eq!(bs.player.active_mon().hp, 400);
    assert_eq!(s.enemy_ai_count, 4);
}

#[test]
fn player_first_initial_bide_still_pays_its_one_pp() {
    let mut s = screen(MoveId::Bide, MoveId::Tackle, false);
    s.battle_state
        .as_mut()
        .unwrap()
        .enemy
        .active_mon_mut()
        .status = StatusCondition::Poison;
    act(&mut s, MoveId::Bide);
    let bs = s.battle_state.as_ref().unwrap();
    assert!(bs.player.has_status1(status1::STORING_ENERGY));
    assert_eq!(bs.player.active_mon().pp[0], 19);
}

#[test]
fn player_first_damage_can_trigger_erikas_potion_before_poison_residual() {
    // Erika checks current HP < max/10, after a faster player's attack.
    let mut observed = false;
    for seed in 0..64 {
        let mut s = BattleScreen::from_parties(
            false,
            &[mon(MoveId::SeismicToss, 200)],
            &[mon(MoveId::Bide, 10)],
            Some(TrainerClass::Erika),
        );
        s.rng = StdBattleRng::from_seed(seed);
        let count = s.enemy_ai_count;
        let bs = s.battle_state.as_mut().unwrap();
        bs.player.set_status2(status2::USING_X_ACCURACY);
        bs.enemy.active_mon_mut().hp = 60;
        bs.enemy.active_mon_mut().status = StatusCondition::Poison;
        bs.enemy.set_status1(status1::STORING_ENERGY);
        bs.enemy.selected_move = MoveId::Bide;
        bs.enemy.num_attacks_left = 3;
        act(&mut s, MoveId::SeismicToss);
        if has_message(&s, "SUPER POTION") {
            let bs = s.battle_state.as_ref().unwrap();
            assert_eq!(bs.enemy.active_mon().hp, 35); // 60 - 50 + 50 - 25
            assert_eq!(bs.player.active_mon().hp, 400);
            assert_eq!(bs.enemy.num_attacks_left, 3);
            assert_eq!(s.enemy_ai_count, count - 1);
            observed = true;
            break;
        }
    }
    assert!(observed, "the original 50% item gate must be exercised");
}

#[test]
fn a_potion_caps_hp_before_the_enemys_poison_tick() {
    let mut observed = false;
    for seed in 0..64 {
        let mut s = BattleScreen::from_parties(
            false,
            &[mon(MoveId::Splash, 200)],
            &[mon(MoveId::Thrash, 10)],
            Some(TrainerClass::Blaine),
        );
        s.rng = StdBattleRng::from_seed(seed);
        let bs = s.battle_state.as_mut().unwrap();
        bs.enemy.active_mon_mut().hp = 399;
        bs.enemy.active_mon_mut().status = StatusCondition::Poison;
        bs.enemy.set_status1(status1::THRASHING_ABOUT);
        bs.enemy.selected_move = MoveId::Thrash;
        bs.enemy.num_attacks_left = 3;
        act(&mut s, MoveId::Splash);
        if has_message(&s, "SUPER POTION") {
            let bs = s.battle_state.as_ref().unwrap();
            assert_eq!(bs.enemy.active_mon().hp, 375); // cap400, then /16 poison
            assert_eq!(bs.enemy.num_attacks_left, 3);
            assert_eq!(bs.player.active_mon().hp, 400);
            observed = true;
            break;
        }
    }
    assert!(observed);
}

#[test]
fn an_ai_switch_uses_outgoing_speed_and_ticks_the_incoming_mon() {
    let mut observed = false;
    for seed in 0..64 {
        let mut outgoing = mon(MoveId::Fly, 10);
        outgoing.status = StatusCondition::Poison;
        let mut incoming = mon(MoveId::Tackle, 1000);
        incoming.status = StatusCondition::Poison;
        let mut s = BattleScreen::from_parties(
            false,
            &[mon(MoveId::SeismicToss, 200)],
            &[outgoing, incoming],
            Some(TrainerClass::Juggler),
        );
        s.rng = StdBattleRng::from_seed(seed);
        let bs = s.battle_state.as_mut().unwrap();
        bs.player.set_status2(status2::USING_X_ACCURACY);
        bs.enemy.set_status1(status1::CHARGING_UP);
        bs.enemy.selected_move = MoveId::Fly;
        act(&mut s, MoveId::SeismicToss);
        if s.battle_state.as_ref().unwrap().enemy.active_pokemon_index == 1 {
            let bs = s.battle_state.as_ref().unwrap();
            assert_eq!(
                bs.enemy.party[0].hp, 350,
                "faster player hits outgoing, which does not tick"
            );
            assert_eq!(
                bs.enemy.active_mon().hp,
                375,
                "incoming gets its own poison residual"
            );
            assert_eq!(bs.player.active_mon().hp, 400);
            assert!(!bs.enemy.has_status1(status1::CHARGING_UP));
            observed = true;
            break;
        }
    }
    assert!(observed);
}
