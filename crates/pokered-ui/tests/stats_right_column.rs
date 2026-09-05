//! Regression: the stats page-1 right column (types / ID № / OT) renders in
//! both languages, and the zh values share one flush right edge.
//!
//! Locks three behaviors at once:
//!
//! 1. v2 `container` elements must declare `layout` + `clip` — `ElementParams`
//!    is an untagged enum and a group without those fields deserializes as
//!    `Border`, silently dropping every child.
//! 2. The zh variant places its values beside (not below) their labels — CJK
//!    glyphs are taller than one tile row — and those values are NOT drawn by
//!    the v2 tile path: it advances a full 8 px tile per character, so
//!    tile-snapped `align = "right"` leaves CJK names and ASCII digits with
//!    ragged right edges. `menus::stats` draws them through the painter's
//!    pixel path instead.
//! 3. `draw_values_flush_right` anchors each value's ink right edge exactly
//!    on its target px.

use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::get_screen_v2_json;
use pokered_ui::menus::stats::{draw_values_flush_right, ZH_RIGHT_COL_RIGHT_PX, ZH_STATS_RIGHT_PX};
use pokered_ui::v2::{self, DataContext};
use pokered_ui::{Painter, Rgba, TilePos, TileRect};

#[derive(Default)]
struct Recorder {
    glyphs: Vec<(TilePos, char)>,
}

impl Painter for Recorder {
    fn clear(&mut self, _color: Rgba) {}
    fn draw_text_box(&mut self, _rect: TileRect, _color: Rgba) {}
    fn draw_text(&mut self, pos: TilePos, text: &str, _color: Rgba) {
        let (x, y) = (pos.tx, pos.ty);
        for (i, ch) in text.chars().enumerate() {
            self.glyphs.push((TilePos::new(x + i as u32, y), ch));
        }
    }
    fn draw_glyph(&mut self, pos: TilePos, glyph: char, _color: Rgba) {
        self.glyphs.push((pos, glyph));
    }
    fn draw_pixel_rect(&mut self, _px: u32, _py: u32, _pw: u32, _ph: u32, _color: Rgba) {}
    fn draw_gb_tile(&mut self, _pos: TilePos, _tile_id: u8, _fallback: &str, _color: Rgba) {}
}

/// Stands in for the framebuffer painter's pixel path: proportional metrics
/// (5 px ASCII / 10 px CJK), so an anchor computed from tile counts (8 px per
/// char) fails the flush-edge assertion.
#[derive(Default)]
struct PxRecorder {
    placed: Vec<(u32, u32, String)>,
}

impl Painter for PxRecorder {
    fn clear(&mut self, _color: Rgba) {}
    fn draw_text_box(&mut self, _rect: TileRect, _color: Rgba) {}
    fn draw_text(&mut self, _pos: TilePos, _text: &str, _color: Rgba) {}
    fn draw_glyph(&mut self, _pos: TilePos, _glyph: char, _color: Rgba) {}
    fn draw_pixel_rect(&mut self, _px: u32, _py: u32, _pw: u32, _ph: u32, _color: Rgba) {}
    fn draw_gb_tile(&mut self, _pos: TilePos, _tile_id: u8, _fallback: &str, _color: Rgba) {}
    fn measure_text_px(&self, text: &str) -> u32 {
        text.chars().map(|c| if c.is_ascii() { 5 } else { 10 }).sum()
    }
    fn draw_text_px(&mut self, px: u32, py: u32, text: &str, _color: Rgba) {
        self.placed.push((px, py, text.to_string()));
    }
}

fn probe_ctx(lang: Lang) -> DataContext {
    let mut ctx = DataContext::new();
    ctx.set("name", "RHYDON");
    ctx.set("level", 62i64);
    ctx.set("hp", 214i64);
    ctx.set("max_hp", 214i64);
    ctx.set("status", "OK");
    ctx.set("dex_num", 112i64);
    ctx.set("attack", 177i64);
    ctx.set("defense", 166i64);
    ctx.set("speed", 63i64);
    ctx.set("special", 70i64);
    ctx.set("type1", "地面");
    ctx.set("type2", "");
    ctx.set("id", 4362i64);
    ctx.set("ot", "RED");
    ctx.set("is_zh", lang == Lang::Zh);
    ctx.set("is_en", lang != Lang::Zh);
    ctx.set("__lang", v2::lang_code(lang));
    ctx
}

fn right_column_glyphs(lang: Lang) -> Vec<(TilePos, char)> {
    let json = get_screen_v2_json("stats").expect("stats v2 json registered");
    let layout = v2::parse_screen(json).expect("stats v2 layout parses");
    let mut rec = Recorder::default();
    v2::render_screen(&layout, &probe_ctx(lang), &mut rec);
    rec.glyphs
        .into_iter()
        .filter(|(p, _)| p.tx >= 10 && p.ty >= 8)
        .collect()
}

#[test]
fn zh_right_column_renders_labels_via_tile_path_only() {
    let glyphs = right_column_glyphs(Lang::Zh);
    let text: String = glyphs.iter().map(|(_, c)| *c).collect();
    // zh labels come from the layout's @t values.
    assert!(text.contains('属'), "属性1/ label missing: {text}");
    assert!(text.contains('编'), "编号/ label missing: {text}");
    assert!(text.contains('主'), "主人/ label missing: {text}");
    // Values left the tile path — they are pixel-drawn by menus::stats, and a
    // tile-path value here would double-draw over them.
    assert!(!text.contains('地'), "type value must not use the tile path: {text}");
    assert!(!text.contains('0'), "id digits must not use the tile path: {text}");
}

#[test]
fn en_right_column_renders_labels_and_values() {
    let glyphs = right_column_glyphs(Lang::En);
    let text: String = glyphs.iter().map(|(_, c)| *c).collect();
    assert!(text.contains("TYPE1/"), "TYPE1/ label missing: {text}");
    assert!(text.contains("04362"), "id value missing: {text}");
    assert!(text.contains("OT/"), "OT/ label missing: {text}");
    assert!(text.contains("RED"), "ot value missing: {text}");
}

#[test]
fn zh_values_anchor_flush_on_their_right_edges() {
    let rows = vec![
        (9u32, ZH_STATS_RIGHT_PX, "177".to_string()),
        (11, ZH_STATS_RIGHT_PX, "63".to_string()),
        (9, ZH_RIGHT_COL_RIGHT_PX, "地面".to_string()),
        (13, ZH_RIGHT_COL_RIGHT_PX, "04362".to_string()),
        (15, ZH_RIGHT_COL_RIGHT_PX, "RED".to_string()),
    ];
    let mut rec = PxRecorder::default();
    draw_values_flush_right(&mut rec, &rows);
    assert_eq!(rec.placed.len(), rows.len(), "every row must be placed");
    for ((ty, right_px, text), (px, py, placed)) in rows.iter().zip(&rec.placed) {
        assert_eq!(placed, text);
        assert_eq!(*py, ty * 8, "value {placed:?} on tile row {ty}");
        assert_eq!(
            px + rec.measure_text_px(placed),
            *right_px,
            "value {placed:?} ink must end at px {right_px}"
        );
    }
}
