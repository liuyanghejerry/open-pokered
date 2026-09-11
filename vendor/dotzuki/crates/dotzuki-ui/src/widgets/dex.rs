//! Dex entry widget — full-screen data card with custom border tiles,
//! divider line, stats, and paginated description text.
//! Uses absolute tile positions matching the classic Game Boy Dex screen layout.

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, TilePos, Ui};

#[derive(Debug, Clone)]
pub struct DexEntry {
    /// Monster name (e.g. "LEAFKIT")
    pub name: String,
    /// Species classification (e.g. "SEED")
    pub species: String,
    /// Dex number (1–151)
    pub dex_num: u16,
    /// Feet component of height
    pub height_ft: u8,
    /// Inches component of height
    pub height_in: u8,
    /// Weight in tenths of pounds (150 = 15.0 lb)
    pub weight_tenths: u16,
    /// Description text, pre-split into lines (each ≤ 18 chars for display)
    pub description: Vec<String>,
    /// Current page index (0-based), 6 lines per page
    pub page: usize,
}

/// GB Dex border tile IDs ($63–$6F)
mod border_tiles {
    pub const TOP_LEFT: u8 = 0x63;
    pub const TOP_HORIZ: u8 = 0x64;
    pub const TOP_RIGHT: u8 = 0x65;
    pub const SIDE_LEFT: u8 = 0x66;
    pub const SIDE_RIGHT: u8 = 0x67;
    pub const DIV_LEFT: u8 = 0x68;
    pub const DIV_MID_A: u8 = 0x69;
    pub const DIV_MID_B: u8 = 0x6B;
    pub const DIV_RIGHT: u8 = 0x6A;
    pub const BOT_LEFT: u8 = 0x6C;
    pub const BOT_RIGHT: u8 = 0x6E;
    pub const BOT_HORIZ: u8 = 0x6F;
}

/// Divider line tile pattern at row 9 (col 0..19).
const DIVIDER_TILES: [(usize, u8); 20] = [
    (0, border_tiles::DIV_LEFT),
    (1, border_tiles::DIV_MID_A),
    (2, border_tiles::DIV_MID_B),
    (3, border_tiles::DIV_MID_A),
    (4, border_tiles::DIV_MID_B),
    (5, border_tiles::DIV_MID_A),
    (6, border_tiles::DIV_MID_B),
    (7, border_tiles::DIV_MID_A),
    (8, border_tiles::DIV_MID_B),
    (9, border_tiles::DIV_MID_B),
    (10, border_tiles::DIV_MID_B),
    (11, border_tiles::DIV_MID_B),
    (12, border_tiles::DIV_MID_A),
    (13, border_tiles::DIV_MID_B),
    (14, border_tiles::DIV_MID_A),
    (15, border_tiles::DIV_MID_B),
    (16, border_tiles::DIV_MID_A),
    (17, border_tiles::DIV_MID_B),
    (18, border_tiles::DIV_MID_A),
    (19, border_tiles::DIV_RIGHT),
];

/// Number of visible description lines per page.
const LINES_PER_PAGE: usize = 6;

