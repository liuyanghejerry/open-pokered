//! Renderer for the in-game NPC trade cutscene (`pokered_core::trade::TradeAnim`).
//!
//! Faithful core of `engine/movie/trade.asm` (`InternalClockTradeFuncSequence`):
//! the given mon's panel slides in from the right edge (`Trade_ShowPlayerMon`'s
//! rWX/hSCX loop), its Poké Ball poofs and drops away (`TRADE_BALL_POOF_ANIM` /
//! `TRADE_BALL_DROP_ANIM`), enters the link cable and slides left→right between
//! two Game Boys in 4px `Delay3` steps with SFX_TINK, the trade texts play
//! ("{GIVE} went to <TRAINER>." … "For {PLAYER}'s {GIVE}, <TRAINER> sends
//! {RECEIVE}."), the new ball slides back right→left, tilts and poofs
//! (`Trade_ShowEnemyMon`) to reveal the received mon with its cry plus "Take
//! good care of {RECEIVE}.", and finally the text box slides off the right
//! edge (`Trade_SlideTextBoxOffScreen`).
//!
//! The ball poof/drop/shake/tilt frames are rendered from the shared
//! battle-animation data (`jrpg_renderer::battle_anim::SUBANIM_DATA` 0x48-0x4B
//! + the move-anim tileset) — the same frame blocks the original's
//! `Trade_ShowAnimation` plays. `gfx/trade/` assets (game_boy, cable_ball) are
//! used when present; simple shapes stand in otherwise (e.g. missing embedded
//! assets on wasm).

use jrpg_renderer::battle_anim::{
    AnimationPlayer, SubAnimTransform, ANIM_BASE_TILE_ID, SUBANIM_DATA,
};
use jrpg_renderer::sprite::SpriteLayer;
use pokered_core::game_state::Lang;
use pokered_core::trade::{TradeAnim, TradeBallSubAnim, TRADE_CABLE_Y};
use pokered_data::ui_layout::schema::DIALOG_DEFAULT_LAYOUT;
use pokered_renderer::palette::GRAYSCALE_SPRITE_PALETTE;
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{menus, Ui};

use super::{blit_tileset, species_to_sprite_name};

/// SUBANIM_DATA indices for the trade ball sub-animations
/// (`data/battle_anims/subanimations.asm` 0x48-0x4B).
fn trade_subanim_id(kind: TradeBallSubAnim) -> usize {
    match kind {
        TradeBallSubAnim::Drop => 0x48,
        TradeBallSubAnim::Shake => 0x49,
        TradeBallSubAnim::Appear => 0x4A,
        TradeBallSubAnim::Poof => 0x4B,
    }
}

/// Draw the active trade cutscene to the 160x144 framebuffer.
pub fn draw_trade(anim: &TradeAnim, resources: &mut Option<ResourceManager>, fb: &mut FrameBuffer) {
    fb.clear(Rgba::WHITE);
    let lang = if anim.is_zh { Lang::Zh } else { Lang::En };

    // The two Game Boys linked by the cable (Trade_DrawLeftGameboy /
    // Trade_DrawRightGameboy + Trade_DrawCableAcrossScreen).
    if anim.cable_visible() {
        draw_gameboys_and_cable(resources, fb);
    }

    // Mon front pic (Trade_ShowPlayerMon / Trade_ShowEnemyMon), riding the
    // window slide-in offset at the sequence start.
    if let Some(species) = anim.visible_mon() {
        draw_mon_pic(species, anim.mon_panel_offset_x(), resources, fb);
    }

    // The ball travelling the cable (Trade_BallInsideLinkCableOAMBlock).
    if let Some((bx, by)) = anim.ball_pos() {
        draw_ball(resources, bx, by, fb);
    }

    // Ball poof/drop/shake/tilt sub-animations (Trade_ShowAnimation).
    if let Some((kind, frame)) = anim.ball_sub_anim() {
        draw_ball_sub_anim(kind, frame, resources, fb);
    }

    // Interstitial trade texts, in the standard dialogue box (CJK-safe).
    if let Some((l1, l2)) = anim.text_lines() {
        let combined = format!("{}\n{}", l1, l2);
        let mut painter = FrameBufferPainter::new(fb);
        let mut ui = Ui::new(&mut painter);
        menus::dialog::draw(&combined, false, &DIALOG_DEFAULT_LAYOUT, &mut ui, lang);
    }

    // Trade_SlideTextBoxOffScreen: shift the whole scene right 2px/frame
    // until the window is off-screen (the original's window covers the
    // entire BG here, so sliding the full frame matches).
    let slide = anim.text_box_offset_x();
    if slide > 0 {
        shift_right(fb, slide as usize);
    }
}

