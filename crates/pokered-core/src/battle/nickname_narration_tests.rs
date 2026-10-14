use super::pokered_rules::runtime::{BattleNameContext, StdBattleRng};
use super::*;
use dotzuki_engine::battle::stack::{TurnEvent, TurnLog};
use dotzuki_engine::battle::BattlerRef;

fn named(species: Species, nickname: &str, level: u8) -> state::Pokemon {
    let mut mon = crate::pokemon::stats::create_pokemon_with_moves(
        species,
        level,
        [255, 255],
        [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
    )
    .unwrap();
    mon.set_nickname(nickname);
    mon
}
fn messages(screen: &BattleScreen) -> Vec<String> {
    match &screen.phase {
        BattlePhase::ShowingText { messages, .. } => messages.clone(),
        _ => screen.current_message.clone().into_iter().collect(),
    }
}

#[test]
fn real_turn_send_out_recall_and_enemy_free_move_use_nicknames() {
    let player = [
        named(Species::Bulbasaur, "Sprout", 30),
        named(Species::Squirtle, "SHELL", 30),
    ];
    let enemy = [named(Species::Rattata, "SQUEAK", 30)];
    let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
    screen.rng = StdBattleRng::from_seed(42);
    assert_eq!(screen.player_display_name(false), "Sprout");
    assert_eq!(screen.enemy_display_name(false), "SQUEAK");
    assert_eq!(screen.player_send_out_message(), "Go! Sprout!");
    screen.execute_turn_with_move(0);
    let text = messages(&screen).join(" ");
    assert!(text.contains("Sprout used TACKLE!"), "{text}");
    assert!(text.contains("Enemy SQUEAK used"), "{text}");
    let switch = screen.apply_player_switch(1).join(" ");
    assert!(switch.contains("Sprout"), "{switch}");
    assert!(switch.contains("SHELL"), "{switch}");
    screen.run_enemy_free_turn_stack(Vec::new(), Some((MoveId::Tackle, 0)));
    assert!(messages(&screen)
        .join(" ")
        .contains("Enemy SQUEAK used TACKLE!"));
}

#[test]
fn nickname_status_residual_and_faint_messages_use_both_original_names() {
    let legacy = state::new_battle_state(
        state::BattleType::Wild,
        vec![named(Species::Bulbasaur, "SPROUT", 30)],
        vec![named(Species::Rattata, "SQUEAK", 30)],
    );
    let (engine, effects) = pokered_rules::runtime::engine_state_from_legacy(&legacy);
    let names = BattleNameContext::from_legacy(&legacy);
    let mut log = TurnLog::new();
    log.push(TurnEvent::StatusInflicted {
        target: BattlerRef::PLAYER,
        status: state::StatusCondition::Poison,
    });
    log.push(TurnEvent::StatChanged {
        target: BattlerRef::OPPONENT,
        stat: stat_stages::StatIndex::Attack,
        delta: -1,
    });
    log.push(TurnEvent::Fainted {
        who: BattlerRef::OPPONENT,
    });
    let text = pokered_rules::runtime::translate_turn_named(&log, &engine, &effects, Some(&names));
    assert_eq!(
        text,
        vec![
            "SPROUT was poisoned!",
            "Enemy SQUEAK's ATTACK fell!",
            "Enemy SQUEAK fainted!"
        ]
    );
}

#[test]
fn transform_hud_and_narration_keep_original_identity_with_and_without_nickname() {
    for nickname in ["COPY", ""] {
        let player = [named(Species::Ditto, nickname, 30)];
        let enemy = [named(Species::Tauros, "BULL", 30)];
        let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
        let bs = screen.battle_state.as_mut().unwrap();
        bs.player.preserve_original_identity();
        bs.player.active_mon_mut().species = Species::Tauros;
        let (engine, effects) = pokered_rules::runtime::engine_state_from_legacy(bs);
        let names = BattleNameContext::from_legacy(bs);
        let mut log = TurnLog::new();
        log.push(TurnEvent::MoveUsed {
            actor: BattlerRef::PLAYER,
            move_: MoveId::Tackle,
        });
        let text =
            pokered_rules::runtime::translate_turn_named(&log, &engine, &effects, Some(&names));
        let expected = if nickname.is_empty() {
            "DITTO"
        } else {
            nickname
        };
        assert_eq!(screen.player_display_name(false), expected);
        assert_eq!(text[0], format!("{expected} used TACKLE!"));
        assert_eq!(screen.enemy_display_name(false), "BULL");
    }
}

#[test]
fn unnamed_battlers_keep_canonical_species_spelling() {
    let screen = BattleScreen::from_parties(
        true,
        &[named(Species::MrMime, "", 30)],
        &[named(Species::Farfetchd, "", 30)],
        None,
    );
    assert_eq!(
        screen.player_display_name(false),
        pokered_data::lang_data::species_name(Species::MrMime, false).to_uppercase()
    );
    assert_eq!(
        screen.enemy_display_name(false),
        pokered_data::lang_data::species_name(Species::Farfetchd, false).to_uppercase()
    );
}

#[test]
fn real_residual_damage_and_recharge_use_the_nickname() {
    let player = [named(Species::Bulbasaur, "SPROUT", 30)];
    let enemy = [named(Species::Rattata, "SQUEAK", 30)];
    let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
    screen.rng = StdBattleRng::from_seed(42);
    screen
        .battle_state
        .as_mut()
        .unwrap()
        .player
        .active_mon_mut()
        .status = state::StatusCondition::Poison;
    screen.execute_turn_with_move(0);
    let text = messages(&screen).join(" ");
    assert!(text.contains("SPROUT is hurt by POISON!"), "{text}");
    let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
    screen.rng = StdBattleRng::from_seed(42);
    screen
        .battle_state
        .as_mut()
        .unwrap()
        .player
        .set_status2(state::status2::NEEDS_TO_RECHARGE);
    screen.execute_turn_with_move(0);
    let text = messages(&screen).join(" ");
    assert!(text.contains("SPROUT must recharge!"), "{text}");
}
