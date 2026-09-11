use crate::layout_engine::elements::group::{Group, GroupLayout};
use crate::layout_engine::registry::ElementRegistry;
use crate::layout_engine::types::{
    Coord, DataContext, ElementParams, LayoutElement, RenderContext, RenderError, ScreenLayout,
};
use dotzuki_engine::render::painter::Painter;
use dotzuki_engine::render::{Rgba, TileRect};

/// Render a screen, first clearing to the theme background. Use for
/// full-screen menus (main, options, stats, …).
pub fn render_layout(
    layout: &ScreenLayout,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    registry: &ElementRegistry,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let bg = parse_bg_color(&layout.theme.bg_color);
    painter.clear(bg);
    render_elements(layout, ctx, render_ctx, registry, painter)
}

/// Render a screen WITHOUT clearing — for overlays drawn on top of an existing
/// scene (e.g. the battle action menu over the battle sprites).
pub fn render_layout_no_clear(
    layout: &ScreenLayout,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    registry: &ElementRegistry,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    render_elements(layout, ctx, render_ctx, registry, painter)
}

fn render_elements(
    layout: &ScreenLayout,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    registry: &ElementRegistry,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let mut elements: Vec<&LayoutElement> = layout.elements.iter().collect();
    elements.sort_by_key(|e| e.z_index);

    for element in &elements {
        if !element.visible.eval(ctx) {
            continue;
        }
        dispatch_element(element, ctx, render_ctx, registry, painter)?;
    }

    Ok(())
}

// ── Dispatch ───────────────────────────────────────────────────────────────

fn dispatch_element(
    element: &LayoutElement,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    registry: &ElementRegistry,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    match element.element_type.as_str() {
        "border" => render_border_element(element, ctx, render_ctx, registry, painter),
        "text" => render_text_element(element, ctx, render_ctx, painter),
        "tile" => render_tile_element(element, ctx, render_ctx, painter),
        "divider" => render_divider_element(element, ctx, render_ctx, painter),
        "image" => render_image_element(element, ctx, render_ctx, painter),
        "list" => render_list_element(element, ctx, render_ctx, painter),
        "flex_list" => render_flex_list_element(element, ctx, render_ctx, painter),
        "cursor" => render_cursor_element(element, ctx, render_ctx, painter),
        "bracket" => render_bracket_element(element, ctx, painter),
        "pixel_rect" => render_pixel_rect_element(element, painter),
        "group" => render_group_element(element, ctx, render_ctx, registry, painter),
        t if t.starts_with("custom:") => {
            if let Some(custom) = registry.get(t) {
                custom.render(element, ctx, render_ctx, painter)
            } else {
                log::warn!("Custom element type '{}' not registered — skipping", t);
                Ok(())
            }
        }
        _ => {
            log::warn!("Unknown element type '{}' — skipping", element.element_type);
            Ok(())
        }
    }
}

// ── Element renderers ──────────────────────────────────────────────────────

fn render_border_element(
    element: &LayoutElement,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    registry: &ElementRegistry,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let ElementParams::Border(ref params) = element.params else {
        return Ok(());
    };

    let rect = TileRect::new(
        element.rect.tx.resolve(ctx),
        element.rect.ty.resolve(ctx),
        element.rect.tw.unwrap_or(1),
        element.rect.th.unwrap_or(1),
    );

    // Draw a Game Boy-style rounded text box (corners, edges, white fill).
    // This uses the painter's native box rendering, which draws the proper
    // 8-piece border instead of the placeholder per-tile fallback.
    painter.draw_text_box(rect, Rgba::INK_BLACK);
    // tileset/style unused in MVP — always default box tiles.

    // Render any nested children (e.g. a dialog box's text). Unlike `group`,
    // a panel does not reposition its children: their rects are already
    // absolute screen coordinates, so dispatch them as-is.
    for child in &params.children {
        if !child.visible.eval(ctx) {
            continue;
        }
        dispatch_element(child, ctx, render_ctx, registry, painter)?;
    }
    Ok(())
}

fn render_text_element(
    element: &LayoutElement,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let ElementParams::Text(ref params) = element.params else {
        return Ok(());
    };
    crate::layout_engine::elements::text::render_text(element, params, ctx, render_ctx, painter)
}

fn render_tile_element(
    element: &LayoutElement,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let ElementParams::Tile(ref params) = element.params else {
        return Ok(());
    };
    crate::layout_engine::elements::tile::render_tile(element, params, ctx, render_ctx, painter)
}

