//! Renderer for the Hall of Fame roll-call movie
//! (`pokered_core::hof_ceremony::HofCeremonyState`).
//!
//! Port of `AnimateHallOfFame` (engine/movie/hall_of_fame.asm): each party
//! mon gets the `HoFShowMonOrPlayer` dual-pic scroll — the BACK pic crosses
//! from the right along the bottom band (hSCY=$d0, hall_of_fame.asm:99-134),
//! then the FRONT pic slides in from the left to its resting spot at
//! hlcoord 12,5 — then the nickname / LEVEL / TYPE1 / TYPE2 info box
//! (`HoFDisplayMonInfo`) and cry, then the "HALL OF FAME" text box; after the
//! last mon the player's pics scroll the same way and the stats page shows
//! (`HoFDisplayPlayerStats`) with the player's front pic still on screen:
//! name, PLAY TIME, MONEY, #DEX seen/owned and the rating.
//!
//! Scoped simplifications: the palette fades are plain white flashes, and the
//! front/back pics keep the reimpl's native sprite sizes (the mon front pics
//! are 40×40 here vs the original's 7×7-tile 56×56).

use crate::alloc_prelude::*;
use pokered_core::game_state::Lang;
use pokered_core::hof_ceremony::{HofCeremonyState, HofPhase, HofScrollStage};
use pokered_data::lang_data;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::palette::GRAYSCALE_SPRITE_PALETTE;
use pokered_renderer::resource::{AssetCategory, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::battle::scale_sprite_by_two;
use pokered_data::ui_text::zh_pc_line;
use super::{blit_tileset, draw_text_box, species_to_sprite_name};

const FG: Rgba = Rgba::BLACK;
const T: u32 = 8;

/// Resting spot of the front pic (hlcoord 12,5 — the 7×7 pic area at
/// x=96..152, y=40..96).
const FRONT_REST_X: u32 = 12 * T;
const FRONT_REST_Y: u32 = 5 * T;
/// The back pic scrolls along the bottom band: hSCY=$d0 shows map row 5 at
/// screen y = (40 − 208) mod 256 = 88.
const BACK_PIC_Y: u32 = 11 * T;

/// Compact description of every value consumed by [`draw_hof_ceremony`].
///
/// Long roll-call dwell phases only advance an invisible counter. The GBA
/// frontend uses this key to retain the existing framebuffer in those
/// intervals while scroll positions and phase transitions still redraw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HofVisualKey {
    visual_phase: u8,
    species: Option<pokered_data::species::Species>,
    scroll_stage: Option<HofScrollStage>,
    scroll_x: u16,
    content_hash: u32,
    is_zh: bool,
}

pub fn hof_visual_key(hof: &HofCeremonyState, lang: Lang) -> HofVisualKey {
    let is_zh = lang == Lang::Zh;
    let mut key = HofVisualKey {
        visual_phase: 0, // Every fade/opening/done frame is plain white.
        species: None,
        scroll_stage: None,
        scroll_x: 0,
        content_hash: 0x811c_9dc5,
        is_zh,
    };
    let hash_byte = |hash: &mut u32, byte: u8| {
        *hash = (*hash ^ byte as u32).wrapping_mul(0x0100_0193);
    };
    let hash_u16 = |hash: &mut u32, value: u16| {
        for byte in value.to_le_bytes() {
            hash_byte(hash, byte);
        }
    };
    let hash_u32 = |hash: &mut u32, value: u32| {
        for byte in value.to_le_bytes() {
            hash_byte(hash, byte);
        }
    };

    match hof.phase() {
        HofPhase::MonScroll | HofPhase::PlayerScroll => {
            let x = hof.scroll_pic_x();
            // draw_scroll_pic exits before loading an asset when the image is
            // outside the 160 px viewport, so these positions are all blank.
            if x >= 160 {
                return key;
            }
            let is_mon = hof.phase() == HofPhase::MonScroll;
            key.visual_phase = if is_mon { 1 } else { 4 };
            key.species = if is_mon {
                hof.current_entry().map(|entry| entry.species)
            } else {
                None
            };
            key.scroll_stage = hof.scroll_stage();
            key.scroll_x = x as u16;
        }
        HofPhase::MonInfo | HofPhase::MonText | HofPhase::MonFade => {
            let Some(entry) = hof.current_entry() else {
                return key;
            };
            key.visual_phase = if hof.phase() == HofPhase::MonText {
                3
            } else {
                2
            };
            key.species = Some(entry.species);
            hash_byte(&mut key.content_hash, entry.species as u8);
            hash_byte(&mut key.content_hash, entry.level);
            for byte in entry.nickname.bytes() {
                hash_byte(&mut key.content_hash, byte);
            }
        }
        HofPhase::PlayerStats => {
            key.visual_phase = 5;
            let stats = hof.stats();
            for byte in stats.name.bytes() {
                hash_byte(&mut key.content_hash, byte);
            }
            hash_u16(&mut key.content_hash, stats.play_time_hours);
            hash_byte(&mut key.content_hash, stats.play_time_minutes);
            hash_u32(&mut key.content_hash, stats.money);
            hash_u16(&mut key.content_hash, stats.dex_seen);
            hash_u16(&mut key.content_hash, stats.dex_owned);
            for byte in stats.rating.bytes() {
                hash_byte(&mut key.content_hash, byte);
            }
        }
        HofPhase::FadeOut
        | HofPhase::Opening
        | HofPhase::FinalFade
        | HofPhase::Done => {}
    }
    key
}

