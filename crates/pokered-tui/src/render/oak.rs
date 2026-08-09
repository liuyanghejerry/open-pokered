use pokered_core::naming_screen::{NamingScreenState, GRID_ROWS};
use pokered_core::oak_speech::{
    entrance_frames, entrance_slide_offset, slide_pic_x, OakSpeechPhase, OakSpeechState,
    PicSlideSubject, DEFAULT_PLAYER_NAMES, DEFAULT_RIVAL_NAMES, INTRO_FADE_PALETTES,
    INTRO_PIC_SLID_X, SHRINK_BEAT_CLEARED_END, SHRINK_BEAT_PIC1_END, SHRINK_BEAT_PIC2_END,
    SHRINK_BEAT_RED_END,
};
use pokered_data::charmap::naming_tiles;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::palette::GRAYSCALE_PALETTE;
use pokered_renderer::resource::{AssetCategory, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};
use dotzuki_renderer::transition::{FadePalette, FADE_PALETTES};

use super::{apply_gb_palette, blit_tileset, draw_centered_sprite, draw_text_box};

const TEXT_BOX_X: u32 = 0;
const TEXT_BOX_Y: u32 = 12 * 8;
const TEXT_BOX_W: u32 = 18;
const TEXT_BOX_H: u32 = 4;