pub fn draw_dex<P: Painter>(entry: &DexEntry, _configs: &[MenuConfig], painter: &mut P) {
    let mut ui = Ui::new(painter);

    // ── Clear screen ──────────────────────────────────────────────────
    ui.clear(Rgba::INK_WHITE);

    // ── Full-screen border (20×18 tiles) ──────────────────────────────
    draw_border(ui.painter());

    // ── Divider line at row 9 ─────────────────────────────────────────
    for &(col, tile_id) in &DIVIDER_TILES {
        ui.painter().draw_gb_tile(
            TilePos::new(col as u32, 9),
            tile_id,
            "\u{2500}",
            Rgba::INK_BLACK,
        );
    }

    // ── Content ───────────────────────────────────────────────────────
    let p = ui.painter();

    // Monster name at (9,2)
    p.draw_text(TilePos::new(9, 2), &entry.name, Rgba::INK_BLACK);

    // Species classification at (9,4)
    p.draw_text(TilePos::new(9, 4), &entry.species, Rgba::INK_BLACK);

    // HT label at (9,6)
    p.draw_text(TilePos::new(9, 6), "HT", Rgba::INK_BLACK);

    // Height value at (12,6) — format: "{ft}′{in:02}″"
    let height_str = format!("{},{:02}", entry.height_ft, entry.height_in);
    p.draw_text(TilePos::new(12, 6), &height_str, Rgba::INK_BLACK);

    // WT label at (9,7)
    p.draw_text(TilePos::new(9, 7), "WT", Rgba::INK_BLACK);

    // Weight value at (11,7) — format: "XX.X lb" (weight_tenths / 10)
    let weight_whole = entry.weight_tenths / 10;
    let weight_frac = entry.weight_tenths % 10;
    let weight_str = format!("{}.{} lb", weight_whole, weight_frac);
    p.draw_text(TilePos::new(11, 7), &weight_str, Rgba::INK_BLACK);

    // Dex number at (2,8) — "No.{:03}" (№ symbol ASCII fallback)
    let dex_str = format!("No.{:03}", entry.dex_num);
    p.draw_text(TilePos::new(2, 8), &dex_str, Rgba::INK_BLACK);

    // ── Description text (rows 11–16, 18 columns wide) ────────────────
    let total_pages = (entry.description.len() + LINES_PER_PAGE - 1).max(1) / LINES_PER_PAGE;
    let page = entry.page.min(total_pages.saturating_sub(1));
    let start = page * LINES_PER_PAGE;
    for i in 0..LINES_PER_PAGE {
        let line_idx = start + i;
        if line_idx < entry.description.len() {
            p.draw_text(
                TilePos::new(1, 11 + i as u32),
                &entry.description[line_idx],
                Rgba::INK_BLACK,
            );
        }
    }

    // ── Page-down arrow (if more pages) ───────────────────────────────
    if page + 1 < total_pages {
        p.draw_glyph(TilePos::new(18, 16), '\u{25BC}', Rgba::INK_BLACK);
    }
}

