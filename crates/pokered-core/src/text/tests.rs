use pokered_data::charmap;
use pokered_data::text_commands::inline_control_chars;

use super::*;
use dotzuki_engine::text::{ControlAction, TextProvider};

#[test]
fn dialog_runner_starts_not_done() {
    let runner = DialogRunner::default();
    assert!(!runner.is_done());
}

#[test]
fn dialog_runner_begin_text_activates() {
    let mut runner = DialogRunner::default();
    let encoded = charmap::encode_string("HI").unwrap();
    let mut data: Vec<u8> = encoded[..encoded.len() - 1].to_vec();
    data.push(inline_control_chars::DONE);
    runner.begin_text(data);
    assert!(runner.engine.is_active());
}

#[test]
fn text_box_standard_dialog() {
    let tb = TextBox::standard_dialog();
    assert_eq!(tb.origin, TileCoord::new(0, 12));
    assert_eq!(tb.width, SCREEN_WIDTH);
    assert_eq!(tb.height, 6);
}

#[test]
fn tile_coord_roundtrip() {
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            let coord = TileCoord::new(x, y);
            let idx = coord.to_tilemap_index();
            let back = TileCoord::from_tilemap_index(idx);
            assert_eq!(back, coord);
        }
    }
}

#[test]
fn tilemap_draw_box_border() {
    let mut tilemap = TilemapBuffer::default();
    let tb = TextBox::new(TileCoord::new(0, 0), 4, 3);
    tilemap.draw_box_border(&tb);
    assert_eq!(tilemap.get(TileCoord::new(0, 0)), TILE_TOP_LEFT);
    assert_eq!(tilemap.get(TileCoord::new(3, 0)), TILE_TOP_RIGHT);
    assert_eq!(tilemap.get(TileCoord::new(0, 2)), TILE_BOTTOM_LEFT);
    assert_eq!(tilemap.get(TileCoord::new(3, 2)), TILE_BOTTOM_RIGHT);
    assert_eq!(tilemap.get(TileCoord::new(1, 0)), TILE_HORIZONTAL);
    assert_eq!(tilemap.get(TileCoord::new(0, 1)), TILE_VERTICAL);
}

#[test]
fn process_simple_text() {
    let mut runner = DialogRunner::default();
    let encoded = charmap::encode_string("HI").unwrap();
    let mut data: Vec<u8> = encoded[..encoded.len() - 1].to_vec();
    data.push(inline_control_chars::DONE);
    runner.engine.open_dialog(&data);
    assert!(runner.engine.is_active());
    while runner.engine.is_active() { runner.engine.update(&mut runner.tile_buffer); }
    let h_byte = charmap::encode_char('H').unwrap();
    let i_byte = charmap::encode_char('I').unwrap();
    runner.tilemap.copy_from_tile_buffer(&runner.tile_buffer);
    assert_eq!(runner.tilemap.get(TileCoord::new(0, 0)), h_byte);
    assert_eq!(runner.tilemap.get(TileCoord::new(1, 0)), i_byte);
}

#[test]
fn process_terminator_ends_text() {
    let mut runner = DialogRunner::default();
    runner.engine.open_dialog(&[charmap::CHAR_TERMINATOR]);
    assert!(runner.engine.is_active());
    runner.engine.update(&mut runner.tile_buffer);
    assert!(!runner.engine.is_active());
}

#[test]
fn process_next_line_control() {
    let provider = PokemonTextProvider::default();
    let mut engine = DialogEngine::new(provider);
    let mut buf = TileBuffer::new(20, 18);
    buf.cursor = dotzuki_engine::text::TilePos::new(5, 5);
    engine.open_dialog(&[inline_control_chars::NEXT, inline_control_chars::DONE]);
    engine.update(&mut buf);
    assert_eq!(buf.cursor.y, 6);
    assert_eq!(buf.cursor.x, 0);
}

#[test]
fn process_player_name_insertion() {
    let mut runner = DialogRunner::default();
    runner.engine.open_dialog(&[inline_control_chars::PLAYER, inline_control_chars::DONE]);
    while runner.engine.is_active() { runner.engine.update(&mut runner.tile_buffer); }
    runner.tilemap.copy_from_tile_buffer(&runner.tile_buffer);
    let r_byte = charmap::encode_char('R').unwrap();
    let e_byte = charmap::encode_char('E').unwrap();
    let d_byte = charmap::encode_char('D').unwrap();
    assert_eq!(runner.tilemap.get(TileCoord::new(0, 0)), r_byte);
    assert_eq!(runner.tilemap.get(TileCoord::new(1, 0)), e_byte);
    assert_eq!(runner.tilemap.get(TileCoord::new(2, 0)), d_byte);
}

#[test]
fn renderer_run_to_completion() {
    let mut runner = DialogRunner::default();
    let encoded = charmap::encode_string("OK").unwrap();
    let mut data: Vec<u8> = encoded[..encoded.len() - 1].to_vec();
    data.push(inline_control_chars::DONE);
    runner.begin_text(data);
    runner.run_to_completion();
    assert!(runner.is_done());
}

#[test]
fn renderer_read_tilemap_text() {
    let mut runner = DialogRunner::default();
    let encoded = charmap::encode_string("HI").unwrap();
    let mut data: Vec<u8> = encoded[..encoded.len() - 1].to_vec();
    data.push(inline_control_chars::DONE);
    runner.begin_text(data);
    runner.run_to_completion();
    let tb = TextBox::standard_dialog();
    let start = tb.text_start_coord();
    let text = runner.read_tilemap_text(start.x, start.y, 2);
    assert_eq!(text, "HI");
}