pub fn draw_oak_speech(
    state: &OakSpeechState,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    fb.clear(Rgba::WHITE);
    let pal = &GRAYSCALE_PALETTE;

    // Naming screen open/submit white flash (GBPalWhiteOutWithDelay3).
    if state.is_flashing() {
        return;
    }

    if let Some(naming) = &state.naming_screen {
        draw_naming_screen(naming, fb);
        return;
    }

    let phase = &state.phase;
    let entrance = entrance_frames(phase);
    let entering = state.phase_frame < entrance;

    let sprite: Option<(&str, &str)> = match phase {
        OakSpeechPhase::Greeting { .. }
        | OakSpeechPhase::Explanation { .. }
        | OakSpeechPhase::IntroduceRival { .. } => Some(("trainer", "prof.oak")),
        OakSpeechPhase::ShowNidorino { .. } => Some(("pokemon_front", "nidorino")),
        OakSpeechPhase::IntroducePlayer { .. } => Some(("player", "red")),
        // oak_speech.asm:105 — RedPicFront is re-shown for the final speech.
        OakSpeechPhase::FinalSpeech { .. } => Some(("player", "red")),
        OakSpeechPhase::PlayerNameChoice { .. } => Some(("player", "red")),
        OakSpeechPhase::RivalNameChoice { .. } => Some(("trainer", "rival1")),
        OakSpeechPhase::SlidePic { subject, .. } => Some(match subject {
            PicSlideSubject::Player => ("player", "red"),
            PicSlideSubject::Rival => ("trainer", "rival1"),
        }),
        OakSpeechPhase::ShrinkPlayer { frame } => {
            let f = *frame;
            let shrink_name = if f < SHRINK_BEAT_RED_END {
                "red"
            } else if f < SHRINK_BEAT_PIC1_END {
                "shrink1"
            } else if f < SHRINK_BEAT_PIC2_END {
                "shrink2"
            } else {
                ""
            };
            if !shrink_name.is_empty() {
                if let Some(ref mut rm) = res {
                    if let Ok(cached) = rm.load(AssetCategory::Player, shrink_name) {
                        let ts = cached.tileset.clone();
                        let w = cached.source_size.0;
                        let h = cached.source_size.1;
                        draw_centered_sprite(fb, &ts, w, h, pal);
                    }
                }
            }
            // GBFadeOutToWhite (FadePal6→8, 3 × 8 frames) before Done.
            if f >= SHRINK_BEAT_CLEARED_END {
                let step = ((f - SHRINK_BEAT_CLEARED_END) / 8).min(2) as usize;
                apply_gb_palette(fb, &FADE_PALETTES[[5, 6, 7][step]]);
            }
            None
        }
        OakSpeechPhase::PlayerNaming | OakSpeechPhase::RivalNaming | OakSpeechPhase::Done => None,
    };

    let explicit_x: Option<u32> = match phase {
        OakSpeechPhase::PlayerNameChoice { .. } | OakSpeechPhase::RivalNameChoice { .. } => {
            Some(INTRO_PIC_SLID_X)
        }
        OakSpeechPhase::SlidePic {
            direction, frame, ..
        } => Some(slide_pic_x(*direction, *frame)),
        _ => None,
    };

    if let Some((category, name)) = sprite {
        if let Some(ref mut rm) = res {
            let result = if category == "trainer" {
                rm.load_trainer(name).ok()
            } else if category == "pokemon_front" {
                rm.load_pokemon_front(name).ok()
            } else if category == "player" {
                rm.load(AssetCategory::Player, name).ok()
            } else {
                None
            };
            if let Some(cached) = result {
                let ts = cached.tileset.clone();
                let w = cached.source_size.0;
                let tiles_per_row = w / TILE_SIZE;
                if let Some(x) = explicit_x {
                    blit_tileset(fb, &ts, x, 4 * TILE_SIZE, tiles_per_row, pal);
                } else {
                    // MovePicLeft entrance: the pic slides in from the right.
                    let offset = if entering
                        && matches!(
                            phase,
                            OakSpeechPhase::ShowNidorino { .. }
                                | OakSpeechPhase::IntroducePlayer { .. }
                        ) {
                        entrance_slide_offset(state.phase_frame)
                    } else {
                        0
                    };
                    let sx = (fb.width().saturating_sub(w)) / 2 + offset;
                    blit_tileset(fb, &ts, sx, 32, tiles_per_row, pal);
                }
            }
        }
    }

    match phase {
        OakSpeechPhase::PlayerNameChoice { cursor } => {
            draw_text_box(fb, 0, 0, 9, 10, Rgba::BLACK);
            draw_text("NAME", 3 * TILE_SIZE, TILE_SIZE, Rgba::BLACK, fb);
            for (i, name) in DEFAULT_PLAYER_NAMES.iter().enumerate() {
                if i == *cursor {
                    draw_text("▶", TILE_SIZE, (2 + i as u32 * 2) * TILE_SIZE, Rgba::BLACK, fb);
                }
                draw_text(
                    name,
                    2 * TILE_SIZE,
                    (2 + i as u32 * 2) * TILE_SIZE,
                    Rgba::BLACK,
                    fb,
                );
            }
            draw_text_box(
                fb,
                TEXT_BOX_X,
                TEXT_BOX_Y,
                TEXT_BOX_W,
                TEXT_BOX_H,
                Rgba::BLACK,
            );
            draw_text(
                "Your name?",
                TILE_SIZE,
                TEXT_BOX_Y + TILE_SIZE,
                Rgba::BLACK,
                fb,
            );
        }
        OakSpeechPhase::RivalNameChoice { cursor } => {
            draw_text_box(fb, 0, 0, 9, 10, Rgba::BLACK);
            draw_text("NAME", 3 * TILE_SIZE, TILE_SIZE, Rgba::BLACK, fb);
            for (i, name) in DEFAULT_RIVAL_NAMES.iter().enumerate() {
                if i == *cursor {
                    draw_text("▶", TILE_SIZE, (2 + i as u32 * 2) * TILE_SIZE, Rgba::BLACK, fb);
                }
                draw_text(
                    name,
                    2 * TILE_SIZE,
                    (2 + i as u32 * 2) * TILE_SIZE,
                    Rgba::BLACK,
                    fb,
                );
            }
            draw_text_box(
                fb,
                TEXT_BOX_X,
                TEXT_BOX_Y,
                TEXT_BOX_W,
                TEXT_BOX_H,
                Rgba::BLACK,
            );
            draw_text(
                "His name?",
                TILE_SIZE,
                TEXT_BOX_Y + TILE_SIZE,
                Rgba::BLACK,
                fb,
            );
        }
        OakSpeechPhase::Done => {
            draw_text("...", 70, 70, Rgba::BLACK, fb);
        }
        OakSpeechPhase::ShrinkPlayer { .. } => {}
        // While the pic entrance animation plays the text has not started
        // printing yet; during the menu slide the intro text stays visible.
        OakSpeechPhase::SlidePic { subject, .. } => {
            let pages: &[pokered_core::oak_speech::TextPage] = match subject {
                PicSlideSubject::Player => pokered_core::oak_speech::INTRODUCE_PLAYER_TEXT_PAGES,
                PicSlideSubject::Rival => pokered_core::oak_speech::INTRODUCE_RIVAL_TEXT_PAGES,
            };
            draw_text_box(
                fb,
                TEXT_BOX_X,
                TEXT_BOX_Y,
                TEXT_BOX_W,
                TEXT_BOX_H,
                Rgba::BLACK,
            );
            if let Some(page) = pages.last() {
                let (line1, line2) = page.get_display_text(state.player_name.as_deref(), u16::MAX);
                draw_text(&line1, TILE_SIZE, TEXT_BOX_Y + TILE_SIZE, Rgba::BLACK, fb);
                draw_text(
                    &line2,
                    TILE_SIZE,
                    TEXT_BOX_Y + TILE_SIZE * 3,
                    Rgba::BLACK,
                    fb,
                );
            }
        }
        _ => {
            if !entering {
                draw_text_box(
                    fb,
                    TEXT_BOX_X,
                    TEXT_BOX_Y,
                    TEXT_BOX_W,
                    TEXT_BOX_H,
                    Rgba::BLACK,
                );

                if let Some(page) = state.current_text_page() {
                    let char_index = state.current_char_index();
                    let (line1, line2) =
                        page.get_display_text(state.player_name.as_deref(), char_index);

                    draw_text(&line1, TILE_SIZE, TEXT_BOX_Y + TILE_SIZE, Rgba::BLACK, fb);
                    draw_text(
                        &line2,
                        TILE_SIZE,
                        TEXT_BOX_Y + TILE_SIZE * 3,
                        Rgba::BLACK,
                        fb,
                    );
                }

                if state.is_waiting_for_input() {
                    let arrow_x = 18 * TILE_SIZE;
                    let arrow_y = 15 * TILE_SIZE;
                    draw_text("▼", arrow_x, arrow_y, Rgba::BLACK, fb);
                }
            }
        }
    }

    // Pic entrance palette effects (the typewriter is gated on these in core).
    if entering {
        match phase {
            // FadeInIntroPic: 6-step IntroFadePalettes ramp, 10 frames/step.
            OakSpeechPhase::Greeting { .. } | OakSpeechPhase::IntroduceRival { .. } => {
                let step = (state.phase_frame / 10).min(5) as usize;
                let bgp = INTRO_FADE_PALETTES[step];
                apply_gb_palette(fb, &FadePalette::new(bgp, bgp, bgp));
            }
            // GBFadeInFromWhite before the final speech: FadePal7→5, 8f/step.
            OakSpeechPhase::FinalSpeech { .. } => {
                let step = (state.phase_frame / 8).min(2) as usize;
                apply_gb_palette(fb, &FADE_PALETTES[[6, 5, 4][step]]);
            }
            _ => {}
        }
    }
}

