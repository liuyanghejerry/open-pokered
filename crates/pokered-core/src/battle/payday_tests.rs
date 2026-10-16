//! EndOfBattle's Pay Day prompt and result-history eligibility. These exercise
//! production battle callers and save writeback; they are not native recordings.
use super::*;
use crate::battle::settlement::settle_battle_into_save;
use crate::overworld::OverworldScreen;
use crate::pokemon::stats::create_pokemon_with_moves;
use crate::save::SaveData;
use crate::snapshot::BattleSnapshot;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};

fn mon(species: Species, level: u8, move_: MoveId) -> state::Pokemon {
    create_pokemon_with_moves(
        species,
        level,
        [0x9a, 0x78],
        [move_, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap()
}

fn wild() -> BattleScreen {
    let mut s = BattleScreen::from_parties(
        true,
        &[mon(Species::Meowth, 100, MoveId::Teleport)],
        &[mon(Species::Snorlax, 100, MoveId::Splash)],
        None,
    );
    s.player_name = Some("RED".to_string());
    s.player_money = 100;
    s.phase = BattlePhase::PlayerMenu;
    s.rng = pokered_rules::runtime::StdBattleRng::from_seed(42);
    s.battle_state.as_mut().unwrap().total_payday_money = 40;
    s
}

fn a() -> BattleInput {
    BattleInput {
        a: true,
        ..BattleInput::none()
    }
}

fn pages(s: &BattleScreen) -> Vec<String> {
    match &s.phase {
        BattlePhase::ShowingText { messages, .. } => messages.clone(),
        _ => Vec::new(),
    }
}

fn finish_text(s: &mut BattleScreen) {
    for frame in 0..400 {
        if !matches!(s.phase, BattlePhase::ShowingText { .. }) {
            return;
        }
        assert!(s.settlement.is_none(), "no settlement before text returns");
        s.update_frame(if frame % 2 == 0 {
            a()
        } else {
            BattleInput::none()
        });
    }
    panic!("text did not return: {:?}", s.phase);
}

fn finish_battle(s: &mut BattleScreen) -> Vec<String> {
    let mut seen = Vec::new();
    for frame in 0..2000 {
        if let Some(text) = s.current_message.as_ref() {
            if seen.last() != Some(text) {
                seen.push(text.clone());
            }
        }
        if matches!(
            s.update_frame(if frame % 2 == 0 {
                a()
            } else {
                BattleInput::none()
            }),
            ScreenAction::Transition(GameScreen::Overworld)
        ) {
            assert!(s.settlement.is_some());
            return seen;
        }
    }
    panic!("battle did not return: {:?}", s.phase);
}

fn use_item(s: &mut BattleScreen, item: ItemId) {
    assert_eq!(s.phase, BattlePhase::PlayerMenu);
    s.player_bag.add_item(item, 1).unwrap();
    s.update_frame(BattleInput {
        down: true,
        ..BattleInput::none()
    });
    s.update_frame(a());
    assert_eq!(s.phase, BattlePhase::BagSelect);
    s.update_frame(a());
}

fn writeback(s: &mut BattleScreen, money: u32) -> (u32, Option<&'static str>) {
    let mut save = SaveData::new();
    save.game_data.player_money = money;
    for mon in &s.battle_state.as_ref().unwrap().player.party {
        save.party.add(*mon).unwrap();
    }
    let mut field = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
    let before_npcs = serde_json::to_value(&field.npc_states).unwrap();
    let out = settle_battle_into_save(s, &mut save, &mut field);
    if matches!(out.outcome, Some("ran" | "fled")) {
        assert_eq!(
            serde_json::to_value(&field.npc_states).unwrap(),
            before_npcs
        );
    }
    (save.game_data.player_money, out.outcome)
}

#[test]
fn trainer_prize_then_pickup_snapshot_waits_for_ack_and_pays_once() {
    let mut s = BattleScreen::from_parties(
        false,
        &[mon(Species::Meowth, 50, MoveId::Swift)],
        &[mon(Species::Rattata, 14, MoveId::Tackle)],
        Some(TrainerClass::Youngster),
    );
    s.player_name = Some("RED".to_string());
    s.end_battle_text = Some("You beat me!".to_string());
    s.battle_state.as_mut().unwrap().total_payday_money = 40;
    s.phase = BattlePhase::TrainerVictory {
        phase: VictoryPhase::EndBattleText,
        wait_frames: 60,
        player_won: true,
    };
    s.update_frame(BattleInput::none());
    assert_eq!(pages(&s), ["You beat me!", "RED got $210 for\nwinning!"]);
    finish_text(&mut s);
    s.update_frame(BattleInput::none());
    assert_eq!(pages(&s), ["RED picked up\n$40!"]);
    assert!(s.payday_pickup_shown && s.settlement.is_none());

    let json = serde_json::to_vec(&BattleSnapshot::capture(&s)).unwrap();
    let snap: BattleSnapshot = serde_json::from_slice(&json).unwrap();
    let mut resumed = BattleScreen::new(true);
    snap.restore_into(&mut resumed);
    for _ in 0..100 {
        resumed.update_frame(BattleInput::none());
    }
    assert!(resumed.settlement.is_none(), "pickup still awaits a button");
    assert_eq!(pages(&resumed), ["RED picked up\n$40!"]);
    let seen = finish_battle(&mut resumed);
    assert_eq!(seen.iter().filter(|s| s.contains("picked up")).count(), 1);
    let settlement = resumed.settlement.as_ref().unwrap();
    assert_eq!(
        (settlement.money_gained, settlement.payday_bonus),
        (250, 40)
    );
    assert_eq!(writeback(&mut resumed, 100), (350, Some("win")));
}

#[test]
fn actual_doll_and_teleport_preserve_zero_result_and_pick_up_coins() {
    for doll in [true, false] {
        let mut s = wild();
        if doll {
            use_item(&mut s, ItemId::PokeDoll);
        } else {
            s.execute_turn_with_move(0);
        }
        let seen = finish_battle(&mut s);
        assert_eq!(s.original_battle_result, Some(0));
        assert_eq!(
            s.settlement.as_ref().unwrap().outcome,
            BattleOutcome::Escaped
        );
        assert_eq!(s.settlement.as_ref().unwrap().money_gained, 40);
        assert_eq!(seen.iter().filter(|s| s.contains("picked up")).count(), 1);
        assert!(!seen.iter().any(|s| s.contains("winning")));
        assert_eq!(
            writeback(&mut s, 100),
            (140, Some(if doll { "fled" } else { "ran" }))
        );
    }
}

fn after_player_faint(enemy_level: u8, backup_move: MoveId, free_turn: bool) -> BattleScreen {
    let mut lead = mon(Species::Meowth, 100, MoveId::Splash);
    lead.hp = 1;
    let mut s = BattleScreen::from_parties(
        true,
        &[lead, mon(Species::Abra, 100, backup_move)],
        &[mon(Species::Snorlax, enemy_level, MoveId::Swift)],
        None,
    );
    s.player_name = Some("RED".to_string());
    s.phase = BattlePhase::PlayerMenu;
    s.battle_state.as_mut().unwrap().total_payday_money = 40;
    s.rng = pokered_rules::runtime::StdBattleRng::from_seed(42);
    if free_turn {
        s.run_enemy_free_turn_stack(Vec::new(), Some((MoveId::Swift, 0)));
    } else {
        s.execute_turn_with_move(0);
    }
    assert_eq!(s.original_battle_result, Some(1));
    finish_text(&mut s);
    assert_eq!(s.phase, BattlePhase::PlayerFaintSwitch);
    s.update_frame(BattleInput {
        down: true,
        ..BattleInput::none()
    });
    s.update_frame(a());
    finish_text(&mut s);
    assert_eq!(s.phase, BattlePhase::PlayerMenu);
    assert_eq!(
        s.original_battle_result,
        Some(1),
        "switching does not reset history"
    );
    s
}

#[test]
fn real_faint_receipts_from_both_turn_callers_survive_switch_and_escape() {
    for free_turn in [false, true] {
        for doll in [false, true] {
            let mut s = after_player_faint(100, MoveId::Teleport, free_turn);
            if doll {
                use_item(&mut s, ItemId::PokeDoll);
            } else {
                s.execute_turn_with_move(0);
            }
            let seen = finish_battle(&mut s);
            assert_eq!(s.original_battle_result, Some(1));
            assert!(!seen.iter().any(|s| s.contains("picked up")));
            assert_eq!(s.settlement.as_ref().unwrap().money_gained, 0);
            assert_eq!(writeback(&mut s, 100).0, 100);
        }
    }
}

#[test]
fn later_real_enemy_faint_restores_pickup_eligibility() {
    let mut s = after_player_faint(1, MoveId::Swift, true);
    s.execute_turn_with_move(0);
    assert_eq!(s.original_battle_result, Some(0));
    let seen = finish_battle(&mut s);
    assert_eq!(seen.iter().filter(|s| s.contains("picked up")).count(), 1);
    assert_eq!(s.settlement.as_ref().unwrap().money_gained, 40);
    assert_eq!(writeback(&mut s, 100), (140, Some("win")));
}

#[test]
fn successful_run_capture_and_safari_run_write_two_and_never_pay() {
    for action in 0..3 {
        let mut s = wild();
        match action {
            0 => s.handle_run(),
            1 => use_item(&mut s, ItemId::MasterBall),
            _ => {
                s.is_safari = true;
                s.resolve_safari_action(menu::SafariMenuAction::Run);
            }
        }
        let seen = finish_battle(&mut s);
        assert_eq!(s.original_battle_result, Some(2));
        assert!(!seen.iter().any(|s| s.contains("picked up")));
        let settlement = s.settlement.as_ref().unwrap();
        assert_eq!((settlement.money_gained, settlement.payday_bonus), (0, 0));
        assert_eq!(writeback(&mut s, 100).0, 100);
    }
}

#[test]
fn zero_link_and_loss_skip_pickup_but_full_wallet_and_capped_pot_still_print() {
    for case in 0..4 {
        let mut s = wild();
        match case {
            0 => s.battle_state.as_mut().unwrap().total_payday_money = 0,
            1 => s.link_mode = true,
            2 => {}
            _ => {
                let bs = s.battle_state.as_mut().unwrap();
                bs.total_payday_money = 999_990;
                bs.player.party[0].moves[0] = MoveId::PayDay;
                bs.player.party[0].pp[0] = 20;
                s.execute_turn_with_move(0);
                assert_eq!(s.battle_state.as_ref().unwrap().total_payday_money, 999_999);
                finish_text(&mut s);
            }
        }
        s.phase = BattlePhase::BattleOver {
            won: case != 2,
            escaped: false,
            wait_frames: 0,
        };
        let seen = finish_battle(&mut s);
        let pickup = seen.iter().filter(|s| s.contains("picked up")).count();
        assert_eq!(pickup, usize::from(case == 3));
        if case == 3 {
            assert!(seen.contains(&"RED picked up\n$999999!".to_string()));
            assert_eq!(writeback(&mut s, 999_999).0, 999_999);
        } else {
            assert_eq!(s.settlement.as_ref().unwrap().money_gained, 0);
        }
    }
}

#[test]
fn legacy_snapshots_do_not_fabricate_missing_escape_history() {
    for won in [false, true] {
        let mut old = serde_json::to_value(BattleSnapshot::capture(&wild())).unwrap();
        old.as_object_mut()
            .unwrap()
            .remove("original_battle_result");
        old.as_object_mut().unwrap().remove("payday_pickup_shown");
        let snap: BattleSnapshot = serde_json::from_value(old).unwrap();
        let mut s = wild();
        snap.restore_into(&mut s);
        assert_eq!(s.original_battle_result, None);
        assert!(!s.payday_pickup_shown);
        if won {
            s.phase = BattlePhase::BattleOver {
                won: true,
                escaped: false,
                wait_frames: 0,
            };
        } else {
            use_item(&mut s, ItemId::PokeDoll);
        }
        let seen = finish_battle(&mut s);
        assert_eq!(
            seen.iter().filter(|s| s.contains("picked up")).count(),
            usize::from(won)
        );
        assert_eq!(
            s.settlement.as_ref().unwrap().money_gained,
            if won { 40 } else { 0 }
        );
    }
}

/// Unit caller input: physical edges remain separate from the one serialized
/// foreground ledger. The ledger starts Unknown and only real text polls fill it.
struct SourceInput {
    owner: crate::foreground_input::ForegroundJoypad,
    physical_previous: u8,
}

impl SourceInput {
    fn new() -> Self {
        Self {
            owner: Default::default(),
            physical_previous: 0,
        }
    }

    fn step(&mut self, s: &mut BattleScreen, held: u8) -> (ScreenAction, FightTextReceipt) {
        use crate::foreground_input::{A, B, DOWN, LEFT, RIGHT, UP};
        let pressed = held & !self.physical_previous;
        self.physical_previous = held;
        let mut receipt = FightTextReceipt::default();
        let action = s.update_frame_with_terminal_input(
            BattleInput {
                a: pressed & A != 0,
                b: pressed & B != 0,
                up: pressed & UP != 0,
                down: pressed & DOWN != 0,
                left: pressed & LEFT != 0,
                right: pressed & RIGHT != 0,
            },
            BattleTextFrame {
                held_buttons: held,
                settings: Some(BattleTextSettings::fight_menu(1, false)),
            },
            &mut receipt,
            &mut self.owner,
        );
        (action, receipt)
    }

    fn reach_prompt(
        &mut self,
        s: &mut BattleScreen,
        kind: crate::text::terminal_input::TerminalTextKind,
    ) {
        use crate::foreground_input::A;
        for frame in 0..4000 {
            if s.terminal_text_work()
                .is_some_and(|work| work.kind == kind && work.waiting_for_input())
            {
                return;
            }
            self.step(s, if frame % 2 == 0 { A } else { 0 });
            assert!(
                s.settlement.is_none(),
                "settlement must wait for the requested real PROMPT"
            );
        }
        panic!("source caller did not reach {kind:?}: {:?}", s.phase);
    }
}

#[test]
fn typed_trainer_prize_then_pickup_snapshot_keeps_the_real_last_and_pays_once() {
    use crate::foreground_input::{A, B, SELECT, START};
    use crate::text::terminal_input::TerminalTextKind;
    let mut s = BattleScreen::from_parties(
        false,
        &[mon(Species::Meowth, 50, MoveId::Swift)],
        &[mon(Species::Rattata, 14, MoveId::Tackle)],
        Some(TrainerClass::Youngster),
    );
    s.player_name = Some("RED".into());
    s.end_battle_text = Some("You beat me!".into());
    s.battle_state.as_mut().unwrap().total_payday_money = 40;
    s.phase = BattlePhase::TrainerVictory {
        phase: VictoryPhase::EndBattleText,
        wait_frames: 60,
        player_won: true,
    };
    let mut input = SourceInput::new();
    input.step(&mut s, SELECT);
    assert_eq!(
        pages(&s),
        ["You beat me!"],
        "the quote is still a separate call"
    );
    assert_eq!(
        input.owner.last(),
        None,
        "constructing the continuation is not a poll"
    );
    input.reach_prompt(&mut s, TerminalTextKind::Prize);
    assert_eq!(
        s.terminal_text_work().unwrap().visible_text(),
        "RED got $210 for\nwinning!"
    );
    input.step(&mut s, 0);
    assert!(input.step(&mut s, A | START | SELECT).1.press_ab);
    input.reach_prompt(&mut s, TerminalTextKind::PayDay);
    assert_eq!(
        s.terminal_text_work().unwrap().visible_text(),
        "RED picked up\n$40!"
    );
    assert!(s.payday_pickup_shown && s.settlement.is_none());
    input.step(&mut s, 0); // Actual ManualTextScroll poll establishes neutral.

    let bytes = serde_json::to_vec(&(BattleSnapshot::capture(&s), input.owner.clone())).unwrap();
    let (snapshot, owner): (BattleSnapshot, crate::foreground_input::ForegroundJoypad) =
        serde_json::from_slice(&bytes).unwrap();
    snapshot.restore_into(&mut s);
    input.owner = owner;
    assert!(s.payday_pickup_shown && s.settlement.is_none());
    assert_eq!(
        s.terminal_text_work().unwrap().kind,
        TerminalTextKind::PayDay
    );
    let (_, receipt) = input.step(&mut s, B | START | SELECT);
    assert!(receipt.press_ab);
    let last = input.owner.last();
    assert_eq!(last, Some(B | START | SELECT));
    let (action, receipt) = input.step(&mut s, A);
    assert!(matches!(
        action,
        ScreenAction::Transition(GameScreen::Overworld)
    ));
    assert!(!receipt.press_ab, "settlement cannot replay the pickup ACK");
    assert_eq!(
        input.owner.last(),
        last,
        "settlement raw is not a foreground poll"
    );
    assert_eq!(s.settlement.as_ref().unwrap().money_gained, 250);
    assert_eq!(s.settlement.as_ref().unwrap().payday_bonus, 40);
    assert_eq!(writeback(&mut s, 100), (350, Some("win")));
}

#[test]
fn typed_actual_doll_and_teleport_return_the_pickup_poll_even_with_escaped_outcome() {
    use crate::foreground_input::{B, SELECT, START};
    use crate::text::terminal_input::TerminalTextKind;
    for doll in [true, false] {
        let mut s = wild();
        if doll {
            use_item(&mut s, ItemId::PokeDoll);
        } else {
            s.execute_turn_with_move(0);
        }
        let mut input = SourceInput::new();
        input.reach_prompt(&mut s, TerminalTextKind::PayDay);
        assert_eq!(s.original_battle_result, Some(0));
        assert_eq!(
            s.terminal_text_work().unwrap().visible_text(),
            "RED picked up\n$40!"
        );
        input.step(&mut s, 0);
        assert!(input.step(&mut s, B | START | SELECT).1.press_ab);
        let last = input.owner.last();
        assert_eq!(last, Some(B | START | SELECT));
        let (action, receipt) = input.step(&mut s, SELECT);
        assert!(matches!(
            action,
            ScreenAction::Transition(GameScreen::Overworld)
        ));
        assert!(!receipt.press_ab);
        assert_eq!(
            input.owner.last(),
            last,
            "escaped is not a history invalidation"
        );
        assert_eq!(
            s.settlement.as_ref().unwrap().outcome,
            BattleOutcome::Escaped
        );
        assert_eq!(
            writeback(&mut s, 100),
            (140, Some(if doll { "fled" } else { "ran" }))
        );
    }
}
