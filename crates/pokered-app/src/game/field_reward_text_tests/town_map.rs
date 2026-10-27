use super::*;
use pokered_data::toggleable_objects::{is_object_hidden, set_object_hidden, set_object_shown};

const DAISY: Case = Case {
    map: MapId::BluesHouse,
    x: 2,
    y: 4,
    flag: EventFlag::EVENT_GOT_TOWN_MAP,
    item: ItemId::TownMap,
    face: UP,
    flag_before_sound: false,
};

fn boot(dex: bool, full: bool) -> Session {
    let items: Vec<_> = if full {
        (2..=22)
            .filter(|&id| ItemId::from_id(id) != ItemId::TownMap)
            .map(|id| (ItemId::from_id(id), 1))
            .collect()
    } else {
        Vec::new()
    };
    assert_eq!(items.len(), if full { 20 } else { 0 });
    Session::boot_with_preparation(DAISY, &items, |save| {
        if dex {
            let flag = EventFlag::EVENT_GOT_POKEDEX;
            save.game_data.event_flags[flag.byte_offset()] |= flag.bit_mask();
        }
        set_object_shown(&mut save.game_data.toggleable_object_flags, 0x27);
        set_object_hidden(&mut save.game_data.toggleable_object_flags, 0x28);
        set_object_shown(&mut save.game_data.toggleable_object_flags, 0x29);
    })
}

fn map_hidden(session: &Session) -> bool {
    is_object_hidden(session.game.overworld.toggleable_object_flags(), 0x29)
}

fn finish(session: &mut Session) {
    for _ in 0..8000 {
        assert!(!session.phase("SoundWait"), "unexpected extra map fanfare");
        if session.phase("OuterAck") {
            break;
        }
        session.step(if session.game.overworld.dialogue_needs_button() { B } else { 0 });
    }
    assert!(session.phase("OuterAck"));
    for _ in 0..32 { session.step(A); }
    assert!(session.game.overworld.displayed_field_dialogue().is_some());
    for _ in 0..32 { session.step(0); }
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
    assert!(session.game.overworld.script_engine_idle());
    assert_eq!((session.game.overworld.state.player.x, session.game.overworld.state.player.y), (2, 4));
}

#[test]
fn town_map_hides_before_fanfare_sets_event_after_it_and_keeps_the_receipt() {
    let mut session = boot(true, false);
    session.reach("SoundWait");
    assert_eq!(session.quantity(), 1);
    assert!(map_hidden(&session));
    assert!(!session.received());
    let mut audible = 0;
    for frame in 0..300 {
        audible += usize::from(session.step(if (8..11).contains(&frame) { B | DOWN } else { 0 }));
        if session.phase("SoundWait") { assert!(!session.received()); }
        assert_eq!(session.quantity(), 1);
        assert!(map_hidden(&session));
        assert_eq!((session.game.overworld.state.player.x, session.game.overworld.state.player.y), (2, 4));
    }
    assert!(audible > 10);
    assert!(session.phase("OuterAck"));
    assert!(session.received());
    let (top, bottom) = session.game.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
    assert!(format!("{top} {bottom}").contains("TOWN MAP"));
    finish(&mut session);
    let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
    assert_eq!(saved.game_data.bag.item_quantity(ItemId::TownMap), 1);
    assert!(is_object_hidden(&saved.game_data.toggleable_object_flags, 0x29));
    assert_ne!(saved.game_data.event_flags[DAISY.flag.byte_offset()] & DAISY.flag.bit_mask(), 0);
    session.talk();
    finish(&mut session);
    assert_eq!(session.quantity(), 1);
}

#[test]
fn town_map_without_pokedex_or_with_full_bag_preserves_the_table_and_inventory() {
    for (dex, full) in [(false, false), (true, true)] {
        let mut session = boot(dex, full);
        let original = import_sram(&session.original_sram).unwrap();
        finish(&mut session);
        assert!(!session.received());
        assert!(!map_hidden(&session));
        assert_eq!(session.quantity(), 0);
        let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
        assert_eq!(saved.game_data.bag, original.game_data.bag);
        assert!(!is_object_hidden(&saved.game_data.toggleable_object_flags, 0x29));
        assert_eq!(saved.game_data.event_flags[DAISY.flag.byte_offset()] & DAISY.flag.bit_mask(), 0);
        assert_eq!(std::fs::read(&session.path).unwrap(), session.original_sram);
        session.talk();
        finish(&mut session);
        assert_eq!(session.quantity(), 0);
    }
}
