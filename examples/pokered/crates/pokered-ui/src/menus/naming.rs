use pokered_core::naming_screen::{InputMode, NamingScreenState, NamingScreenType, ED_TILE_ID, GRID_ROWS};
use pokered_data::charmap::{decode_char, naming_tiles};
use pokered_data::lang_data;
use pokered_data::ui_layout::schema::NamingDefaultLayout;

use crate::engine::{InkColor, Painter, Ui};

const TITLE_TX: u32 = 1;
const TITLE_TY: u32 = 1;

const NAME_BOX_TX: u32 = 10;
const NAME_BOX_TY: u32 = 3;
const UNDERSCORE_TY: u32 = NAME_BOX_TY + 1;

const KEYBOARD_TX: u32 = 2;
const KEYBOARD_TY: u32 = 6;
const KEYBOARD_COL_STEP: u32 = 2;
const CURSOR_DX: u32 = 1;

pub fn draw<P: Painter>(state: &NamingScreenState, layout: &NamingDefaultLayout, ui: &mut Ui<P>, is_zh: bool) {
    ui.clear(InkColor::White);

    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |_| {});

    ui.text_box(layout.region_0.rect, layout.region_0.color, false, |frame| {
        frame.label(TITLE_TX, TITLE_TY, title(state.screen_type(), is_zh), InkColor::Black);
        frame.label(NAME_BOX_TX, NAME_BOX_TY, state.name(), InkColor::Black);

        let name_len = state.name().len() as u32;
        let max_len = state.max_length() as u32;
        for i in 0..max_len {
            let is_filled = i < name_len;
            let is_current = i == name_len;
            let tile_id = if is_current && !is_filled {
                naming_tiles::RAISED_UNDERSCORE
            } else {
                naming_tiles::UNDERSCORE
            };
            let fallback = decode_char(tile_id).unwrap_or("_");
            frame.gb_tile(NAME_BOX_TX + i, UNDERSCORE_TY, tile_id, fallback, InkColor::Black);
        }

        let alphabet = state.current_alphabet();
        let cursor_row = state.cursor_row();
        let cursor_col = state.cursor_col();
        let in_pinyin = state.input_mode == InputMode::Pinyin;
        for (row_i, row) in alphabet.iter().enumerate() {
            let ty = KEYBOARD_TY + row_i as u32;
            for (col_i, &tile_id) in row.iter().enumerate() {
                let tx = KEYBOARD_TX + col_i as u32 * KEYBOARD_COL_STEP;
                if row_i == cursor_row && col_i == cursor_col && !in_pinyin {
                    frame.gb_tile(tx - CURSOR_DX, ty, naming_tiles::CURSOR_ARROW, "▶", InkColor::Black);
                }
                let fallback = if tile_id == ED_TILE_ID { "ED" } else { decode_char(tile_id).unwrap_or("?") };
                frame.gb_tile(tx, ty, tile_id, fallback, InkColor::Black);
            }
        }

        let case_ty = KEYBOARD_TY + GRID_ROWS as u32;
        if cursor_row == GRID_ROWS && !in_pinyin {
            frame.gb_tile(KEYBOARD_TX - CURSOR_DX, case_ty, naming_tiles::CURSOR_ARROW, "▶", InkColor::Black);
        }
        let case_text = if state.input_mode == InputMode::Pinyin {
            "拼音 ABC"
        } else if state.is_lowercase() {
            if is_zh { "大写" } else { "UPPER CASE" }
        } else {
            if is_zh { "小写" } else { "lower case" }
        };
        frame.label(KEYBOARD_TX, case_ty, case_text, InkColor::Black);

        // ── Pinyin buffer and candidates ──
        if in_pinyin {
            let pinyin_y = case_ty + 2;
            let pinyin_display = if state.pinyin_buf.is_empty() { "_" } else { &state.pinyin_buf };
            frame.label(KEYBOARD_TX, pinyin_y, &format!("拼音: {}", pinyin_display), InkColor::Black);
            if !state.pinyin_candidates.is_empty() {
                let cand_y = pinyin_y + 2;
                let mut cand_str = String::new();
                for (i, &ch) in state.pinyin_candidates.iter().enumerate() {
                    if i == state.candidate_idx { cand_str.push('['); }
                    cand_str.push(ch);
                    if i == state.candidate_idx { cand_str.push(']'); }
                    cand_str.push(' ');
                }
                frame.label(KEYBOARD_TX, cand_y, &cand_str, InkColor::Black);
            }
        }
    });
}

fn title(screen_type: NamingScreenType, is_zh: bool) -> &'static str {
    if is_zh {
        match screen_type {
            NamingScreenType::Player => "你的名字？",
            NamingScreenType::Rival => "劲敌的名字？",
            NamingScreenType::Pokemon => "昵称？",
        }
    } else {
        match screen_type {
            NamingScreenType::Player => "YOUR NAME?",
            NamingScreenType::Rival => "RIVAL's NAME?",
            NamingScreenType::Pokemon => "NICKNAME?",
        }
    }
}
