use super::*;

const SAMPLE: Case = Case {
    map: MapId::Route1,
    x: 6,
    y: 25,
    flag: EventFlag::EVENT_GOT_POTION_SAMPLE,
    item: ItemId::Potion,
    face: LEFT,
    flag_before_sound: true,
};

fn finish(session: &mut Session) {
    for _ in 0..8000 {
        assert!(
            !session.phase("SoundWait"),
            "unexpected extra sample fanfare"
        );
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
}

fn assert_saved(session: &Session, quantity: u16) {
    let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
    assert_ne!(
        saved.game_data.event_flags[SAMPLE.flag.byte_offset()] & SAMPLE.flag.bit_mask(),
        0
    );
    assert_eq!(saved.game_data.bag.item_quantity(ItemId::Potion), quantity);
    // In-memory serialization must not rewrite the SRAM used to enter.
    assert_eq!(std::fs::read(&session.path).unwrap(), session.original_sram);
}

#[test]
fn route1_sample_keeps_receipt_after_audio_until_outer_confirmation() {
    let mut session = Session::boot(SAMPLE, false);
    assert!(
        session.received(),
        "CheckAndSetEvent runs before the introduction"
    );
    assert_eq!(session.quantity(), 0);
    session.reach("SoundWait");
    assert_eq!(session.quantity(), 1);
    let position = (
        session.game.overworld.state.player.x,
        session.game.overworld.state.player.y,
    );
    let mut audible = 0;
    for frame in 0..300 {
        audible += usize::from(session.step(if (8..11).contains(&frame) {
            B | DOWN
        } else {
            0
        }));
        assert!(session.received());
    }
    assert!(audible > 10);
    assert!(session.phase("OuterAck"));
    for _ in 0..180 {
        session.step(0);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_some());
    assert_eq!(
        (
            session.game.overworld.state.player.x,
            session.game.overworld.state.player.y
        ),
        position
    );
    finish(&mut session);
    assert_saved(&session, 1);
    session.talk();
    finish(&mut session);
    assert_eq!(session.quantity(), 1);
}

#[test]
fn route1_full_bag_still_consumes_the_one_time_sample_offer() {
    let items: Vec<_> = (1..=21)
        .map(ItemId::from_id)
        .filter(|item| *item != ItemId::Potion)
        .map(|item| (item, 1))
        .collect();
    assert_eq!(items.len(), 20);
    let mut session = Session::boot_with_items(SAMPLE, &items);
    assert!(session.received());
    finish(&mut session);
    assert_eq!(session.quantity(), 0);
    assert_eq!(session.game.save_data.game_data.bag.count(), 20);
    assert_saved(&session, 0);
    session.talk();
    finish(&mut session);
    assert!(session.received());
    for (item, quantity) in items {
        assert_eq!(
            session.game.save_data.game_data.bag.item_quantity(item),
            u16::from(quantity)
        );
    }
}
