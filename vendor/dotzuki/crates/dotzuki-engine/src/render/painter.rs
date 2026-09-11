use crate::render::{BracketSides, Rgba, TilePos, TileRect};

pub trait Painter {
    fn clear(&mut self, color: Rgba);
    fn draw_text_box(&mut self, rect: TileRect, color: Rgba);
    fn draw_text(&mut self, pos: TilePos, text: &str, color: Rgba);
    fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba);
    fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, color: Rgba);
    /// Pixel backend draws `tile_id` via the fallback text glyph. Recording backends log only `tile_id`.
    fn draw_gb_tile(&mut self, pos: TilePos, tile_id: u8, fallback: &str, color: Rgba);

    // ── Proportional (pixel-precise) text — opt-in high-resolution path ──────
    //
    // The legacy methods above place every glyph on the 8×8 tile grid, which is
    // correct for Game Boy half-width fonts but clips/overlaps proportional CJK
    // glyphs. These methods let the layout engine render at true pixel precision
    // with per-glyph advance. They have default impls that fall back to the tile
    // path, so existing `Painter` implementations (and all recording/mock
    // painters) keep compiling unchanged; only pixel backends override them.

    /// Draw `text` starting at pixel `(px, py)` with proportional per-glyph
    /// advance. Default: round to the nearest tile and use [`Painter::draw_text`].
    fn draw_text_px(&mut self, px: u32, py: u32, text: &str, color: Rgba) {
        self.draw_text(TilePos::new(px / 8, py / 8), text, color);
    }

    /// The pixel width `text` would occupy via [`Painter::draw_text_px`].
    /// Default: 8px per char (the tile cell width).
    fn measure_text_px(&self, text: &str) -> u32 {
        text.chars().count() as u32 * 8
    }

    /// Draw `text` at pixel `(px, py)` scaled by an integer factor — every glyph
    /// pixel becomes a `scale × scale` block. Powers big title/heading text. The
    /// default ignores `scale` and falls back to [`Painter::draw_text_px`], so
    /// recording/mock backends stay unchanged; pixel backends override it.
    fn draw_text_px_scaled(&mut self, px: u32, py: u32, text: &str, scale: u32, color: Rgba) {
        let _ = scale;
        self.draw_text_px(px, py, text, color);
    }

    /// The pixel width `text` occupies via [`Painter::draw_text_px_scaled`].
    /// Default: [`Painter::measure_text_px`] × `scale`.
    fn measure_text_px_scaled(&self, text: &str, scale: u32) -> u32 {
        self.measure_text_px(text) * scale.max(1)
    }

    /// Whether this backend renders true proportional pixel text (overrides the
    /// two methods above). The layout engine only takes its proportional path
    /// when this is `true`, so recording/mock backends stay on the tile path.
    fn supports_proportional(&self) -> bool {
        false
    }

    /// Blit a full-colour `src_w × src_h` RGBA image (row-major) into the pixel
    /// box `(dst_px, dst_py, dst_w, dst_h)`, nearest-neighbour scaled to fill the
    /// box. Fully-transparent (`a == 0`) source pixels are skipped; `flip_x`/
    /// `flip_y` mirror the source. Used by the layout engine's `image` element.
    ///
    /// Default: per-destination-pixel via [`Painter::draw_pixel_rect`], so every
    /// existing backend (incl. recording/mock painters) works unchanged; pixel
    /// backends may override for speed.
    fn draw_rgba(
        &mut self,
        dst_px: u32,
        dst_py: u32,
        dst_w: u32,
        dst_h: u32,
        pixels: &[Rgba],
        src_w: u32,
        src_h: u32,
        flip_x: bool,
        flip_y: bool,
    ) {
        if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
            return;
        }
        for dy in 0..dst_h {
            let mut sy = dy * src_h / dst_h;
            if flip_y {
                sy = src_h - 1 - sy;
            }
            for dx in 0..dst_w {
                let mut sx = dx * src_w / dst_w;
                if flip_x {
                    sx = src_w - 1 - sx;
                }
                let idx = (sy * src_w + sx) as usize;
                let Some(&c) = pixels.get(idx) else { continue };
                if c.a == 0 {
                    continue;
                }
                self.draw_pixel_rect(dst_px + dx, dst_py + dy, 1, 1, c);
            }
        }
    }
}

