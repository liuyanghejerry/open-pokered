use pokered_core::game_state::Lang;
use pokered_core::start_menu::StartMenuState;
use pokered_data::lang_data::ui_label;
use pokered_data::ui_layout::schema::{Justify, SizeMode, StartDefaultLayout};

use crate::engine::{InkColor, Painter, TileRect, Ui};

pub fn draw<P: Painter>(state: &StartMenuState, player_name: &str, layout: &StartDefaultLayout, ui: &mut Ui<P>, lang: Lang) {
    let labels = state.item_labels(player_name);
    let num_items = labels.len() as u32;
    if num_items == 0 {
        return;
    }
    let flex = &layout.menu;

    let localized: Vec<String> = labels
        .iter()
        .map(|l| match l.as_str() {
            s if s == player_name => s.to_string(),
            s => ui_label(s, lang == Lang::Zh).to_string(),
        })
        .collect();
    let content_w = localized.iter().map(|l| l.chars().count() as u32).max().unwrap_or(1);
    let eff_w = match flex.width_mode {
        SizeMode::Fixed => flex.rect.tw,
        SizeMode::Auto => clamp(
            content_w + flex.padding.left + flex.padding.right + 2,
            flex.min_width,
            flex.max_width,
        ),
    };
    let content_h = num_items + num_items.saturating_sub(1) * flex.gap;
    let eff_h = match flex.height_mode {
        SizeMode::Fixed => flex.rect.th,
        SizeMode::Auto => clamp(
            content_h + flex.padding.top + flex.padding.bottom + 2,
            flex.min_height,
            flex.max_height,
        ),
    };
    let rect = TileRect::new(flex.rect.tx, flex.rect.ty, eff_w, eff_h);

    let start_y = match flex.justify {
        Justify::Start => flex.padding.top,
        Justify::Center => {
            flex.padding.top + (eff_h.saturating_sub(flex.padding.top + flex.padding.bottom).saturating_sub(content_h)) / 2
        }
        Justify::End => eff_h.saturating_sub(flex.padding.bottom + content_h),
    };

    ui.text_box(rect, flex.color, true, |frame| {
        for (i, label) in localized.iter().enumerate() {
            let y = start_y + i as u32 * (1 + flex.gap);
            frame.label(1, y, label, InkColor::Black);
        }
        if let Some(cursor) = &flex.cursor {
            let cur_y = start_y + state.cursor() as u32 * (1 + flex.gap);
            frame.cursor_glyph_at(0, cur_y, cursor.glyph, cursor.color);
        }
    });
}

fn clamp(val: u32, min: Option<u32>, max: Option<u32>) -> u32 {
    let v = min.map_or(val, |m| val.max(m));
    max.map_or(v, |m| v.min(m))
}