fn render_divider_element(
    element: &LayoutElement,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let ElementParams::Divider(ref params) = element.params else {
        return Ok(());
    };
    crate::layout_engine::elements::divider::render_divider(
        element, params, ctx, render_ctx, painter,
    )
}

fn render_image_element(
    element: &LayoutElement,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let ElementParams::Image(ref params) = element.params else {
        return Ok(());
    };
    crate::layout_engine::elements::image::render_image(element, params, ctx, render_ctx, painter)
}

fn render_list_element(
    element: &LayoutElement,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let ElementParams::List(ref params) = element.params else {
        return Ok(());
    };
    crate::layout_engine::elements::list::render_list(
        params,
        element.rect.tx.resolve(ctx),
        element.rect.ty.resolve(ctx),
        ctx,
        render_ctx.theme,
        painter,
    );
    Ok(())
}

fn render_flex_list_element(
    element: &LayoutElement,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let ElementParams::FlexList(ref params) = element.params else {
        return Ok(());
    };
    crate::layout_engine::elements::flex_list::render_flex_list(
        params,
        element.rect.tx.resolve(ctx),
        element.rect.ty.resolve(ctx),
        ctx,
        render_ctx.theme,
        painter,
    );
    Ok(())
}

fn render_cursor_element(
    element: &LayoutElement,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let ElementParams::Cursor(ref params) = element.params else {
        return Ok(());
    };
    crate::layout_engine::elements::cursor::render_cursor(
        element,
        params,
        ctx,
        render_ctx.theme,
        painter,
    );
    Ok(())
}

fn render_bracket_element(
    element: &LayoutElement,
    ctx: &DataContext,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let ElementParams::Bracket(ref params) = element.params else {
        return Ok(());
    };
    crate::layout_engine::elements::primitives::render_bracket(element, params, ctx, painter);
    Ok(())
}

fn render_pixel_rect_element(
    element: &LayoutElement,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let ElementParams::PixelRect(ref params) = element.params else {
        return Ok(());
    };
    crate::layout_engine::elements::primitives::render_pixel_rect(params, painter);
    Ok(())
}

