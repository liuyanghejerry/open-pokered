//! Render smoke test: compile representative pokered `.gui` layouts, render
//! them through the layout engine with mock data, and assert that text glyphs
//! are actually drawn. This is the end-to-end guard against the "blank text in
//! the LayoutEditor preview" regression — the round-trip test proves elements
//! deserialize correctly, this proves they then paint.

use crate::alloc_prelude::*;
use std::collections::{BTreeMap, HashMap};

use dotzuki_engine::render::painter::Painter;
use dotzuki_engine::render::{Rgba, TilePos, TileRect};

use dotzuki_renderer::layout_engine::deserialize::parse_layout;
use dotzuki_renderer::layout_engine::registry::ElementRegistry;
use dotzuki_renderer::layout_engine::renderer::render_layout;
use dotzuki_renderer::layout_engine::types::{DataContext, DataValue, RenderContext, ScreenLayout};

/// Records every glyph drawn so tests can reconstruct on-screen text.
#[derive(Default)]
struct RecordingPainter {
    glyphs: Vec<(u32, u32, char)>, // (ty, tx, glyph)
    text_boxes: usize,
    pixel_colors: Vec<Rgba>, // every draw_pixel_rect colour (image blits + placeholders)
}

impl RecordingPainter {
    /// Reconstruct all drawn text as one string, rows top-to-bottom and each
    /// row left-to-right (single spaces between non-adjacent columns collapsed
    /// away — we only care about substring presence).
    fn drawn_text(&self) -> String {
        let mut rows: BTreeMap<u32, BTreeMap<u32, char>> = BTreeMap::new();
        for &(ty, tx, ch) in &self.glyphs {
            rows.entry(ty).or_default().insert(tx, ch);
        }
        rows.values()
            .map(|cols| cols.values().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Painter for RecordingPainter {
    fn clear(&mut self, _color: Rgba) {}
    fn draw_text_box(&mut self, _rect: TileRect, _color: Rgba) {
        self.text_boxes += 1;
    }
    fn draw_text(&mut self, pos: TilePos, text: &str, color: Rgba) {
        for (i, ch) in text.chars().enumerate() {
            self.draw_glyph(TilePos::new(pos.tx + i as u32, pos.ty), ch, color);
        }
    }
    fn draw_glyph(&mut self, pos: TilePos, glyph: char, _color: Rgba) {
        self.glyphs.push((pos.ty, pos.tx, glyph));
    }
    fn draw_pixel_rect(&mut self, _px: u32, _py: u32, _pw: u32, _ph: u32, color: Rgba) {
        self.pixel_colors.push(color);
    }
    fn draw_gb_tile(&mut self, _pos: TilePos, _tile_id: u8, _fallback: &str, _color: Rgba) {}
}

fn compile_gui(src: &str, path: &str) -> ScreenLayout {
    let tokens = dotzuki_engine_dsl::lexer::Lexer::new(src, path)
        .tokenize()
        .unwrap_or_else(|e| panic!("lex {path}: {e:?}"));
    let (doc, errs) = dotzuki_engine_dsl::parser::Parser::new(tokens, src).parse();
    assert!(errs.is_empty(), "parse {path}: {errs:?}");
    let json = match doc {
        Some(dotzuki_engine_dsl::ast::Document::Screen(screen)) => {
            dotzuki_engine_dsl::codegen::json_ui::compile_screen(&screen)
                .unwrap_or_else(|e| panic!("codegen {path}: {e}"))
        }
        _ => panic!("{path}: not a screen"),
    };
    parse_layout(&json).unwrap_or_else(|e| panic!("parse_layout {path}: {e:?}\n{json}"))
}

fn render(layout: &ScreenLayout, ctx: &DataContext) -> RecordingPainter {
    let fonts: HashMap<String, ()> = HashMap::new();
    let tilesets: HashMap<String, ()> = HashMap::new();
    let render_ctx = RenderContext::new(&layout.screen, &layout.theme, &fonts, &tilesets);
    let registry = ElementRegistry::new();
    let mut painter = RecordingPainter::default();
    render_layout(layout, ctx, &render_ctx, &registry, &mut painter)
        .expect("render should not error");
    painter
}

/// Static labels and resolved template variables both paint as glyphs
/// (regression: `text(...)` compiled to `content` → blank).
#[test]
fn save_menu_draws_label_and_resolved_text() {
    let src = r#"screen Save {
  panel { rect = {tx: 4, ty: 0, tw: 15, th: 10} style = "default" }
  text("PLAYER")        { rect = {tx: 5,  ty: 2, tw: 7, th: 1} }
  text("{player_name}") { rect = {tx: 13, ty: 2, tw: 5, th: 1} }
  text("BADGES")        { rect = {tx: 5,  ty: 4, tw: 7, th: 1} }
}"#;
    let layout = compile_gui(src, "save.gui");

    let mut ctx = DataContext::new();
    ctx.set("player_name", "ASH");

    let painter = render(&layout, &ctx);
    let text = painter.drawn_text();

    assert!(painter.text_boxes >= 1, "panel box should draw");
    assert!(
        text.contains("PLAYER"),
        "static label missing; drew: {text:?}"
    );
    assert!(
        text.contains("BADGES"),
        "static label missing; drew: {text:?}"
    );
    assert!(
        text.contains("ASH"),
        "resolved {{player_name}} missing; drew: {text:?}"
    );
}

/// An `image(...)` element survives the full `.gui` pipeline (the DSL emits `src`,
/// which `parse_layout` must accept via the `source` alias — else `compile_gui`
/// panics) and blits the registered image rather than the placeholder.
#[test]
fn image_element_compiles_and_blits_registered_image() {
    use dotzuki_renderer::layout_engine::types::{ImageData, ImageRegistry};

    let src = r#"screen Pic {
  image("hero") { rect = {tx: 0, ty: 0, tw: 1, th: 1} }
}"#;
    let layout = compile_gui(src, "pic.gui"); // panics if the image element fails to parse_layout

    let red = Rgba::new(255, 0, 0, 255);
    let mut images = ImageRegistry::new();
    images.insert("hero".to_string(), ImageData::new(1, 1, vec![red]));

    let ctx = DataContext::new();
    let fonts: HashMap<String, ()> = HashMap::new();
    let tilesets: HashMap<String, ()> = HashMap::new();
    let render_ctx =
        RenderContext::new(&layout.screen, &layout.theme, &fonts, &tilesets).with_images(&images);
    let registry = ElementRegistry::new();
    let mut painter = RecordingPainter::default();
    render_layout(&layout, &ctx, &render_ctx, &registry, &mut painter).expect("render");

    assert!(
        !painter.pixel_colors.is_empty(),
        "image element should blit pixels"
    );
    assert!(
        painter.pixel_colors.iter().all(|c| *c == red),
        "only the registered image colour should be drawn (no placeholder gray); got {:?}",
        painter.pixel_colors
    );
}

/// With no registry entry for the key, the same element falls back to the striped
/// placeholder (so the LayoutEditor still shows "image goes here").
#[test]
fn image_element_without_registry_draws_placeholder() {
    let src = r#"screen Pic {
  image("missing") { rect = {tx: 0, ty: 0, tw: 1, th: 1} }
}"#;
    let layout = compile_gui(src, "pic.gui");
    let ctx = DataContext::new();
    let painter = render(&layout, &ctx); // empty image registry
    assert!(
        !painter.pixel_colors.is_empty(),
        "placeholder should draw at least one pixel rect"
    );
}

