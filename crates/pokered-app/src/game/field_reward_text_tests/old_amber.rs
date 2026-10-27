use super::*;
use pokered_data::toggleable_objects::{is_object_hidden, set_object_shown, ToggleableObject};

const SCIENTIST: Case = Case {
    map: MapId::Museum1F,
    x: 15,
    y: 3,
    flag: EventFlag::EVENT_GOT_OLD_AMBER,
    item: ItemId::OldAmber,
    face: UP,
    flag_before_sound: true,
};

fn boot(full: bool) -> Session {
    let items: Vec<_> = if full {
        (2..=21).map(|id| (ItemId::from_id(id), 1)).collect()
    } else {
        Vec::new()
    };
    Session::boot_with_preparation(SCIENTIST, &items, |save| {
        set_object_shown(
            &mut save.game_data.toggleable_object_flags,
            ToggleableObject::OLD_AMBER.bit_index(),
        );
    })
}

fn amber_hidden(session: &Session) -> bool {
    is_object_hidden(
        session.game.overworld.toggleable_object_flags(),
        ToggleableObject::OLD_AMBER.bit_index(),
    )
}

fn finish(session: &mut Session) {
    for _ in 0..8000 {
        assert!(!session.phase("SoundWait"), "unexpected extra amber fanfare");
        if session.phase("OuterAck") { break; }
        session.step(if session.game.overworld.dialogue_needs_button() { B } else { 0 });
    }
    assert!(session.phase("OuterAck"));
    for _ in 0..32 { session.step(A); }
    assert!(session.game.overworld.displayed_field_dialogue().is_some());
    for _ in 0..32 { session.step(0); }
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
    assert!(session.game.overworld.script_engine_idle());
    assert_eq!((session.game.overworld.state.player.x, session.game.overworld.state.player.y), (15, 3));
}

#[test]
fn old_amber_hides_before_fanfare_and_waits_for_receipt_confirmation() {
    let mut session = boot(false);
    assert!(!amber_hidden(&session));
    session.reach("SoundWait");
    assert_eq!(session.quantity(), 1);
    assert!(session.received());
    assert!(amber_hidden(&session));
    let mut audible = 0;
    for frame in 0..300 {
        audible += usize::from(session.step(if (8..11).contains(&frame) { B | DOWN } else { 0 }));
        assert!(session.received());
        assert!(amber_hidden(&session));
        assert_eq!(session.quantity(), 1);
        assert_eq!((session.game.overworld.state.player.x, session.game.overworld.state.player.y), (15, 3));
    }
    assert!(audible > 10);
    assert!(session.phase("OuterAck"));
    let (top, bottom) = session.game.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
    assert!(format!("{top} {bottom}").contains("OLD AMBER"));
    finish(&mut session);
    let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
    assert_eq!(saved.game_data.bag.item_quantity(ItemId::OldAmber), 1);
    assert!(is_object_hidden(&saved.game_data.toggleable_object_flags, ToggleableObject::OLD_AMBER.bit_index()));
    assert_ne!(saved.game_data.event_flags[SCIENTIST.flag.byte_offset()] & SCIENTIST.flag.bit_mask(), 0);
    session.talk();
    finish(&mut session);
    assert_eq!(session.quantity(), 1);
    assert!(amber_hidden(&session));
}

#[test]
fn old_amber_full_bag_keeps_the_display_on_repeated_talk() {
    let mut session = boot(true);
    let original = import_sram(&session.original_sram).unwrap();
    for attempt in 0..2 {
        finish(&mut session);
        assert!(!session.received());
        assert!(!amber_hidden(&session));
        assert_eq!(session.quantity(), 0);
        let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
        assert_eq!(saved.game_data.bag, original.game_data.bag);
        assert!(!is_object_hidden(&saved.game_data.toggleable_object_flags, ToggleableObject::OLD_AMBER.bit_index()));
        assert_eq!(saved.game_data.event_flags[SCIENTIST.flag.byte_offset()] & SCIENTIST.flag.bit_mask(), 0);
        assert_eq!(std::fs::read(&session.path).unwrap(), session.original_sram);
        if attempt == 0 { session.talk(); }
    }
}