pub struct Ui<'p, P: Painter> {
    painter: &'p mut P,
    origin_tx: u32,
    origin_ty: u32,
}

impl<'p, P: Painter> Ui<'p, P> {
    pub fn new(painter: &'p mut P) -> Self {
        Self {
            painter,
            origin_tx: 0,
            origin_ty: 0,
        }
    }

    /// Returns a mutable reference to the underlying [`Painter`].
    pub fn painter(&mut self) -> &mut P {
        self.painter
    }

    pub fn clear(&mut self, color: impl Into<Rgba>) {
        self.painter.clear(color.into());
    }

    pub fn text_box<F>(&mut self, rect: TileRect, color: impl Into<Rgba>, border: bool, body: F)
    where
        F: FnOnce(&mut Frame<'_, P>),
    {
        let absolute = rect.translated(self.origin_tx, self.origin_ty);
        if border {
            self.painter.draw_text_box(absolute, color.into());
        }
        let inset: u32 = if border { 1 } else { 0 };
        let mut frame = Frame {
            painter: self.painter,
            origin_tx: absolute.tx + inset,
            origin_ty: absolute.ty + inset,
        };
        body(&mut frame);
    }
}

pub struct Frame<'p, P: Painter> {
    painter: &'p mut P,
    origin_tx: u32,
    origin_ty: u32,
}

impl<'p, P: Painter> Frame<'p, P> {
    pub fn label(&mut self, tx: u32, ty: u32, text: &str, color: impl Into<Rgba>) {
        self.painter.draw_text(
            TilePos::new(self.origin_tx + tx, self.origin_ty + ty),
            text,
            color.into(),
        );
    }

    pub fn cursor_at(&mut self, tx: u32, ty: u32, color: impl Into<Rgba>) {
        self.cursor_glyph_at(tx, ty, '\u{25B6}', color);
    }

    pub fn cursor_glyph_at(&mut self, tx: u32, ty: u32, glyph: char, color: impl Into<Rgba>) {
        self.painter.draw_glyph(
            TilePos::new(self.origin_tx + tx, self.origin_ty + ty),
            glyph,
            color.into(),
        );
    }

    /// Draw a glyph at a screen-absolute tile position. Bypasses the frame
    /// origin so the glyph appears at the requested column/row regardless of
    /// where the enclosing text_box is placed. Useful for cursors that sit at
    /// the border edge of a box (where frame-relative coordinates would be
    /// negative and thus inexpressible as u32).
    pub fn abs_glyph(&mut self, tx: u32, ty: u32, glyph: char, color: impl Into<Rgba>) {
        self.painter
            .draw_glyph(TilePos::new(tx, ty), glyph, color.into());
    }

    pub fn menu_list(
        &mut self,
        tx: u32,
        ty: u32,
        items: &[&str],
        cursor: usize,
        row_step: u32,
        color: impl Into<Rgba>,
    ) {
        let color = color.into();
        for (i, item) in items.iter().enumerate() {
            let row = ty + (i as u32) * row_step;
            self.label(tx + 1, row, item, color);
            if i == cursor {
                self.cursor_at(tx, row, color);
            }
        }
    }

    pub fn pixel_rect(
        &mut self,
        dx_px: u32,
        dy_px: u32,
        w_px: u32,
        h_px: u32,
        color: impl Into<Rgba>,
    ) {
        let (ox_px, oy_px) = TilePos::new(self.origin_tx, self.origin_ty).to_pixels();
        self.painter
            .draw_pixel_rect(ox_px + dx_px, oy_px + dy_px, w_px, h_px, color.into());
    }

