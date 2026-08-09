//! Pokémon-specific layout elements, registered with the v2 layout engine as
//! `custom:*` types.
//!
//! The engine ships only game-agnostic primitives; anything with Gen-I
//! semantics lives here. Each element has a build-time schema twin in
//! `pokered-data/ui_layouts/components.gui` — the DSL compiler validates
//! `.gui` use sites against that declaration, and [`element_registry`]'s
//! schemas re-validate loaded layouts at runtime
//! (`ElementRegistry::validate_layout`). The `stats_layout_validates` test
//! guards the two against drifting apart.

use std::sync::OnceLock;

use dotzuki_engine::render::{Painter, Rgba};
use dotzuki_renderer::layout_engine::registry::{
    ComponentSchema, CustomElement, ElementRegistry, PropSpec, PropType,
};
use dotzuki_renderer::layout_engine::types::{
    Coord, DataContext, ElementParams, LayoutElement, RenderContext, RenderError,
};

const TILE: u32 = 8;

// Gen-I HP bar shades — original `GetHealthBarColor` picks them at >50% /
// >20% / below.
const HP_FULL: Rgba = Rgba::rgb(0x20, 0x20, 0x20);
const HP_CAUTION: Rgba = Rgba::rgb(0x70, 0x70, 0x70);
const HP_CRITICAL: Rgba = Rgba::rgb(0x40, 0x40, 0x40);

/// Params of [`HpBarElement`], deserialized from the element's JSON props.
/// Mirrors the `component hp_bar` declaration in `components.gui`.
#[derive(Debug, serde::Deserialize)]
struct HpBarParams {
    current: Coord,
    max: Coord,
}

/// `custom:hp_bar` — the Game Boy HP bar.
///
/// Position = element rect (tiles); width = `rect.tw` (default 6); 4px tall,
/// offset +2px to vertically centre in the tile row. Tri-color fill at the
/// original `GetHealthBarColor` thresholds.
#[derive(Debug)]
pub struct HpBarElement;

impl CustomElement for HpBarElement {
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

/// The registry of pokered's custom elements, built once and shared by every
/// v2 render and load-time validation.
pub fn element_registry() -> &'static ElementRegistry {
    static REGISTRY: OnceLock<ElementRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut registry = ElementRegistry::new();
        registry.register(Box::new(HpBarElement));
        registry
    })
}