/// Draw the roll call to the 160x144 framebuffer.
pub fn draw_hof_ceremony(
    hof: &HofCeremonyState,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let is_zh = lang == Lang::Zh;
    fb.clear(Rgba::WHITE);
    match hof.phase() {
        HofPhase::FadeOut | HofPhase::Opening | HofPhase::FinalFade | HofPhase::Done => {}
        HofPhase::MonScroll => {
            if let Some(entry) = hof.current_entry() {
                draw_scroll_pic(
                    Some(entry.species),
                    hof.scroll_stage(),
                    hof.scroll_pic_x(),
                    resources,
                    fb,
                );
            }
        }
        HofPhase::MonInfo | HofPhase::MonText | HofPhase::MonFade => {
            if let Some(entry) = hof.current_entry() {
                draw_mon_front(entry.species, FRONT_REST_X, FRONT_REST_Y, resources, fb);
                draw_mon_info(entry, fb, is_zh);
                if hof.phase() == HofPhase::MonText {
                    // hlcoord 2, 13 / "HALL OF FAME" (hall_of_fame.asm:96-99).
                    draw_text_box(fb, 2 * T, 13 * T, 14, 2, FG);
                    draw_text(
                        lang_data::ui_label("HALL OF FAME", is_zh),
                        4 * T,
                        14 * T,
                        FG,
                        fb,
                    );
                }
            }
        }
        HofPhase::PlayerScroll => {
            draw_scroll_pic(
                None,
                hof.scroll_stage(),
                hof.scroll_pic_x(),
                resources,
                fb,
            );
        }
        HofPhase::PlayerStats => {
            // The player's front pic stays on screen (HoFShowMonOrPlayer left
            // it at hlcoord 12,5; HoFDisplayPlayerStats does not clear).
            draw_player_front(FRONT_REST_X, FRONT_REST_Y, resources, fb);
            draw_player_stats(hof, fb, is_zh);
        }
    }
}

/// Draw the scrolling pic for a mon or (player case, `species = None`) the
/// player: back pic along the bottom band, then front pic sliding in from
/// the left (`HoFShowMonOrPlayer`, hall_of_fame.asm:97-157).
fn draw_scroll_pic(
    species: Option<pokered_data::species::Species>,
    stage: Option<HofScrollStage>,
    x: u32,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    let x = x as i32;
    if x >= fb.width() as i32 {
        return; // off-screen right (the pics enter from the edges)
    }
    match stage {
        Some(HofScrollStage::Back) => match species {
            Some(sp) => draw_mon_back(sp, x as u32, BACK_PIC_Y, resources, fb),
            None => draw_player_back(x as u32, BACK_PIC_Y, resources, fb),
        },
        Some(HofScrollStage::Front) => match species {
            Some(sp) => draw_mon_front(sp, x as u32, FRONT_REST_Y, resources, fb),
            None => draw_player_front(x as u32, FRONT_REST_Y, resources, fb),
        },
        None => {}
    }
}

fn draw_mon_front(
    species: pokered_data::species::Species,
    x: u32,
    y: u32,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    if let Some(rm) = resources.as_mut() {
        let sprite = species_to_sprite_name(&format!("{}", species));
        if let Ok(cached) = rm.load_pokemon_front(&sprite) {
            blit_native(fb, cached, x, y);
        }
    }
}