/// A `list` renders its data-bound items as text (regression: `list` compiled
/// to `type:"flex_list"` with a leaked `source` field → blank menu).
#[test]
fn start_menu_list_draws_items() {
    let src = r#"screen Start {
  panel { rect = {tx: 10, ty: 0, tw: 10, th: 15} style = "default" }
  list {
    rect = {tx: 11, ty: 1, tw: 8, th: 13}
    source = "{items}"
    item_template = {height: 1, gap: 1}
    cursor = {tile: 223, position: "left"}
    max_visible = 7
  }
}"#;
    let layout = compile_gui(src, "start.gui");

    let mut ctx = DataContext::new();
    ctx.set(
        "items",
        vec![
            DataValue::from("MONSTER"),
            DataValue::from("ITEM"),
            DataValue::from("SAVE"),
        ],
    );

    let text = render(&layout, &ctx).drawn_text();
    assert!(
        text.contains("MONSTER"),
        "list item missing; drew: {text:?}"
    );
    assert!(text.contains("SAVE"), "list item missing; drew: {text:?}");
}

/// `@t("en", "中文")` labels render the variant for the active language:
/// `DataContext.__lang` switches the compiled `{ "en": …, "zh": … }` value.
/// Drives the full pipeline: .gui → JSON {en,zh} → parse_layout → render.
#[test]
fn at_t_labels_render_active_language() {
    let src = r#"screen Options {
  text(@t("TEXT SPEED", "文字速度")) { rect = {tx: 1, ty: 1, tw: 16, th: 1} }
  text(@t("CANCEL", "取消"))         { rect = {tx: 2, ty: 4, tw: 8,  th: 1} }
}"#;
    let layout = compile_gui(src, "options.gui");

    // Default (no __lang) → English variant.
    let en = render(&layout, &DataContext::new()).drawn_text();
    assert!(en.contains("TEXT SPEED"), "EN label missing; drew: {en:?}");
    assert!(en.contains("CANCEL"), "EN label missing; drew: {en:?}");
    assert!(
        !en.contains("文字速度"),
        "EN render must not show zh; drew: {en:?}"
    );

    // __lang = "zh" → Chinese variant.
    let mut ctx_zh = DataContext::new();
    ctx_zh.set("__lang", "zh");
    let zh = render(&layout, &ctx_zh).drawn_text();
    assert!(zh.contains("文字速度"), "ZH label missing; drew: {zh:?}");
    assert!(zh.contains("取消"), "ZH label missing; drew: {zh:?}");
    assert!(
        !zh.contains("TEXT SPEED"),
        "ZH render must not show en; drew: {zh:?}"
    );
}