/// Draw the full-screen Dex border using GB tile indices.
fn draw_border<P: Painter>(painter: &mut P) {
    // Top row: $63, $64 repeated × 18, $65
    painter.draw_gb_tile(
        TilePos::new(0, 0),
        border_tiles::TOP_LEFT,
        "\u{250C}",
        Rgba::INK_BLACK,
    );
    for x in 1..19u32 {
        painter.draw_gb_tile(
            TilePos::new(x, 0),
            border_tiles::TOP_HORIZ,
            "\u{2500}",
            Rgba::INK_BLACK,
        );
    }
    painter.draw_gb_tile(
        TilePos::new(19, 0),
        border_tiles::TOP_RIGHT,
        "\u{2510}",
        Rgba::INK_BLACK,
    );

    // Side rows: $66 at col 0, $67 at col 19, for rows 1..17 (1–16 inclusive)
    for y in 1..17u32 {
        painter.draw_gb_tile(
            TilePos::new(0, y),
            border_tiles::SIDE_LEFT,
            "\u{2502}",
            Rgba::INK_BLACK,
        );
        painter.draw_gb_tile(
            TilePos::new(19, y),
            border_tiles::SIDE_RIGHT,
            "\u{2502}",
            Rgba::INK_BLACK,
        );
    }

    // Bottom row (row 17): $6C, $6F repeated × 18, $6E
    painter.draw_gb_tile(
        TilePos::new(0, 17),
        border_tiles::BOT_LEFT,
        "\u{2514}",
        Rgba::INK_BLACK,
    );
    for x in 1..19u32 {
        painter.draw_gb_tile(
            TilePos::new(x, 17),
            border_tiles::BOT_HORIZ,
            "\u{2500}",
            Rgba::INK_BLACK,
        );
    }
    painter.draw_gb_tile(
        TilePos::new(19, 17),
        border_tiles::BOT_RIGHT,
        "\u{2518}",
        Rgba::INK_BLACK,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render::TileRect;

    #[derive(Debug, Default)]
    struct RecordingPainter {
        clears: Vec<Rgba>,
        text_boxes: Vec<(TileRect, Rgba)>,
        texts: Vec<(TilePos, String, Rgba)>,
        pixel_rects: Vec<(u32, u32, u32, u32, Rgba)>,
        glyphs: Vec<(TilePos, char, Rgba)>,
        gb_tiles: Vec<(TilePos, u8, String, Rgba)>,
    }
    impl Painter for RecordingPainter {
        fn clear(&mut self, color: Rgba) {
            self.clears.push(color);
        }
        fn draw_text_box(&mut self, rect: TileRect, color: Rgba) {
            self.text_boxes.push((rect, color));
        }
        fn draw_text(&mut self, pos: TilePos, text: &str, color: Rgba) {
            self.texts.push((pos, text.to_string(), color));
        }
        fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba) {
            self.glyphs.push((pos, glyph, color));
        }
        fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, color: Rgba) {
            self.pixel_rects.push((px, py, pw, ph, color));
        }
        fn draw_gb_tile(&mut self, pos: TilePos, tile_id: u8, fallback: &str, color: Rgba) {
            self.gb_tiles
                .push((pos, tile_id, fallback.to_string(), color));
        }
    }

    fn test_entry() -> DexEntry {
        DexEntry {
            name: "SPARKIT".into(),
            species: "MOUSE".into(),
            dex_num: 25,
            height_ft: 1,
            height_in: 4,
            weight_tenths: 132, // 13.2 lb
            description: vec![
                "When several of".into(),
                "these monsters".into(),
                "gather, their".into(),
                "electricity could".into(),
                "build and cause".into(),
                "lightning storms.".into(),
                "Page 2 line 1.".into(),
                "Page 2 line 2.".into(),
            ],
            page: 0,
        }
    }

    // ── Border tests ──────────────────────────────────────────────────

    #[test]
    fn draws_full_screen_border() {
        let mut painter = RecordingPainter::default();
        draw_dex(&test_entry(), &[], &mut painter);
        // Should draw border tiles, not a text_box
        assert_eq!(painter.text_boxes.len(), 0);
        assert!(!painter.gb_tiles.is_empty(), "Should draw GB border tiles");
        // Top row
        assert!(painter
            .gb_tiles
            .iter()
            .any(|(p, id, _, _)| p.tx == 0 && p.ty == 0 && *id == 0x63));
        assert!(painter
            .gb_tiles
            .iter()
            .any(|(p, id, _, _)| p.tx == 19 && p.ty == 0 && *id == 0x65));
        // Bottom row
        assert!(painter
            .gb_tiles
            .iter()
            .any(|(p, id, _, _)| p.tx == 0 && p.ty == 17 && *id == 0x6C));
        assert!(painter
            .gb_tiles
            .iter()
            .any(|(p, id, _, _)| p.tx == 19 && p.ty == 17 && *id == 0x6E));
    }

    #[test]
    fn draws_divider_line() {
        let mut painter = RecordingPainter::default();
        draw_dex(&test_entry(), &[], &mut painter);
        // Divider at row 9
        let divider_tiles: Vec<_> = painter
            .gb_tiles
            .iter()
            .filter(|(p, _, _, _)| p.ty == 9)
            .collect();
        assert!(
            !divider_tiles.is_empty(),
            "Should draw divider tiles at row 9"
        );
        // Check first and last tile IDs
        assert!(divider_tiles
            .iter()
            .any(|(p, id, _, _)| p.tx == 0 && *id == 0x68));
        assert!(divider_tiles
            .iter()
            .any(|(p, id, _, _)| p.tx == 19 && *id == 0x6A));
    }

    // ── Content tests ─────────────────────────────────────────────────

    #[test]
    fn draws_name() {
        let mut painter = RecordingPainter::default();
        draw_dex(&test_entry(), &[], &mut painter);
        assert!(painter
            .texts
            .iter()
            .any(|(p, t, _)| p.tx == 9 && p.ty == 2 && t == "SPARKIT"));
    }

    #[test]
    fn draws_species() {
        let mut painter = RecordingPainter::default();
        draw_dex(&test_entry(), &[], &mut painter);
        assert!(painter
            .texts
            .iter()
            .any(|(p, t, _)| p.tx == 9 && p.ty == 4 && t == "MOUSE"));
    }

    #[test]
    fn draws_height_with_format() {
        let mut painter = RecordingPainter::default();
        draw_dex(&test_entry(), &[], &mut painter);
        // HT label
        assert!(painter
            .texts
            .iter()
            .any(|(p, t, _)| p.tx == 9 && p.ty == 6 && t == "HT"));
        // Height value at (12,6): "1,04"
        assert!(painter
            .texts
            .iter()
            .any(|(p, t, _)| p.tx == 12 && p.ty == 6 && t == "1,04"));
    }

    #[test]
    fn draws_weight_with_decimal() {
        let mut painter = RecordingPainter::default();
        draw_dex(&test_entry(), &[], &mut painter);
        // WT label
        assert!(painter
            .texts
            .iter()
            .any(|(p, t, _)| p.tx == 9 && p.ty == 7 && t == "WT"));
        // Weight at (11,7): "13.2 lb"
        assert!(painter
            .texts
            .iter()
            .any(|(p, t, _)| p.tx == 11 && p.ty == 7 && t == "13.2 lb"));
    }

    #[test]
    fn draws_dex_number() {
        let mut painter = RecordingPainter::default();
        draw_dex(&test_entry(), &[], &mut painter);
        // At (2,8): "No.025"
        assert!(painter
            .texts
            .iter()
            .any(|(p, t, _)| p.tx == 2 && p.ty == 8 && t == "No.025"));
    }

    #[test]
    fn draws_description_page_0() {
        let mut painter = RecordingPainter::default();
        draw_dex(&test_entry(), &[], &mut painter);
        // First 6 lines at rows 11..16
        assert!(painter
            .texts
            .iter()
            .any(|(p, t, _)| p.tx == 1 && p.ty == 11 && t.contains("When several")));
        assert!(painter
            .texts
            .iter()
            .any(|(p, t, _)| p.tx == 1 && p.ty == 16 && t.contains("lightning storms")));
    }

    #[test]
    fn draws_description_page_1() {
        let mut entry = test_entry();
        entry.page = 1;
        let mut painter = RecordingPainter::default();
        draw_dex(&entry, &[], &mut painter);
        // Page 2 lines at rows 11..12
        assert!(painter
            .texts
            .iter()
            .any(|(p, t, _)| p.tx == 1 && p.ty == 11 && t.contains("Page 2 line 1")));
    }

    #[test]
    fn draws_page_arrow_when_more_pages() {
        let mut painter = RecordingPainter::default();
        draw_dex(&test_entry(), &[], &mut painter);
        // Arrow at (18,16) when more pages exist
        assert!(painter
            .glyphs
            .iter()
            .any(|(p, g, _)| p.tx == 18 && p.ty == 16 && *g == '\u{25BC}'));
    }

    #[test]
    fn no_page_arrow_on_last_page() {
        let mut entry = test_entry();
        entry.page = 1; // last page
        let mut painter = RecordingPainter::default();
        draw_dex(&entry, &[], &mut painter);
        // No arrow on last page
        assert!(!painter.glyphs.iter().any(|(_, g, _)| *g == '\u{25BC}'));
    }

    #[test]
    fn clears_screen() {
        let mut painter = RecordingPainter::default();
        draw_dex(&test_entry(), &[], &mut painter);
        assert_eq!(painter.clears.len(), 1);
        assert_eq!(painter.clears[0], Rgba::INK_WHITE);
    }
}
