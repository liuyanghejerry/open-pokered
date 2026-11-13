use super::*;

const EXCHANGE: Case = Case {
    map: MapId::BikeShop,
    x: 6,
    y: 4,
    flag: EventFlag::EVENT_GOT_BICYCLE,
    item: ItemId::Bicycle,
    face: UP,
    flag_before_sound: true,
};

fn finish(session: &mut Session) {
    for _ in 0..8000 {
        assert!(!session.phase("SoundWait"), "unexpected extra exchange fanfare");
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
        (session.game.overworld.state.player.x, session.game.overworld.state.player.y),
        (EXCHANGE.x.into(), EXCHANGE.y.into())
    );
}

#[test]
fn bicycle_exchange_keeps_receipt_after_fanfare_and_persists_only_one_bicycle() {
    let mut session = Session::boot_with_items(EXCHANGE, &[(ItemId::BikeVoucher, 1)]);
    session.reach("SoundWait");
    assert!(session.received());
    assert_eq!(session.quantity(), 1);
    assert_eq!(session.game.save_data.game_data.bag.item_quantity(ItemId::BikeVoucher), 0);
    let mut audible = 0;
    for frame in 0..300 {
        audible += usize::from(session.step(if (8..11).contains(&frame) { B | DOWN } else { 0 }));
        assert!(session.received());
        assert_eq!(session.quantity(), 1);
        assert_eq!(
            (session.game.overworld.state.player.x, session.game.overworld.state.player.y),
            (EXCHANGE.x.into(), EXCHANGE.y.into())
        );
    }
    assert!(audible > 10);
    assert!(session.phase("OuterAck"));
    let text = session.game.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
    assert!(format!("{} {}", text.0, text.1).contains("exchanged"));
    finish(&mut session);
    let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
    assert_eq!(saved.game_data.bag.item_quantity(ItemId::Bicycle), 1);
    assert_eq!(saved.game_data.bag.item_quantity(ItemId::BikeVoucher), 0);
    assert_ne!(
        saved.game_data.event_flags[EXCHANGE.flag.byte_offset()] & EXCHANGE.flag.bit_mask(),
        0
    );
    session.talk();
    finish(&mut session);
    assert_eq!(session.quantity(), 1);
}

#[test]
fn full_bag_rejects_bicycle_before_removing_the_voucher_or_setting_its_event() {
    // The original GiveItem runs before RemoveItemByID, so the voucher's
    // slot cannot be reused when all 20 physical bag slots are occupied.
    let mut items: Vec<_> = (2..=21)
        .filter(|&id| ItemId::from_id(id) != ItemId::Bicycle)
        .map(|id| (ItemId::from_id(id), 1u8))
        .collect();
    items.push((ItemId::BikeVoucher, 1));
    assert_eq!(items.len(), 20);
    let mut session = Session::boot_with_items(EXCHANGE, &items);
    finish(&mut session);
    assert!(!session.received());
    assert_eq!(session.quantity(), 0);
    let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
    for (item, quantity) in items {
        assert_eq!(saved.game_data.bag.item_quantity(item), u16::from(quantity));
    }
    assert_eq!(std::fs::read(&session.path).unwrap(), session.original_sram);
}
