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

use pokered_core::evolution_screen::{EvolutionPhase, EvolutionScreenState};
use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::DIALOG_DEFAULT_LAYOUT;
use pokered_renderer::palette::{Palette, GRAYSCALE_SPRITE_PALETTE};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{menus, Ui};

use super::{blit_tileset, species_to_sprite_name};

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
