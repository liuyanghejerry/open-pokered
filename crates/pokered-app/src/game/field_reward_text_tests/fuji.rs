use super::*;

const FUJI: Case = Case {
    map: MapId::MrFujisHouse,
    x: 3,
    y: 2,
    flag: EventFlag::EVENT_GOT_POKE_FLUTE,
    item: ItemId::PokeFlute,
    face: UP,
    flag_before_sound: false,
};

#[test]
fn flute_receipt_waits_for_confirmation_and_commits_after_the_explanation() {
    let mut session = Session::boot(FUJI, false);
    session.reach("SoundWait");
    assert_eq!((session.quantity(), session.received()), (1, false));
    let mut audible = 0;
    for frame in 0..300 {
        if session.step(if (8..11).contains(&frame) { B | DOWN } else { 0 }) {
            audible += 1;
        }
        assert!(!session.received());
    }
    assert!(audible > 10);
    assert!(session.phase("Prompt(FirstParagraph)"));
    let (first, second) = session
        .game
        .overworld
        .displayed_field_dialogue()
        .unwrap()
        .get_display_text()
        .unwrap();
    assert!(format!("{first} {second}").contains("received"));
    assert_eq!(
        (session.game.overworld.state.player.x, session.game.overworld.state.player.y),
        (3, 2)
    );
    session.reach("OuterAck");
    assert_eq!((session.quantity(), session.received()), (1, true));
    for _ in 0..32 {
        session.step(A);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_some());
    for _ in 0..32 {
        session.step(0);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
    let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
    assert_eq!(saved.game_data.bag.item_quantity(ItemId::PokeFlute), 1);
    assert_ne!(saved.game_data.event_flags[FUJI.flag.byte_offset()] & FUJI.flag.bit_mask(), 0);
    session.talk();
    session.reach("OuterAck");
    assert_eq!(session.quantity(), 1);
    session.step(B);
    for _ in 0..16 {
        session.step(0);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
}

#[test]
fn full_bag_keeps_fuji_available_and_waits_for_the_outer_acknowledgement() {
    let mut session = Session::boot(FUJI, true);
    session.reach("OuterAck");
    assert_eq!((session.quantity(), session.received()), (0, false));
    assert_eq!(session.game.save_data.game_data.bag.count(), 20);
    for _ in 0..120 {
        session.step(0);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_some());
    session.step(B);
    for _ in 0..16 {
        session.step(0);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
    assert!(!session.received());
    assert_eq!(std::fs::read(&session.path).unwrap(), session.original_sram);
}
