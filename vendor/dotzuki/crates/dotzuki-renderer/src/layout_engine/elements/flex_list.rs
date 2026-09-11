//! Multi-column list element renderer.
//!
//! Renders a list where each row contains multiple columns (e.g. name,
//! quantity, price). Columns are defined with widths and text alignment,
//! and the cursor highlights the active row.
//!
//! ## Features
//! - Multi-column item rows with named columns
//! - Column widths and text alignment (Left / Center / Right)
//! - `gap` between rows, `padding` around the content area
//! - Cursor ▶ on the active row

use dotzuki_engine::render::painter::Painter;
use dotzuki_engine::render::{Rgba, TilePos};

use crate::layout_engine::types::{
    ColumnDef, DataContext, DataValue, FlexListParams, TextAlign, Theme,
};

/// Extra tile units consumed by the cursor glyph (▶) + a space.
const CURSOR_WIDTH_TILES: u32 = 1;

/// Render a multi-column flex list into the framebuffer via `painter`.
///
/// Each item in `items` must be a `DataValue::List` whose elements
/// correspond to the columns defined in `item_layout` by position.
///
/// # Arguments
/// * `params` — Deserialised [`FlexListParams`] from the layout definition.
/// * `tx`, `ty` — Top-left tile position of the element.
/// * `ctx` — Data context for resolving item values.
/// * `painter` — Drawing backend.
pub fn render_flex_list(
    params: &FlexListParams,
    tx: u32,
    ty: u32,
    ctx: &DataContext,
    theme: &Theme,
    painter: &mut dyn Painter,
) {
    let ncols = params.item_layout.len();
    let items: Vec<DataValue> = match ctx.get(super::list::strip_braces(&params.items)) {
        Some(DataValue::List(v)) => v.clone(),
        // A plain string (e.g. from an editor variable override) is split into
        // one row per line, then each line into columns.
        Some(DataValue::Str(s)) => s
            .lines()
            .map(|l| l.trim_end())
            .filter(|l| !l.is_empty())
            .map(|l| split_row_into_columns(l, ncols))
            .collect(),
        _ => return,
    };
    if items.is_empty() || params.item_layout.is_empty() {
        return;
    }

    // Selected index — resolves a literal or a `{cursor}` template against the
    // data context; defaults to the first item for static previews.
    let cursor = params
        .selected
        .as_ref()
        .map(|c| c.resolve(ctx))
        .unwrap_or(0) as usize;
    let padding = &params.padding;
    let gap = params.gap;

    // Content area origin (after padding)
    let content_tx = tx + padding.left;
    let content_ty = ty + padding.top;

    // Compute column X positions
    let column_positions = compute_column_positions(content_tx, &params.item_layout);

    // ── Proportional (pixel-precise) path — themed colours + CJK columns ──
    if theme.proportional(painter.supports_proportional()) {
        let ink = theme.ink_color();
        let cur = theme.cursor_ink();
        let cursor_adv = 10u32; // ▶ glyph advance
                                // Rows must clear the full CJK glyph height; `gap` (tiles) adds leading.
        let row_pitch = ((1 + gap) * 8).max(13);
        let mut y = content_ty * 8;
        for (row_idx, item) in items.iter().enumerate() {
            let row_values = item_to_strings(item);
            let is_active = row_idx == cursor;
            if is_active {
                painter.draw_text_px(content_tx * 8, y, "\u{25B6}", cur);
            }
            for (col_idx, column) in params.item_layout.iter().enumerate() {
                let value = row_values
                    .get(col_idx)
                    .cloned()
                    .unwrap_or_else(|| "?".to_string());
                let prefix = column.prefix.as_deref().unwrap_or("");
                let display_text = format!("{prefix}{value}");
                let align = column.align.as_ref().unwrap_or(&TextAlign::Left);
                let col_x = column_positions[col_idx] * 8;
                let col_w = column.width * 8;
                let text_w = painter.measure_text_px(&display_text);
                // Reserve the cursor gutter on the first column for EVERY row
                // (not just the active one) so only the ▶ moves between rows and
                // the text stays put.
                let gutter = if col_idx == 0 { cursor_adv } else { 0 };
                let avail = col_w.saturating_sub(gutter);
                let x = gutter
                    + match align {
                        TextAlign::Left => col_x,
                        TextAlign::Center => col_x + avail.saturating_sub(text_w) / 2,
                        TextAlign::Right => col_x + avail.saturating_sub(text_w),
                    };
                painter.draw_text_px(x, y, &display_text, ink);
            }
            y += row_pitch;
        }
        return;
    }

    let mut row_ty = content_ty;

    for (row_idx, item) in items.iter().enumerate() {
        let row_values = item_to_strings(item);
        let is_active = row_idx == cursor;

        if is_active {
            painter.draw_glyph(
                TilePos::new(content_tx, row_ty),
                '\u{25B6}',
                Rgba::INK_BLACK,
            );
        }

        for (col_idx, column) in params.item_layout.iter().enumerate() {
            let value = row_values
                .get(col_idx)
                .cloned()
                .unwrap_or_else(|| "?".to_string());

            let prefix = column.prefix.as_deref().unwrap_or("");
            let display_text = format!("{}{}", prefix, value);
            let align = column.align.as_ref().unwrap_or(&TextAlign::Left);

            let text_x = align_text(
                column_positions[col_idx],
                column.width,
                &display_text,
                align,
            );

            // First column text shifts right by cursor width when cursor is active
            let col_x = if col_idx == 0 && is_active {
                text_x + CURSOR_WIDTH_TILES
            } else {
                text_x
            };

            painter.draw_text(TilePos::new(col_x, row_ty), &display_text, Rgba::INK_BLACK);
        }

        row_ty += 1 + gap;
    }
}