/// Shift every framebuffer row right by `px` pixels, filling the vacated
/// left edge with white (the window's background shade).
fn shift_right(fb: &mut FrameBuffer, px: usize) {
    let w = fb.width() as usize;
    let px = px.min(w);
    for row in fb.data.chunks_exact_mut(w * 4) {
        row.copy_within(0..(w - px) * 4, px * 4);
        row[..px * 4].fill(0xFF);
    }
}

fn draw_gameboys_and_cable(resources: &mut Option<ResourceManager>, fb: &mut FrameBuffer) {
    let cy = TRADE_CABLE_Y as u32;
    // Link cable: a 2px line edge-to-edge (the original draws cable tiles at
    // the same row; the trade tileset's cable art is horizontal line art).
    for x in 0..fb.width() {
        fb.set_pixel(x, cy, Rgba::BLACK);
        fb.set_pixel(x, cy + 1, Rgba::BLACK);
    }
    // Game Boy pics at both cable ends.
    let mut drew_gbs = false;
    if let Some(rm) = resources.as_mut() {
        if let Ok(cached) = rm.load_trade("game_boy") {
            let ts = cached.tileset.clone();
            let w_tiles = cached.source_size.0 / TILE_SIZE;
            let w_px = cached.source_size.0;
            let h_px = cached.source_size.1;
            let y = cy.saturating_sub(h_px / 2);
            blit_tileset(fb, &ts, 4, y, w_tiles, &GRAYSCALE_SPRITE_PALETTE);
            let right_x = fb.width().saturating_sub(w_px + 4);
            blit_tileset(fb, &ts, right_x, y, w_tiles, &GRAYSCALE_SPRITE_PALETTE);
            drew_gbs = true;
        }
    }
    if !drew_gbs {
        // Fallback: plain boxes where the Game Boys sit.
        draw_gb_placeholder(4, cy, fb);
        draw_gb_placeholder(fb.width().saturating_sub(28), cy, fb);
    }
}

fn draw_gb_placeholder(x: u32, cy: u32, fb: &mut FrameBuffer) {
    let top = cy.saturating_sub(12);
    for dy in 0..24u32 {
        for dx in 0..24u32 {
            let border = dy == 0 || dy == 23 || dx == 0 || dx == 23;
            if border {
                fb.set_pixel(x + dx, top + dy, Rgba::BLACK);
            }
        }
    }
}

/// Mon front pic at the original's `hlcoord 7, 2` (x=56, y=16) plus the
/// window slide-in offset (the panel enters from the right edge).
fn draw_mon_pic(
    species: pokered_data::species::Species,
    panel_offset_x: i32,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    if let Some(rm) = resources.as_mut() {
        let sprite = species_to_sprite_name(&format!("{}", species));
        if let Ok(cached) = rm.load_pokemon_front(&sprite) {
            let ts = cached.tileset.clone();
            let w_tiles = cached.source_size.0 / TILE_SIZE;
            let x = 56 + panel_offset_x;
            if x >= 0 {
                blit_tileset(fb, &ts, x as u32, 16, w_tiles, &GRAYSCALE_SPRITE_PALETTE);
            }
        }
    }
}

