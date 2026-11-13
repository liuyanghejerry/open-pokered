use crate::alloc_prelude::*;
use pokered_core::game_state::Lang;
use pokered_core::start_menu::StartMenuState;
use pokered_data::lang_data::ui_label;
use pokered_data::ui_layout::schema::{Justify, SizeMode, StartDefaultLayout};

use crate::engine::{InkColor, Painter, Rgba, TilePos, TileRect, Ui};

pub fn draw<P: Painter>(state: &StartMenuState, player_name: &str, layout: &StartDefaultLayout, ui: &mut Ui<P>, lang: Lang) {
    let (border_visible, published_rows) = state.entry_visual();
    if !border_visible { return; }
    // PrintSafariZoneSteps (player_state.asm:219-255): inside the Safari Zone
    // a small box at (0,0) shows "NNN/500" and "BALL×× NN" before the menu.
    if let Some(info) = state.safari_info.filter(|_| published_rows != 0) {
        // Interior 7×3 plus its borders (player_state.asm:225): the labels
        // land at screen (1,1) and (1,3). The earlier 7×4 TOTAL box had only
        // two interior rows, pushing the ball row onto the bottom border.
        ui.text_box(safari_rect(), InkColor::Black, true, |frame| {
            frame.label(0, 0, &format!("{:03}/500", info.steps), InkColor::Black);
            frame.label(0, 2, &format!("BALL×{:02}", info.balls), InkColor::Black);
        });
    }

    let localized = menu_labels(state, player_name, lang);
    let num_items = localized.len() as u32;
    if num_items == 0 { return; }
    let flex = &layout.menu;
    let rect = menu_rect(&localized, layout);
    let eff_h = rect.th;
    let content_h = num_items + num_items.saturating_sub(1) * flex.gap;

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
    // AutoBGTransfer publishes DrawStartMenu in three six-row bands while
    // HandleMenuInput waits Delay3. Erase unpublished interior rows without
    // allocating another framebuffer; the earlier border remains visible.
    let top = ((rect.ty + 1) * 8).max(published_rows);
    let bottom = (rect.ty + rect.th.saturating_sub(1)) * 8;
    if top < bottom {
        ui.painter().draw_pixel_rect((rect.tx + 1) * 8, top, rect.tw.saturating_sub(2) * 8, bottom - top, Rgba::INK_WHITE);
    }
}

fn safari_rect() -> TileRect { TileRect::new(0, 0, 9, 5) }

fn menu_labels(state: &StartMenuState, player_name: &str, lang: Lang) -> Vec<String> {
    state.item_labels(player_name).iter().map(|label| {
        let label = label.as_str();
        if label == player_name { label.to_string() }
        else { ui_label(label, lang == Lang::Zh).to_string() }
    }).collect()
}

fn menu_rect(localized: &[String], layout: &StartDefaultLayout) -> TileRect {
    let flex = &layout.menu;
    let num_items = localized.len() as u32;
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
    TileRect::new(flex.rect.tx, flex.rect.ty, eff_w, eff_h)

}

/// Logical opaque UI cells used by UpdateSprites before any AutoBG publication.
/// Reuse the same resolved menu geometry as drawing, including localization.
pub fn sprite_occlusion_tiles(state: &StartMenuState, player_name: &str,
    layout: &StartDefaultLayout, lang: Lang) -> [bool; 360] {
    let mut covered = [false; 360];
    let mut mark = |rect: TileRect| {
        for y in rect.ty..(rect.ty + rect.th).min(18) {
            for x in rect.tx..(rect.tx + rect.tw).min(20) {
                covered[(y * 20 + x) as usize] = true;
            }
        }
    };
    let labels = menu_labels(state, player_name, lang);
    if !labels.is_empty() { mark(menu_rect(&labels, layout)); }
    if state.safari_info.is_some() { mark(safari_rect()); }
    covered
}

/// Repaint only the changed cursor cells of an already-rendered START menu.
pub fn redraw_cursor<P: Painter>(
    item_count: usize,
    previous_cursor: usize,
    current_cursor: usize,
    layout: &StartDefaultLayout,
    painter: &mut P,
) {
    if item_count == 0 {
        return;
    }
    let flex = &layout.menu;
    let num_items = item_count as u32;
    let content_h = num_items + num_items.saturating_sub(1) * flex.gap;
    let eff_h = match flex.height_mode {
        SizeMode::Fixed => flex.rect.th,
        SizeMode::Auto => clamp(
            content_h + flex.padding.top + flex.padding.bottom + 2,
            flex.min_height,
            flex.max_height,
        ),
    };
    let start_y = match flex.justify {
        Justify::Start => flex.padding.top,
        Justify::Center => {
            flex.padding.top
                + (eff_h
                    .saturating_sub(flex.padding.top + flex.padding.bottom)
                    .saturating_sub(content_h))
                    / 2
        }
        Justify::End => eff_h.saturating_sub(flex.padding.bottom + content_h),
    };
    let position = |cursor: usize| {
        TilePos::new(
            flex.rect.tx + 1,
            flex.rect.ty + 1 + start_y + cursor as u32 * (1 + flex.gap),
        )
    };
    let old = position(previous_cursor);
    painter.draw_pixel_rect(old.tx * 8, old.ty * 8, 8, 9, Rgba::INK_WHITE);
    if let Some(cursor) = &flex.cursor {
        painter.draw_glyph(
            position(current_cursor),
            cursor.glyph,
            cursor.color.into(),
        );
    }
}

/// Cursor ink region for a START-menu row.
pub fn cursor_damage(cursor: usize, layout: &StartDefaultLayout) -> crate::DamageRect {
    crate::DamageRect::cursor(TilePos::new(
        layout.menu.rect.tx + 1,
        layout.menu.rect.ty + 1 + layout.menu.padding.top
            + cursor as u32 * (1 + layout.menu.gap),
    ))
}

fn clamp(val: u32, min: Option<u32>, max: Option<u32>) -> u32 {
    let v = min.map_or(val, |m| val.max(m));
    max.map_or(v, |m| v.min(m))
}