/// Compute the starting tile X position for each column.
fn compute_column_positions(start_tx: u32, columns: &[ColumnDef]) -> Vec<u32> {
    let mut positions = Vec::with_capacity(columns.len());
    let mut x = start_tx;
    for col in columns {
        positions.push(x);
        x += col.width;
    }
    positions
}

/// Compute the tile X position for text with the given alignment within a
/// column of `width` tiles.
fn align_text(col_x: u32, width: u32, text: &str, align: &TextAlign) -> u32 {
    match align {
        TextAlign::Left => col_x,
        TextAlign::Center => {
            let text_len = text.len() as u32;
            if text_len >= width {
                col_x
            } else {
                col_x + (width - text_len) / 2
            }
        }
        TextAlign::Right => {
            let text_len = text.len() as u32;
            if text_len >= width {
                col_x
            } else {
                col_x + width - text_len
            }
        }
    }
}

/// Split a plain-string row into `ncols` column values.
///
/// Tokens are whitespace-separated. When there are more tokens than columns,
/// the first column absorbs the extra leading tokens (item names may contain
/// spaces, e.g. "BALL"), and the trailing tokens map to the remaining
/// columns. When there are fewer, the missing columns are left empty.
fn split_row_into_columns(s: &str, ncols: usize) -> DataValue {
    if ncols <= 1 {
        return DataValue::List(vec![DataValue::Str(s.trim().to_string())]);
    }
    let tokens: Vec<&str> = s.split_whitespace().collect();
    let cols: Vec<DataValue> = if tokens.len() <= ncols {
        let mut v: Vec<DataValue> = tokens
            .iter()
            .map(|t| DataValue::Str(t.to_string()))
            .collect();
        while v.len() < ncols {
            v.push(DataValue::Str(String::new()));
        }
        v
    } else {
        let trailing = ncols - 1;
        let split_at = tokens.len() - trailing;
        let first = tokens[..split_at].join(" ");
        let mut v = vec![DataValue::Str(first)];
        v.extend(
            tokens[split_at..]
                .iter()
                .map(|t| DataValue::Str(t.to_string())),
        );
        v
    };
    DataValue::List(cols)
}

