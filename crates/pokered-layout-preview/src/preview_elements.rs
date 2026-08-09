//! Preview implementations of pokered's `custom:*` layout elements.
//!
//! The layout-editor preview renders compiled `.gui` layouts without the
//! game's crates (pulling pokered-ui into this WASM target would drag in
//! pokered-core/-data). Elements registered here mirror the canonical
//! implementations so the preview looks like the game; keep them in sync:
//!
//! - `custom:hp_bar` → `pokered_ui::custom_elements::HpBarElement`

use jrpg_engine::render::{Painter, Rgba};
use jrpg_renderer::layout_engine::registry::{
    ComponentSchema, CustomElement, ElementRegistry, PropSpec, PropType,
};
use jrpg_renderer::layout_engine::types::{
    Coord, DataContext, ElementParams, LayoutElement, RenderContext, RenderError,
};

const TILE: u32 = 8;

const HP_FULL: Rgba = Rgba::rgb(0x20, 0x20, 0x20);
const HP_CAUTION: Rgba = Rgba::rgb(0x70, 0x70, 0x70);
const HP_CRITICAL: Rgba = Rgba::rgb(0x40, 0x40, 0x40);

#[derive(Debug, serde::Deserialize)]
struct HpBarParams {
    current: Coord,
    max: Coord,
}

/// Preview twin of pokered's `custom:hp_bar`.
#[derive(Debug)]
struct HpBarPreview;

impl CustomElement for HpBarPreview {
    fn element_type(&self) -> &'static str {
        "custom:hp_bar"
    }

    fn schema(&self) -> ComponentSchema {
        ComponentSchema::new(vec![
            PropSpec::required("current", PropType::Expr),
            PropSpec::required("max", PropType::Expr),
        ])
    }

    fn render(
        &self,
        element: &LayoutElement,
        ctx: &DataContext,
        _render_ctx: &RenderContext,
        painter: &mut dyn Painter,
    ) -> Result<(), RenderError> {
        let ElementParams::Custom(ref value) = element.params else {
            return Err(RenderError::InvalidLayout);
        };
        let params: HpBarParams =
            serde_json::from_value(value.clone()).map_err(|_| RenderError::InvalidLayout)?;

        let tx = element.rect.tx.resolve(ctx);
        let ty = element.rect.ty.resolve(ctx);
        let width_tiles = element.rect.tw.unwrap_or(6);
        let current = params.current.resolve(ctx);
        let max = params.max.resolve(ctx);

        let bar_x = tx * TILE;
        let bar_y = ty * TILE + 2;
        let bar_w = width_tiles * TILE;
        const BAR_H: u32 = 4;

        painter.draw_pixel_rect(bar_x, bar_y, bar_w, BAR_H, Rgba::INK_BLACK);
        painter.draw_pixel_rect(bar_x + 1, bar_y + 1, bar_w - 2, BAR_H - 2, Rgba::INK_WHITE);

        if max == 0 {
            return Ok(());
        }
        let inner_w = bar_w - 2;
        let fill = (current * inner_w) / max;
        if fill == 0 {
            return Ok(());
        }
        let color = if current * 2 > max {
            HP_FULL
        } else if current * 5 > max {
            HP_CAUTION
        } else {
            HP_CRITICAL
        };
        painter.draw_pixel_rect(bar_x + 1, bar_y + 1, fill.min(inner_w), BAR_H - 2, color);
        Ok(())
    }
}

/// Registry with all preview elements, for the editor's render path.
pub fn preview_registry() -> ElementRegistry {
    let mut registry = ElementRegistry::new();
    registry.register(Box::new(HpBarPreview));
    registry
}
