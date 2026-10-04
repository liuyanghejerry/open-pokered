use dotzuki_renderer::transition::{FadePalette, FADE_PALETTES};
use pokered_core::game_state::Lang;
use pokered_core::naming_screen::NamingScreenState;
use pokered_core::oak_speech::{
    entrance_frames, entrance_slide_offset, slide_pic_x, OakSpeechPhase, OakSpeechState,
    PicSlideSubject, DEFAULT_PLAYER_NAMES, DEFAULT_RIVAL_NAMES, INTRODUCE_PLAYER_TEXT_PAGES,
    INTRODUCE_PLAYER_TEXT_PAGES_ZH, INTRODUCE_RIVAL_TEXT_PAGES, INTRODUCE_RIVAL_TEXT_PAGES_ZH,
    INTRO_FADE_PALETTES, INTRO_PIC_SLID_X, SHRINK_BEAT_CLEARED_END, SHRINK_BEAT_PIC1_END,
    SHRINK_BEAT_PIC2_END, SHRINK_BEAT_RED_END,
};
use pokered_data::ui_layout::schema::{
    NAMING_DEFAULT_LAYOUT, OAK_SPEECH_NAME_CHOICE_LAYOUT, OAK_SPEECH_TEXT_PHASE_LAYOUT,
};
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::palette::GRAYSCALE_SPRITE_PALETTE;
use pokered_renderer::resource::{AssetCategory, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{menus, Ui};

use super::{apply_gb_palette, blit_front_pic, blit_tileset, draw_centered_sprite};

pub fn draw_oak_speech(
    state: &OakSpeechState,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    language: Lang,
) {
    fb.clear(Rgba::WHITE);
    let sprite_pal = &GRAYSCALE_SPRITE_PALETTE;

    // Naming screen open/submit white flash (GBPalWhiteOutWithDelay3,
    // naming_screen.asm:88/163).
    if state.is_flashing() {
        return;
    }

    if let Some(naming) = &state.naming_screen {
        draw_naming_screen(naming, res, fb, language);
        return;
    }

    let phase = &state.phase;
    let entrance = entrance_frames(phase);
    let entering = state.phase_frame < entrance;

    // Pic drawn at an explicit left-edge x (tilemap position) rather than
    // centered: the name-choice rest position (hlcoord 12,4) and the SlidePic
    // animation both place it directly.
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
                // SFX_SHRINK plays while RedPicFront is still shown.
                "red"
            } else if f < SHRINK_BEAT_PIC1_END {
                "shrink1"
            } else if f < SHRINK_BEAT_PIC2_END {
                "shrink2"
            } else {
                // 7×7 area at (6,5) cleared (oak_speech.asm:156-159).
                ""
            };
            if !shrink_name.is_empty() {
                if let Some(ref mut rm) = res {
                    if let Ok(cached) = rm.load(AssetCategory::Player, shrink_name) {
                        let ts = &cached.tileset;
                        let w = cached.source_size.0;
                        let h = cached.source_size.1;
                        draw_centered_sprite(fb, &ts, w, h, sprite_pal);
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
                let ts = &cached.tileset;
                let w = cached.source_size.0;
                let tiles_per_row = w / TILE_SIZE;
                if let Some(x) = explicit_x {
                    blit_tileset(fb, &ts, x, 4 * TILE_SIZE, tiles_per_row, sprite_pal);
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
                    let sx = 6 * TILE_SIZE + offset;
                    if category == "pokemon_front" {
                        blit_front_pic(fb, cached, sx as i32, 32, true);
                    } else {
                        blit_tileset(fb, &ts, sx, 32, tiles_per_row, sprite_pal);
                    }
                }
            }
        }
    }

    match phase {
        OakSpeechPhase::PlayerNameChoice { cursor } => {
            let mut painter = FrameBufferPainter::new(fb).with_lang(language);
            let mut ui = Ui::new(&mut painter);
            let prompt = if language == Lang::Zh {
                "你的名字？"
            } else {
                "Your name?"
            };
            menus::oak_speech::draw_name_choice(
                &DEFAULT_PLAYER_NAMES,
                *cursor,
                prompt,
                &OAK_SPEECH_NAME_CHOICE_LAYOUT,
                &mut ui,
            );
        }
        OakSpeechPhase::RivalNameChoice { cursor } => {
            let mut painter = FrameBufferPainter::new(fb).with_lang(language);
            let mut ui = Ui::new(&mut painter);
            let prompt = if language == Lang::Zh {
                "他的名字？"
            } else {
                "His name?"
            };
            menus::oak_speech::draw_name_choice(
                &DEFAULT_RIVAL_NAMES,
                *cursor,
                prompt,
                &OAK_SPEECH_NAME_CHOICE_LAYOUT,
                &mut ui,
            );
        }
        OakSpeechPhase::Done => {
            draw_text("...", 70, 70, Rgba::BLACK, fb);
        }
        OakSpeechPhase::ShrinkPlayer { .. } => {}
        // During the slide the intro text box stays on screen (the menu only
        // appears once the slide finishes, oak_speech2.asm:2-5); while the
        // pic entrance animation plays the text has not started printing yet.
        OakSpeechPhase::SlidePic { subject, .. } => {
            let pages: &[pokered_core::oak_speech::TextPage] = match (subject, language) {
                (PicSlideSubject::Player, Lang::Zh) => INTRODUCE_PLAYER_TEXT_PAGES_ZH,
                (PicSlideSubject::Player, _) => INTRODUCE_PLAYER_TEXT_PAGES,
                (PicSlideSubject::Rival, Lang::Zh) => INTRODUCE_RIVAL_TEXT_PAGES_ZH,
                (PicSlideSubject::Rival, _) => INTRODUCE_RIVAL_TEXT_PAGES,
            };
            if let Some(page) = pages.last() {
                let (line1, line2) = page.get_display_text(state.player_name.as_deref(), u16::MAX);
                let mut painter = FrameBufferPainter::new(fb).with_lang(language);
                let mut ui = Ui::new(&mut painter);
                menus::oak_speech::draw_text_phase_localized(
                    &line1,
                    &line2,
                    false,
                    &OAK_SPEECH_TEXT_PHASE_LAYOUT,
                    &mut ui,
                    language,
                );
            }
        }
        _ => {
            if !entering {
                let (line1, line2) = if let Some(page) =
                    pokered_core::oak_speech::text_pages_for_lang(phase, language)
                {
                    let char_index = state.current_char_index();
                    page.get_display_text(state.player_name.as_deref(), char_index)
                } else {
                    (String::new(), String::new())
                };
                let show_arrow = state.is_waiting_for_input();
                let mut painter = FrameBufferPainter::new(fb).with_lang(language);
                let mut ui = Ui::new(&mut painter);
                menus::oak_speech::draw_text_phase_localized(
                    &line1,
                    &line2,
                    show_arrow,
                    &OAK_SPEECH_TEXT_PHASE_LAYOUT,
                    &mut ui,
                    language,
                );
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

pub fn draw_naming_screen(
    naming: &NamingScreenState,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    language: Lang,
) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(language);
    let mut ui = Ui::new(&mut painter);
    menus::naming::draw(
        naming,
        &NAMING_DEFAULT_LAYOUT,
        &mut ui,
        language == Lang::Zh,
    );
    if let (Some(species), Some(rm)) = (naming.species, res.as_mut()) {
        use pokered_renderer::mon_icon::{
            draw_mon_icon, icon_y_offset, load_mon_icon_tiles, IconFrame,
        };
        let kind = pokered_data::mon_party_icons::icon_for_species(species);
        let frame = IconFrame::from_counter(naming.animation_frame, 17);
        if let Ok(tiles) = load_mon_icon_tiles(rm, kind, frame) {
            draw_mon_icon(
                fb,
                tiles,
                8,
                icon_y_offset(kind, frame),
                &pokered_renderer::palette::GRAYSCALE_SPRITE_PALETTE,
            );
        }
    }
}