/// The back pic is a 4×4-tile sprite scaled to 7×7 by `ScaleSpriteByTwo`
/// (LoadMonBackPic, engine/battle/core.asm:6904-6905).
fn draw_mon_back(
    species: pokered_data::species::Species,
    x: u32,
    y: u32,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    if let Some(rm) = resources.as_mut() {
        let sprite = format!("{}b", species_to_sprite_name(&format!("{}", species)));
        if let Ok(cached) = rm.load_pokemon_back(&sprite) {
            blit_scaled(fb, cached, x, y);
        }
    }
}

fn draw_player_front(
    x: u32,
    y: u32,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    if let Some(rm) = resources.as_mut() {
        if let Ok(cached) = rm.load(AssetCategory::Player, "red") {
            blit_native(fb, cached, x, y);
        }
    }
}

/// RedPicBack is scaled 4×4 → 7×7 (`ScaleSpriteByTwo`, hall_of_fame.asm
/// HoFLoadPlayerPics).
fn draw_player_back(
    x: u32,
    y: u32,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    if let Some(rm) = resources.as_mut() {
        if let Ok(cached) = rm.load(AssetCategory::Player, "redb") {
            blit_scaled(fb, cached, x, y);
        }
    }
}

fn blit_native(
    fb: &mut FrameBuffer,
    cached: &pokered_renderer::resource::CachedTileSet,
    x: u32,
    y: u32,
) {
    let w_tiles = cached.source_size.0 / TILE_SIZE;
    blit_tileset(fb, &cached.tileset, x, y, w_tiles, &GRAYSCALE_SPRITE_PALETTE);
}

fn blit_scaled(
    fb: &mut FrameBuffer,
    cached: &pokered_renderer::resource::CachedTileSet,
    x: u32,
    y: u32,
) {
    let src_tpr = (cached.source_size.0 / TILE_SIZE) as usize;
    let scaled = scale_sprite_by_two(&cached.tileset, src_tpr);
    blit_tileset(fb, &scaled, x, y, 7, &GRAYSCALE_SPRITE_PALETTE);
}

/// `HoFMonInfoText` box (hall_of_fame.asm:178-200): nickname, LEVEL/, TYPE1/,
/// TYPE2/.
fn draw_mon_info(entry: &pokered_core::hof_ceremony::HofEntry, fb: &mut FrameBuffer, is_zh: bool) {
    // Inner height 9 (hall_of_fame.asm:159-176): TYPE2's value sits on the
    // last interior row — height 8 pushed it onto the bottom border.
    draw_text_box(fb, 0, 2 * T, 10, 9, FG);
    draw_text(&entry.nickname, T, 4 * T, FG, fb);
    draw_text(lang_data::ui_label("LEVEL/", is_zh), 2 * T, 6 * T, FG, fb);
    draw_text(&format!(":L{}", entry.level), 8 * T, 7 * T, FG, fb);
    draw_text(lang_data::ui_label("TYPE1/", is_zh), 2 * T, 8 * T, FG, fb);
    if let Some(stats) = pokered_data::pokemon_data::get_base_stats(entry.species) {
        draw_text(
            pokered_data::lang_data::type_name(stats.type1, is_zh),
            3 * T,
            9 * T,
            FG,
            fb,
        );
        // The original only prints TYPE2 when it differs (PrintMonType).
        if stats.type1 != stats.type2 {
            draw_text(lang_data::ui_label("TYPE2/", is_zh), 2 * T, 10 * T, FG, fb);
            draw_text(
                pokered_data::lang_data::type_name(stats.type2, is_zh),
                3 * T,
                11 * T,
                FG,
                fb,
            );
        }
    }
}

