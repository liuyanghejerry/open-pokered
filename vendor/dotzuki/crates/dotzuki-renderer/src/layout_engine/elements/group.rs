//! Layout container — groups children with coordinate translation.
//!
//! A [`Group`] is a rectangular container that manages child elements,
//! translating their coordinates from group-relative to screen-absolute
//! space. It supports three layout modes:
//!
//! * **Absolute** — children use their own explicit coordinates within
//!   the group.
//! * **Horizontal** — children are laid out left-to-right with a
//!   configurable gap.
//! * **Vertical** — children are laid out top-to-bottom with a
//!   configurable gap.
//!
//! Groups can optionally apply a [`Border`] around their bounds and clip
//! child rendering to the group rect.

use crate::layout_engine::elements::border::Border;
use crate::layout_engine::types::{
    DataContext, Direction, LayoutConfig, LayoutElement, RenderContext, RenderError,
};
use dotzuki_engine::render::{Painter, TilePos, TileRect};

// ── ChildRect ────────────────────────────────────────────────────────────

/// Describes a child element's rectangle resolved for layout.
///
/// This is used when the group computes a child's actual position on
/// screen after applying the layout rule (absolute / horizontal / vertical).
#[derive(Debug, Clone, Copy)]
pub struct ChildRect {
    /// The child's computed absolute tile rectangle on screen.
    pub rect: TileRect,
    /// The child's original z-index (passed through from [`LayoutElement`]).
    pub z_index: i32,
    /// The child's visibility flag.
    pub visible: bool,
}

// ── GroupLayout ──────────────────────────────────────────────────────────

/// Layout mode for a group's children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupLayout {
    /// Children are placed at their own explicit `rect.tx` / `rect.ty`
    /// positions relative to the group's top-left corner.
    Absolute,
    /// Children are laid out left-to-right. Each child after the first
    /// is offset by the cumulative width of preceding children plus `gap`.
    Horizontal { gap: u32 },
    /// Children are laid out top-to-bottom. Each child after the first
    /// is offset by the cumulative height of preceding children plus `gap`.
    Vertical { gap: u32 },
}

impl GroupLayout {
    /// Derive a [`GroupLayout`] from a [`LayoutConfig`].
    pub fn from_config(config: &LayoutConfig) -> Self {
        match config.direction {
            Some(Direction::Horizontal) => GroupLayout::Horizontal { gap: config.gap },
            Some(Direction::Vertical) => GroupLayout::Vertical { gap: config.gap },
            None => GroupLayout::Absolute,
        }
    }

    /// Whether children are automatically positioned (horizontal or vertical).
    #[inline]
    pub fn is_auto(&self) -> bool {
        !matches!(self, GroupLayout::Absolute)
    }
}

impl Default for GroupLayout {
    fn default() -> Self {
        GroupLayout::Absolute
    }
}

// ── Group ────────────────────────────────────────────────────────────────

/// A layout container that manages children with coordinate translation.
///
/// # Examples
///
/// ```
/// use dotzuki_renderer::layout_engine::elements::group::{Group, GroupLayout};
/// use dotzuki_engine::render::TileRect;
///
/// let group = Group::new(TileRect::new(0, 0, 10, 10))
///     .with_layout(GroupLayout::Vertical { gap: 1 })
///     .with_clip(true);
/// ```
#[derive(Debug, Clone)]
pub struct Group {
    /// The group's bounding rectangle in screen-absolute tile coordinates.
    pub rect: TileRect,
    /// How children are positioned within the group.
    pub layout: GroupLayout,
    /// Whether child rendering is clipped to `rect`.
    pub clip: bool,
    /// Optional border rendered around the group.
    pub border: Option<Border>,
}

impl Group {
    /// Create a new group with absolute layout, no clip, and no border.
    #[inline]
    pub fn new(rect: TileRect) -> Self {
        Self {
            rect,
            layout: GroupLayout::Absolute,
            clip: false,
            border: None,
        }
    }

