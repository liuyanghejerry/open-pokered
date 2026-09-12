//! Renderer for the evolution cutscene
//! (`pokered_core::evolution_screen::EvolutionScreenState`).
//!
//! Port of `EvolveMon` (engine/movie/evolution.asm): the mon's front pic is
//! shown in its palette, then the whole screen goes black (`PAL_BLACK`,
//! evolution.asm:49-50) while the pic flickers between the old and new
//! species (`Evolution_BackAndForthAnim`), and finally the evolved (or, on a
//! B-cancel, the original) species is revealed. The texts play in the
//! standard dialogue box. On the GB the flicker swaps tile IDs in place; here
//! we redraw the alternating pics on a black background, with the pic itself
//! drawn in an inverted "silhouette" palette to read as the original flash.

use crate::alloc_prelude::*;
use pokered_core::evolution_screen::{EvolutionPhase, EvolutionScreenState};
use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::DIALOG_DEFAULT_LAYOUT;
use pokered_renderer::palette::{Palette, GRAYSCALE_SPRITE_PALETTE};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{menus, Ui};

use super::{blit_tileset, species_to_sprite_name};

/// Compact description of every value consumed by [`draw_evolution`].
///
/// The evolution state machine advances several invisible delay counters.
/// On GBA, equality lets the frontend keep the already-rendered framebuffer
/// while those counters continue to advance at 60 Hz.
#[cfg(any(test, target_os = "none"))]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EvolutionVisualKey {
    black_palette: bool,
    visible_species: Option<pokered_data::species::Species>,
    text_hash: u32,
    show_input_hint: bool,
    is_zh: bool,
}

#[cfg(any(test, target_os = "none"))]
pub fn evolution_visual_key(anim: &EvolutionScreenState) -> EvolutionVisualKey {
    let mut text_hash = 0x811c_9dc5u32;
    let mut hash_byte = |byte: u8| {
        text_hash = (text_hash ^ byte as u32).wrapping_mul(0x0100_0193);
    };
    if let Some((line1, line2)) = anim.text_lines() {
        hash_byte(1);
        for byte in line1.bytes() {
            hash_byte(byte);
        }
        hash_byte(0xff);
        for byte in line2.bytes() {
            hash_byte(byte);
        }
    } else {
        hash_byte(0);
    }

    EvolutionVisualKey {
        black_palette: anim.black_palette(),
        visible_species: anim.visible_species(),
        text_hash,
        show_input_hint: matches!(
            anim.phase(),
            EvolutionPhase::IntroText | EvolutionPhase::StoppedText
        ),
        is_zh: anim.is_zh,
    }
}

/// Inverted palette for the black-screen morph flash (approximates the
/// original's PAL_BLACK whole-screen palette during the flicker): the
/// silhouette renders light-on-black.
fn morph_flash_palette() -> Palette {
    let mut p = GRAYSCALE_SPRITE_PALETTE;
    p.colors[1] = Rgba::rgb(0xAA, 0xAA, 0xAA);
    p.colors[2] = Rgba::rgb(0x55, 0x55, 0x55);
    p.colors[3] = Rgba::WHITE;
    p
}

/// Draw the active evolution cutscene to the 160x144 framebuffer.
pub fn draw_evolution(
    anim: &EvolutionScreenState,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    let black = anim.black_palette();
    fb.clear(if black { Rgba::BLACK } else { Rgba::WHITE });

    // The mon pic (old species pre-morph, flickering old/new during the
    // morph, final species afterwards) — `Evolution_LoadPic`
    // (evolution.asm:100-103) centers the 7x7 front pic at hlcoord 7, 2.
    if let Some(species) = anim.visible_species() {
        draw_mon_pic(species, black, resources, fb);
    }

    // Text beats in the standard dialogue box (CJK-safe).
    if let Some((l1, l2)) = anim.text_lines() {
        let combined = if l2.is_empty() {
            l1
        } else {
            format!("{}\n{}", l1, l2)
        };
        let lang = if anim.is_zh { Lang::Zh } else { Lang::En };
        let mut painter = FrameBufferPainter::new(fb);
        let mut ui = Ui::new(&mut painter);
        menus::dialog::draw(&combined, false, &DIALOG_DEFAULT_LAYOUT, &mut ui, lang);
    }

    // Subtle "press A" hint on the phases that wait for a button (the
    // cancelled-evolution prompt and the Rare Candy pre-message).
    if matches!(
        anim.phase(),
        EvolutionPhase::IntroText | EvolutionPhase::StoppedText
    ) {
        let x = fb.width().saturating_sub(12);
        let y = fb.height().saturating_sub(8);
        fb.set_pixel(x, y, Rgba::BLACK);
        fb.set_pixel(x + 1, y - 1, Rgba::BLACK);
        fb.set_pixel(x + 2, y - 2, Rgba::BLACK);
    }
}

