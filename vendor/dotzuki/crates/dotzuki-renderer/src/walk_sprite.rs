//! Full-color overworld walk sprite — the render backend for the engine's generic
//! [`dotzuki_engine::overworld::actor::OverworldActor`].
//!
//! An RGBA sheet sliced into a `rows × cols` grid (**row = facing** down/up/left/right,
//! **col = walk frame**), produced by the `character-sprite-gen` skill. The actor picks
//! `(row, col)`; this blits that cell bottom-centred on the foot tile (a 24×32 sprite
//! stands on a 16×16 tile with its head above). pokered's overworld uses GB OAM sprites
//! ([`crate::sprite`]) instead — same actor, different painter.

use dotzuki_engine::render::{FrameBuffer, Rgba};

/// A walk-cycle sprite sheet sliced into uniform `frame_w × frame_h` cells.
pub struct WalkSprite {
    /// Whole sheet, row-major RGBA, `sheet_w * sheet_h` pixels.
    pixels: Vec<Rgba>,
    sheet_w: u32,
    pub frame_w: u32,
    pub frame_h: u32,
    pub rows: u32,
    pub cols: u32,
}

impl WalkSprite {
    /// Build from decoded RGBA pixels (`sheet_w * sheet_h`), sliced into cells.
    pub fn from_rgba(
        pixels: Vec<Rgba>,
        sheet_w: u32,
        sheet_h: u32,
        frame_w: u32,
        frame_h: u32,
    ) -> Result<Self, String> {
        if frame_w == 0
            || frame_h == 0
            || sheet_w < frame_w
            || sheet_h < frame_h
            || pixels.len() != (sheet_w * sheet_h) as usize
        {
            return Err(format!(
                "bad sheet {sheet_w}x{sheet_h} / frame {frame_w}x{frame_h} / {} px",
                pixels.len()
            ));
        }
        Ok(Self {
            pixels,
            sheet_w,
            frame_w,
            frame_h,
            rows: sheet_h / frame_h,
            cols: sheet_w / frame_w,
        })
    }

    /// Load + slice a PNG sprite sheet (`data/gfx/overworld/<id>/sheet.png`).
    #[cfg(feature = "image-assets")]
    pub fn load(path: &std::path::Path, frame_w: u32, frame_h: u32) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let img = image::load_from_memory(&bytes)
            .map_err(|e| format!("decode {}: {e}", path.display()))?
            .to_rgba8();
        let (w, h) = img.dimensions();
        let pixels = img
            .pixels()
            .map(|p| Rgba::new(p.0[0], p.0[1], p.0[2], p.0[3]))
            .collect();
        Self::from_rgba(pixels, w, h, frame_w, frame_h)
    }

    /// Pixel of frame `(row, col)` at frame-local `(fx, fy)`; transparent if out of range.
    #[inline]
    pub fn pixel(&self, row: u32, col: u32, fx: u32, fy: u32) -> Rgba {
        if row >= self.rows || col >= self.cols || fx >= self.frame_w || fy >= self.frame_h {
            return Rgba::TRANSPARENT;
        }
        let x = col * self.frame_w + fx;
        let y = row * self.frame_h + fy;
        self.pixels
            .get((y * self.sheet_w + x) as usize)
            .copied()
            .unwrap_or(Rgba::TRANSPARENT)
    }

    /// Blit frame `(row, col)` bottom-centred on the foot tile whose top-left is
    /// `(foot_x, foot_y)` in screen pixels (tile size `tile`); the head extends above.
    /// Transparent pixels are skipped; drawing is clipped to the framebuffer.
    pub fn draw_on_tile(
        &self,
        fb: &mut FrameBuffer,
        row: u32,
        col: u32,
        foot_x: i32,
        foot_y: i32,
        tile: i32,
    ) {
        let sx = foot_x - (self.frame_w as i32 - tile) / 2;
        let sy = foot_y - (self.frame_h as i32 - tile);
        let (w, h) = (fb.width() as i32, fb.height() as i32);
        for fy in 0..self.frame_h {
            let py = sy + fy as i32;
            if py < 0 || py >= h {
                continue;
            }
            for fx in 0..self.frame_w {
                let c = self.pixel(row, col, fx, fy);
                if c.a == 0 {
                    continue;
                }
                let px = sx + fx as i32;
                if px < 0 || px >= w {
                    continue;
                }
                fb.set_pixel(px as u32, py as u32, c);
            }
        }
    }
}