    /// Set the layout mode.
    #[inline]
    pub fn with_layout(mut self, layout: GroupLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Enable or disable clipping.
    #[inline]
    pub fn with_clip(mut self, clip: bool) -> Self {
        self.clip = clip;
        self
    }

    /// Attach a border to the group.
    #[inline]
    pub fn with_border(mut self, border: Border) -> Self {
        self.border = Some(border);
        self
    }

    // ── Coordinate translation ────────────────────────────────────────

    /// Translate a group-relative tile position to screen-absolute.
    ///
    /// Adds the group's top-left offset to the given coordinates.
    /// This is the core coordinate translation used when rendering
    /// children that specify their positions relative to the group.
    #[inline]
    pub fn to_absolute(&self, tx: u32, ty: u32) -> TilePos {
        TilePos::new(self.rect.tx + tx, self.rect.ty + ty)
    }

    /// Translate a group-relative tile rectangle to screen-absolute.
    #[inline]
    pub fn rect_to_absolute(&self, relative: TileRect) -> TileRect {
        TileRect::new(
            self.rect.tx + relative.tx,
            self.rect.ty + relative.ty,
            relative.tw,
            relative.th,
        )
    }

    /// Compute the screen-absolute rectangle for a child element.
    ///
    /// For [`GroupLayout::Absolute`], the child's own `rect.tx`/`rect.ty`
    /// are treated as offsets from the group origin.
    ///
    /// For automatic layouts ([`GroupLayout::Horizontal`] /
    /// [`GroupLayout::Vertical`]), the child's explicit `tx`/`ty` are
    /// overridden by the computed layout position; the child still
    /// controls its own `tw`/`th`.
    pub fn child_rect(&self, child: &LayoutElement, _index: usize, ctx: &DataContext) -> TileRect {
        let child_tw = child.rect.tw.unwrap_or(0);
        let child_th = child.rect.th.unwrap_or(0);

        let (rel_tx, rel_ty) = match self.layout {
            GroupLayout::Absolute => (child.rect.tx.resolve(ctx), child.rect.ty.resolve(ctx)),
            GroupLayout::Horizontal { .. } => (0, 0),
            GroupLayout::Vertical { .. } => (0, 0),
        };

        TileRect::new(
            self.rect.tx + rel_tx,
            self.rect.ty + rel_ty,
            child_tw,
            child_th,
        )
    }

    // ── Layout computation ────────────────────────────────────────────

    /// Compute the layout offset for the child at `index` from the actual
    /// child list.
    ///
    /// Returns the (x, y) offset in tiles from the group origin that this
    /// child should be placed at, based on the layout mode and the actual
    /// widths/heights of previous children.
    ///
    /// For [`GroupLayout::Absolute`], always returns `(0, 0)` — the child's
    /// own `rect.tx`/`rect.ty` are used directly.
    pub fn layout_offset(&self, index: usize, children: &[LayoutElement]) -> (u32, u32) {
        match self.layout {
            GroupLayout::Absolute => (0, 0),
            GroupLayout::Horizontal { gap } => {
                let mut x = 0u32;
                for child in children.iter().take(index) {
                    x += child.rect.tw.unwrap_or(0) + gap;
                }
                (x, 0)
            }
            GroupLayout::Vertical { gap } => {
                let mut y = 0u32;
                for child in children.iter().take(index) {
                    y += child.rect.th.unwrap_or(0) + gap;
                }
                (0, y)
            }
        }
    }

    /// Resolve all children to their screen-absolute rectangles.
    ///
    /// Returns a vector of [`ChildRect`] in the same order as `children`,
    /// each with a computed absolute position based on the layout mode.
    pub fn resolve_children(
        &self,
        children: &[LayoutElement],
        ctx: &DataContext,
    ) -> Vec<ChildRect> {
        children
            .iter()
            .enumerate()
            .map(|(i, child)| {
                let (layout_dx, layout_dy) = self.layout_offset(i, children);
                let child_tx = match self.layout {
                    GroupLayout::Absolute => child.rect.tx.resolve(ctx),
                    _ => layout_dx,
                };
                let child_ty = match self.layout {
                    GroupLayout::Absolute => child.rect.ty.resolve(ctx),
                    _ => layout_dy,
                };

                ChildRect {
                    rect: TileRect::new(
                        self.rect.tx + child_tx,
                        self.rect.ty + child_ty,
                        child.rect.tw.unwrap_or(0),
                        child.rect.th.unwrap_or(0),
                    ),
                    z_index: child.z_index,
                    visible: child.visible.eval(ctx),
                }
            })
            .collect()
    }

    // ── Rendering ──────────────────────────────────────────────────────

    /// Render the group's border (if any) into the given [`Painter`].
    ///
    /// This does **not** render children — it only draws the group's
    /// own border or background fill. Child rendering is handled by
    /// the layout engine's element dispatch.
    pub fn render_border(&self, painter: &mut dyn Painter) {
        if let Some(ref border) = self.border {
            border.render(painter);
        }
    }

    /// Render the group including border and children via the layout engine.
    ///
    /// When `clip` is enabled, pixels outside `rect` should be masked.
    /// Current implementation renders all visible children; clipping is
    /// a future enhancement for when a clip-stack is added to the painter.
    pub fn render(
        &self,
        children: &[LayoutElement],
        ctx: &DataContext,
        render_ctx: &RenderContext,
        painter: &mut dyn Painter,
        registry: &crate::layout_engine::registry::ElementRegistry,
    ) -> Result<(), RenderError> {
        // 1. Render the border
        self.render_border(painter);

        // 2. Resolve child positions
        let resolved = self.resolve_children(children, ctx);

        // 3. Sort by z_index (stable sort preserves original order for equal z)
        let mut sorted: Vec<(usize, &ChildRect)> = resolved.iter().enumerate().collect();
        sorted.sort_by_key(|(_, cr)| cr.z_index);

        // 4. Render children
        for (child_idx, child_rect) in sorted {
            if !child_rect.visible {
                continue;
            }
            let child_elem = &children[child_idx];

            // Skip children outside group rect when clipping is enabled
            if self.clip && !self.overlaps(child_rect.rect) {
                continue;
            }

            // Look up the element type in the registry
            if let Some(custom_elem) = registry.get(&child_elem.element_type) {
                custom_elem.render(child_elem, ctx, render_ctx, painter)?;
            }
        }

        Ok(())
    }

    /// Check whether a rectangle overlaps with the group's rect.
    fn overlaps(&self, r: TileRect) -> bool {
        let gx2 = self.rect.tx + self.rect.tw;
        let gy2 = self.rect.ty + self.rect.th;
        let rx2 = r.tx + r.tw;
        let ry2 = r.ty + r.th;

        r.tx < gx2 && self.rect.tx < rx2 && r.ty < gy2 && self.rect.ty < ry2
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_engine::types::{Coord, EdgeInsets, ElementRect};
    use dotzuki_engine::render::{Rgba, TileRect};

    // Helper: create a minimal LayoutElement for testing
    fn make_element(tx: u32, ty: u32, tw: u32, th: u32) -> LayoutElement {
        LayoutElement {
            id: String::new(),
            element_type: "text".to_string(),
            rect: ElementRect {
                tx: Coord::Literal(tx),
                ty: Coord::Literal(ty),
                tw: Some(tw),
                th: Some(th),
            },
            visible: crate::layout_engine::types::Visibility::Static(true),
            z_index: 0,
            params: crate::layout_engine::types::ElementParams::Custom(serde_json::Value::Null),
        }
    }

    // ── GroupLayout tests ──────────────────────────────────────────────

    #[test]
    fn layout_from_config_horizontal() {
        let config = LayoutConfig {
            direction: Some(Direction::Horizontal),
            gap: 2,
            padding: EdgeInsets::default(),
        };
        let layout = GroupLayout::from_config(&config);
        assert_eq!(layout, GroupLayout::Horizontal { gap: 2 });
    }

    #[test]
    fn layout_from_config_vertical() {
        let config = LayoutConfig {
            direction: Some(Direction::Vertical),
            gap: 1,
            padding: EdgeInsets::default(),
        };
        let layout = GroupLayout::from_config(&config);
        assert_eq!(layout, GroupLayout::Vertical { gap: 1 });
    }

    #[test]
    fn layout_from_config_absolute_when_no_direction() {
        let config = LayoutConfig {
            direction: None,
            gap: 5,
            padding: EdgeInsets::default(),
        };
        let layout = GroupLayout::from_config(&config);
        assert_eq!(layout, GroupLayout::Absolute);
    }

    #[test]
    fn is_auto_returns_false_for_absolute() {
        assert!(!GroupLayout::Absolute.is_auto());
    }

    #[test]
    fn is_auto_returns_true_for_horizontal_and_vertical() {
        assert!(GroupLayout::Horizontal { gap: 0 }.is_auto());
        assert!(GroupLayout::Vertical { gap: 0 }.is_auto());
    }

    // ── Group construction tests ───────────────────────────────────────

    #[test]
    fn group_defaults() {
        let rect = TileRect::new(5, 5, 10, 8);
        let g = Group::new(rect);
        assert_eq!(g.rect, rect);
        assert_eq!(g.layout, GroupLayout::Absolute);
        assert!(!g.clip);
        assert!(g.border.is_none());
    }

    #[test]
    fn group_builder_pattern() {
        let rect = TileRect::new(0, 0, 20, 18);
        let border = Border::new(rect, Rgba::INK_BLACK);
        let g = Group::new(rect)
            .with_layout(GroupLayout::Vertical { gap: 2 })
            .with_clip(true)
            .with_border(border);

        assert_eq!(g.layout, GroupLayout::Vertical { gap: 2 });
        assert!(g.clip);
        assert!(g.border.is_some());
    }

    // ── Coordinate translation tests ──────────────────────────────────

    #[test]
    fn to_absolute_adds_group_offset() {
        let g = Group::new(TileRect::new(3, 7, 10, 10));
        assert_eq!(g.to_absolute(0, 0), TilePos::new(3, 7));
        assert_eq!(g.to_absolute(2, 3), TilePos::new(5, 10));
        assert_eq!(g.to_absolute(9, 9), TilePos::new(12, 16));
    }

    #[test]
    fn rect_to_absolute_preserves_size() {
        let g = Group::new(TileRect::new(2, 3, 10, 10));
        let rel = TileRect::new(1, 1, 4, 3);
        let abs = g.rect_to_absolute(rel);
        assert_eq!(abs.tx, 3); // 2 + 1
        assert_eq!(abs.ty, 4); // 3 + 1
        assert_eq!(abs.tw, 4);
        assert_eq!(abs.th, 3);
    }

    // ── Layout offset tests ───────────────────────────────────────────

    #[test]
    fn absolute_layout_offset_is_zero() {
        let g = Group::new(TileRect::new(0, 0, 20, 18));
        let children = vec![make_element(0, 0, 5, 2), make_element(0, 0, 3, 2)];
        assert_eq!(g.layout_offset(0, &children), (0, 0));
        assert_eq!(g.layout_offset(1, &children), (0, 0));
    }

    #[test]
    fn horizontal_layout_offset_accumulates_widths() {
        let g =
            Group::new(TileRect::new(0, 0, 20, 18)).with_layout(GroupLayout::Horizontal { gap: 1 });
        let children = vec![
            make_element(0, 0, 5, 2), // index 0: x=0
            make_element(0, 0, 3, 2), // index 1: x=5+1=6
            make_element(0, 0, 4, 2), // index 2: x=6+3+1=10
        ];

        assert_eq!(g.layout_offset(0, &children), (0, 0));
        assert_eq!(g.layout_offset(1, &children), (6, 0));
        assert_eq!(g.layout_offset(2, &children), (10, 0));
    }

    #[test]
    fn vertical_layout_offset_accumulates_heights() {
        let g =
            Group::new(TileRect::new(0, 0, 20, 18)).with_layout(GroupLayout::Vertical { gap: 2 });
        let children = vec![
            make_element(0, 0, 10, 3), // index 0: y=0
            make_element(0, 0, 10, 5), // index 1: y=3+2=5
            make_element(0, 0, 10, 2), // index 2: y=5+5+2=12
        ];

        assert_eq!(g.layout_offset(0, &children), (0, 0));
        assert_eq!(g.layout_offset(1, &children), (0, 5));
        assert_eq!(g.layout_offset(2, &children), (0, 12));
    }

    #[test]
    fn horizontal_layout_no_gap() {
        let g =
            Group::new(TileRect::new(0, 0, 20, 18)).with_layout(GroupLayout::Horizontal { gap: 0 });
        let children = vec![make_element(0, 0, 3, 1), make_element(0, 0, 7, 1)];
        assert_eq!(g.layout_offset(1, &children), (3, 0));
    }

    // ── Resolve children tests ────────────────────────────────────────

    #[test]
    fn resolve_absolute_children_preserves_positions() {
        let g = Group::new(TileRect::new(2, 3, 10, 10));
        let children = vec![make_element(0, 0, 4, 2), make_element(1, 1, 6, 3)];
        let ctx = DataContext::new();

        let resolved = g.resolve_children(&children, &ctx);
        assert_eq!(resolved.len(), 2);

        // child 0 at (2+0, 3+0)
        assert_eq!(resolved[0].rect, TileRect::new(2, 3, 4, 2));
        // child 1 at (2+1, 3+1)
        assert_eq!(resolved[1].rect, TileRect::new(3, 4, 6, 3));
    }

    #[test]
    fn resolve_horizontal_children() {
        let g =
            Group::new(TileRect::new(1, 1, 20, 10)).with_layout(GroupLayout::Horizontal { gap: 1 });
        let children = vec![
            make_element(0, 0, 5, 3),
            make_element(0, 0, 4, 3),
            make_element(0, 0, 6, 3),
        ];
        let ctx = DataContext::new();

        let resolved = g.resolve_children(&children, &ctx);

        // child 0 at (1+0, 1+0) = (1, 1)
        assert_eq!(resolved[0].rect, TileRect::new(1, 1, 5, 3));
        // child 1 at (1+6, 1+0) = (7, 1)
        assert_eq!(resolved[1].rect, TileRect::new(7, 1, 4, 3));
        // child 2 at (1+11, 1+0) = (12, 1)
        assert_eq!(resolved[2].rect, TileRect::new(12, 1, 6, 3));
    }

    #[test]
    fn resolve_vertical_children() {
        let g =
            Group::new(TileRect::new(0, 2, 10, 15)).with_layout(GroupLayout::Vertical { gap: 0 });
        let children = vec![make_element(0, 0, 10, 3), make_element(0, 0, 10, 4)];
        let ctx = DataContext::new();

        let resolved = g.resolve_children(&children, &ctx);
        // child 0 at (0, 2+0) = (0, 2)
        assert_eq!(resolved[0].rect, TileRect::new(0, 2, 10, 3));
        // child 1 at (0, 2+3) = (0, 5)
        assert_eq!(resolved[1].rect, TileRect::new(0, 5, 10, 4));
    }

    #[test]
    fn resolve_passes_z_index_and_visibility() {
        let g = Group::new(TileRect::new(0, 0, 10, 10));
        let mut c0 = make_element(0, 0, 3, 3);
        c0.z_index = 5;
        c0.visible = crate::layout_engine::types::Visibility::Static(false);
        let c1 = make_element(1, 1, 3, 3);
        let ctx = DataContext::new();

        let resolved = g.resolve_children(&[c0, c1], &ctx);
        assert_eq!(resolved[0].z_index, 5);
        assert!(!resolved[0].visible);
        assert_eq!(resolved[1].z_index, 0);
        assert!(resolved[1].visible);
    }

    // ── Overlap test ─────────────────────────────────────────────────

    #[test]
    fn overlaps_detects_intersection() {
        let g = Group::new(TileRect::new(0, 0, 10, 10));

        // Fully inside
        assert!(g.overlaps(TileRect::new(2, 2, 4, 4)));

        // Partially overlapping
        assert!(g.overlaps(TileRect::new(8, 8, 6, 6)));

        // Left edge
        assert!(g.overlaps(TileRect::new(0, 1, 1, 1)));

        // Top edge
        assert!(g.overlaps(TileRect::new(1, 0, 1, 1)));
    }

    #[test]
    fn overlaps_detects_non_intersection() {
        let g = Group::new(TileRect::new(0, 0, 10, 10));

        // Completely to the right
        assert!(!g.overlaps(TileRect::new(10, 0, 1, 1)));

        // Completely below
        assert!(!g.overlaps(TileRect::new(0, 10, 1, 1)));

        // Completely to the left (wraps would be negative, so use 5,5 with tw=5 → 5+5=10)
        // Negative tx is not possible with u32, test far-away positions
        assert!(!g.overlaps(TileRect::new(20, 20, 1, 1)));
    }

    // ── Same position for multiple children ───────────────────────────

    #[test]
    fn resolve_single_child_at_origin() {
        let g = Group::new(TileRect::new(0, 0, 20, 18));
        let children = vec![make_element(0, 0, 20, 18)];
        let ctx = DataContext::new();
        let resolved = g.resolve_children(&children, &ctx);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].rect, TileRect::new(0, 0, 20, 18));
    }

