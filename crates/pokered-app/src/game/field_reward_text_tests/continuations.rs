use super::*;

const BILL: Case = Case {
    map: MapId::BillsHouse,
    x: 4,
    y: 5,
    flag: EventFlag::EVENT_GOT_SS_TICKET,
    item: ItemId::SsTicket,
    face: UP,
    flag_before_sound: false,
};
const ROD: Case = Case {
    map: MapId::Route12SuperRodHouse,
    x: 2,
    y: 5,
    item: ItemId::SuperRod,
    // SUPER_ROD uses status byte 1, bit 5, not an EventFlag. These tests
    // use obtained() and never the parent helper's event-flag accessor.
    ..BILL
};

fn boot(case: Case, full: bool) -> Session {
    let items: Vec<_> = if full {
        (2..=21).map(|id| (ItemId::from_id(id), 1)).collect()
    } else {
        Vec::new()
    };
    Session::boot_with_preparation(case, &items, |save| {
        if case.map == MapId::BillsHouse {
            save.game_data.last_map = MapId::Route25 as u8;
            for flag in [
                EventFlag::EVENT_MET_BILL,
                EventFlag::EVENT_MET_BILL_2,
                EventFlag::EVENT_USED_CELL_SEPARATOR_ON_BILL,
                EventFlag::EVENT_BILL_SAID_USE_CELL_SEPARATOR,
            ] {
                save.game_data.event_flags[flag.byte_offset()] |= flag.bit_mask();
            }
            for (bit, hidden) in [(0x61, true), (0x62, false), (0x63, true), (7, true), (9, false)] {
                let byte = &mut save.game_data.toggleable_object_flags[bit / 8];
                if hidden { *byte |= 1 << (bit % 8); }
                else { *byte &= !(1 << (bit % 8)); }
            }
        }
    })
}

fn obtained(session: &Session) -> bool {
    if session.case.map == MapId::BillsHouse {
        session.received()
    } else {
        session.game.overworld.script_flags().get("EVENT_GOT_SUPER_ROD").copied().unwrap_or(false)
    }
}

fn guards(session: &Session) -> (bool, bool) {
    let flags = session.game.overworld.toggleable_object_flags();
    (flags[7 / 8] & (1 << (7 % 8)) != 0, flags[9 / 8] & (1 << (9 % 8)) != 0)
}

fn visible(session: &Session) -> String {
    session.game.overworld.displayed_field_dialogue()
        .and_then(|text| text.get_display_text())
        .map(|(first, second)| format!("{first} {second}"))
        .unwrap_or_default()
}

fn answer(session: &mut Session, yes: bool) {
    for frame in 0..8000 {
        if session.game.overworld.active_script_effect_value()
            .is_some_and(|value| value["state"]["phase"] == "Poll") {
            session.step(0);
            session.step(if yes { A } else { B });
            for _ in 0..20 { session.step(0); }
            return;
        }
        // AskFieldYesNo exposes menu readiness separately from the nested
        // question's paragraph waits. Release B before the choice opens.
        let question_wait = session.game.overworld.pending_choice.is_none()
            && session.game.overworld.displayed_field_dialogue()
                .is_some_and(|text| text.waiting_for_input());
        session.step(if question_wait && frame % 8 < 2 { B } else { 0 });
    }
    panic!("question never reached the choice: {:?}", session.game.overworld.active_script_effect_value());
}

fn finish(session: &mut Session, forbid_fanfare: bool) {
    for _ in 0..8000 {
        if forbid_fanfare { assert!(!session.phase("SoundWait")); }
        if session.phase("OuterAck") { break; }
        session.step(if session.game.overworld.dialogue_needs_button() { B } else { 0 });
    }
    assert!(session.phase("OuterAck"));
    for _ in 0..30 { session.step(0); }
    assert!(session.game.overworld.displayed_field_dialogue().is_some());
    for _ in 0..30 { session.step(A); }
    assert!(session.game.overworld.displayed_field_dialogue().is_some());
    for _ in 0..30 { session.step(0); }
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
    assert!(session.game.overworld.script_engine_idle());
    assert_eq!((session.game.overworld.state.player.x, session.game.overworld.state.player.y),
        (session.case.x.into(), session.case.y.into()));
}

