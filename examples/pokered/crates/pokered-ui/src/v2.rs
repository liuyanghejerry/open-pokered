//! Bridge to the v2 layout engine (`jrpg-renderer`) for menus migrated off the
//! v1 hardcoded-layout path.
//!
//! A migrated menu fetches its compiled element-format JSON via
//! [`pokered_data::ui_layout::schema::get_screen_v2_json`], binds runtime state
//! into a [`DataContext`], and renders through the shared [`Painter`] — the
//! same painter the v1 menus use, so app and TUI backends work unchanged.

use std::collections::HashMap;

use jrpg_engine::render::painter::Painter;
use jrpg_renderer::layout_engine::deserialize::parse_layout;
use jrpg_renderer::layout_engine::renderer::{render_layout, render_layout_no_clear};
use jrpg_renderer::layout_engine::types::{RenderContext, ScreenLayout};
use pokered_core::game_state::Lang;

pub use jrpg_renderer::layout_engine::types::{DataContext, DataValue};

/// Locale code for the reserved `__lang` data key the engine reads to pick a
/// `@t("en", "中文")` variant.
pub fn lang_code(lang: Lang) -> &'static str {
    match lang {
        Lang::Zh => "zh",
        _ => "en",
    }
}

/// Parse a compiled schema-v2 screen JSON (from `get_screen_v2_json`) into a
/// mutable layout, validating `custom:*` elements against the registered
/// schemas. Returns `None` on parse failure or schema violation (logged) so
/// callers can skip the frame rather than panic.
pub fn parse_screen(json: &str) -> Option<ScreenLayout> {
    let layout = parse_layout(json).ok()?;
    if let Err(violations) = crate::custom_elements::element_registry().validate_layout(&layout) {
        for v in &violations {
            log::error!("layout '{}' schema violation — {}", layout.screen, v);
        }
        return None;
    }
    Some(layout)
}

/// Override the height of the screen's first `border` (panel) element.
///
/// The v2 engine has no flex auto-sizing yet, so menus whose box grows with
/// item count (e.g. the title menu: 2 vs 3 entries) set the panel height from
/// the live state to match the v1 rendering. No-op if there is no border.
pub fn set_panel_height(layout: &mut ScreenLayout, th: u32) {
    for el in &mut layout.elements {
        if el.element_type == "border" {
            el.rect.th = Some(th);
            break;
        }
    }
}

/// Render a parsed v2 layout against `painter`, binding values from `ctx`.
///
/// The engine clears to the theme background first (matching the v1 menus'
/// `ui.clear`). A render error — which should not occur for build-validated
/// `.gui` output — is swallowed so a stray frame renders empty instead of
/// panicking the game loop.
pub fn render_screen(layout: &ScreenLayout, ctx: &DataContext, painter: &mut dyn Painter) {
    let fonts: HashMap<String, ()> = HashMap::new();
    let tilesets: HashMap<String, ()> = HashMap::new();
    let render_ctx = RenderContext::new(&layout.screen, &layout.theme, &fonts, &tilesets);
    let registry = crate::custom_elements::element_registry();
    let _ = render_layout(layout, ctx, &render_ctx, registry, painter);
}

/// Render a parsed v2 layout as an OVERLAY (no clear) on top of whatever is
/// already in the framebuffer — for menus drawn over a live scene, e.g. the
/// battle action menu over the battle sprites.
pub fn render_screen_overlay(layout: &ScreenLayout, ctx: &DataContext, painter: &mut dyn Painter) {
    let fonts: HashMap<String, ()> = HashMap::new();
    let tilesets: HashMap<String, ()> = HashMap::new();
    let render_ctx = RenderContext::new(&layout.screen, &layout.theme, &fonts, &tilesets);
    let registry = crate::custom_elements::element_registry();
    let _ = render_layout_no_clear(layout, ctx, &render_ctx, registry, painter);
}