fn render_group_element(
    element: &LayoutElement,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    registry: &ElementRegistry,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let ElementParams::Group(ref params) = element.params else {
        return Ok(());
    };

    let rect = TileRect::new(
        element.rect.tx.resolve(ctx),
        element.rect.ty.resolve(ctx),
        element.rect.tw.unwrap_or(20),
        element.rect.th.unwrap_or(18),
    );

    let layout = GroupLayout::from_config(&params.layout);
    let group = Group::new(rect).with_layout(layout).with_clip(params.clip);

    // Resolve children to absolute positions
    let resolved = group.resolve_children(&params.children, ctx);

    // Sort children by z_index
    let mut indices: Vec<usize> = (0..resolved.len()).collect();
    indices.sort_by_key(|&i| resolved[i].z_index);

    // Render children
    for &child_idx in &indices {
        let child_rect = &resolved[child_idx];
        if !child_rect.visible {
            continue;
        }
        let child = &params.children[child_idx];

        // Create a temporary element with the resolved absolute rect
        let mut resolved_child = child.clone();
        resolved_child.rect.tx = Coord::Literal(child_rect.rect.tx);
        resolved_child.rect.ty = Coord::Literal(child_rect.rect.ty);

        dispatch_element(&resolved_child, ctx, render_ctx, registry, painter)?;
    }

    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn parse_bg_color(hex: &str) -> Rgba {
    match hex.to_lowercase().as_str() {
        // Preserve the exact Game Boy ink-ramp mappings (pokered relies on these:
        // e.g. "#FFFFFF" → the off-white INK_WHITE, not pure 0xFFFFFF).
        "#000000" | "black" => Rgba::INK_BLACK,
        "#606060" | "#808080" | "darkgray" | "dark_gray" => Rgba::INK_DARK_GRAY,
        "#a0a0a0" | "#c0c0c0" | "lightgray" | "light_gray" => Rgba::INK_LIGHT_GRAY,
        "#ffffff" | "#e0e0e0" | "white" => Rgba::INK_WHITE,
        // Any other hex value (e.g. a full-colour theme like wuxia's parchment
        // "#18140F") is parsed as a literal colour.
        other if other.starts_with('#') => crate::layout_engine::elements::text::parse_color(other),
        // Unrecognised non-hex names keep the legacy white fallback.
        other => {
            log::warn!("Unknown bg_color '{}' — falling back to White", other);
            Rgba::INK_WHITE
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_engine::types::Theme;
    use dotzuki_engine::render::Rgba as EngineRgba;
    use dotzuki_engine::render::TilePos;
    use dotzuki_engine::hash::HashMap;

    // ── RecordingPainter ──────────────────────────────────────────────

    #[derive(Debug, Clone)]
    enum DrawOp {
        Clear(EngineRgba),
        TextBox(TileRect, EngineRgba),
        Text(TilePos, String, EngineRgba),
        Glyph(TilePos, char, EngineRgba),
        PixelRect(u32, u32, u32, u32, EngineRgba),
        GbTile(TilePos, u8, String, EngineRgba),
    }

    #[derive(Debug, Default)]
    struct RecordingPainter {
        ops: Vec<DrawOp>,
    }

    impl RecordingPainter {
        fn new() -> Self {
            Self::default()
        }

        fn has_op<F: Fn(&DrawOp) -> bool>(&self, pred: F) -> bool {
            self.ops.iter().any(pred)
        }

        fn gb_tiles(&self) -> Vec<(TilePos, u8)> {
            self.ops
                .iter()
                .filter_map(|op| match op {
                    DrawOp::GbTile(pos, id, _, _) => Some((*pos, *id)),
                    _ => None,
                })
                .collect()
        }

        fn glyphs(&self) -> Vec<(TilePos, char)> {
            self.ops
                .iter()
                .filter_map(|op| match op {
                    DrawOp::Glyph(pos, ch, _) => Some((*pos, *ch)),
                    _ => None,
                })
                .collect()
        }
    }

    impl Painter for RecordingPainter {
        fn clear(&mut self, color: EngineRgba) {
            self.ops.push(DrawOp::Clear(color));
        }

        fn draw_text_box(&mut self, rect: TileRect, color: EngineRgba) {
            self.ops.push(DrawOp::TextBox(rect, color));
        }

        fn draw_text(&mut self, pos: TilePos, text: &str, color: EngineRgba) {
            self.ops.push(DrawOp::Text(pos, text.to_string(), color));
        }

        fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: EngineRgba) {
            self.ops.push(DrawOp::Glyph(pos, glyph, color));
        }

        fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, color: EngineRgba) {
            self.ops.push(DrawOp::PixelRect(px, py, pw, ph, color));
        }

        fn draw_gb_tile(&mut self, pos: TilePos, tile_id: u8, fallback: &str, color: EngineRgba) {
            self.ops
                .push(DrawOp::GbTile(pos, tile_id, fallback.to_string(), color));
        }
    }

    // ── Helpers ────────────────────────────────────────────────────────

    fn make_theme() -> Theme {
        Theme {
            bg_color: "#FFFFFF".to_string(),
            default_font: "default".to_string(),
            ..Default::default()
        }
    }

    fn make_fonts() -> HashMap<String, ()> {
        HashMap::default()
    }

    fn make_tilesets() -> HashMap<String, ()> {
        HashMap::default()
    }

    fn make_render_ctx<'a>(
        theme: &'a Theme,
        fonts: &'a HashMap<String, ()>,
        tilesets: &'a HashMap<String, ()>,
    ) -> RenderContext<'a> {
        RenderContext {
            screen: "test",
            theme,
            fonts,
            tilesets,
            images: crate::layout_engine::types::empty_image_registry(),
        }
    }

    // ── test_dispatch_border ──────────────────────────────────────────

    #[test]
    fn test_dispatch_border() {
        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "elements": [
                { "type": "border", "rect": { "tx": 1, "ty": 1, "tw": 10, "th": 5 } }
            ]
        }"##;
        let layout: ScreenLayout = serde_json::from_str(json).unwrap();
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts = make_fonts();
        let tilesets = make_tilesets();
        let rc = make_render_ctx(&theme, &fonts, &tilesets);
        let registry = ElementRegistry::new();
        let mut painter = RecordingPainter::new();

        render_layout(&layout, &ctx, &rc, &registry, &mut painter).unwrap();

        // Border draws a Game Boy-style text box (corners, edges, fill)
        assert!(
            painter.has_op(|op| matches!(op, DrawOp::TextBox(..))),
            "border should draw a text box"
        );
    }

    // ── test_dispatch_text ────────────────────────────────────────────

    #[test]
    fn test_dispatch_text() {
        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "elements": [
                { "type": "text", "rect": { "tx": 2, "ty": 3, "tw": 10, "th": 2 }, "value": "Hello" }
            ]
        }"##;
        let layout: ScreenLayout = serde_json::from_str(json).unwrap();
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts = make_fonts();
        let tilesets = make_tilesets();
        let rc = make_render_ctx(&theme, &fonts, &tilesets);
        let registry = ElementRegistry::new();
        let mut painter = RecordingPainter::new();

        render_layout(&layout, &ctx, &rc, &registry, &mut painter).unwrap();

        let glyphs = painter.glyphs();
        assert!(!glyphs.is_empty(), "text should draw glyphs");
        assert_eq!(glyphs[0].0, TilePos::new(2, 3));
        assert_eq!(glyphs[0].1, 'H');
    }

    // ── test_dispatch_tile ────────────────────────────────────────────

    #[test]
    fn test_dispatch_tile() {
        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "elements": [
                { "type": "tile", "rect": { "tx": 5, "ty": 5, "tw": 1, "th": 1 }, "tile_id": 42 }
            ]
        }"##;
        let layout: ScreenLayout = serde_json::from_str(json).unwrap();
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts = make_fonts();
        let tilesets = make_tilesets();
        let rc = make_render_ctx(&theme, &fonts, &tilesets);
        let registry = ElementRegistry::new();
        let mut painter = RecordingPainter::new();

        render_layout(&layout, &ctx, &rc, &registry, &mut painter).unwrap();

        let tiles = painter.gb_tiles();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0], (TilePos::new(5, 5), 42));
    }

    // ── test_dispatch_group ───────────────────────────────────────────

    #[test]
    fn test_dispatch_group() {
        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "elements": [
                {
                    "type": "group",
                    "rect": { "tx": 2, "ty": 2, "tw": 16, "th": 14 },
                    "layout": { "gap": 0 },
                    "clip": false,
                    "children": [
                        { "type": "text", "rect": { "tx": 0, "ty": 0, "tw": 5, "th": 1 }, "value": "A" },
                        { "type": "text", "rect": { "tx": 1, "ty": 1, "tw": 5, "th": 1 }, "value": "B" }
                    ]
                }
            ]
        }"##;
        let layout: ScreenLayout = serde_json::from_str(json).unwrap();
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts = make_fonts();
        let tilesets = make_tilesets();
        let rc = make_render_ctx(&theme, &fonts, &tilesets);
        let registry = ElementRegistry::new();
        let mut painter = RecordingPainter::new();

        render_layout(&layout, &ctx, &rc, &registry, &mut painter).unwrap();

        let glyphs = painter.glyphs();

        // Child "A" should be at group origin: (2+0, 2+0) = (2, 2)
        let a_glyph = glyphs.iter().find(|(_, ch)| *ch == 'A');
        assert!(a_glyph.is_some(), "'A' should be rendered");
        assert_eq!(a_glyph.unwrap().0, TilePos::new(2, 2));

        // Child "B" should be at (2+1, 2+1) = (3, 3)
        let b_glyph = glyphs.iter().find(|(_, ch)| *ch == 'B');
        assert!(b_glyph.is_some(), "'B' should be rendered");
        assert_eq!(b_glyph.unwrap().0, TilePos::new(3, 3));
    }

    // ── test_dispatch_invisible_element ───────────────────────────────

    #[test]
    fn test_dispatch_invisible_element() {
        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "elements": [
                { "type": "text", "rect": { "tx": 0, "ty": 0 }, "value": "hidden", "visible": false },
                { "type": "text", "rect": { "tx": 0, "ty": 1 }, "value": "visible" }
            ]
        }"##;
        let layout: ScreenLayout = serde_json::from_str(json).unwrap();
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts = make_fonts();
        let tilesets = make_tilesets();
        let rc = make_render_ctx(&theme, &fonts, &tilesets);
        let registry = ElementRegistry::new();
        let mut painter = RecordingPainter::new();

        render_layout(&layout, &ctx, &rc, &registry, &mut painter).unwrap();

        let glyphs = painter.glyphs();
        let hidden = glyphs.iter().any(|(_, ch)| *ch == 'h');
        assert!(!hidden, "invisible element should not render");
        let visible = glyphs.iter().any(|(_, ch)| *ch == 'v');
        assert!(visible, "visible element should render");
    }

    // ── test_dispatch_unknown_type ────────────────────────────────────

    #[test]
    fn test_dispatch_unknown_type() {
        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "elements": [
                { "type": "fantasy_element", "rect": { "tx": 0, "ty": 0 } }
            ]
        }"##;
        let layout: ScreenLayout = serde_json::from_str(json).unwrap();
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts = make_fonts();
        let tilesets = make_tilesets();
        let rc = make_render_ctx(&theme, &fonts, &tilesets);
        let registry = ElementRegistry::new();
        let mut painter = RecordingPainter::new();

        let result = render_layout(&layout, &ctx, &rc, &registry, &mut painter);
        assert!(result.is_ok(), "unknown type should not cause error");

        // Only the clear op should be present
        let non_clear_ops: Vec<_> = painter
            .ops
            .iter()
            .filter(|op| !matches!(op, DrawOp::Clear(_)))
            .collect();
        assert!(
            non_clear_ops.is_empty(),
            "unknown type should skip rendering"
        );
    }

    // ── test_dispatch_custom ──────────────────────────────────────────

    #[test]
    fn test_dispatch_custom() {
        use crate::layout_engine::registry::CustomElement;

        #[derive(Debug)]
        struct TestCustom;

        impl CustomElement for TestCustom {
            fn element_type(&self) -> &'static str {
                "custom:test"
            }

            fn render(
                &self,
                _element: &LayoutElement,
                _ctx: &DataContext,
                _render_ctx: &RenderContext,
                painter: &mut dyn Painter,
            ) -> Result<(), RenderError> {
                painter.draw_glyph(TilePos::new(99, 99), 'X', EngineRgba::INK_BLACK);
                Ok(())
            }
        }

        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "elements": [
                { "type": "custom:test", "rect": { "tx": 0, "ty": 0 } }
            ]
        }"##;
        let layout: ScreenLayout = serde_json::from_str(json).unwrap();
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts = make_fonts();
        let tilesets = make_tilesets();
        let rc = make_render_ctx(&theme, &fonts, &tilesets);
        let mut registry = ElementRegistry::new();
        registry.register(Box::new(TestCustom));
        let mut painter = RecordingPainter::new();

        render_layout(&layout, &ctx, &rc, &registry, &mut painter).unwrap();

        let glyphs = painter.glyphs();
        assert_eq!(glyphs.len(), 1);
        assert_eq!(glyphs[0], (TilePos::new(99, 99), 'X'));
    }

    // ── test_dispatch_z_order ─────────────────────────────────────────

    #[test]
    fn test_dispatch_z_order() {
        // Lower z_index draws first, so higher z_index elements appear
        // "on top" in the ops list.
        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "elements": [
                { "type": "tile", "rect": { "tx": 0, "ty": 0, "tw": 1, "th": 1 }, "tile_id": 99, "z_index": 0 },
                { "type": "tile", "rect": { "tx": 0, "ty": 0, "tw": 1, "th": 1 }, "tile_id": 1, "z_index": 10 },
                { "type": "tile", "rect": { "tx": 0, "ty": 0, "tw": 1, "th": 1 }, "tile_id": 50, "z_index": 5 }
            ]
        }"##;
        let layout: ScreenLayout = serde_json::from_str(json).unwrap();
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts = make_fonts();
        let tilesets = make_tilesets();
        let rc = make_render_ctx(&theme, &fonts, &tilesets);
        let registry = ElementRegistry::new();
        let mut painter = RecordingPainter::new();

        render_layout(&layout, &ctx, &rc, &registry, &mut painter).unwrap();

        let tiles = painter.gb_tiles();
        // Sorted by z_index: 0 (99), 5 (50), 10 (1)
        assert_eq!(tiles.len(), 3);
        assert_eq!(tiles[0].1, 99);
        assert_eq!(tiles[1].1, 50);
        assert_eq!(tiles[2].1, 1);
    }

    // ── test_render_title_screen ──────────────────────────────────────

    #[test]
    fn test_render_title_screen() {
        let json = r##"{
            "schema_version": 1,
            "screen": "title_screen",
            "theme": { "bg_color": "#FFFFFF", "default_font": "default" },
            "elements": [
                {
                    "type": "border",
                    "rect": { "tx": 1, "ty": 5, "tw": 18, "th": 7 },
                    "style": "Single"
                },
                {
                    "type": "text",
                    "rect": { "tx": 5, "ty": 7, "tw": 10, "th": 1 },
                    "value": "MONSTER RED",
                    "color": "black",
                    "align": "Center"
                },
                {
                    "type": "text",
                    "rect": { "tx": 6, "ty": 9, "tw": 8, "th": 1 },
                    "value": "Press START",
                    "color": "darkgray",
                    "align": "Center"
                }
            ]
        }"##;
        let layout: ScreenLayout = serde_json::from_str(json).unwrap();
        let ctx = DataContext::new();
        let theme = layout.theme.clone();
        let fonts = make_fonts();
        let tilesets = make_tilesets();
        let rc = make_render_ctx(&theme, &fonts, &tilesets);
        let registry = ElementRegistry::new();
        let mut painter = RecordingPainter::new();

        render_layout(&layout, &ctx, &rc, &registry, &mut painter).unwrap();

        // Clear op fired
        assert!(painter.has_op(|op| matches!(op, DrawOp::Clear(Rgba::INK_WHITE))));

        // Border draws a Game Boy-style text box
        assert!(painter.has_op(|op| matches!(op, DrawOp::TextBox(..))));

        // Text draws glyphs
        let glyphs = painter.glyphs();
        assert!(!glyphs.is_empty(), "title screen should have text glyphs");
    }

    // ── test_dispatch_divider ─────────────────────────────────────────

    #[test]
    fn test_dispatch_divider() {
        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "elements": [
                {
                    "type": "divider",
                    "rect": { "tx": 0, "ty": 0, "tw": 5, "th": 1 },
                    "tiles": [120, 121],
                    "repeat": 3
                }
            ]
        }"##;
        let layout: ScreenLayout = serde_json::from_str(json).unwrap();
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts = make_fonts();
        let tilesets = make_tilesets();
        let rc = make_render_ctx(&theme, &fonts, &tilesets);
        let registry = ElementRegistry::new();
        let mut painter = RecordingPainter::new();

        render_layout(&layout, &ctx, &rc, &registry, &mut painter).unwrap();

        let tiles = painter.gb_tiles();
        assert_eq!(tiles.len(), 5);
        assert_eq!(tiles[0].1, 120);
        assert_eq!(tiles[1].1, 121);
        // Remaining repeated tiles
        assert_eq!(tiles[2].1, 121);
        assert_eq!(tiles[3].1, 121);
        assert_eq!(tiles[4].1, 121);
    }

    // ── test_dispatch_image ───────────────────────────────────────────

    #[test]
    fn test_dispatch_image() {
        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "elements": [
                {
                    "type": "image",
                    "rect": { "tx": 1, "ty": 1, "tw": 3, "th": 3 },
                    "source": "sparkit"
                }
            ]
        }"##;
        let layout: ScreenLayout = serde_json::from_str(json).unwrap();
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts = make_fonts();
        let tilesets = make_tilesets();
        let rc = make_render_ctx(&theme, &fonts, &tilesets);
        let registry = ElementRegistry::new();
        let mut painter = RecordingPainter::new();

        let result = render_layout(&layout, &ctx, &rc, &registry, &mut painter);
        assert!(result.is_ok(), "image dispatch should not error");

        // Image with no sprite data draws a placeholder (pixel rects)
        assert!(
            painter.has_op(|op| matches!(op, DrawOp::PixelRect(..))),
            "image should draw placeholder"
        );
    }

    // ── test_parse_bg_color ───────────────────────────────────────────

    #[test]
    fn test_parse_bg_color_white() {
        assert_eq!(parse_bg_color("#FFFFFF"), Rgba::INK_WHITE);
        assert_eq!(parse_bg_color("white"), Rgba::INK_WHITE);
    }

    #[test]
    fn test_parse_bg_color_black() {
        assert_eq!(parse_bg_color("#000000"), Rgba::INK_BLACK);
        assert_eq!(parse_bg_color("black"), Rgba::INK_BLACK);
    }

    #[test]
    fn test_parse_bg_color_darkgray() {
        assert_eq!(parse_bg_color("#808080"), Rgba::INK_DARK_GRAY);
        assert_eq!(parse_bg_color("darkgray"), Rgba::INK_DARK_GRAY);
    }

    #[test]
    fn test_parse_bg_color_unknown() {
        assert_eq!(parse_bg_color("pink"), Rgba::INK_WHITE);
    }
}