#[test]
fn bill_keeps_ticket_receipt_and_guard_state_until_confirmation_after_actual_audio() {
    let mut session = boot(BILL, false);
    session.reach("SoundWait");
    assert_eq!((session.quantity(), obtained(&session), guards(&session)), (1, false, (true, false)));
    let mut audible = 0;
    for frame in 0..300 {
        audible += usize::from(session.step(if (8..11).contains(&frame) { B | DOWN } else { 0 }));
        assert!(!obtained(&session));
        assert_eq!(guards(&session), (true, false));
    }
    assert!(audible > 10);
    assert!(session.phase("Prompt(Return)"));
    assert!(visible(&session).contains("S.S.TICKET"));
    session.step(A);
    for _ in 0..16 { session.step(0); }
    assert!(obtained(&session));
    assert_eq!(guards(&session), (false, true));
    finish(&mut session, true);
    let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
    assert_eq!(saved.game_data.bag.item_quantity(ItemId::SsTicket), 1);
    assert_ne!(saved.game_data.event_flags[BILL.flag.byte_offset()] & BILL.flag.bit_mask(), 0);
    assert_eq!(saved.game_data.toggleable_object_flags[0] & 0x80, 0);
    assert_ne!(saved.game_data.toggleable_object_flags[1] & 2, 0);
    session.talk();
    finish(&mut session, true);
    assert_eq!(session.quantity(), 1);
}

#[test]
fn bill_accepts_either_button_held_across_the_real_fanfare() {
    for button in [A, B] {
        let mut session = boot(BILL, false);
        session.reach("SoundWait");
        for _ in 0..4 { session.step(0); }
        let mut audible = 0;
        for _ in 0..300 {
            audible += usize::from(session.step(button));
            if obtained(&session) { break; }
        }
        assert!(audible > 10);
        assert!(obtained(&session), "no release/new press is required after WaitForSoundToFinish");
        // The scene dispatches the following two guard commands after SetFlag.
        // Keep the same button held while those commands finish.
        for _ in 0..16 { session.step(button); }
        assert_eq!(guards(&session), (false, true));
        assert_eq!(session.quantity(), 1);
    }
}

#[test]
fn super_rod_keeps_receipt_for_the_leading_paragraph_and_persists_its_early_status_bit() {
    let mut session = boot(ROD, false);
    answer(&mut session, true);
    assert_eq!((session.quantity(), obtained(&session)), (1, true));
    session.reach("SoundWait");
    let mut audible = 0;
    for frame in 0..300 {
        audible += usize::from(session.step(if (8..11).contains(&frame) { A | DOWN } else { 0 }));
        assert_eq!((session.quantity(), obtained(&session)), (1, true));
    }
    assert!(audible > 10);
    assert!(session.phase("Prompt(FirstParagraph)"));
    assert!(visible(&session).contains("SUPER ROD"));
    finish(&mut session, true);
    let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
    assert_eq!(saved.game_data.bag.item_quantity(ItemId::SuperRod), 1);
    assert_ne!(saved.game_data.status_flags[0] & (1 << 5), 0);
    session.talk();
    finish(&mut session, true);
    assert_eq!(session.quantity(), 1);
}

#[test]
fn bill_and_super_rod_full_bag_leave_gifts_and_bill_guards_unchanged() {
    for case in [BILL, ROD] {
        let mut session = boot(case, true);
        if case.map == MapId::Route12SuperRodHouse { answer(&mut session, true); }
        finish(&mut session, true);
        assert!(!obtained(&session));
        assert_eq!(session.quantity(), 0);
        assert_eq!(session.game.save_data.game_data.bag.count(), 20);
        if case.map == MapId::BillsHouse { assert_eq!(guards(&session), (true, false)); }
        assert_eq!(std::fs::read(&session.path).unwrap(), session.original_sram);
    }
}

#[test]
fn super_rod_decline_does_not_give_an_item_or_set_the_status_bit() {
    let mut session = boot(ROD, false);
    answer(&mut session, false);
    finish(&mut session, true);
    assert_eq!((session.quantity(), obtained(&session)), (0, false));
    assert_eq!(std::fs::read(&session.path).unwrap(), session.original_sram);
}