/// `HoFDisplayPlayerStats` (hall_of_fame.asm:203-228): player name, PLAY
/// TIME, MONEY, #DEX seen/owned, rating.
fn draw_player_stats(hof: &HofCeremonyState, fb: &mut FrameBuffer, is_zh: bool) {
    let stats = hof.stats();
    // Name box (hlcoord 5,0) + stats box (hlcoord 0,4).
    draw_text_box(fb, 5 * T, 0, 9, 2, FG);
    draw_text(&stats.name, 7 * T, 2 * T, FG, fb);
    draw_text_box(fb, 0, 4 * T, 10, 6, FG);
    draw_text(lang_data::ui_label("PLAY TIME", is_zh), T, 6 * T, FG, fb);
    draw_text(
        &format!("{}:{:02}", stats.play_time_hours, stats.play_time_minutes),
        5 * T,
        7 * T,
        FG,
        fb,
    );
    draw_text(lang_data::ui_label("MONEY", is_zh), T, 9 * T, FG, fb);
    draw_text(&format!("${}", stats.money), 4 * T, 10 * T, FG, fb);
    // DexSeenOwnedText / DexRatingText equivalents.
    let seen = if is_zh {
        format!("图鉴已见{:>3}", stats.dex_seen)
    } else {
        format!("#DEX SEEN {:>3}", stats.dex_seen)
    };
    draw_text(&seen, T, 12 * T, FG, fb);
    let owned = if is_zh {
        format!("拥有     {:>3}", stats.dex_owned)
    } else {
        format!("     OWNED {:>3}", stats.dex_owned)
    };
    draw_text(&owned, T, 13 * T, FG, fb);
    for (i, line) in stats.rating.split('\n').take(2).enumerate() {
        let shown = if is_zh { zh_pc_line(line) } else { line.to_string() };
        draw_text(&shown, T, (14 + i as u32) * T, FG, fb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render_config::RenderConfig;
    use pokered_core::hof_ceremony::{HofEntry, HofPlayerStats};
    use pokered_data::species::Species;
    use pokered_renderer::resource::AssetRoot;

    fn new_fb() -> FrameBuffer {
        FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE)
    }

    fn test_resources() -> Option<ResourceManager> {
        let candidate = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gfx");
        if candidate.is_dir() {
            AssetRoot::new(candidate).ok().map(ResourceManager::new)
        } else {
            None
        }
    }

    fn assert_framebuffers_equal(
        actual: &FrameBuffer,
        expected: &FrameBuffer,
        context: &str,
    ) {
        assert_eq!(actual.width(), expected.width());
        assert_eq!(actual.height(), expected.height());
        for y in 0..actual.height() {
            for x in 0..actual.width() {
                assert_eq!(
                    actual.get_pixel(x, y),
                    expected.get_pixel(x, y),
                    "framebuffer mismatch at ({x}, {y}); {context}",
                );
            }
        }
    }

    fn ceremony() -> HofCeremonyState {
        HofCeremonyState::new(
            vec![
                HofEntry {
                    species: Species::Doduo,
                    level: 24,
                    nickname: "DODUO".to_string(),
                },
                HofEntry {
                    species: Species::Lapras,
                    level: 35,
                    nickname: "LAPRAS".to_string(),
                },
            ],
            HofPlayerStats {
                name: "RED".to_string(),
                play_time_hours: 12,
                play_time_minutes: 34,
                money: 99_999,
                dex_seen: 100,
                dex_owned: 80,
                rating: "Great! Keep catching Pokemon!",
            },
        )
    }

    #[test]
    fn visual_key_only_reuses_pixel_identical_hof_frames() {
        for lang in [Lang::En, Lang::Zh] {
            let mut resources = test_resources();
            let mut hof = ceremony();
            let mut previous: Option<(HofVisualKey, HofPhase, u16, FrameBuffer)> = None;
            let mut reused = 0;
            let mut ticks = 0;

            loop {
                let key = hof_visual_key(&hof, lang);
                let mut current = new_fb();
                draw_hof_ceremony(&hof, &mut resources, &mut current, lang);
                if let Some((previous_key, previous_phase, previous_phase_frame, previous_frame)) =
                    previous.as_ref()
                {
                    if *previous_key == key {
                        let context = format!(
                            "previous={previous_phase:?}/{previous_phase_frame}, current={:?}/{}, key={key:?}",
                            hof.phase(),
                            hof.phase_frame(),
                        );
                        assert_framebuffers_equal(previous_frame, &current, &context);
                        reused += 1;
                    }
                }
                previous = Some((key, hof.phase(), hof.phase_frame(), current));

                if hof.phase() == HofPhase::Done {
                    break;
                }
                hof.update_frame();
                hof.take_sfx();
                hof.take_music_pending();
                hof.take_music_fade_pending();
                ticks += 1;
                assert!(ticks < 2000, "Hall of Fame ceremony must terminate");
            }

            assert!(reused > 900, "long dwell phases should be reusable");
        }
    }
}