/// The `cursor` element computes its position from base (rect) + col/row grid
/// offsets bound from the data context — the basis for the battle menu's 2×2
/// FIGHT/PKMN/ITEM/RUN grid. Drives the full pipeline: .gui → JSON → render.
#[test]
fn cursor_element_grid_follows_bindings() {
    let src = r#"screen Grid {
  text("FIGHT") { rect = {tx: 10, ty: 14, tw: 5, th: 1} }
  text("RUN")   { rect = {tx: 16, ty: 16, tw: 3, th: 1} }
  cursor {
    rect = {tx: 9, ty: 14, tw: 1, th: 1}
    col_step = 6
    row_step = 2
    col = "{c}"
    row = "{r}"
    glyph = "▶"
  }
}"#;
    let layout = compile_gui(src, "grid.gui");
    let triangle = '\u{25B6}';

    // col=0,row=0 → base (9,14)
    let mut ctx = DataContext::new();
    ctx.set("c", 0i64);
    ctx.set("r", 0i64);
    let p = render(&layout, &ctx);
    assert!(
        p.glyphs
            .iter()
            .any(|&(ty, tx, ch)| tx == 9 && ty == 14 && ch == triangle),
        "cursor should be at (9,14); glyphs: {:?}",
        p.glyphs
    );

    // col=1,row=1 → (9+6, 14+2) = (15,16)
    let mut ctx = DataContext::new();
    ctx.set("c", 1i64);
    ctx.set("r", 1i64);
    let p = render(&layout, &ctx);
    assert!(
        p.glyphs
            .iter()
            .any(|&(ty, tx, ch)| tx == 15 && ty == 16 && ch == triangle),
        "cursor should be at (15,16); glyphs: {:?}",
        p.glyphs
    );
    assert!(p.drawn_text().contains("FIGHT"));
}

/// A panel with nested children paints both the box and the children
/// (regression: `panel { text {...} }` → `border` with `children` rejected by
/// the deserializer → entire box blank).
#[test]
fn dialog_panel_draws_nested_text() {
    let src = r#"screen Dialog {
  panel {
    rect = {tx: 0, ty: 12, tw: 20, th: 6}
    style = "default"
    text("{text}") {
      rect = {tx: 1, ty: 13, tw: 18, th: 4}
      wrap = "word"
      line_spacing = 1
    }
  }
}"#;
    let layout = compile_gui(src, "dialog.gui");

    let mut ctx = DataContext::new();
    ctx.set("text", "HELLO THERE");

    let painter = render(&layout, &ctx);
    let text = painter.drawn_text();

    assert!(painter.text_boxes >= 1, "dialog box should draw");
    assert!(
        text.contains("HELLO"),
        "nested panel text missing; drew: {text:?}"
    );
}
