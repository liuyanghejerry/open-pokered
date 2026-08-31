use pokered_data::ui_layout::schema::PokedexDefaultLayout;
use pokered_data::TILE_SIZE_PX;

use crate::engine::{InkColor, Painter, Ui};

/// Display width of a char in half-width tiles: CJK glyphs render full-width
/// (2 tiles) in the Fusion Pixel font, everything else 1. Range-based mirror
/// of the renderer's glyph-table classification (same convention as the
/// battle-text wrapping in core).
fn char_tile_width(c: char) -> usize {
    let cp = c as u32;
    let wide = (0x1100..=0x115F).contains(&cp)
        || (0x2010..=0x2027).contains(&cp)
        || (0x2E80..=0xA4CF).contains(&cp)
        || (0xAC00..=0xD7A3).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0xFE30..=0xFE4F).contains(&cp)
        || (0xFF00..=0xFF60).contains(&cp)
        || (0xFFE0..=0xFFE6).contains(&cp)
        || (0x20000..=0x3FFFD).contains(&cp);
    usize::from(wide) + 1
}

/// Interior x for the category label: the asm prints it at (8,3); wide CJK
/// categories (e.g. 毛毛虫宝可梦) shift left just enough to end at the
/// interior's right edge (18 tiles) instead of clipping.
fn category_x(category: &str) -> u32 {
    let w: usize = category.chars().map(char_tile_width).sum();
    if 8 + w <= 18 {
        8
    } else {
        (18 - w).max(0) as u32
    }
}

/// Borrowed view of a Pokédex entry consumed by [`draw`]. The native renderer
/// builds this from its internal `DexEntry`; the wasm preview can supply mock
/// data without depending on `pokered-app` internals.
pub struct PokedexEntryView<'a> {
    pub display_name: &'a str,
    pub category: &'a str,
    pub dex_num: u16,
    pub height_ft: u8,
    pub height_in: u8,
    pub weight_lb: &'a str,
    pub description: &'a [&'a str],
    /// Whether the player owns the Pokémon. Seen-but-not-owned entries print
    /// "HT  ?′??″ / WT   ???lb" and no description
    /// (`ShowPokedexDataInternal`, engine/menus/pokedex.asm:513-571).
    pub owned: bool,
}

/// Draws the Pokédex entry overlay (border, static labels, dynamic stats,
/// description page slice, and conditional page-down arrow) at the layout
/// specified in `pokedex.json`.
///
/// The Pokémon sprite is intentionally NOT painted here — `pokered-ui`'s
/// `Painter` trait does not expose tileset blitting, so the caller is
/// responsible for blitting the sprite into the reserved 7×7 tile area at
/// (1, 1) AFTER invoking this function: the frame's interior is filled white,
/// which would erase a sprite blitted beforehand (the sprite's white pixels
/// are a no-op over the white interior, so painting it last is safe).
pub fn draw<P: Painter>(entry: &PokedexEntryView<'_>, page: usize, layout: &PokedexDefaultLayout, ui: &mut Ui<P>) -> usize {
    let lines_per_page = 3usize;
    // Seen-but-not-owned entries print no flavor text (the original jumps to
    // the button wait before the description).
    let description: &[&str] = if entry.owned { entry.description } else { &[] };
    let total_pages = description.len().div_ceil(lines_per_page).max(1);
    let page = page.min(total_pages.saturating_sub(1));

    ui.text_box(layout.frame.rect, layout.frame.color, true, |frame| {
        for label in layout.frame.labels.iter() {
            frame.label(label.tx, label.ty, &label.text, label.color);
        }

        // Dynamic header fields. Coordinates are box-interior (origin at frame.tx+1,
        // frame.ty+1 == 1,1), so a JSON-relative tile (9,2) maps to frame-relative (8,1).
        // We hardcode (interior) coordinates that mirror the legacy absolute (9,2)/(9,4)
        // /(2,8)/(11,8)/(12,6)/(15,6)/(17,6) positions documented in the JSON's labels list
        // for HT/WT and in the legacy ShowPokedexDataInternal layout for everything else.
        frame.label(8, 1, entry.display_name, InkColor::Black);
        frame.label(category_x(entry.category), 3, entry.category, InkColor::Black);

        if entry.owned {
            let feet_str = format!("{}", entry.height_ft);
            frame.label(11, 5, &feet_str, InkColor::Black);
            frame.label(11 + feet_str.len() as u32, 5, "'", InkColor::Black);
            let inches_str = format!("{:02}", entry.height_in);
            frame.label(14, 5, &inches_str, InkColor::Black);
            frame.label(16, 5, "\"", InkColor::Black);

            let wt_str = format!("{}lb", entry.weight_lb);
            frame.label(10, 7, &wt_str, InkColor::Black);
        } else {
            // HeightWeightText placeholders for unowned entries.
            frame.label(11, 5, "?'??\"", InkColor::Black);
            frame.label(10, 7, "???lb", InkColor::Black);
        }

        let num_line = format!("No.{:03}", entry.dex_num);
        frame.label(1, 7, &num_line, InkColor::Black);

        // Horizontal separator: 2-pixel-tall ink line spanning the interior at
        // pixel-row 75-76 (tile row 9 + 3..5px). The frame interior origin is
        // (TILE_SIZE, TILE_SIZE), so the relative pixel y is 9*TILE_SIZE + 3 - TILE_SIZE
        // = 8*TILE_SIZE + 3. Width spans interior cols 0..18 (18 tiles).
        let interior_w_px = 18 * TILE_SIZE_PX;
        let sep_y_px = 8 * TILE_SIZE_PX + 3;
        frame.pixel_rect(0, sep_y_px, interior_w_px, 2, InkColor::Black);

        // Description body: 3 lines per page, 2-tile vertical spacing.
        // Legacy absolute rows 11/13/15 → interior rows 10/12/14.
        let start = page * lines_per_page;
        let end = description.len().min(start + lines_per_page);
        for (i, line) in description[start..end].iter().enumerate() {
            frame.label(0, 10 + (i as u32) * 2, line, InkColor::Black);
        }

        if page + 1 < total_pages {
            let cursor = &layout.cursor;
            let rel_tx = cursor.tx.saturating_sub(layout.frame.rect.tx + 1);
            let rel_ty = cursor.base_ty.saturating_sub(layout.frame.rect.ty + 1);
            frame.cursor_glyph_at(rel_tx, rel_ty, cursor.glyph, cursor.color);
        }
    });

    total_pages
}

#[cfg(test)]
mod tests {
    use super::category_x;

    #[test]
    fn category_stays_at_asm_position_unless_too_wide() {
        // Short EN categories keep the original (8,3) label position.
        assert_eq!(category_x("SEED"), 8);
        assert_eq!(category_x("POISON BEE"), 8); // 10 tiles: exactly fits
        // Wide CJK categories shift left to end at the interior's right edge.
        assert_eq!(category_x("毛毛虫宝可梦"), 6); // 12 tiles
        assert_eq!(category_x("百万吨宝可梦"), 6); // 12 tiles
    }
}
