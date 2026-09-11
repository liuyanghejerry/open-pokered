//! Generic yes/no choice widget.
//!
//! Renders two options (typically "YES"/"NO") in a bordered box with a
//! cursor indicator. Uses `&[MenuConfig]` — first config is the choice box.

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, TileRect, Ui};

#[derive(Debug, Clone)]
#[deprecated(note = "Use draw_yes_no with &[MenuConfig] and separate options/selected instead")]
pub struct YesNoConfig {
    pub rect: TileRect,
    pub color: Rgba,
    pub options: [String; 2],
    pub selected: usize,
    pub cursor_tx: u32,
    pub cursor_base_ty: u32,
    pub cursor_row_step: u32,
    pub cursor_glyph: char,
    pub cursor_color: Rgba,
}

#[allow(deprecated)]
impl Default for YesNoConfig {
    fn default() -> Self {
        Self {
            rect: TileRect::new(10, 16, 10, 5),
            color: Rgba::INK_BLACK,
            options: ["YES".to_string(), "NO".to_string()],
            selected: 0,
            cursor_tx: 0,
            cursor_base_ty: 1,
            cursor_row_step: 2,
            cursor_glyph: '\u{25B6}',
            cursor_color: Rgba::INK_BLACK,
        }
    }
}

#[allow(deprecated)]
impl YesNoConfig {
    pub fn new(tx: u32, ty: u32, tw: u32, th: u32) -> Self {
        Self {
            rect: TileRect::new(tx, ty, tw, th),
            ..Default::default()
        }
    }
    pub fn with_options(mut self, yes: impl Into<String>, no: impl Into<String>) -> Self {
        self.options = [yes.into(), no.into()];
        self
    }
    pub fn with_selected(mut self, idx: usize) -> Self {
        self.selected = idx.min(1);
        self
    }
    pub fn with_cursor(
        mut self,
        tx: u32,
        base_ty: u32,
        row_step: u32,
        glyph: char,
        color: Rgba,
    ) -> Self {
        self.cursor_tx = tx;
        self.cursor_base_ty = base_ty;
        self.cursor_row_step = row_step;
        self.cursor_glyph = glyph;
        self.cursor_color = color;
        self
    }
    pub fn with_color(mut self, color: Rgba) -> Self {
        self.color = color;
        self
    }
}

pub fn draw_yes_no<P: Painter>(
    options: &[String],
    selected: usize,
    configs: &[MenuConfig],
    painter: &mut P,
) {
    let Some(config) = configs.first() else {
        return;
    };
    if options.is_empty() || options.iter().all(|s| s.is_empty()) {
        return;
    }
    let mut ui = Ui::new(painter);
    ui.text_box(config.area, Rgba::INK_BLACK, true, |frame| {
        let rel_tx = config.content.tx.saturating_sub(config.area.tx + 1);
        let rel_ty = config.content.ty.saturating_sub(config.area.ty + 1);
        for (i, opt) in options.iter().enumerate() {
            if opt.is_empty() {
                continue;
            }
            let row = rel_ty + (i as u32) * 2;
            frame.label(rel_tx + 1, row, opt, Rgba::INK_BLACK);
        }
        if config.cursor.tile.is_some() {
            let cursor_ty = rel_ty + (selected as u32) * 2;
            frame.cursor_glyph_at(rel_tx, cursor_ty, '\u{25B6}', Rgba::INK_BLACK);
        }
    });
}

#[deprecated(note = "Use draw_yes_no with &[MenuConfig] instead")]
pub fn draw_yes_no_legacy<P: Painter>(config: &YesNoConfig, painter: &mut P) {
    let content = TileRect::new(
        config.rect.tx + 1,
        config.rect.ty + 1,
        config.rect.tw.saturating_sub(2),
        config.rect.th.saturating_sub(2),
    );
    let cursor = dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default());
    let mc = MenuConfig::new(config.rect, None, content, cursor);
    let options: Vec<String> = config.options.iter().cloned().collect();
    draw_yes_no(&options, config.selected, &[mc], painter);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render::TilePos;

    #[derive(Debug, Default)]
    struct RecordingPainter {
        text_boxes: Vec<(TileRect, Rgba)>,
        texts: Vec<(TilePos, String, Rgba)>,
        glyphs: Vec<(TilePos, char, Rgba)>,
    }
    impl Painter for RecordingPainter {
        fn clear(&mut self, _: Rgba) {}
        fn draw_text_box(&mut self, rect: TileRect, color: Rgba) {
            self.text_boxes.push((rect, color));
        }
        fn draw_text(&mut self, pos: TilePos, text: &str, color: Rgba) {
            self.texts.push((pos, text.to_string(), color));
        }
        fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba) {
            self.glyphs.push((pos, glyph, color));
        }
        fn draw_pixel_rect(&mut self, _: u32, _: u32, _: u32, _: u32, _: Rgba) {}
        fn draw_gb_tile(&mut self, _: TilePos, _: u8, _: &str, _: Rgba) {}
    }

    fn yes_no_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(10, 16, 10, 5),
            None,
            TileRect::new(11, 17, 8, 3),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }

    #[test]
    fn draw_both_options() {
        let config = yes_no_config();
        let mut painter = RecordingPainter::default();
        draw_yes_no(&["YES".into(), "NO".into()], 0, &[config], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "YES"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "NO"));
    }
    #[test]
    fn draw_cursor() {
        let config = yes_no_config();
        let mut painter = RecordingPainter::default();
        draw_yes_no(&["YES".into(), "NO".into()], 1, &[config], &mut painter);
        assert!(!painter.glyphs.is_empty());
        assert_eq!(painter.glyphs[0].1, '\u{25B6}');
        assert_eq!(painter.glyphs[0].0.ty, 19);
    }
    #[test]
    fn skip_empty_options() {
        let config = yes_no_config();
        let mut painter = RecordingPainter::default();
        draw_yes_no(&["YES".into(), "".into()], 0, &[config], &mut painter);
        assert_eq!(painter.texts.len(), 1);
    }
    #[test]
    fn empty_all_returns_early() {
        let config = yes_no_config();
        let mut painter = RecordingPainter::default();
        draw_yes_no(&["".into(), "".into()], 0, &[config], &mut painter);
        assert!(painter.text_boxes.is_empty());
    }
    #[test]
    fn no_configs() {
        let mut painter = RecordingPainter::default();
        draw_yes_no(&["YES".into()], 0, &[], &mut painter);
        assert!(painter.texts.is_empty());
    }
}
