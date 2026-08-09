use pokered_data::charmap;
use pokered_data::text_commands::{
    inline_control_chars, TextCommand, TEXT_COMMAND_COUNT, TX_END, TX_SOUND_LEVEL_UP,
};
use pokered_data::text_scripts::TextScriptId;

#[test]
fn charmap_encode_uppercase_a() {
    assert_eq!(charmap::encode_char('A'), Some(0x80));
}

#[test]
fn charmap_encode_zero() {
    assert_eq!(charmap::encode_char('0'), Some(0xF6));
}

#[test]
fn charmap_encode_space() {
    assert_eq!(charmap::encode_char(' '), Some(0x7F));
}

#[test]
fn charmap_decode_0x80_is_a() {
    assert_eq!(charmap::decode_char(0x80), Some("A"));
}

#[test]
fn charmap_decode_terminator() {
    assert_eq!(charmap::decode_char(0x50), None);
}

#[test]
fn charmap_roundtrip_encode_decode() {
    for ch in 'A'..='Z' {
        let encoded = charmap::encode_char(ch).unwrap();
        let decoded = charmap::decode_char(encoded).unwrap();
        assert_eq!(decoded, ch.to_string());
    }
}

#[test]
fn charmap_encode_string_with_terminator() {
    let encoded = charmap::encode_string("RED").unwrap();
    assert_eq!(encoded.len(), 4); // R, E, D, terminator
    assert_eq!(*encoded.last().unwrap(), charmap::CHAR_TERMINATOR);
}

#[test]
fn charmap_decode_string_stops_at_terminator() {
    let data = vec![0x91, 0x84, 0x83, 0x50]; // R, E, D, @
    let decoded = charmap::decode_string(&data);
    assert_eq!(decoded, "RED");
}

#[test]
fn charmap_char_space_constant() {
    assert_eq!(charmap::CHAR_SPACE, 0x7F);
}

#[test]
fn charmap_char_terminator_constant() {
    assert_eq!(charmap::CHAR_TERMINATOR, 0x50);
}

// ── TextCommand tests ─────────────────────────────────────────────

#[test]
fn text_command_from_byte_tx_start() {
    assert_eq!(TextCommand::from_byte(0x00), Some(TextCommand::TxStart));
}

#[test]
fn text_command_from_byte_tx_far() {
    assert_eq!(TextCommand::from_byte(0x17), Some(TextCommand::TxFar));
}

#[test]
fn text_command_from_byte_out_of_range() {
    assert_eq!(TextCommand::from_byte(0x18), None);
    assert_eq!(TextCommand::from_byte(0x50), None);
    assert_eq!(TextCommand::from_byte(0xFF), None);
}

#[test]
fn text_command_to_byte_roundtrip() {
    for b in 0x00..=0x17u8 {
        let cmd = TextCommand::from_byte(b).unwrap();
        assert_eq!(cmd.to_byte(), b);
    }
}

#[test]
fn text_command_count_is_24() {
    assert_eq!(TEXT_COMMAND_COUNT, 24);
}

#[test]
fn tx_end_is_0x50() {
    assert_eq!(TX_END, 0x50);
}

#[test]
fn tx_sound_level_up_aliases_get_item_1() {
    assert_eq!(TX_SOUND_LEVEL_UP, TextCommand::TxSoundGetItem1.to_byte());
}

#[test]
fn text_command_sound_commands_flagged() {
    assert!(TextCommand::TxSoundGetItem1.is_sound_command());
    assert!(TextCommand::TxSoundCryDewgong.is_sound_command());
    assert!(!TextCommand::TxStart.is_sound_command());
    assert!(!TextCommand::TxFar.is_sound_command());
}

#[test]
fn text_command_param_counts() {
    assert_eq!(TextCommand::TxStart.param_byte_count(), 2);
    assert_eq!(TextCommand::TxRam.param_byte_count(), 2);
    assert_eq!(TextCommand::TxBcd.param_byte_count(), 3);
    assert_eq!(TextCommand::TxMove.param_byte_count(), 2);
    assert_eq!(TextCommand::TxBox.param_byte_count(), 4);
    assert_eq!(TextCommand::TxLow.param_byte_count(), 0);
    assert_eq!(TextCommand::TxFar.param_byte_count(), 3);
    assert_eq!(TextCommand::TxDots.param_byte_count(), 1);
    assert_eq!(TextCommand::TxNum.param_byte_count(), 3);
}

// ── Inline control char tests ─────────────────────────────────────

#[test]
fn inline_control_next_is_0x4e() {
    assert_eq!(inline_control_chars::NEXT, 0x4E);
}

#[test]
fn inline_control_done_is_0x57() {
    assert_eq!(inline_control_chars::DONE, 0x57);
}

#[test]
fn inline_control_is_control_true() {
    assert!(inline_control_chars::is_inline_control(0x4E)); // NEXT
    assert!(inline_control_chars::is_inline_control(0x51)); // PARA
    assert!(inline_control_chars::is_inline_control(0x57)); // DONE
}

#[test]
fn inline_control_is_control_false_for_printable() {
    assert!(!inline_control_chars::is_inline_control(0x80)); // 'A'
    assert!(!inline_control_chars::is_inline_control(0x7F)); // space
}

// ── TextScriptId tests ────────────────────────────────────────────

#[test]
fn text_script_pokecenter_nurse() {
    assert_eq!(
        TextScriptId::from_byte(0xFF),
        Some(TextScriptId::PokecenterNurse)
    );
}

#[test]
fn text_script_mart() {
    assert_eq!(TextScriptId::from_byte(0xFE), Some(TextScriptId::Mart));
}

#[test]
fn text_script_vending_machine() {
    assert_eq!(
        TextScriptId::from_byte(0xF5),
        Some(TextScriptId::VendingMachine)
    );
}

#[test]
fn text_script_unused_gaps_return_none() {
    assert_eq!(TextScriptId::from_byte(0xF8), None);
    assert_eq!(TextScriptId::from_byte(0xFA), None);
    assert_eq!(TextScriptId::from_byte(0xFB), None);
}

#[test]
fn text_script_out_of_range_returns_none() {
    assert_eq!(TextScriptId::from_byte(0x00), None);
    assert_eq!(TextScriptId::from_byte(0xF4), None);
}

#[test]
fn text_script_roundtrip() {
    let scripts = [0xF5, 0xF6, 0xF7, 0xF9, 0xFC, 0xFD, 0xFE, 0xFF];
    for &b in &scripts {
        let script = TextScriptId::from_byte(b).unwrap();
        assert_eq!(script.to_byte(), b);
    }
}

#[test]
fn text_script_count_is_8() {
    let all_scripts: Vec<_> = (0x00..=0xFF_u8)
        .filter_map(TextScriptId::from_byte)
        .collect();
    assert_eq!(all_scripts.len(), 8);
}
