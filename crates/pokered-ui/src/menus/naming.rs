use pokered_core::naming_screen::{
    InputMode, NamingScreenState, NamingScreenType, CANDIDATES_PER_LINE, ED_TILE_ID, GRID_ROWS,
    PINYIN_GRID_ROWS,
};
use pokered_data::charmap::{decode_char, naming_tiles};
use pokered_data::ui_layout::schema::NamingDefaultLayout;

use crate::engine::{InkColor, Painter, Ui};

/// Screen width in tiles (20×18 GB screen).
const SCREEN_TW: u32 = 20;

const TITLE_TY: u32 = 1;

const NAME_BOX_TY: u32 = 3;
const UNDERSCORE_TY: u32 = NAME_BOX_TY + 1;

const KEYBOARD_TX: u32 = 2;
const KEYBOARD_TY: u32 = 6;
const KEYBOARD_COL_STEP: u32 = 2;
const CURSOR_DX: u32 = 1;

/// Vertical rhythm inside the panel (interior rows 6..=16 of the 20×13 box):
/// - alphabet mode: letter rows spaced 2 rows (6,8,10,12,14), the case label
///   sits on row 16 — the panel is filled edge to edge with even breathing room.
/// - pinyin mode: only the 3 navigable letter rows are drawn (step 1, rows
///   6..=8); the pinyin buffer sits at row 10 and candidate lines at 12/14.
///   CJK glyphs are 10px tall, so lines of Chinese text stay 2 rows (16px)
///   apart. The candidate at the cursor is drawn as "[X]", others as " X ".
const ROW_STEP_ALPHA: u32 = 2;
const ROW_STEP_PINYIN: u32 = 1;
const CASE_ROW_GAP: u32 = 2;
/// Gap between the last pinyin letter row and the buffer line, and between
/// the buffer and the first candidate line.
const INFO_ROW_STEP: u32 = 2;

/// Each candidate occupies a fixed 3-column slot ("[X]" or " X "), so the
/// row is stable while moving the cursor; 6 slots fill the 18-column panel
/// interior (matches `CANDIDATES_PER_LINE` in pokered-core).
const CANDIDATE_SLOT_W: u32 = 3;

pub fn draw<P: Painter>(state: &NamingScreenState, layout: &NamingDefaultLayout, ui: &mut Ui<P>, is_zh: bool) {
    ui.clear(InkColor::White);

    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |_| {});

    ui.text_box(layout.region_0.rect, layout.region_0.color, false, |frame| {
        // Title is centered on its text width (ASCII = 1 tile per char).
        let title = title(state.screen_type(), is_zh);
        let title_tx = (SCREEN_TW - title.chars().count() as u32) / 2;
        frame.label(title_tx, TITLE_TY, title, InkColor::Black);

        // Name box centered by max length (7 for player/rival, 10 for nick).
        let max_len = state.max_length() as u32;
        let name_tx = (SCREEN_TW - max_len) / 2;

        // Fill one underscore slot per width unit (ASCII 1, CJK 2) — NOT one
        // per byte or per char: a 3-CJK name fills 6 of 7 slots and shows the
        // raised cursor on the last free one.
        let name_units = state.used_units() as u32;
        for i in 0..max_len {
            let is_filled = i < name_units;
            let is_current = i == name_units;
            let tile_id = if is_current && !is_filled {
                naming_tiles::RAISED_UNDERSCORE
            } else {
                naming_tiles::UNDERSCORE
            };
            let fallback = decode_char(tile_id).unwrap_or("_");
            frame.gb_tile(name_tx + i, UNDERSCORE_TY, tile_id, fallback, InkColor::Black);
        }

        // Draw the name AFTER the underscore slots. The Fusion Pixel glyphs
        // are 10px tall (one 8px tile row plus a couple of pixels below), so
        // they extend into the underscore row; drawing them last keeps the
        // underscore slots' background fill from clipping the glyph bottoms.
        frame.label(name_tx, NAME_BOX_TY, state.name(), InkColor::Black);

        let alphabet = state.current_alphabet();
        let cursor_row = state.cursor_row();
        let cursor_col = state.cursor_col();
        let in_pinyin = state.input_mode == InputMode::Pinyin;
        // Pinyin mode only navigates letter rows 0..=2 (A-Z); the specials,
        // ED, and case rows are alphabet-mode only and are not drawn.
        let grid_rows = if in_pinyin { PINYIN_GRID_ROWS } else { GRID_ROWS };
        let row_step = if in_pinyin { ROW_STEP_PINYIN } else { ROW_STEP_ALPHA };
        for row_i in 0..grid_rows {
            let row = &alphabet[row_i];
            let ty = KEYBOARD_TY + row_i as u32 * row_step;
            for (col_i, &tile_id) in row.iter().enumerate() {
                let tx = KEYBOARD_TX + col_i as u32 * KEYBOARD_COL_STEP;
                if row_i == cursor_row && col_i == cursor_col {
                    frame.gb_tile(tx - CURSOR_DX, ty, naming_tiles::CURSOR_ARROW, "▶", InkColor::Black);
                }
                let fallback = if tile_id == ED_TILE_ID { "ED" } else { decode_char(tile_id).unwrap_or("?") };
                frame.gb_tile(tx, ty, tile_id, fallback, InkColor::Black);
            }
        }

        if !in_pinyin {
            let case_ty = KEYBOARD_TY + (GRID_ROWS as u32 - 1) * row_step + CASE_ROW_GAP;
            if cursor_row == GRID_ROWS {
                frame.gb_tile(KEYBOARD_TX - CURSOR_DX, case_ty, naming_tiles::CURSOR_ARROW, "▶", InkColor::Black);
            }
            let case_text = if state.is_lowercase() {
                if is_zh { "大写" } else { "UPPER CASE" }
            } else {
                if is_zh { "小写" } else { "lower case" }
            };
            frame.label(KEYBOARD_TX, case_ty, case_text, InkColor::Black);
        }

        // ── Pinyin buffer and candidates ──
        if in_pinyin {
            let pinyin_y = KEYBOARD_TY + PINYIN_GRID_ROWS as u32 * ROW_STEP_PINYIN + 1;
            let pinyin_display = if state.pinyin_buf.is_empty() { "_" } else { &state.pinyin_buf };
            frame.label(KEYBOARD_TX, pinyin_y, &format!("拼音: {}", pinyin_display), InkColor::Black);
            if !state.pinyin_candidates.is_empty() {
                let cand_y = pinyin_y + INFO_ROW_STEP;
                for (i, &ch) in state.pinyin_candidates.iter().enumerate() {
                    let line = (i / CANDIDATES_PER_LINE) as u32;
                    let col = (i % CANDIDATES_PER_LINE) as u32;
                    let at_cursor = cursor_row == PINYIN_GRID_ROWS + (i / CANDIDATES_PER_LINE)
                        && cursor_col == col as usize;
                    let display = if at_cursor {
                        format!("[{ch}]")
                    } else {
                        format!(" {ch} ")
                    };
                    frame.label(KEYBOARD_TX + col * CANDIDATE_SLOT_W, cand_y + line * INFO_ROW_STEP, &display, InkColor::Black);
                }
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