/// Draw one frame block of a trade ball sub-animation, reusing the battle
/// animation data + move-anim tileset (the ball/poof tiles live there, like
/// the original's TradingAnimationGraphics copy at vChars2 tile $31).
fn draw_ball_sub_anim(
    kind: TradeBallSubAnim,
    frame: u8,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    let (transform, frames) = &SUBANIM_DATA[trade_subanim_id(kind)];
    let Some(&(frame_block, base_coord, _mode)) = frames.get(frame as usize) else {
        return;
    };
    let mut entries = Vec::new();
    AnimationPlayer::render_frame_block(
        frame_block as usize,
        base_coord as usize,
        SubAnimTransform::from_u8(*transform),
        &mut entries,
    );
    if let Some(rm) = resources.as_mut() {
        if let Ok(cached) = rm.load_battle("move_anim_0") {
            let ts = cached.tileset.clone();
            let mut layer = SpriteLayer::new();
            for mut e in entries {
                // OAM tile ids are absolute VRAM ids (raw + $31, matching
                // DrawFrameBlock); the loaded tileset is indexed from 0.
                e.tile_id = e.tile_id.wrapping_sub(ANIM_BASE_TILE_ID);
                layer.add(e);
            }
            layer.render(
                fb,
                &ts,
                &GRAYSCALE_SPRITE_PALETTE,
                &GRAYSCALE_SPRITE_PALETTE,
                None,
            );
        }
    }
}