fn draw_mon_pic(
    species: pokered_data::species::Species,
    morph_flash: bool,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    if let Some(rm) = resources.as_mut() {
        let sprite = species_to_sprite_name(&format!("{}", species));
        if let Ok(cached) = rm.load_pokemon_front(&sprite) {
            let ts = cached.tileset.clone();
            let w_tiles = cached.source_size.0 / TILE_SIZE;
            let w_px = cached.source_size.0;
            let x = (fb.width().saturating_sub(w_px)) / 2;
            let pal;
            let pal = if morph_flash {
                pal = morph_flash_palette();
                &pal
            } else {
                &GRAYSCALE_SPRITE_PALETTE
            };
            blit_tileset(fb, &ts, x, 8, w_tiles, pal);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render_config::RenderConfig;
    use pokered_core::evolution_screen::{EvolutionInput, PendingEvolution};
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

    fn assert_framebuffers_equal(actual: &FrameBuffer, expected: &FrameBuffer) {
        assert_eq!(actual.width(), expected.width());
        assert_eq!(actual.height(), expected.height());
        for y in 0..actual.height() {
            for x in 0..actual.width() {
                assert_eq!(
                    actual.get_pixel(x, y),
                    expected.get_pixel(x, y),
                    "framebuffer mismatch at ({x}, {y})",
                );
            }
        }
    }

    fn animation(is_zh: bool) -> EvolutionScreenState {
        EvolutionScreenState::new(
            vec![PendingEvolution {
                party_index: 0,
                from: Species::Bulbasaur,
                to: Species::Ivysaur,
                name: if is_zh {
                    "妙蛙种子".to_string()
                } else {
                    "BULBASAUR".to_string()
                },
                force: false,
            }],
            Some(if is_zh {
                "妙蛙种子升到了\n16级！".to_string()
            } else {
                "BULBASAUR grew to\nlevel 16!".to_string()
            }),
            is_zh,
        )
    }

    fn verify_flow(is_zh: bool, cancel: bool) -> usize {
        let mut resources = test_resources();
        let mut anim = animation(is_zh);
        let mut previous: Option<(EvolutionVisualKey, FrameBuffer)> = None;
        let mut reused = 0;
        let mut stopped_hold = 0;
        let mut ticks = 0;

        loop {
            let key = evolution_visual_key(&anim);
            let mut current = new_fb();
            draw_evolution(&anim, &mut resources, &mut current);
            if let Some((previous_key, previous_frame)) = previous.as_ref() {
                if *previous_key == key {
                    assert_framebuffers_equal(previous_frame, &current);
                    reused += 1;
                }
            }
            previous = Some((key, current));

            if anim.is_done() {
                break;
            }
            let input = match anim.phase() {
                EvolutionPhase::IntroText => EvolutionInput { a: true, b: false },
                EvolutionPhase::Morph if cancel && anim.cancel_window_open() => {
                    EvolutionInput { a: false, b: true }
                }
                EvolutionPhase::StoppedText if stopped_hold >= 10 => {
                    EvolutionInput { a: true, b: false }
                }
                EvolutionPhase::StoppedText => {
                    stopped_hold += 1;
                    EvolutionInput::none()
                }
                _ => EvolutionInput::none(),
            };
            anim.tick(input);
            anim.pending_sfx.clear();
            ticks += 1;
            assert!(ticks < 2000, "evolution animation must terminate");
        }

        reused
    }

    #[test]
    fn visual_key_only_reuses_pixel_identical_evolution_frames() {
        for is_zh in [false, true] {
            assert!(
                verify_flow(is_zh, false) > 300,
                "success path should reuse static delay frames"
            );
            assert!(
                verify_flow(is_zh, true) > 150,
                "cancel path should reuse static delay and prompt frames"
            );
        }
    }
}