const NAME_BOX_X: u32 = 10;
const NAME_BOX_Y: u32 = 3;
const KEYBOARD_X: u32 = 2;
const KEYBOARD_Y: u32 = 6;

pub fn draw_naming_screen(
    naming: &NamingScreenState,
    fb: &mut FrameBuffer,
) {
    fb.clear(Rgba::WHITE);

    draw_text_box(fb, 0, 5 * TILE_SIZE, 18, 9, Rgba::BLACK);

    let title = match naming.screen_type() {
        pokered_core::naming_screen::NamingScreenType::Player => "YOUR NAME?",
        pokered_core::naming_screen::NamingScreenType::Rival => "RIVAL's NAME?",
        pokered_core::naming_screen::NamingScreenType::Pokemon => "NICKNAME?",
    };
    draw_text(title, TILE_SIZE, TILE_SIZE, Rgba::BLACK, fb);

    let name = naming.name();
    let max_len = naming.max_length();

    draw_text(
        name,
        NAME_BOX_X * TILE_SIZE,
        NAME_BOX_Y * TILE_SIZE,
        Rgba::BLACK,
        fb,
    );

    let underscore_y = (NAME_BOX_Y + 1) * TILE_SIZE;
    let name_len = name.len() as u32;

    for i in 0..max_len as u32 {
        let is_filled = i < name_len;
        let is_current = i == name_len;

        let underscore_tile_id = if is_current && !is_filled {
            naming_tiles::RAISED_UNDERSCORE
        } else {
            naming_tiles::UNDERSCORE
        };

        if let Some(ch) = pokered_data::charmap::decode_char(underscore_tile_id) {
            draw_text(
                ch,
                (NAME_BOX_X + i) * TILE_SIZE,
                underscore_y,
                Rgba::BLACK,
                fb,
            );
        }
    }

    let alphabet = naming.current_alphabet();
    let cursor_row = naming.cursor_row();
    let cursor_col = naming.cursor_col();

    for (row_i, row) in alphabet.iter().enumerate() {
        let y = (KEYBOARD_Y + row_i as u32) * TILE_SIZE;
        for (col_i, &tile_id) in row.iter().enumerate() {
            let x = (KEYBOARD_X + col_i as u32 * 2) * TILE_SIZE;

            if row_i == cursor_row && col_i == cursor_col {
                draw_text("▶", x - TILE_SIZE, y, Rgba::BLACK, fb);
            }

            let display_str = pokered_data::charmap::decode_char(tile_id).unwrap_or("?");
            draw_text(display_str, x, y, Rgba::BLACK, fb);
        }
    }

    let case_row_y = (KEYBOARD_Y + GRID_ROWS as u32) * TILE_SIZE;
    if cursor_row == GRID_ROWS {
        draw_text(
            "▶",
            KEYBOARD_X * TILE_SIZE - TILE_SIZE,
            case_row_y,
            Rgba::BLACK,
            fb,
        );
    }
    let case_text = if naming.is_lowercase() {
        "UPPER CASE"
    } else {
        "lower case"
    };
    draw_text(
        case_text,
        KEYBOARD_X * TILE_SIZE,
        case_row_y,
        Rgba::BLACK,
        fb,
    );
}
