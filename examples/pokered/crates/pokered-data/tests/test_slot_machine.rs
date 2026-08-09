use pokered_data::slot_machine::*;

#[test]
fn wheel1_has_18_symbols() {
    assert_eq!(SLOT_MACHINE_WHEEL1.len(), WHEEL_SIZE);
}

#[test]
fn wheel2_has_18_symbols() {
    assert_eq!(SLOT_MACHINE_WHEEL2.len(), WHEEL_SIZE);
}

#[test]
fn wheel3_has_18_symbols() {
    assert_eq!(SLOT_MACHINE_WHEEL3.len(), WHEEL_SIZE);
}

#[test]
fn wheel1_starts_with_seven() {
    assert_eq!(SLOT_MACHINE_WHEEL1[0], SlotSymbol::Seven);
}

#[test]
fn wheel2_starts_with_seven() {
    assert_eq!(SLOT_MACHINE_WHEEL2[0], SlotSymbol::Seven);
}

#[test]
fn wheel3_starts_with_seven() {
    assert_eq!(SLOT_MACHINE_WHEEL3[0], SlotSymbol::Seven);
}

#[test]
fn symbol_high_bytes() {
    assert_eq!(SlotSymbol::Seven.high_byte(), 0x02);
    assert_eq!(SlotSymbol::Bar.high_byte(), 0x06);
    assert_eq!(SlotSymbol::Cherry.high_byte(), 0x0A);
    assert_eq!(SlotSymbol::Fish.high_byte(), 0x0E);
    assert_eq!(SlotSymbol::Bird.high_byte(), 0x12);
    assert_eq!(SlotSymbol::Mouse.high_byte(), 0x16);
}

#[test]
fn symbol_low_bytes() {
    assert_eq!(SlotSymbol::Seven.low_byte(), 0x00);
    assert_eq!(SlotSymbol::Bar.low_byte(), 0x04);
    assert_eq!(SlotSymbol::Cherry.low_byte(), 0x08);
    assert_eq!(SlotSymbol::Fish.low_byte(), 0x0C);
    assert_eq!(SlotSymbol::Bird.low_byte(), 0x10);
    assert_eq!(SlotSymbol::Mouse.low_byte(), 0x14);
}

#[test]
fn from_high_byte_roundtrip() {
    for sym in &[
        SlotSymbol::Seven,
        SlotSymbol::Bar,
        SlotSymbol::Cherry,
        SlotSymbol::Fish,
        SlotSymbol::Bird,
        SlotSymbol::Mouse,
    ] {
        assert_eq!(SlotSymbol::from_high_byte(sym.high_byte()), Some(*sym));
    }
}

#[test]
fn from_high_byte_invalid() {
    assert_eq!(SlotSymbol::from_high_byte(0x00), None);
    assert_eq!(SlotSymbol::from_high_byte(0x01), None);
    assert_eq!(SlotSymbol::from_high_byte(0xFF), None);
}

#[test]
fn reward_payouts() {
    assert_eq!(reward_for_symbol(SlotSymbol::Seven).payout, 300);
    assert_eq!(reward_for_symbol(SlotSymbol::Bar).payout, 100);
    assert_eq!(reward_for_symbol(SlotSymbol::Cherry).payout, 8);
    assert_eq!(reward_for_symbol(SlotSymbol::Fish).payout, 15);
    assert_eq!(reward_for_symbol(SlotSymbol::Bird).payout, 15);
    assert_eq!(reward_for_symbol(SlotSymbol::Mouse).payout, 15);
}

#[test]
fn reward_flash_counts() {
    assert_eq!(reward_for_symbol(SlotSymbol::Seven).flash_count, 20);
    assert_eq!(reward_for_symbol(SlotSymbol::Bar).flash_count, 8);
    assert_eq!(reward_for_symbol(SlotSymbol::Cherry).flash_count, 2);
    assert_eq!(reward_for_symbol(SlotSymbol::Fish).flash_count, 4);
    assert_eq!(reward_for_symbol(SlotSymbol::Bird).flash_count, 4);
    assert_eq!(reward_for_symbol(SlotSymbol::Mouse).flash_count, 4);
}

#[test]
fn constants_values() {
    assert_eq!(WHEEL_OFFSET_MAX, 30);
    assert_eq!(INITIAL_SLIP_COUNTER, 4);
    assert_eq!(INITIAL_WHEEL_OFFSET, 0x1C);
    assert_eq!(SEVEN_AND_BAR_MODE_LUCKY, 250);
    assert_eq!(SEVEN_AND_BAR_MODE_NORMAL, 253);
    assert_eq!(ALLOW_MATCHES_DURATION, 60);
}

#[test]
fn bit_flag_values() {
    assert_eq!(BIT_SLOTS_CAN_WIN, 6);
    assert_eq!(BIT_SLOTS_CAN_WIN_WITH_7_OR_BAR, 7);
    assert_eq!(SLOTS_CAN_WIN, 0x40);
    assert_eq!(SLOTS_CAN_WIN_WITH_7_OR_BAR, 0x80);
}

#[test]
fn wheel1_seven_count() {
    let count = SLOT_MACHINE_WHEEL1.iter().filter(|s| **s == SlotSymbol::Seven).count();
    assert_eq!(count, 4);
}

#[test]
fn wheel1_cherry_count() {
    let count = SLOT_MACHINE_WHEEL1.iter().filter(|s| **s == SlotSymbol::Cherry).count();
    assert_eq!(count, 3);
}

#[test]
fn wheel1_bar_count() {
    let count = SLOT_MACHINE_WHEEL1.iter().filter(|s| **s == SlotSymbol::Bar).count();
    assert_eq!(count, 3);
}

#[test]
fn wheel3_bar_count() {
    // Wheel 3 has only 1 bar (making bar matches rare)
    let count = SLOT_MACHINE_WHEEL3.iter().filter(|s| **s == SlotSymbol::Bar).count();
    assert_eq!(count, 1);
}

#[test]
fn machine_status_constants() {
    assert_eq!(SLOTS_OUTOFORDER, 0);
    assert_eq!(SLOTS_OUTTOLUNCH, 1);
    assert_eq!(SLOTS_SOMEONESKEYS, 2);
}
