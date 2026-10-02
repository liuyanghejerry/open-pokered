//! Production regression cases checked against pret/pokered core.asm/effects.asm.
//! The expected values come from ROM control flow, not the legacy turn oracle.
use dotzuki_engine::battle::rng::ScriptedRng;
use dotzuki_engine::battle::stack::{StackDriver, TurnEvent};
use dotzuki_engine::battle::{BattleAction, BattlerRef};
use pokered_core::battle::menu::{MoveMenuState, MoveSlot};
use pokered_core::battle::pokered_rules::{self as rules, PokeredRules};
use pokered_core::battle::state::*;
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_data::{move_data::MoveData, moves::MoveId, species::Species, types::PokemonType};

fn mon(species: Species, level: u8, mv: MoveId) -> Pokemon {
    Pokemon {
        species,
        nickname: [0x50; 11],
        level,
        hp: 400,
        max_hp: 400,
        attack: 100,
        defense: 100,
        speed: 100,
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
fn state(mv: MoveId, level: u8) -> BattleState {
    let mut p = mon(Species::Pikachu, level, mv);
    p.speed = 200;
    new_battle_state(
        BattleType::Wild,
        vec![p],
        vec![mon(Species::Snorlax, 50, MoveId::Splash)],
    )
}
fn turn(bs: &mut BattleState, mv: MoveId, bytes: Vec<u8>) -> Vec<TurnEvent<PokeredRules>> {
    rules::install_canonical();
    rules::clear_current_moves();
    rules::clear_levels();
    rules::set_mimic_choice(None, false);
    rules::set_current_move(BattlerRef::PLAYER, *MoveData::get(mv).unwrap());
    rules::set_current_move(
        BattlerRef::OPPONENT,
        *MoveData::get(MoveId::Splash).unwrap(),
    );
    let (mut es, mut fx) = rules::runtime::engine_state_from_legacy(bs);
    let (_, log) = StackDriver::execute_turn_logged(
        &PokeredRules,
        &mut es,
        &mut fx,
        [BattleAction::Fight { move_: mv }, BattleAction::Nothing],
        &mut ScriptedRng::new(bytes),
    );
    rules::runtime::apply_engine_to_legacy(bs, &es, &fx);
    log.events
}
fn screen_turn(screen: &mut BattleScreen, mv: MoveId) {
    screen.rng = rules::runtime::StdBattleRng::from_seed(42);
    screen.phase = BattlePhase::MoveSelect;
    screen.move_menu = Some(MoveMenuState::new(vec![MoveSlot {
        move_id: mv,
        current_pp: 20,
        max_pp: 20,
        is_disabled: false,
    }]));
    screen.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
}

#[test]
fn x_accuracy_bypasses_roll_but_not_dig() {
    // core.asm MoveHitTest checks INVULNERABLE before USING_X_ACCURACY.
    let mut bs = state(MoveId::Tackle, 50);
    bs.player.set_status2(status2::USING_X_ACCURACY);
    let log = turn(&mut bs, MoveId::Tackle, vec![255, 255, 255, 255]);
    assert!(!log
        .iter()
        .any(|e| matches!(e,TurnEvent::Missed{actor} if *actor==BattlerRef::PLAYER)));
    assert!(bs.enemy.active_mon().hp < 400);
    bs.enemy.set_status1(status1::INVULNERABLE);
    let hp = bs.enemy.active_mon().hp;
    turn(&mut bs, MoveId::Tackle, vec![255, 255]);
    assert_eq!(bs.enemy.active_mon().hp, hp);

    let mut bs = state(MoveId::Fissure, 50);
    bs.player.set_status2(status2::USING_X_ACCURACY);
    bs.player.active_mon_mut().speed = 50;
    turn(&mut bs, MoveId::Fissure, vec![255]);
    assert_eq!(
        bs.enemy.active_mon().hp,
        400,
        "OHKO still rejects a slower user"
    );
}

#[test]
fn confusion_uses_typeless_modified_own_stats_and_cancels_charge() {
    // core.asm 3672-3697: CalculateDamage only; no AdjustDamageForMoveType.
    let mut bs = state(MoveId::Fly, 50);
    bs.player
        .set_status1(status1::CONFUSED | status1::CHARGING_UP | status1::INVULNERABLE);
    bs.player.confused_turns_left = 3;
    bs.player.selected_move = MoveId::Fly;
    turn(&mut bs, MoveId::Fly, vec![0, 255, 255]);
    assert_eq!(bs.player.active_mon().hp, 381); // (22*40/50)+2 = 19
    assert!(!bs.player.has_status1(status1::CHARGING_UP));
    assert!(
        !bs.player.has_status1(status1::INVULNERABLE),
        "self-hit clears every other status1 bit"
    );
    let mut bs = state(MoveId::Tackle, 50);
    bs.player.set_status1(status1::CONFUSED);
    bs.player.confused_turns_left = 3;
    bs.player.stat_stages.attack = 2;
    bs.player.active_mon_mut().status = StatusCondition::Burn;
    turn(&mut bs, MoveId::Tackle, vec![0, 255]);
    assert_eq!(bs.player.active_mon().hp, 356); // 19 self-hit plus burn 25
}

#[test]
fn full_paralysis_cancels_fly_but_preserves_original_invulnerability_bug() {
    let mut bs = state(MoveId::Fly, 50);
    bs.player.active_mon_mut().status = StatusCondition::Paralysis;
    bs.player
        .set_status1(status1::CHARGING_UP | status1::INVULNERABLE);
    bs.player.selected_move = MoveId::Fly;
    turn(&mut bs, MoveId::Fly, vec![0]);
    assert!(!bs.player.has_status1(status1::CHARGING_UP));
    assert!(bs.player.has_status1(status1::INVULNERABLE));
    assert_eq!(bs.enemy.active_mon().hp, 400);
}

#[test]
fn fixed_level_damage_and_psywave_rejection_use_real_level() {
    for mv in [MoveId::SeismicToss, MoveId::NightShade] {
        let mut bs = state(mv, 10);
        turn(&mut bs, mv, vec![0, 255, 255]);
        assert_eq!(bs.enemy.active_mon().hp, 390);
    }
    let mut bs = state(MoveId::Psywave, 50);
    turn(&mut bs, MoveId::Psywave, vec![0, 0, 75, 255, 74]);
    assert_eq!(
        bs.enemy.active_mon().hp,
        326,
        "player rejects zero and values >= 75"
    );
}

#[test]
fn self_boosts_and_heals_skip_enemy_accuracy_and_invulnerability() {
    let mut bs = state(MoveId::SwordsDance, 50);
    bs.enemy
        .set_status1(status1::CHARGING_UP | status1::INVULNERABLE);
    bs.enemy.selected_move = MoveId::Dig;
    turn(&mut bs, MoveId::SwordsDance, vec![255]);
    assert_eq!(bs.player.stat_stages.attack, 2);
    for mv in [MoveId::Recover, MoveId::Softboiled, MoveId::Rest] {
        let mut bs = state(mv, 50);
        bs.player.active_mon_mut().hp = 145;
        turn(&mut bs, mv, vec![255]);
        assert_eq!(
            bs.player.active_mon().hp,
            145,
            "original 255-HP recovery bug for {mv:?}"
        );
        assert_eq!(bs.player.active_mon().status, StatusCondition::None);
    }
}

#[test]
fn disable_selects_known_slot_and_skips_empty_pp_on_player() {
    let mut bs = state(MoveId::Disable, 50);
    turn(&mut bs, MoveId::Disable, vec![0, 0, 0]);
    assert_eq!(bs.enemy.disabled_move, 1, "target need never have moved");
    let mut bs = state(MoveId::Splash, 50);
    bs.player.active_mon_mut().moves = [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None];
    bs.player.active_mon_mut().pp = [0, 2, 0, 0];
    rules::install_canonical();
    rules::clear_current_moves();
    rules::set_current_move(
        BattlerRef::OPPONENT,
        *MoveData::get(MoveId::Disable).unwrap(),
    );
    let (mut es, mut fx) = rules::runtime::engine_state_from_legacy(&bs);
    StackDriver::execute_turn(
        &PokeredRules,
        &mut es,
        &mut fx,
        [
            BattleAction::Nothing,
            BattleAction::Fight {
                move_: MoveId::Disable,
            },
        ],
        &mut ScriptedRng::new(vec![0, 0, 1, 0]),
    );
    rules::runtime::apply_engine_to_legacy(&mut bs, &es, &fx);
    assert_eq!(
        bs.player.disabled_move, 2,
        "random zero-PP slot must be redrawn"
    );
}

#[test]
fn blocked_turns_and_charge_gather_do_not_spend_pp() {
    for status in [StatusCondition::Sleep(3), StatusCondition::Freeze] {
        let mut p = mon(Species::Pikachu, 10, MoveId::Tackle);
        p.status = status;
        let mut screen = BattleScreen::from_parties(
            true,
            &[p],
            &[mon(Species::Snorlax, 50, MoveId::Splash)],
            None,
        );
        screen_turn(&mut screen, MoveId::Tackle);
        assert_eq!(
            screen.battle_state.as_ref().unwrap().player.active_mon().pp[0],
            20
        );
    }
    let p = mon(Species::Pikachu, 50, MoveId::Fly);
    let mut screen = BattleScreen::from_parties(
        true,
        &[p],
        &[mon(Species::Snorlax, 50, MoveId::Splash)],
        None,
    );
    screen_turn(&mut screen, MoveId::Fly);
    assert_eq!(
        screen.battle_state.as_ref().unwrap().player.active_mon().pp[0],
        20
    );
    screen_turn(&mut screen, MoveId::Fly);
    assert_eq!(
        screen.battle_state.as_ref().unwrap().player.active_mon().pp[0],
        19
    );
}

#[test]
fn failed_mirror_move_spends_pp_only_after_status_gates() {
    for (status, expected) in [(StatusCondition::None, 19), (StatusCondition::Sleep(3), 20)] {
        let mut player = mon(Species::Pikachu, 50, MoveId::MirrorMove);
        player.status = status;
        let enemy = mon(Species::Snorlax, 50, MoveId::Splash);
        let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
        screen_turn(&mut screen, MoveId::MirrorMove);
        assert_eq!(
            screen.battle_state.as_ref().unwrap().player.active_mon().pp[0],
            expected
        );
    }
}

#[test]
fn mimic_choices_preserve_remaining_pp_and_original_party_moves() {
    let p = mon(Species::Pikachu, 50, MoveId::Mimic);
    let mut e = mon(Species::Snorlax, 50, MoveId::Splash);
    e.moves = [MoveId::Splash, MoveId::Growl, MoveId::None, MoveId::None];
    e.pp = [20, 20, 0, 0];
    let mut screen = BattleScreen::from_parties(true, &[p], &[e], None);
    screen_turn(&mut screen, MoveId::Mimic);
    assert_eq!(screen.phase, BattlePhase::MoveSelect);
    assert_eq!(
        screen.move_menu.as_ref().unwrap().moves()[1].move_id,
        MoveId::Growl
    );
    assert_eq!(
        screen.battle_state.as_ref().unwrap().player.active_mon().pp[0],
        20,
        "pending menu is reversible"
    );
    screen.update_frame(BattleInput {
        down: true,
        ..BattleInput::none()
    });
    screen.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
    let bs = screen.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.active_mon().moves[0], MoveId::Growl);
    assert_eq!(bs.player.active_mon().pp[0], 19);
    screen_turn(&mut screen, MoveId::Growl);
    let bs = screen.battle_state.as_mut().unwrap();
    bs.player.reset_volatile_status();
    assert_eq!(bs.player.active_mon().moves[0], MoveId::Mimic);
    assert_eq!(bs.player.active_mon().pp[0], 18);
}

#[test]
fn transform_restores_party_identity_and_copies_effective_types() {
    let mut bs = state(MoveId::Transform, 50);
    bs.player.active_mon_mut().species = Species::Ditto;
    bs.enemy.active_mon_mut().dv_bytes = [0xab, 0xcd];
    bs.enemy.transform_catch_rate = Some(123);
    bs.enemy.conversion_type1 = Some(PokemonType::Ghost);
    bs.enemy.conversion_type2 = Some(PokemonType::Bug);
    turn(&mut bs, MoveId::Transform, vec![255]);
    assert_eq!(bs.player.active_mon().species, Species::Snorlax);
    assert_eq!(bs.player.active_mon().pp, [5, 0, 0, 0]);
    assert_eq!(bs.player.active_mon().dv_bytes, [0xab, 0xcd]);
    assert_eq!(bs.player.transform_catch_rate, Some(123));
    assert_eq!(bs.player.conversion_type1, Some(PokemonType::Ghost));
    bs.player.active_mon_mut().hp = 321;
    bs.player.reset_volatile_status();
    assert_eq!(bs.player.active_mon().species, Species::Ditto);
    assert_eq!(bs.player.active_mon().moves[0], MoveId::Transform);
    assert_eq!(bs.player.active_mon().hp, 321);
    assert_eq!(bs.player.active_mon().dv_bytes, [0xff; 2]);
    assert_eq!(bs.player.transform_catch_rate, None);
    let p = mon(Species::Ditto, 50, MoveId::Transform);
    let e = mon(Species::Snorlax, 50, MoveId::Splash);
    let mut screen = BattleScreen::from_parties(true, &[p], &[e], None);
    screen_turn(&mut screen, MoveId::Transform);
    let mut save = pokered_core::save::SaveData::new();
    let mut ow = pokered_core::overworld::screen::OverworldScreen::new(
        pokered_data::maps::MapId::Route1,
        None,
        pokered_data::impl_traits::PokemonRedData,
    );
    pokered_core::battle::settlement::settle_battle_into_save(&mut screen, &mut save, &mut ow);
    assert_eq!(save.party.get(0).unwrap().species, Species::Ditto);
}

#[test]
fn catching_a_transformed_non_ditto_preserves_original_ditto_assumption_bug() {
    use pokered_core::battle::menu::BagMenuState;
    use pokered_data::items::ItemId;
    let p = mon(Species::Snorlax, 50, MoveId::Splash);
    let mut e = mon(Species::Pidgey, 10, MoveId::Transform);
    e.dv_bytes = [0x12, 0x34];
    let mut screen = BattleScreen::from_parties(true, &[p], &[e], None);
    let bs = screen.battle_state.as_mut().unwrap();
    rules::install_canonical();
    rules::clear_current_moves();
    rules::set_current_move(
        BattlerRef::OPPONENT,
        *MoveData::get(MoveId::Transform).unwrap(),
    );
    let (mut es, mut fx) = rules::runtime::engine_state_from_legacy(bs);
    StackDriver::execute_turn(
        &PokeredRules,
        &mut es,
        &mut fx,
        [
            BattleAction::Nothing,
            BattleAction::Fight {
                move_: MoveId::Transform,
            },
        ],
        &mut ScriptedRng::new(vec![255]),
    );
    rules::runtime::apply_engine_to_legacy(bs, &es, &fx);
    assert_eq!(bs.enemy.active_mon().species, Species::Snorlax);
    bs.enemy.active_mon_mut().hp = 23;
    bs.enemy.active_mon_mut().status = StatusCondition::Burn;
    screen.player_bag.add_item(ItemId::MasterBall, 1);
    screen.phase = BattlePhase::BagSelect;
    screen.bag_menu = Some(BagMenuState::new(vec![(ItemId::MasterBall, 1)]));
    screen.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
    let caught = screen.captured_mon.as_ref().unwrap();
    assert_eq!(caught.species, Species::Ditto);
    assert_eq!(caught.dv_bytes, [0x12, 0x34]);
    assert_eq!(
        caught.moves,
        [MoveId::Transform, MoveId::None, MoveId::None, MoveId::None]
    );
    assert_eq!(caught.pp, [10, 0, 0, 0]);
    assert_eq!(caught.hp, 23);
    assert_eq!(caught.status, StatusCondition::Burn);
    assert_eq!(caught.level, 10);
}

#[test]
fn conversion_and_transform_preserve_their_own_invulnerability_checks() {
    let mut bs = state(MoveId::Conversion, 50);
    bs.enemy.set_status1(status1::INVULNERABLE);
    turn(&mut bs, MoveId::Conversion, vec![255]);
    assert_eq!(bs.player.conversion_type1, None);
    let mut bs = state(MoveId::Transform, 50);
    bs.player.set_status1(status1::INVULNERABLE);
    turn(&mut bs, MoveId::Transform, vec![255]);
    assert_eq!(bs.player.active_mon().species, Species::Pikachu);
}

#[test]
fn transformed_exp_uses_original_species_growth_and_learnset() {
    let original = pokered_core::pokemon::stats::create_pokemon_with_moves(
        Species::Ditto,
        6,
        [0xff; 2],
        [MoveId::Transform, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let enemy = mon(Species::Snorlax, 50, MoveId::Splash);
    let mut normal = new_battle_state(
        BattleType::Wild,
        vec![original.clone()],
        vec![enemy.clone()],
    );
    let mut transformed = new_battle_state(BattleType::Wild, vec![original], vec![enemy]);
    turn(&mut transformed, MoveId::Transform, vec![255]);
    pokered_core::battle::experience::gain::gain_experience(
        &mut normal,
        Species::Snorlax,
        20,
        false,
    );
    pokered_core::battle::experience::gain::gain_experience(
        &mut transformed,
        Species::Snorlax,
        20,
        false,
    );
    let normal = normal.player.active_mon();
    let restored = transformed.player.persistent_party();
    assert_eq!(restored[0].species, Species::Ditto);
    assert_eq!(restored[0].level, normal.level);
    assert_eq!(restored[0].total_exp, normal.total_exp);
    assert_eq!(restored[0].stat_exp, normal.stat_exp);
    assert_eq!(restored[0].max_hp, normal.max_hp);
}

#[test]
fn converted_poison_type_blocks_primary_poison() {
    let mut bs = state(MoveId::Poisonpowder, 50);
    bs.enemy.conversion_type1 = Some(PokemonType::Poison);
    turn(&mut bs, MoveId::Poisonpowder, vec![0]);
    assert_eq!(bs.enemy.active_mon().status, StatusCondition::None);
}

#[test]
fn sleep_overwrites_existing_status_on_hyperbeam_recharge() {
    let mut bs = state(MoveId::Hypnosis, 50);
    bs.enemy.active_mon_mut().status = StatusCondition::Burn;
    bs.enemy.set_status2(status2::NEEDS_TO_RECHARGE);
    turn(&mut bs, MoveId::Hypnosis, vec![255, 0, 3]);
    assert_eq!(bs.enemy.active_mon().status, StatusCondition::Sleep(7)); // 255&7
    assert!(!bs.enemy.has_status2(status2::NEEDS_TO_RECHARGE));
}

#[test]
fn critical_rounding_and_screen_stat_low_byte_match_rom() {
    use pokered_core::battle::damage::{calculate_damage, crit_chance, DamageParams};
    assert_eq!(
        crit_chance(75, true, true),
        72,
        "two shifts before high-crit shifts"
    );
    let mut p = DamageParams {
        attacker_level: 50,
        move_power: 100,
        move_type: PokemonType::Normal,
        move_id: MoveId::Pound,
        attack_stat: 200,
        defense_stat: 600,
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: PokemonType::Fire,
        attacker_type2: PokemonType::Fire,
        defender_type1: PokemonType::Normal,
        defender_type2: PokemonType::Normal,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: true,
        is_explode_effect: false,
        attacker_burned: false,
    };
    assert_eq!(calculate_damage(&p).damage, 52, "1200/4 low byte is 44");
    p.is_explode_effect = true;
    assert_eq!(
        calculate_damage(&p).damage,
        102,
        "Explosion halves after scaling/low-byte wrapping"
    );
}

#[test]
fn critical_damage_uses_party_stats_instead_of_badge_or_transform_copies() {
    // core.asm 4060-4074: the critical attacker is wPartyMonNAttack.
    let mut bs = state(MoveId::Tackle, 50);
    bs.player.badge_boosted_stats = Some([300, 100, 200, 100]);
    bs.player.stat_stages.attack = 6;
    bs.player.active_mon_mut().status = StatusCondition::Burn;
    bs.enemy.set_status3(status3::HAS_REFLECT_UP);
    turn(&mut bs, MoveId::Tackle, vec![0, 0, 255]);
    assert_eq!(
        bs.enemy.active_mon().hp,
        354,
        "critical 31 damage, Normal STAB => 46"
    );

    let mut bs = state(MoveId::Transform, 50);
    bs.enemy.active_mon_mut().attack = 300;
    turn(&mut bs, MoveId::Transform, vec![255]);
    assert_eq!(bs.player.active_mon().attack, 300);
    turn(&mut bs, MoveId::Tackle, vec![0, 0, 255]);
    assert_eq!(
        bs.enemy.active_mon().hp,
        354,
        "Transform does not change party critical Attack"
    );
}

#[test]
fn transformed_enemy_crit_recalculates_own_level_except_in_link_battles() {
    for (link, damage) in [(false, 4), (true, 13)] {
        let mut p = mon(Species::Pikachu, 50, MoveId::Splash);
        p.speed = 200;
        let e = mon(Species::Ditto, 10, MoveId::Transform);
        let mut bs = new_battle_state(BattleType::Wild, vec![p], vec![e]);
        bs.link_battle = link;
        let enemy_turn = |bs: &mut BattleState, mv, bytes| {
            rules::install_canonical();
            rules::clear_current_moves();
            rules::set_current_move(BattlerRef::OPPONENT, *MoveData::get(mv).unwrap());
            let (mut es, mut fx) = rules::runtime::engine_state_from_legacy(bs);
            StackDriver::execute_turn(
                &PokeredRules,
                &mut es,
                &mut fx,
                [BattleAction::Nothing, BattleAction::Fight { move_: mv }],
                &mut ScriptedRng::new(bytes),
            );
            rules::runtime::apply_engine_to_legacy(bs, &es, &fx);
        };
        enemy_turn(&mut bs, MoveId::Transform, vec![255]);
        assert_eq!(bs.enemy.active_mon().species, Species::Pikachu);
        enemy_turn(&mut bs, MoveId::Tackle, vec![0, 0, 255]);
        // GetEnemyMonStat: non-link (55+15)*2*10/100+5=19 Attack;
        // link uses original party Attack=100. Critical own level is 10.
        assert_eq!(bs.player.active_mon().hp, 400 - damage, "link={link}");
    }
}

#[test]
fn exp_flags_reset_and_only_final_level_move_is_learned() {
    let mut bs = state(MoveId::Tackle, 50);
    bs.player
        .party
        .push(mon(Species::Charmander, 50, MoveId::Scratch));
    bs.party_gain_exp_flags[0] = true;
    bs.party_gain_exp_flags[1] = true;
    let result = pokered_core::battle::experience::gain::gain_experience(
        &mut bs,
        Species::Snorlax,
        50,
        false,
    );
    assert_eq!(bs.party_gain_exp_flags[..2], [true, false]);
    assert_eq!(result.exp_gains.len(), 2);
    assert_eq!(result.exp_gains[0].1, result.exp_gains[1].1);
    let mut mon = mon(Species::Bulbasaur, 6, MoveId::Tackle);
    mon.total_exp = pokered_core::battle::experience::growth::exp_for_level(
        pokered_data::species::GrowthRate::MediumSlow,
        9,
    );
    let result = pokered_core::battle::experience::level_up::process_level_up(&mut mon);
    assert_eq!(result.new_level, 9);
    assert!(
        !mon.moves.contains(&MoveId::LeechSeed),
        "skipped level seven is not learned"
    );
}

#[test]
fn exp_notices_keep_each_mon_gain_and_level_together() {
    use pokered_core::battle::experience::gain::{gain_experience, ExperienceNotice};
    let make = |species| {
        pokered_core::pokemon::stats::create_pokemon_with_moves(
            species,
            6,
            [0xff; 2],
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    let mut bs = new_battle_state(
        BattleType::Wild,
        vec![make(Species::Bulbasaur), make(Species::Charmander)],
        vec![mon(Species::Snorlax, 50, MoveId::Splash)],
    );
    bs.party_gain_exp_flags[..2].fill(true);
    let result = gain_experience(&mut bs, Species::Snorlax, 50, false);
    assert!(matches!(
        result.notices[0],
        ExperienceNotice::Gained { party_index: 0, .. }
    ));
    assert!(matches!(
        result.notices[1],
        ExperienceNotice::Leveled { party_index: 0, .. }
    ));
    let next = result
        .notices
        .iter()
        .position(|n| matches!(n, ExperienceNotice::Gained { party_index: 1, .. }))
        .unwrap();
    assert!(next >= 2);
    assert!(matches!(
        result.notices[next + 1],
        ExperienceNotice::Leveled { party_index: 1, .. }
    ));
}

#[test]
fn trainer_ai_uses_first_matching_type_chart_row() {
    let p = mon(Species::Bulbasaur, 50, MoveId::Splash);
    let mut e = mon(Species::NidoranM, 50, MoveId::PoisonSting);
    e.moves[1] = MoveId::Scratch;
    let mut bs = new_battle_state(BattleType::Trainer, vec![p], vec![e]);
    bs.player.active_mon_mut().type1 = PokemonType::Grass;
    bs.player.active_mon_mut().type2 = PokemonType::Poison;
    let mut scores = [10; 4];
    pokered_core::battle::trainer_ai::move_choice::apply_layer3(
        &mut scores,
        &bs.enemy.active_mon().moves,
        &bs.enemy,
        &bs.player,
    );
    assert_eq!(
        scores[..2],
        [9, 10],
        "Poison→Grass row precedes Poison→Poison"
    );
}
