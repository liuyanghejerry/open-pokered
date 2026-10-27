use super::*;

const WARDEN: Case = Case {
    map: MapId::WardensHouse,
    x: 2,
    y: 4,
    flag: EventFlag::EVENT_GOT_HM04,
    item: ItemId::Hm04,
    face: UP,
    flag_before_sound: false,
};
const CHAIRMAN: Case = Case {
    map: MapId::PokemonFanClub,
    x: 2,
    y: 1,
    flag: EventFlag::EVENT_GOT_BIKE_VOUCHER,
    item: ItemId::BikeVoucher,
    face: RIGHT,
    flag_before_sound: false,
};

fn visible(session: &Session) -> String {
    session
        .game
        .overworld
        .displayed_field_dialogue()
        .and_then(|text| text.get_display_text())
        .map(|(first, second)| format!("{first} {second}"))
        .unwrap_or_default()
}

fn answer(session: &mut Session, yes: bool) {
    for frame in 0..8000 {
        if session
            .game
            .overworld
            .active_script_effect_value()
            .is_some_and(|value| value["state"]["phase"] == "Poll")
        {
            session.step(0);
            session.step(if yes { A } else { B });
            for _ in 0..20 {
                session.step(0);
            }
            return;
        }
        let wait = session.game.overworld.pending_choice.is_none()
            && session
                .game
                .overworld
                .displayed_field_dialogue()
                .is_some_and(|text| text.waiting_for_input());
        session.step(if wait && frame % 8 < 2 { B } else { 0 });
    }
    panic!("chairman's question never reached YES/NO");
}

fn finish(session: &mut Session) {
    for _ in 0..8000 {
        assert!(!session.phase("SoundWait"), "unexpected extra gift fanfare");
        if session.phase("OuterAck") {
            break;
        }
        session.step(if session.game.overworld.dialogue_needs_button() {
            B
        } else {
            0
        });
    }
    assert!(session.phase("OuterAck"));
    for _ in 0..32 {
        session.step(A);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_some());
    for _ in 0..32 {
        session.step(0);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
    assert!(session.game.overworld.script_engine_idle());
    assert_eq!(
        (
            session.game.overworld.state.player.x,
            session.game.overworld.state.player.y
        ),
        (session.case.x.into(), session.case.y.into())
    );
}

fn teeth_given(session: &Session) -> bool {
    let flag = EventFlag::EVENT_GAVE_GOLD_TEETH;
    session.game.overworld.unified_flags().as_bytes()[flag.byte_offset()] & flag.bit_mask() != 0
}

#[test]
fn warden_keeps_teeth_until_both_receipt_and_popped_teeth_confirmation() {
    for full in [false, true] {
        let mut items = if full {
            (2..=20)
                .map(|id| (ItemId::from_id(id), 1))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        items.push((ItemId::GoldTeeth, 1));
        let mut session = Session::boot_with_items(WARDEN, &items);
        session.reach("SoundWait");
        let mut audible = 0;
        for frame in 0..300 {
            audible += usize::from(session.step(if (8..11).contains(&frame) {
                B | DOWN
            } else {
                0
            }));
            assert!(!session.received() && !teeth_given(&session));
            assert_eq!(
                session
                    .game
                    .save_data
                    .game_data
                    .bag
                    .item_quantity(ItemId::GoldTeeth),
                1
            );
        }
        assert!(audible > 10);
        assert!(session.phase("Prompt(FirstParagraph)"));
        assert!(visible(&session).contains("WARDEN"));
        session.step(B);
        for _ in 0..16 {
            session.step(0);
        }
        session.reach("Prompt(Return)");
        assert!(!teeth_given(&session));
        assert_eq!(
            session
                .game
                .save_data
                .game_data
                .bag
                .item_quantity(ItemId::GoldTeeth),
            1
        );
        session.step(B);
        for _ in 0..16 {
            session.step(0);
        }
        session.reach("SoundWait");
        assert!(teeth_given(&session));
        assert_eq!(
            session
                .game
                .save_data
                .game_data
                .bag
                .item_quantity(ItemId::GoldTeeth),
            0
        );
        assert_eq!((session.quantity(), session.received()), (1, false));
        for frame in 0..600 {
            if session.phase("OuterAck") {
                break;
            }
            session.step(if (8..11).contains(&frame) {
                A | DOWN
            } else {
                0
            });
            if session.phase("SoundWait") {
                assert!(!session.received());
            }
        }
        assert!(session.phase("OuterAck") && session.received());
        assert!(visible(&session).contains("HM04"));
        assert_eq!(
            session.game.save_data.game_data.bag.count(),
            if full { 20 } else { 1 }
        );
        finish(&mut session);
        let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
        assert_eq!(saved.game_data.bag.item_quantity(ItemId::GoldTeeth), 0);
        assert_eq!(saved.game_data.bag.item_quantity(ItemId::Hm04), 1);
        assert_ne!(
            saved.game_data.event_flags[WARDEN.flag.byte_offset()] & WARDEN.flag.bit_mask(),
            0
        );
        session.talk();
        finish(&mut session);
        assert_eq!(session.quantity(), 1);
    }
}

#[test]
fn bike_voucher_waits_at_receipt_and_commits_only_after_explanation() {
    let mut session = Session::boot(CHAIRMAN, false);
    answer(&mut session, true);
    session.reach("SoundWait");
    assert_eq!((session.quantity(), session.received()), (1, false));
    let mut audible = 0;
    for frame in 0..300 {
        audible += usize::from(session.step(if (8..11).contains(&frame) {
            B | DOWN
        } else {
            0
        }));
        assert!(!session.received());
    }
    assert!(audible > 10);
    assert!(session.phase("Prompt(FirstParagraph)"));
    assert!(visible(&session).contains("VOUCHER"));
    finish(&mut session);
    assert!(session.received());
    let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
    assert_eq!(saved.game_data.bag.item_quantity(ItemId::BikeVoucher), 1);
    assert_ne!(
        saved.game_data.event_flags[CHAIRMAN.flag.byte_offset()] & CHAIRMAN.flag.bit_mask(),
        0
    );
    session.talk();
    finish(&mut session);
    assert_eq!(session.quantity(), 1);
}

#[test]
fn chairman_decline_full_bag_and_existing_bike_items_never_duplicate_the_reward() {
    // A bicycle makes the chairman skip the offer before checking bag capacity.
    let full = (2..=22)
        .map(ItemId::from_id)
        .filter(|item| *item != ItemId::Bicycle)
        .map(|item| (item, 1))
        .collect::<Vec<_>>();
    assert_eq!(full.len(), 20);
    for (items, choice) in [
        (vec![], Some(false)),
        (full, Some(true)),
        (vec![(ItemId::Bicycle, 1)], None),
        (vec![(ItemId::BikeVoucher, 1)], None),
    ] {
        let mut session = Session::boot_with_items(CHAIRMAN, &items);
        if let Some(yes) = choice {
            answer(&mut session, yes);
        }
        finish(&mut session);
        assert!(!session.received());
        assert_eq!(
            session.game.save_data.game_data.bag.count() as usize,
            items.len()
        );
        for (item, quantity) in items {
            assert_eq!(
                session.game.save_data.game_data.bag.item_quantity(item),
                u16::from(quantity)
            );
        }
        assert_eq!(std::fs::read(&session.path).unwrap(), session.original_sram);
    }
}
