//! Game Freak shooting-star splash renderer (`PlayShootingStar`,
//! engine/movie/intro.asm:305-341 + `AnimateShootingStar`,
//! engine/movie/splash.asm:27-146).
//!
//! - [`SplashPhase::BlackDelay`]: the three copyright lines
//!   (`CopyrightTextString`, engine/movie/title.asm:390-394) at hlcoord 2,7,
//!   black on white.
//! - Later phases: white field with black letterbox bars
//!   (`IntroDrawBlackBars`, rows 0-3 / 14-17), the Game Freak logo
//!   (gfx/splash/gamefreak_logo.png, OAM dbsprite 10,9 → screen (72,56))
//!   flashing via the `rOBP0` value from core, and the "GAME FREAK" wordmark
//!   (OAM row 12 → screen y=80, tiles 6-15).
//! - Stars use the shared original tile/OAM/palette renderer in
//!   `pokered_renderer::gamefreak_stars`.

use pokered_core::gamefreak_splash::{GameFreakSplashState, SplashPhase};
use pokered_data::layout_constants;
use pokered_renderer::embedded_font::{draw_text, measure_text};
use pokered_renderer::palette::{PaletteState, GRAYSCALE_PALETTE};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba};

use super::blit_tileset;

/// Logo OAM anchor is `dbsprite 10, 9` (splash.asm:212) → screen (72, 56).
const LOGO_SCREEN_X: u32 = 10 * 8 - 8;
const LOGO_SCREEN_Y: u32 = 9 * 8 - 16;
/// Wordmark OAM row 12 (splash.asm:218-227) → screen y 80, tiles 6-15 wide.
const WORDMARK_SCREEN_Y: u32 = 12 * 8 - 16;

pub fn draw_gamefreak_splash(
    state: &GameFreakSplashState,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    fb.clear(Rgba::WHITE);

    if state.phase == SplashPhase::BlackDelay {
        // Copyright lines, left-aligned at x=16 (hlcoord 2,7 in the original);
        // the 10-px font needs 10-px line spacing (the old 8-px tile font
        // used 8-px spacing and would overlap now).
        draw_text("©'95.'96.'98 Nintendo", 16, 56, Rgba::BLACK, fb);
        draw_text("©'95.'96.'98 Creatures inc.", 16, 66, Rgba::BLACK, fb);
        draw_text("©'95.'96.'98 GAME FREAK inc.", 16, 76, Rgba::BLACK, fb);
        return;
    }

    draw_black_bars(fb);

    // Game Freak logo, flashing via the core-computed rOBP0 (splash.asm:72-82).
    let mut palette_state = PaletteState::new(GRAYSCALE_PALETTE);
    palette_state.obp0 = state.logo_obp0();
    let logo_pal = palette_state.obj_palette0();

    if let Some(ref mut rm) = res {
        if let Ok(logo) = rm.load_splash("gamefreak_logo") {
            let ts = &logo.tileset;
            blit_tileset(fb, &ts, LOGO_SCREEN_X, LOGO_SCREEN_Y, 2, &logo_pal);
        }
        // "GAME FREAK" wordmark, centered on the screen width.
        let wordmark_x = (fb.width() - measure_text("GAME FREAK")) / 2;
        draw_text("GAME FREAK", wordmark_x, WORDMARK_SCREEN_Y, Rgba::BLACK, fb);

        pokered_renderer::gamefreak_stars::draw_stars(state, rm, fb);
    } else {
        let wordmark_x = (fb.width() - measure_text("GAME FREAK")) / 2;
        draw_text("GAME FREAK", wordmark_x, WORDMARK_SCREEN_Y, Rgba::BLACK, fb);
    }
}

/// `IntroDrawBlackBars` (engine/movie/intro.asm): black rows 0-3 and 14-17.
fn draw_black_bars(fb: &mut FrameBuffer) {
    fb.fill_rect(
        0,
        0,
        fb.width(),
        layout_constants::intro_scene::BLACK_BAR_TOP_PIXEL_H,
        Rgba::BLACK,
    );
    fb.fill_rect(
        0,
        layout_constants::intro_scene::BLACK_BAR_BOTTOM_PIXEL_Y,
        fb.width(),
        fb.height() - layout_constants::intro_scene::BLACK_BAR_BOTTOM_PIXEL_Y,
        Rgba::BLACK,
    );
}