#[test]
fn name_buffers_default_player_red() {
    let names = NameBuffers::default();
    let decoded = charmap::decode_string(&names.player_name);
    assert_eq!(decoded, "RED");
}

#[test]
fn name_buffers_default_rival_blue() {
    let names = NameBuffers::default();
    let decoded = charmap::decode_string(&names.rival_name);
    assert_eq!(decoded, "BLUE");
}

#[test]
fn tilemap_clear_area() {
    let mut tilemap = TilemapBuffer::default();
    tilemap.set(TileCoord::new(1, 1), 0x80);
    tilemap.set(TileCoord::new(2, 1), 0x81);
    tilemap.clear_area(TileCoord::new(1, 1), 2, 1);
    assert_eq!(tilemap.get(TileCoord::new(1, 1)), charmap::CHAR_SPACE);
    assert_eq!(tilemap.get(TileCoord::new(2, 1)), charmap::CHAR_SPACE);
}

#[test]
fn process_wait_button_command() {
    let mut runner = DialogRunner::default();
    runner.engine.open_dialog(&[0x0D]);
    assert!(runner.engine.is_active());
    runner.engine.update(&mut runner.tile_buffer);
    assert_eq!(runner.engine.state.mode, dotzuki_engine::text::DialogMode::WaitingForInput);
    assert!(runner.engine.is_active());
}

#[test]
fn text_engine_pause_handling() {
    let mut runner = DialogRunner::default();
    runner.engine.open_dialog(&[0x0A, inline_control_chars::DONE]);
    runner.engine.update(&mut runner.tile_buffer);
    assert_eq!(runner.engine.state.mode, dotzuki_engine::text::DialogMode::Paused);
    assert!(runner.engine.is_active());
    runner.engine.advance();
    assert_eq!(runner.engine.state.mode, dotzuki_engine::text::DialogMode::Typing);
}

#[test]
fn decode_hello_stream() {
    let provider = PokemonTextProvider::default();
    let h = charmap::encode_char('H').unwrap();
    let i = charmap::encode_char('I').unwrap();
    let stream = provider.decode_stream(&[h, i, inline_control_chars::DONE]);
    assert!(stream.chars.len() >= 3);
}

#[test]
fn provider_is_control_code() {
    let provider = PokemonTextProvider::default();
    let h = charmap::encode_char('H').unwrap();
    assert!(!provider.is_control_code(&provider::PokemonChar::Tile(h)));
    assert!(provider.is_control_code(&provider::PokemonChar::Done));
    assert!(!provider.is_control_code(&provider::PokemonChar::PlayerName));
}

#[test]
fn provider_string_width_basic() {
    let provider = PokemonTextProvider::default();
    let h = charmap::encode_char('H').unwrap();
    let i = charmap::encode_char('I').unwrap();
    let chars = vec![provider::PokemonChar::Tile(h), provider::PokemonChar::Tile(i)];
    assert_eq!(provider.string_width(&chars), 16);
    assert_eq!(provider.string_width(&[]), 0);
}

#[test]
fn provider_string_width_player_name() {
    let provider = PokemonTextProvider::default();
    let chars = vec![provider::PokemonChar::PlayerName];
    assert_eq!(provider.string_width(&chars), 24);
}

#[test]
fn provider_decode_byte_control_codes() {
    let provider = PokemonTextProvider::default();
    assert!(matches!(provider.decode_byte(inline_control_chars::DONE), Some(provider::PokemonChar::Done)));
    assert!(matches!(provider.decode_byte(inline_control_chars::PLAYER), Some(provider::PokemonChar::PlayerName)));
    assert!(matches!(provider.decode_byte(inline_control_chars::RIVAL), Some(provider::PokemonChar::RivalName)));
    assert!(matches!(provider.decode_byte(inline_control_chars::NEXT), Some(provider::PokemonChar::NextLine)));
}

#[test]
fn provider_process_control_done() {
    let provider = PokemonTextProvider::default();
    let mut state = dotzuki_engine::text::DialogState::default();
    let action = provider.process_control(&provider::PokemonChar::Done, &mut state);
    assert_eq!(action, ControlAction::Done);
}

#[test]
fn provider_process_control_newline() {
    let provider = PokemonTextProvider::default();
    let mut state = dotzuki_engine::text::DialogState::default();
    let action = provider.process_control(&provider::PokemonChar::NextLine, &mut state);
    assert_eq!(action, ControlAction::Newline);
}

#[test]
fn tilemap_scroll_lines_up() {
    let mut tilemap = TilemapBuffer::default();
    tilemap.set(TileCoord::new(1, 15), 0x80);
    tilemap.scroll_lines_up(14, 3);
    assert_eq!(tilemap.get(TileCoord::new(1, 14)), 0x80);
}

#[test]
fn provider_render_char_advances_cursor() {
    let provider = PokemonTextProvider::default();
    let mut buf = TileBuffer::new(20, 18);
    let h_byte = charmap::encode_char('H').unwrap();
    let h = provider::PokemonChar::Tile(h_byte);
    let start_x = buf.cursor.x;
    provider.render_char(&h, &mut buf);
    assert_eq!(buf.cursor.x, start_x + 1);
}

#[test]
fn dialog_runner_default_provider_names() {
    let runner = DialogRunner::default();
    let provider = &runner.engine.provider;
    let decoded_player = charmap::decode_string(&provider.player_name);
    assert_eq!(decoded_player, "RED");
    let decoded_rival = charmap::decode_string(&provider.rival_name);
    assert_eq!(decoded_rival, "BLUE");
}
