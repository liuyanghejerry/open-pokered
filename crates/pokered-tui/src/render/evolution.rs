//! TUI renderer for the evolution cutscene
//! (`pokered_core::evolution_screen::EvolutionScreenState`) — mirrors
//! `pokered-app/src/render/evolution.rs`: mon front pic (flickering old/new
//! on a black screen during the morph) plus the dialogue-box text beats.

use pokered_core::evolution_screen::EvolutionScreenState;
use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::DIALOG_DEFAULT_LAYOUT;
use pokered_renderer::palette::{Palette, GRAYSCALE_SPRITE_PALETTE};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};
use pokered_ui::backends::framebuffer::FrameBufferPainter;
use pokered_ui::{menus, Ui};

use super::{blit_tileset, species_to_sprite_name};

/// Light-on-black silhouette palette for the morph flicker (approximates the
/// original's PAL_BLACK whole-screen palette, evolution.asm:49-50).
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

    if let Some(species) = anim.visible_species() {
        if let Some(rm) = resources.as_mut() {
            let sprite = species_to_sprite_name(&format!("{}", species));
            if let Ok(cached) = rm.load_pokemon_front(&sprite) {
                let ts = cached.tileset.clone();
                let w_tiles = cached.source_size.0 / TILE_SIZE;
                let w_px = cached.source_size.0;
                let x = (fb.width().saturating_sub(w_px)) / 2;
                let pal;
                let pal = if black {
                    pal = morph_flash_palette();
                    &pal
                } else {
                    &GRAYSCALE_SPRITE_PALETTE
                };
                blit_tileset(fb, &ts, x, 8, w_tiles, pal);
            }
        }
    }

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
}