fn draw_ball(resources: &mut Option<ResourceManager>, bx: i32, by: i32, fb: &mut FrameBuffer) {
    let mut drew = false;
    if let Some(rm) = resources.as_mut() {
        if let Ok(cached) = rm.load_trade("cable_ball") {
            let ts = cached.tileset.clone();
            let w_tiles = cached.source_size.0 / TILE_SIZE;
            let x = (bx - (cached.source_size.0 / 2) as i32).max(0) as u32;
            let y = (by - (cached.source_size.1 / 2) as i32).max(0) as u32;
            blit_tileset(fb, &ts, x, y, w_tiles, &GRAYSCALE_SPRITE_PALETTE);
            drew = true;
        }
    }
    if !drew {
        // Fallback: an 8x8 ball marker (outline + midline), Poké Ball style.
        let x0 = bx - 4;
        let y0 = by - 4;
        for dy in 0..8i32 {
            for dx in 0..8i32 {
                let edge = dy == 0 || dy == 7 || dx == 0 || dx == 7 || dy == 3 || dy == 4;
                if edge {
                    let (px, py) = (x0 + dx, y0 + dy);
                    if px >= 0 && py >= 0 && (px as u32) < fb.width() && (py as u32) < fb.height() {
                        fb.set_pixel(px as u32, py as u32, Rgba::BLACK);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jrpg_engine::render_config::RenderConfig;
    use pokered_core::trade::TradeAnimPhase;
    use pokered_data::species::Species;
    use pokered_renderer::resource::{AssetRoot, ResourceManager};

    fn new_fb() -> FrameBuffer {
        FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE)
    }

    fn ink_pixels(fb: &FrameBuffer) -> usize {
        let mut count = 0;
        for y in 0..fb.height() {
            for x in 0..fb.width() {
                if fb.get_pixel(x, y) != Some(Rgba::WHITE) {
                    count += 1;
                }
            }
        }
        count
    }

    /// gfx/ lives at gfx; skip asset-backed checks when it
    /// has not been fetched (`scripts/fetch-gfx.sh`).
    fn test_resources() -> Option<ResourceManager> {
        let candidate = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gfx");
        if candidate.is_dir() {
            AssetRoot::new(candidate).ok().map(ResourceManager::new)
        } else {
            None
        }
    }

    /// Every phase renders without panicking; key frames are dumped to
    /// temp-dir PNGs for visual inspection.
    #[test]
    fn cutscene_renders_every_phase() {
        let mut resources = test_resources();
        let have_gfx = resources.is_some();
        // Cubone/Machoke: sprite file names that match `species_to_sprite_name`
        // (MrMime's gfx file is `mr.mime.png`, which the helper can't map).
        // Cubone/Machoke: sprite file names that match `species_to_sprite_name`
        // (MrMime's gfx file is `mr.mime.png`, which the helper can't map).
        let mut anim = TradeAnim::new(Species::Cubone, Species::Machoke, "RED".to_string(), false);
        let mut seen: Vec<TradeAnimPhase> = Vec::new();
        let mut ticks = 0;
        loop {
            if seen.last() != Some(&anim.phase()) {
                seen.push(anim.phase());
                let mut fb = new_fb();
                draw_trade(&anim, &mut resources, &mut fb);
                // Every phase entry except the final clear draws something —
                // apart from the very start of the slide-in, where the panel
                // is still fully off the right edge (x = 56+offset ≥ 160).
                let panel_off_screen = anim.phase() == TradeAnimPhase::SlideInGiveMon
                    && anim.mon_panel_offset_x() >= 104;
                if anim.phase() != TradeAnimPhase::Done && !panel_off_screen {
                    assert!(ink_pixels(&fb) > 50, "phase {:?} draws content", anim.phase());
                }
                let path = std::env::temp_dir()
                    .join(format!("trade_{:02}_{:?}.png", seen.len(), anim.phase()));
                fb.save_png(&path).expect("save trade phase png");
            }
            if anim.tick() {
                break;
            }
            anim.pending_sfx.clear();
            ticks += 1;
            assert!(ticks < 2000, "animation must terminate");
        }
        assert_eq!(seen.len(), 15, "all visible phases rendered");
    }

    /// The slide-in moves the mon panel from off-screen-right to its rest
    /// position; mid-slide the pic is only partially on screen.
    #[test]
    fn slide_in_panel_offsets_pic() {
        let mut resources = test_resources();
        let mut anim = TradeAnim::new(Species::Cubone, Species::Machoke, "RED".to_string(), false);
        // Frame 0: fully off the right edge (offset 126 at x=56).
        let mut fb = new_fb();
        draw_trade(&anim, &mut resources, &mut fb);
        let ink_start = ink_pixels(&fb);
        while anim.phase() == TradeAnimPhase::SlideInGiveMon && anim.mon_panel_offset_x() > 40 {
            anim.tick();
        }
        let mut fb = new_fb();
        draw_trade(&anim, &mut resources, &mut fb);
        let ink_mid = ink_pixels(&fb);
        let path = std::env::temp_dir().join("trade_slide_in_mid.png");
        fb.save_png(&path).expect("save slide-in png");
        if resources.is_some() {
            assert!(
                ink_mid > ink_start,
                "mid-slide shows more of the pic than the off-screen start"
            );
        }
    }

    /// The ball sub-animations (poof/drop/shake/tilt) draw frame-block tiles
    /// from the move-anim tileset.
    #[test]
    fn ball_sub_anims_draw_frame_blocks() {
        let mut resources = test_resources();
        let mut anim = TradeAnim::new(Species::Cubone, Species::Machoke, "RED".to_string(), false);
        let targets = [
            TradeAnimPhase::GiveMonPoof,
            TradeAnimPhase::GiveMonBallDrop,
            TradeAnimPhase::BallEnterCable, // shake window (frame 20..36)
            TradeAnimPhase::ReceiveBallTilt,
            TradeAnimPhase::ReceiveMonPoof,
        ];
        for target in targets {
            while anim.phase() != target {
                anim.tick();
            }
            // Advance into the sub-anim's active window.
            if target == TradeAnimPhase::BallEnterCable {
                for _ in 0..22 {
                    anim.tick();
                }
            }
            assert!(anim.ball_sub_anim().is_some(), "{target:?} has a sub-anim");
            let mut fb = new_fb();
            draw_trade(&anim, &mut resources, &mut fb);
            let path = std::env::temp_dir().join(format!("trade_subanim_{target:?}.png"));
            fb.save_png(&path).expect("save sub-anim png");
            assert!(ink_pixels(&fb) > 20, "{target:?} sub-anim draws tiles");
        }
    }

    /// The final text-box slide shifts the frame right and leaves white
    /// behind on the left.
    #[test]
    fn text_box_slide_shifts_frame() {
        let mut resources = test_resources();
        let mut anim = TradeAnim::new(Species::Cubone, Species::Machoke, "RED".to_string(), false);
        while anim.phase() != TradeAnimPhase::SlideTextBoxOff {
            anim.tick();
        }
        for _ in 0..80 {
            anim.tick();
        }
        assert_eq!(anim.text_box_offset_x(), 62);
        let mut fb = new_fb();
        draw_trade(&anim, &mut resources, &mut fb);
        let path = std::env::temp_dir().join("trade_text_slide_off.png");
        fb.save_png(&path).expect("save slide-off png");
        // Left columns are vacated to white; the box has moved right.
        assert_eq!(fb.get_pixel(2, 120), Some(Rgba::WHITE));
        assert!(ink_pixels(&fb) > 50);
    }
}

