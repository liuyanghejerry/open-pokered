//! Scrollable list element renderer.
//!
//! Renders a vertical list of items with cursor navigation, scroll
//! indicators, and an optional footer. Items are resolved from the
//! [`DataContext`] by the `items` key.
//!
//! ## Features
//! - `max_visible` controls how many items are shown at once
//! - Cursor ▶ highlights the active item
//! - Scroll indicators ▲/▼ when content overflows
//! - Footer text (e.g. "CANCEL") drawn below the list

use dotzuki_engine::render::painter::Painter;
use dotzuki_engine::render::{Rgba, TilePos};

use crate::layout_engine::types::{DataContext, DataValue, ListParams, Theme};

// ── Public API ──────────────────────────────────────────────────────────────

/// Render a scrollable list into the framebuffer via `painter`.
///
/// # Arguments
/// * `params` — Deserialised [`ListParams`] from the layout definition.
/// * `tx`, `ty` — Top-left tile position of the list element.
/// * `ctx` — Data context for resolving item values and template strings.
/// * `painter` — Drawing backend.
pub fn render_list(
    params: &ListParams,
    tx: u32,
    ty: u32,
    ctx: &DataContext,
    theme: &Theme,
    painter: &mut dyn Painter,
) {
    // ── Resolve items from the data context ──────────────────────────
    // `items` may be written as a bare key (`items`) or a template
    // reference (`{items}`); strip the surrounding braces before lookup.
    let items_key = strip_braces(&params.items);
    let items: Vec<DataValue> = match ctx.get(items_key) {
        Some(DataValue::List(v)) => v.clone(),
        // A plain string (e.g. from an editor variable override, where every
        // value arrives as a string) is treated as one row per line.
        Some(DataValue::Str(s)) => s
            .lines()
            .map(|l| l.trim_end())
            .filter(|l| !l.is_empty())
            .map(|l| DataValue::Str(l.to_string()))
            .collect(),
        _ => return,
    };
    if items.is_empty() {
        return;
    }

    // Selected index — resolves a literal or a `{cursor}` template against the
    // data context (so the live cursor follows menu state); defaults to the
    // first item for static previews.
    let cursor = params
        .selected
        .as_ref()
        .map(|c| c.resolve(ctx))
        .unwrap_or(0) as usize;
    let max_visible = params.max_visible.unwrap_or(items.len()).max(1);
    let row_height = params.item_template.height;
    let gap = params.item_template.gap;

    // ── Calculate visible scroll window ──────────────────────────────
    let scroll_start = calculate_scroll_window(cursor, max_visible, items.len());
    let visible_end = (scroll_start + max_visible).min(items.len());
    let has_scroll_up = scroll_start > 0;
    let has_scroll_down = visible_end < items.len();

    // ── Proportional (pixel-precise) path — themed colours + CJK advance ──
    if theme.proportional(painter.supports_proportional()) {
        let ink = theme.ink_color();
        let cur = theme.cursor_ink();
        let base_px = tx * 8;
        let row_pitch = (row_height + gap).max(1) * 8;
        let cursor_adv = 10u32; // ▶ glyph advance
        let mut y = ty * 8;
        if has_scroll_up {
            painter.draw_text_px(base_px, y, "\u{25B2}", ink);
            y += row_pitch;
        }
        for item_idx in scroll_start..visible_end {
            let item_text = data_value_to_string(&items[item_idx]);
            if item_idx == cursor {
                painter.draw_text_px(base_px, y, "\u{25B6}", cur);
            }
            painter.draw_text_px(base_px + cursor_adv, y, &item_text, ink);
            y += row_pitch;
        }
        if has_scroll_down {
            painter.draw_text_px(base_px, y, "\u{25BC}", ink);
            y += row_pitch;
        }
        if let Some(footer) = &params.footer {
            let footer_text = ctx.resolve(footer);
            painter.draw_text_px(base_px, y, &footer_text, ink);
        }
        return;
    }

    let mut row_ty = ty;

    // ── Scroll-up indicator ▲ ────────────────────────────────────────
    if has_scroll_up {
        painter.draw_glyph(
            TilePos::new(tx, row_ty),
            '\u{25B2}', // ▲
            Rgba::INK_BLACK,
        );
        row_ty += 1;
    }

    // ── Visible items ────────────────────────────────────────────────
    for item_idx in scroll_start..visible_end {
        let item_text = data_value_to_string(&items[item_idx]);

        if item_idx == cursor {
            // ▶ cursor at active item
            painter.draw_glyph(
                TilePos::new(tx, row_ty),
                '\u{25B6}', // ▶
                Rgba::INK_BLACK,
            );
            painter.draw_text(TilePos::new(tx + 1, row_ty), &item_text, Rgba::INK_BLACK);
        } else {
            // Non-active item — indented one tile to align with cursor row
            painter.draw_text(TilePos::new(tx + 1, row_ty), &item_text, Rgba::INK_BLACK);
        }
        row_ty += row_height + gap;
    }

    // ── Scroll-down indicator ▼ ──────────────────────────────────────
    if has_scroll_down {
        painter.draw_glyph(
            TilePos::new(tx, row_ty),
            '\u{25BC}', // ▼
            Rgba::INK_BLACK,
        );
        row_ty += 1;
    }

    // ── Footer ───────────────────────────────────────────────────────
    if let Some(footer) = &params.footer {
        let footer_text = ctx.resolve(footer);
        painter.draw_text(TilePos::new(tx, row_ty), &footer_text, Rgba::INK_BLACK);
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Strip a single pair of surrounding `{ }` braces (and whitespace) from a
/// variable reference, returning the bare key. `"{items}"` → `"items"`,
/// `"items"` → `"items"`.
pub(crate) fn strip_braces(s: &str) -> &str {
    let t = s.trim();
    t.strip_prefix('{')
        .and_then(|r| r.strip_suffix('}'))
        .unwrap_or(t)
        .trim()
}

/// Compute the starting index of the visible scroll window so the cursor
/// stays within the view.
fn calculate_scroll_window(cursor: usize, max_visible: usize, total: usize) -> usize {
    if total <= max_visible {
        return 0;
    }
    if cursor <= max_visible.saturating_sub(1) {
        0
    } else {
        // Cursor at bottom of visible window, or as far as we can scroll
        cursor
            .saturating_sub(max_visible - 1)
            .min(total - max_visible)
    }
}

/// Convert a [`DataValue`] to its string representation.
fn data_value_to_string(value: &DataValue) -> String {
    match value {
        DataValue::Str(s) => s.clone(),
        DataValue::Int(n) => n.to_string(),
        DataValue::Float(f) => f.to_string(),
        DataValue::Bool(b) => b.to_string(),
        DataValue::TileId(t) => t.to_string(),
        DataValue::List(v) => v
            .iter()
            .map(data_value_to_string)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render::TileRect;
    use core::cell::RefCell;

    // ── Test double: MockPainter ─────────────────────────────────────

    /// A recorded draw operation.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum DrawOp {
        Text { tx: u32, ty: u32, text: String },
        Glyph { tx: u32, ty: u32, glyph: char },
        TextBox { tx: u32, ty: u32, tw: u32, th: u32 },
        PixelRect { px: u32, py: u32, pw: u32, ph: u32 },
    }

    /// A [`Painter`] that records every call for verification.
    struct MockPainter {
        ops: RefCell<Vec<DrawOp>>,
    }

    impl MockPainter {
        fn new() -> Self {
            Self {
                ops: RefCell::new(Vec::new()),
            }
        }

        fn ops(&self) -> core::cell::Ref<'_, Vec<DrawOp>> {
            self.ops.borrow()
        }

        fn find_text(&self, needle: &str) -> Vec<(u32, u32)> {
            self.ops()
                .iter()
                .filter_map(|op| match op {
                    DrawOp::Text { tx, ty, text } if text.contains(needle) => Some((*tx, *ty)),
                    _ => None,
                })
                .collect()
        }

        fn find_glyph(&self, needle: char) -> Option<(u32, u32)> {
            self.ops().iter().find_map(|op| match op {
                DrawOp::Glyph { tx, ty, glyph } if *glyph == needle => Some((*tx, *ty)),
                _ => None,
            })
        }
    }

    impl Painter for MockPainter {
        fn clear(&mut self, _color: Rgba) {}

        fn draw_text_box(&mut self, rect: TileRect, _color: Rgba) {
            self.ops.borrow_mut().push(DrawOp::TextBox {
                tx: rect.tx,
                ty: rect.ty,
                tw: rect.tw,
                th: rect.th,
            });
        }

        fn draw_text(&mut self, pos: TilePos, text: &str, color: Rgba) {
            let _ = color;
            self.ops.borrow_mut().push(DrawOp::Text {
                tx: pos.tx,
                ty: pos.ty,
                text: text.to_string(),
            });
        }

        fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba) {
            let _ = color;
            self.ops.borrow_mut().push(DrawOp::Glyph {
                tx: pos.tx,
                ty: pos.ty,
                glyph,
            });
        }

        fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, _color: Rgba) {
            self.ops
                .borrow_mut()
                .push(DrawOp::PixelRect { px, py, pw, ph });
        }

        fn draw_gb_tile(&mut self, _pos: TilePos, _tile_id: u8, _fallback: &str, _color: Rgba) {}
    }

    // ── Helper: build ListParams ─────────────────────────────────────

    fn make_list_params(
        items_key: &str,
        cursor: u32,
        max_visible: usize,
        footer: Option<&str>,
    ) -> ListParams {
        ListParams {
            items: items_key.to_string(),
            item_template: crate::layout_engine::types::ItemTemplate { height: 1, gap: 0 },
            cursor: crate::layout_engine::types::ListCursor::default(),
            selected: Some(cursor.into()),
            max_visible: Some(max_visible),
            footer: footer.map(|s| s.to_string()),
        }
    }

    // ── Tests: calculate_scroll_window ───────────────────────────────

    #[test]
    fn scroll_window_cursor_at_top() {
        assert_eq!(calculate_scroll_window(0, 5, 20), 0);
        assert_eq!(calculate_scroll_window(2, 5, 20), 0);
    }

    #[test]
    fn scroll_window_cursor_in_middle() {
        // cursor=5, max_visible=5 → window starts at 1 (items 1-5 visible)
        assert_eq!(calculate_scroll_window(5, 5, 20), 1);
    }

    #[test]
    fn scroll_window_cursor_at_bottom() {
        // cursor=19, max_visible=5, total=20 → window starts at 15
        assert_eq!(calculate_scroll_window(19, 5, 20), 15);
    }

    #[test]
    fn scroll_window_small_list() {
        assert_eq!(calculate_scroll_window(2, 5, 3), 0);
    }

    #[test]
    fn scroll_window_empty_list() {
        assert_eq!(calculate_scroll_window(0, 5, 0), 0);
    }

    #[test]
    fn scroll_window_exact_fit() {
        assert_eq!(calculate_scroll_window(3, 5, 5), 0);
    }

    // ── Tests: data_value_to_string ──────────────────────────────────

    #[test]
    fn string_value_to_string() {
        assert_eq!(
            data_value_to_string(&DataValue::Str("HELLO".into())),
            "HELLO"
        );
    }

    #[test]
    fn int_value_to_string() {
        assert_eq!(data_value_to_string(&DataValue::Int(42)), "42");
    }

    #[test]
    fn bool_value_to_string() {
        assert_eq!(data_value_to_string(&DataValue::Bool(true)), "true");
    }

    #[test]
    fn list_value_to_string() {
        let list = DataValue::List(vec![DataValue::Str("A".into()), DataValue::Str("B".into())]);
        assert_eq!(data_value_to_string(&list), "A, B");
    }

    // ── Tests: render_list ───────────────────────────────────────────

    #[test]
    fn renders_items_with_cursor() {
        let mut ctx = DataContext::new();
        ctx.set(
            "my_items",
            DataValue::List(vec![
                DataValue::Str("BALL".into()),
                DataValue::Str("POTION".into()),
                DataValue::Str("ANTIDOTE".into()),
            ]),
        );

        let params = make_list_params("my_items", 0, 5, None);
        let mut painter = MockPainter::new();

        render_list(&params, 2, 3, &ctx, &Theme::default(), &mut painter);

        // First item should have a cursor ▶ at (2, 3)
        assert!(painter.find_glyph('\u{25B6}').is_some());
        // Text content should be present
        assert!(!painter.find_text("BALL").is_empty());
        assert!(!painter.find_text("POTION").is_empty());
        assert!(!painter.find_text("ANTIDOTE").is_empty());
    }

    #[test]
    fn cursor_on_second_item() {
        let mut ctx = DataContext::new();
        ctx.set(
            "items",
            DataValue::List(vec![
                DataValue::Str("A".into()),
                DataValue::Str("B".into()),
                DataValue::Str("C".into()),
            ]),
        );

        let params = make_list_params("items", 1, 5, None);
        let mut painter = MockPainter::new();

        render_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        // Cursor ▶ should be on the second row (ty=1, since height=1, gap=0)
        let cursor_glyph = painter.find_glyph('\u{25B6}');
        assert!(cursor_glyph.is_some());
        let (_, cursor_ty) = cursor_glyph.unwrap();
        assert_eq!(cursor_ty, 1);
    }

    #[test]
    fn scroll_up_indicator_when_scrolled_down() {
        let mut ctx = DataContext::new();
        let items: Vec<DataValue> = (0..20)
            .map(|i| DataValue::Str(format!("Item {}", i)))
            .collect();
        ctx.set("items", DataValue::List(items));

        // cursor at item 10, max_visible=5 → scroll_start > 0
        let params = make_list_params("items", 10, 5, None);
        let mut painter = MockPainter::new();

        render_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        // Should have ▲ (scroll up) indicator
        assert!(painter.find_glyph('\u{25B2}').is_some());
    }

    #[test]
    fn scroll_down_indicator_when_content_below() {
        let mut ctx = DataContext::new();
        let items: Vec<DataValue> = (0..20)
            .map(|i| DataValue::Str(format!("Item {}", i)))
            .collect();
        ctx.set("items", DataValue::List(items));

        // cursor at 0, max_visible=5, total=20 → content below
        let params = make_list_params("items", 0, 5, None);
        let mut painter = MockPainter::new();

        render_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        assert!(painter.find_glyph('\u{25BC}').is_some());
    }

    #[test]
    fn no_scroll_indicators_when_fits() {
        let mut ctx = DataContext::new();
        ctx.set(
            "items",
            DataValue::List(vec![DataValue::Str("A".into()), DataValue::Str("B".into())]),
        );

        let params = make_list_params("items", 0, 5, None);
        let mut painter = MockPainter::new();

        render_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        assert!(painter.find_glyph('\u{25B2}').is_none());
        assert!(painter.find_glyph('\u{25BC}').is_none());
    }

    #[test]
    fn footer_rendered() {
        let mut ctx = DataContext::new();
        ctx.set("items", DataValue::List(vec![DataValue::Str("X".into())]));

        let params = make_list_params("items", 0, 1, Some("CANCEL"));
        let mut painter = MockPainter::new();

        render_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        assert!(!painter.find_text("CANCEL").is_empty());
    }

    #[test]
    fn footer_resolves_template() {
        let mut ctx = DataContext::new();
        ctx.set("items", DataValue::List(vec![DataValue::Str("X".into())]));
        ctx.set("action", "CANCEL");

        let params = make_list_params("items", 0, 1, Some("{action}"));
        let mut painter = MockPainter::new();

        render_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        assert!(!painter.find_text("CANCEL").is_empty());
    }

    #[test]
    fn empty_items_no_render() {
        let mut ctx = DataContext::new();
        ctx.set("items", DataValue::List(vec![]));

        let params = make_list_params("items", 0, 5, None);
        let mut painter = MockPainter::new();

        render_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        assert!(painter.ops().is_empty());
    }

    #[test]
    fn missing_items_key_no_render() {
        let ctx = DataContext::new();
        let params = make_list_params("nonexistent", 0, 5, None);
        let mut painter = MockPainter::new();

        render_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        assert!(painter.ops().is_empty());
    }
}