    #[test]
    fn resolve_empty_children() {
        let g = Group::new(TileRect::new(0, 0, 10, 10));
        let resolved = g.resolve_children(&[], &DataContext::new());
        assert!(resolved.is_empty());
    }

    // ── child_rect method tests ────────────────────────────────────────

    #[test]
    fn child_rect_absolute_mode() {
        let g = Group::new(TileRect::new(5, 5, 20, 18));
        let child = make_element(2, 3, 8, 4);
        let ctx = DataContext::new();

        let cr = g.child_rect(&child, 0, &ctx);
        assert_eq!(cr.tx, 7); // 5 + 2
        assert_eq!(cr.ty, 8); // 5 + 3
        assert_eq!(cr.tw, 8);
        assert_eq!(cr.th, 4);
    }

    #[test]
    fn child_rect_horizontal_mode() {
        let g =
            Group::new(TileRect::new(1, 1, 20, 10)).with_layout(GroupLayout::Horizontal { gap: 1 });
        let child = make_element(99, 99, 4, 3); // explicit tx/ty ignored
        let ctx = DataContext::new();
        let cr = g.child_rect(&child, 0, &ctx);
        assert_eq!(cr.tx, 1); // 1 + 0 (first child)
        assert_eq!(cr.ty, 1); // 1 + 0
        assert_eq!(cr.tw, 4);
        assert_eq!(cr.th, 3);
    }

    #[test]
    fn child_rect_vertical_mode() {
        let g =
            Group::new(TileRect::new(2, 2, 10, 20)).with_layout(GroupLayout::Vertical { gap: 2 });
        let child = make_element(42, 42, 6, 5);
        let ctx = DataContext::new();
        let cr = g.child_rect(&child, 0, &ctx);
        assert_eq!(cr.tx, 2); // 2 + 0
        assert_eq!(cr.ty, 2); // 2 + 0
        assert_eq!(cr.tw, 6);
        assert_eq!(cr.th, 5);
    }
}
