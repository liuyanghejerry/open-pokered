//! Renderer for the evolution cutscene
//! (`pokered_core::evolution_screen::EvolutionScreenState`).
//!
//! Port of `EvolveMon` (engine/movie/evolution.asm): the mon's front pic is
//! swapped between old and new species using the original padded 7×7 pic.
//! `PAL_BLACK` is an SGB command; ordinary Game Boy keeps the normal BGP.

use pokered_core::evolution_screen::{EvolutionPhase, EvolutionScreenState};
use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::DIALOG_DEFAULT_LAYOUT;
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{menus, Ui};

use super::species_to_sprite_name;

/// Compact description of every value consumed by [`draw_evolution`].
///
/// The evolution state machine advances several invisible delay counters.
/// On GBA, equality lets the frontend keep the already-rendered framebuffer
/// while those counters continue to advance at 60 Hz.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EvolutionVisualKey {
    black_palette: bool,
    visible_species: Option<pokered_data::species::Species>,
    text_hash: u32,
    show_input_hint: bool,
    is_zh: bool,
}

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
    _morph_flash: bool,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    if let Some(rm) = resources.as_mut() {
        let sprite = species_to_sprite_name(&format!("{}", species));
        if let Ok(cached) = rm.load_pokemon_front(&sprite) {
            super::blit_front_pic(fb, cached, 56, 16, true);
        }
    }
}