    pub fn sub_text_box<F>(&mut self, rect: TileRect, color: impl Into<Rgba>, body: F)
    where
        F: FnOnce(&mut Frame<'_, P>),
    {
        let absolute = rect.translated(self.origin_tx, self.origin_ty);
        self.painter.draw_text_box(absolute, color.into());
        let mut child = Frame {
            painter: self.painter,
            origin_tx: absolute.tx + 1,
            origin_ty: absolute.ty + 1,
        };
        body(&mut child);
    }

    pub fn gb_tile(
        &mut self,
        tx: u32,
        ty: u32,
        tile_id: u8,
        fallback: &str,
        color: impl Into<Rgba>,
    ) {
        self.painter.draw_gb_tile(
            TilePos::new(self.origin_tx + tx, self.origin_ty + ty),
            tile_id,
            fallback,
            color.into(),
        );
    }

    /// Draws a partial border (one or more sides) inside the tile rect
    /// `rect`. Pixel offsets match the interior corner of the GB box-drawing
    /// glyphs (`+6` on the right of a tile column, `+6` on the bottom of a
    /// tile row), so the bracket aligns exactly with `text_box` borders.
    ///
    /// If `with_arrow` is true, draws a 4px halfarrow (`<`) at the
    /// far-left of the bottom edge — matching original `DrawLineBox`
    /// terminator. Requires `sides.bottom = true`.
    pub fn bracket_box(
        &mut self,
        rect: TileRect,
        sides: BracketSides,
        with_arrow: bool,
        color: impl Into<Rgba>,
    ) {
        let color = color.into();
        // Interior-corner offsets: right edge x = (last_col)*8 + 6,
        // bottom edge y = (last_row)*8 + 6 — matches box_tiles glyph
        // pixels in pokered-renderer::embedded_font.
        let left_px = rect.tx * 8;
        let right_px = (rect.tx + rect.tw - 1) * 8 + 6;
        let top_px = rect.ty * 8;
        let bot_px = (rect.ty + rect.th - 1) * 8 + 6;

        if sides.right {
            self.pixel_rect(right_px, top_px, 1, bot_px - top_px + 1, color);
        }
        if sides.left {
            self.pixel_rect(left_px, top_px, 1, bot_px - top_px + 1, color);
        }
        if sides.top {
            self.pixel_rect(left_px, top_px, right_px - left_px + 1, 1, color);
        }
        if sides.bottom {
            self.pixel_rect(left_px, bot_px, right_px - left_px + 1, 1, color);
            if with_arrow && left_px >= 3 {
                let arrow_left = left_px - 3;
                self.pixel_rect(arrow_left, bot_px, 4, 1, color);
                self.pixel_rect(arrow_left, bot_px - 1, 1, 1, color);
                self.pixel_rect(arrow_left, bot_px + 1, 1, 1, color);
            }
        }
    }

    /// 1-pixel-wide vertical line at the right interior edge of tile column
    /// `tx`, spanning `length_tiles` tile rows starting at `ty`.
    pub fn vline(&mut self, tx: u32, ty: u32, length_tiles: u32, color: impl Into<Rgba>) {
        let px = tx * 8 + 6;
        let py = ty * 8;
        self.pixel_rect(px, py, 1, length_tiles * 8, color);
    }

    /// 1-pixel-tall horizontal line at the bottom interior edge of tile row
    /// `ty`, spanning `length_tiles` tile columns starting at `tx`.
    pub fn hline(&mut self, tx: u32, ty: u32, length_tiles: u32, color: impl Into<Rgba>) {
        let px = tx * 8;
        let py = ty * 8 + 6;
        self.pixel_rect(px, py, length_tiles * 8, 1, color);
    }

    /// Draws a vertical sequence of label-value pairs starting at tile
    /// `(tx, ty)`. Each pair places `label` at `(tx, row)` and `value` at
    /// `(tx + value_indent_x, row + 1)`, advancing by `row_step` tile rows.
    pub fn label_value_grid(
        &mut self,
        tx: u32,
        ty: u32,
        rows: &[LabelValue<'_>],
        value_indent_x: u32,
        row_step: u32,
        label_color: impl Into<Rgba>,
        value_color: impl Into<Rgba>,
    ) {
        let label_color = label_color.into();
        let value_color = value_color.into();
        for (i, lv) in rows.iter().enumerate() {
            let row = ty + (i as u32) * row_step;
            self.label(tx, row, lv.label, label_color);
            self.label(tx + value_indent_x, row + 1, &lv.value, value_color);
        }
    }
}

/// Label and formatted value, for [`Frame::label_value_grid`].
#[derive(Debug, Clone)]
pub struct LabelValue<'a> {
    pub label: &'a str,
    pub value: String,
}

impl<'a> LabelValue<'a> {
    pub fn new(label: &'a str, value: impl Into<String>) -> Self {
        Self {
            label,
            value: value.into(),
        }
    }
}