/// Extract per-column string values from a `DataValue` row item.
///
/// Expects the item to be a `DataValue::List`; each element corresponds to a
/// column value. Non-list values are returned as a single-element vector.
fn item_to_strings(item: &DataValue) -> Vec<String> {
    match item {
        DataValue::List(v) => v.iter().map(data_value_to_string).collect(),
        other => vec![data_value_to_string(other)],
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_engine::types::EdgeInsets;
    use dotzuki_engine::render::TileRect;
    use core::cell::RefCell;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum DrawOp {
        Text { tx: u32, ty: u32, text: String },
        Glyph { tx: u32, ty: u32, glyph: char },
        TextBox { tx: u32, ty: u32, tw: u32, th: u32 },
        PixelRect { px: u32, py: u32, pw: u32, ph: u32 },
    }

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

        fn find_text(&self, needle: &str) -> Vec<(u32, u32, String)> {
            self.ops()
                .iter()
                .filter_map(|op| match op {
                    DrawOp::Text { tx, ty, text } if text.contains(needle) => {
                        Some((*tx, *ty, text.clone()))
                    }
                    _ => None,
                })
                .collect()
        }

        fn find_glyph(&self, needle: char) -> Vec<(u32, u32)> {
            self.ops()
                .iter()
                .filter_map(|op| match op {
                    DrawOp::Glyph { tx, ty, glyph } if *glyph == needle => Some((*tx, *ty)),
                    _ => None,
                })
                .collect()
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
        fn draw_text(&mut self, pos: TilePos, text: &str, _color: Rgba) {
            self.ops.borrow_mut().push(DrawOp::Text {
                tx: pos.tx,
                ty: pos.ty,
                text: text.to_string(),
            });
        }
        fn draw_glyph(&mut self, pos: TilePos, glyph: char, _color: Rgba) {
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

    fn make_flex_list_params(
        items_key: &str,
        columns: Vec<ColumnDef>,
        cursor: u32,
        gap: u32,
        padding: EdgeInsets,
    ) -> FlexListParams {
        FlexListParams {
            items: items_key.to_string(),
            item_layout: columns,
            padding,
            gap,
            cursor: crate::layout_engine::types::ListCursor::default(),
            selected: Some(cursor.into()),
        }
    }

    fn default_padding() -> EdgeInsets {
        EdgeInsets {
            top: 0,
            bottom: 0,
            left: 0,
            right: 0,
        }
    }

    // ── compute_column_positions ─────────────────────────────────────

    #[test]
    fn column_positions_single() {
        let cols = vec![ColumnDef {
            field: "name".into(),
            width: 10,
            align: None,
            prefix: None,
        }];
        assert_eq!(compute_column_positions(0, &cols), vec![0]);
    }

    #[test]
    fn column_positions_multiple() {
        let cols = vec![
            ColumnDef {
                field: "name".into(),
                width: 10,
                align: None,
                prefix: None,
            },
            ColumnDef {
                field: "qty".into(),
                width: 4,
                align: None,
                prefix: None,
            },
            ColumnDef {
                field: "price".into(),
                width: 6,
                align: None,
                prefix: None,
            },
        ];
        assert_eq!(compute_column_positions(0, &cols), vec![0, 10, 14]);
    }

    #[test]
    fn column_positions_with_offset() {
        let cols = vec![ColumnDef {
            field: "name".into(),
            width: 8,
            align: None,
            prefix: None,
        }];
        assert_eq!(compute_column_positions(2, &cols), vec![2]);
    }

    #[test]
    fn column_positions_empty() {
        assert_eq!(compute_column_positions(0, &[]), Vec::<u32>::new());
    }

    // ── align_text ───────────────────────────────────────────────────

    #[test]
    fn align_left() {
        let x = align_text(0, 10, "HI", &TextAlign::Left);
        assert_eq!(x, 0);
    }

    #[test]
    fn align_center() {
        let x = align_text(0, 10, "HI", &TextAlign::Center);
        // (10 - 2) / 2 = 4
        assert_eq!(x, 4);
    }

    #[test]
    fn align_right() {
        let x = align_text(0, 10, "HI", &TextAlign::Right);
        // 10 - 2 = 8
        assert_eq!(x, 8);
    }

    #[test]
    fn align_center_long_text_does_not_underflow() {
        // text is longer than or equal to width: should anchor left
        let x = align_text(0, 2, "LONG", &TextAlign::Center);
        assert_eq!(x, 0);
    }

    #[test]
    fn align_right_long_text_does_not_underflow() {
        let x = align_text(0, 2, "LONG", &TextAlign::Right);
        assert_eq!(x, 0);
    }

    // ── item_to_strings ──────────────────────────────────────────────

    #[test]
    fn item_list_to_strings() {
        let item = DataValue::List(vec![
            DataValue::Str("POTION".into()),
            DataValue::Int(3),
            DataValue::Str("¥300".into()),
        ]);
        let strings = item_to_strings(&item);
        assert_eq!(strings, vec!["POTION", "3", "¥300"]);
    }

    #[test]
    fn item_scalar_to_strings() {
        let item = DataValue::Str("ONLY".into());
        let strings = item_to_strings(&item);
        assert_eq!(strings, vec!["ONLY"]);
    }

    // ── render_flex_list ─────────────────────────────────────────────

    #[test]
    fn renders_multi_column_items() {
        let mut ctx = DataContext::new();
        ctx.set(
            "shop",
            DataValue::List(vec![
                DataValue::List(vec![
                    DataValue::Str("BALL".into()),
                    DataValue::Int(1),
                    DataValue::Str("¥200".into()),
                ]),
                DataValue::List(vec![
                    DataValue::Str("POTION".into()),
                    DataValue::Int(3),
                    DataValue::Str("¥300".into()),
                ]),
            ]),
        );

        let cols = vec![
            ColumnDef {
                field: "name".into(),
                width: 10,
                align: None,
                prefix: None,
            },
            ColumnDef {
                field: "qty".into(),
                width: 3,
                align: Some(TextAlign::Center),
                prefix: Some("x".into()),
            },
            ColumnDef {
                field: "price".into(),
                width: 5,
                align: Some(TextAlign::Right),
                prefix: None,
            },
        ];

        let params = make_flex_list_params("shop", cols, 0, 0, default_padding());
        let mut painter = MockPainter::new();

        render_flex_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        assert!(!painter.find_text("BALL").is_empty());
        assert!(!painter.find_text("x1").is_empty());
        assert!(!painter.find_text("¥200").is_empty());
        assert!(!painter.find_text("POTION").is_empty());
        assert!(!painter.find_text("x3").is_empty());
        assert!(!painter.find_text("¥300").is_empty());
    }

    #[test]
    fn cursor_on_active_row() {
        let mut ctx = DataContext::new();
        ctx.set(
            "shop",
            DataValue::List(vec![
                DataValue::List(vec![DataValue::Str("A".into()), DataValue::Int(1)]),
                DataValue::List(vec![DataValue::Str("B".into()), DataValue::Int(2)]),
            ]),
        );

        let cols = vec![
            ColumnDef {
                field: "name".into(),
                width: 5,
                align: None,
                prefix: None,
            },
            ColumnDef {
                field: "qty".into(),
                width: 3,
                align: None,
                prefix: None,
            },
        ];

        // cursor on second row (index 1)
        let params = make_flex_list_params("shop", cols, 1, 0, default_padding());
        let mut painter = MockPainter::new();

        render_flex_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        let cursors = painter.find_glyph('\u{25B6}');
        assert_eq!(cursors.len(), 1);
        // Cursor should be on row 1 (second item, since gap=0)
        let (_, cursor_ty) = cursors[0];
        assert_eq!(cursor_ty, 1);
    }

    #[test]
    fn gap_between_rows() {
        let mut ctx = DataContext::new();
        ctx.set(
            "shop",
            DataValue::List(vec![
                DataValue::List(vec![DataValue::Str("A".into())]),
                DataValue::List(vec![DataValue::Str("B".into())]),
            ]),
        );

        let cols = vec![ColumnDef {
            field: "name".into(),
            width: 5,
            align: None,
            prefix: None,
        }];

        // gap = 2 means row positions: 0, 3, 6, ...
        let params = make_flex_list_params("shop", cols, 0, 2, default_padding());
        let mut painter = MockPainter::new();

        render_flex_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        let a_texts = painter.find_text("A");
        let b_texts = painter.find_text("B");
        assert!(!a_texts.is_empty());
        assert!(!b_texts.is_empty());

        // First row "A" at ty=0, second row "B" at ty=3 (1 + gap = 3)
        let b_ty = b_texts[0].1;
        assert_eq!(b_ty, 3);
    }

    #[test]
    fn padding_applied() {
        let mut ctx = DataContext::new();
        ctx.set(
            "shop",
            DataValue::List(vec![DataValue::List(vec![DataValue::Str("ITEM".into())])]),
        );

        let cols = vec![ColumnDef {
            field: "name".into(),
            width: 5,
            align: None,
            prefix: None,
        }];

        let padding = EdgeInsets {
            top: 2,
            bottom: 0,
            left: 3,
            right: 0,
        };
        let params = make_flex_list_params("shop", cols, 0, 0, padding);
        let mut painter = MockPainter::new();

        render_flex_list(&params, 5, 0, &ctx, &Theme::default(), &mut painter);

        let item_texts = painter.find_text("ITEM");
        assert!(!item_texts.is_empty());
        // content_tx = tx(5) + padding.left(3) + cursor_width(1) = 9
        let (item_tx, item_ty, _) = &item_texts[0];
        assert_eq!(*item_tx, 9);
        // content_ty = ty(0) + padding.top(2) = 2
        assert_eq!(*item_ty, 2);
    }

    #[test]
    fn empty_items_no_render() {
        let mut ctx = DataContext::new();
        ctx.set("shop", DataValue::List(vec![]));

        let cols = vec![ColumnDef {
            field: "name".into(),
            width: 5,
            align: None,
            prefix: None,
        }];
        let params = make_flex_list_params("shop", cols, 0, 0, default_padding());
        let mut painter = MockPainter::new();

        render_flex_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);
        assert!(painter.ops().is_empty());
    }

    #[test]
    fn empty_columns_no_render() {
        let mut ctx = DataContext::new();
        ctx.set("shop", DataValue::List(vec![DataValue::Str("X".into())]));

        let params = make_flex_list_params("shop", vec![], 0, 0, default_padding());
        let mut painter = MockPainter::new();

        render_flex_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);
        assert!(painter.ops().is_empty());
    }

    #[test]
    fn missing_items_key_no_render() {
        let ctx = DataContext::new();
        let cols = vec![ColumnDef {
            field: "name".into(),
            width: 5,
            align: None,
            prefix: None,
        }];
        let params = make_flex_list_params("nonexistent", cols, 0, 0, default_padding());
        let mut painter = MockPainter::new();

        render_flex_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);
        assert!(painter.ops().is_empty());
    }

    #[test]
    fn column_alignment_center() {
        let mut ctx = DataContext::new();
        ctx.set(
            "shop",
            DataValue::List(vec![DataValue::List(vec![DataValue::Str("AB".into())])]),
        );

        let cols = vec![ColumnDef {
            field: "name".into(),
            width: 8,
            align: Some(TextAlign::Center),
            prefix: None,
        }];
        let params = make_flex_list_params("shop", cols, 0, 0, default_padding());
        let mut painter = MockPainter::new();

        render_flex_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        let texts = painter.find_text("AB");
        assert!(!texts.is_empty());
        // text "AB" (len=2) centered in width=8: offset = (8-2)/2 = 3
        // content_tx = 0 (no padding), cursor adds +1 → base=1, centered → 1+3 = 4
        let (tx, _, _) = &texts[0];
        assert_eq!(*tx, 4);
    }

    #[test]
    fn column_alignment_right() {
        let mut ctx = DataContext::new();
        ctx.set(
            "shop",
            DataValue::List(vec![DataValue::List(vec![DataValue::Str("AB".into())])]),
        );

        let cols = vec![ColumnDef {
            field: "name".into(),
            width: 8,
            align: Some(TextAlign::Right),
            prefix: None,
        }];
        let params = make_flex_list_params("shop", cols, 0, 0, default_padding());
        let mut painter = MockPainter::new();

        render_flex_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        let texts = painter.find_text("AB");
        assert!(!texts.is_empty());
        // text "AB" (len=2) right-aligned in width=8: offset = 8-2 = 6
        // content_tx=0, cursor adds +1 → base=1, right → 1+6 = 7
        let (tx, _, _) = &texts[0];
        assert_eq!(*tx, 7);
    }

    #[test]
    fn prefix_appended_to_value() {
        let mut ctx = DataContext::new();
        ctx.set(
            "shop",
            DataValue::List(vec![DataValue::List(vec![DataValue::Int(5)])]),
        );

        let cols = vec![ColumnDef {
            field: "qty".into(),
            width: 3,
            align: None,
            prefix: Some("x".into()),
        }];
        let params = make_flex_list_params("shop", cols, 0, 0, default_padding());
        let mut painter = MockPainter::new();

        render_flex_list(&params, 0, 0, &ctx, &Theme::default(), &mut painter);

        let texts = painter.find_text("x5");
        assert!(!texts.is_empty());
    }
}
