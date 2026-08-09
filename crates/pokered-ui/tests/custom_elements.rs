//! Integration tests for pokered's `custom:*` layout elements.
//!
//! Covers the full bridge: compiled `.gui` JSON → `v2::parse_screen`
//! (load-time schema validation) → `v2::render_screen` (registry dispatch).

use dotzuki_renderer::layout_engine::deserialize::parse_layout;
use dotzuki_renderer::layout_engine::types::LayoutElement;
use pokered_ui::custom_elements::element_registry;
use pokered_ui::v2::{self, DataContext};
use pokered_ui::{Painter, Rgba, TilePos, TileRect};

#[derive(Debug, Default)]
struct PxRec {
    rects: Vec<(u32, u32, u32, u32, Rgba)>,
}

impl Painter for PxRec {
    fn clear(&mut self, _c: Rgba) {}
    fn draw_text_box(&mut self, _r: TileRect, _c: Rgba) {}
    fn draw_text(&mut self, _p: TilePos, _t: &str, _c: Rgba) {}
    fn draw_glyph(&mut self, _p: TilePos, _g: char, _c: Rgba) {}
    fn draw_pixel_rect(&mut self, x: u32, y: u32, w: u32, h: u32, c: Rgba) {
        self.rects.push((x, y, w, h, c));
    }
    fn draw_gb_tile(&mut self, _p: TilePos, _t: u8, _f: &str, _c: Rgba) {}
}

fn hp_bar_screen_json() -> &'static str {
    r##"{
        "schema_version": 2,
        "screen": "test",
        "theme": { "bg_color": "#FFFFFF", "default_font": "default" },
        "elements": [
            { "type": "custom:hp_bar",
              "rect": { "tx": 13, "ty": 3, "tw": 6, "th": 1 },
              "current": "{hp}", "max": "{max_hp}" }
        ]
    }"##
}

/// Render through the full v2 bridge and return the recorded pixel rects.
fn render_hp_bar(hp: i64, max_hp: i64) -> Vec<(u32, u32, u32, u32, Rgba)> {
    let layout = v2::parse_screen(hp_bar_screen_json()).expect("layout should validate");
    let mut ctx = DataContext::new();
    ctx.set("hp", hp);
    ctx.set("max_hp", max_hp);
    let mut painter = PxRec::default();
    v2::render_screen(&layout, &ctx, &mut painter);
    painter.rects
}

#[test]
fn hp_bar_full_health_renders_full_fill() {
    let rects = render_hp_bar(20, 20);
    // outline, inner background, fill
    assert_eq!(rects.len(), 3, "rects: {rects:?}");
    assert_eq!(rects[0], (13 * 8, 3 * 8 + 2, 6 * 8, 4, Rgba::INK_BLACK));
    assert_eq!(rects[1].4, Rgba::INK_WHITE);
    let fill = rects[2];
    assert_eq!(fill.2, 6 * 8 - 2, "full bar fills the inner width");
    assert_eq!(fill.4, Rgba::rgb(0x20, 0x20, 0x20), "HP_FULL shade");
}

#[test]
fn hp_bar_low_health_renders_critical_shade() {
    let rects = render_hp_bar(1, 20); // 5% → critical
    let fill = rects.last().unwrap();
    assert_eq!(fill.4, Rgba::rgb(0x40, 0x40, 0x40), "HP_CRITICAL shade");
}

#[test]
fn hp_bar_zero_max_draws_no_fill() {
    let rects = render_hp_bar(0, 0);
    assert_eq!(rects.len(), 2, "outline + background only: {rects:?}");
}

#[test]
fn parse_screen_rejects_schema_violations() {
    // `max` is required by HpBarElement's schema — parse_screen must refuse
    // the layout rather than render a broken element every frame.
    let json = r##"{
        "schema_version": 2,
        "screen": "test",
        "theme": { "bg_color": "#FFFFFF", "default_font": "default" },
        "elements": [
            { "type": "custom:hp_bar",
              "rect": { "tx": 0, "ty": 0, "tw": 6, "th": 1 },
              "current": "{hp}" }
        ]
    }"##;
    assert!(v2::parse_screen(json).is_none());
}

// ── drift guard ───────────────────────────────────────────────────────────

fn find_custom(el: &LayoutElement, found: &mut Vec<String>) {
    if el.element_type.starts_with("custom:") {
        found.push(el.element_type.clone());
    }
    use dotzuki_renderer::layout_engine::types::ElementParams;
    match &el.params {
        ElementParams::Group(g) => g.children.iter().for_each(|c| find_custom(c, found)),
        ElementParams::Border(b) => b.children.iter().for_each(|c| find_custom(c, found)),
        _ => {}
    }
}

/// The compiled stats screen (built from `stats.gui` + the `components.gui`
/// prelude by pokered-data's build.rs) must contain the hp_bar as a custom
/// element AND satisfy the registered schema. Fails when the `.gui`
/// declaration and `HpBarElement::schema()` drift apart.
#[test]
fn compiled_stats_layout_uses_custom_hp_bar_and_validates() {
    let json = pokered_data::ui_layout::schema::get_screen_v2_json("stats")
        .expect("stats v2 layout registered");
    let layout = parse_layout(json).expect("stats layout parses");

    let mut custom = Vec::new();
    for el in &layout.elements {
        find_custom(el, &mut custom);
    }
    assert!(
        custom.iter().any(|t| t == "custom:hp_bar"),
        "stats layout should contain custom:hp_bar, found: {custom:?}"
    );

    element_registry()
        .validate_layout(&layout)
        .unwrap_or_else(|v| panic!("stats layout violates registered schemas: {v:?}"));
}
