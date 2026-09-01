//! Pokédex CONTENTS list + side menu + entry + AREA views
//! (engine/menus/pokedex.asm, engine/items/town_map.asm `LoadTownMap_Nest`),
//! plus the shared entry renderer used by both the list screen and the
//! overworld's script-driven entry overlay.

use pokered_core::pokedex_screen::{
    PokedexScreenMode, PokedexScreenState, LIST_ROWS,
};
use pokered_data::lang_data::ui_label;
use pokered_data::map_names::map_name_for_map;
use pokered_data::maps::MapId;
use pokered_data::pokedex::PokedexEntry;
use pokered_data::species::Species;
use pokered_data::town_map_data::{decode_town_map_tilemap, town_map_position, TOWN_MAP_WIDTH};
use pokered_data::ui_layout::schema::POKEDEX_DEFAULT_LAYOUT;
use pokered_renderer::embedded_font::{draw_text, fill_tile};
use pokered_renderer::palette::{Palette, GRAYSCALE_PALETTE};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::Ui;

use super::{blit_single_tile, blit_single_tile_flipped, draw_text_box, species_to_sprite_name};

/// Draw the full Pokédex screen (list, side menu, entry or area, per the
/// state machine). `current_map` is the player's location — the AREA page
/// marks it like the original's player sprite on the town map.
pub fn draw_pokedex_screen(
    state: &PokedexScreenState,
    current_map: MapId,
    is_zh: bool,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    match state.mode() {
        PokedexScreenMode::List => draw_dex_list(state, is_zh, fb),
        PokedexScreenMode::SideMenu => draw_dex_list(state, is_zh, fb),
        PokedexScreenMode::Entry => {
            let sp = state.cursor_species();
            let owned = state.is_owned(state.cursor());
            draw_entry_for_species(sp, state.entry_page(), owned, is_zh, res, fb);
        }
        PokedexScreenMode::Area => draw_dex_area(state, current_map, is_zh, res, fb),
    }
}

/// The scrolling CONTENTS list (`HandlePokedexListMenu`):
/// "001 <ball>BULBASAUR" rows — ball mark only for owned, "----------" for
/// unseen — with SEEN/OWN totals and the always-visible DATA/CRY/AREA/QUIT
/// side-menu items on the right (`PokedexMenuItemsText`, placed at (16,10)).
/// In [`PokedexScreenMode::SideMenu`] an arrow marks the selected item.
fn draw_dex_list(state: &PokedexScreenState, is_zh: bool, fb: &mut FrameBuffer) {
    fb.clear(Rgba::WHITE);
    let fg = Rgba::BLACK;
    let t = TILE_SIZE;

    // Right column divider lines (asm: horizontal at (15,8), vertical at
    // (14,1..9) — drawn first so long names overwrite them, as in the asm).
    for col in 15..20 {
        fill_tile(col * t, 8 * t, fg, fb);
    }
    for row in 1..10 {
        fill_tile(14 * t, row * t, fg, fb);
    }

    draw_text(if is_zh { "内容" } else { "CONTENTS" }, t, t, fg, fb);

    let scroll = state.scroll_offset();
    for row in 0..LIST_ROWS {
        let n = scroll + 1 + row;
        if n > state.max_seen() {
            break;
        }
        let y = (3 + row * 2) as u32 * t;
        draw_text(&format!("{:03}", n), t, y, fg, fb);
        let name_x = t * 5;
        if state.is_seen(n) {
            if state.is_owned(n) {
                draw_pokeball_mark(t * 4, y, fb);
            }
            let name = if is_zh {
                pokered_data::lang_data::species_name(Species::from_index_id(n as u8), true)
                    .to_string()
            } else {
                pokered_data::lang_data::species_name(Species::from_index_id(n as u8), false)
                    .to_uppercase()
            };
            draw_text(&name, name_x, y, fg, fb);
        } else {
            draw_text("----------", name_x, y, fg, fb);
        }
    }

    // Cursor arrow on the selected row.
    let sel_row = state.cursor() - 1 - scroll;
    draw_text("▶", 0, (3 + sel_row * 2) as u32 * t, fg, fb);

    // Right column: SEEN/OWN totals (asm prints at hlcoord 16,2-6).
    draw_text(ui_label("SEEN", is_zh), 16 * t, 2 * t, fg, fb);
    draw_text(&format!("{:3}", state.seen_count()), 16 * t, 3 * t, fg, fb);
    draw_text(ui_label("OWN", is_zh), 16 * t, 5 * t, fg, fb);
    draw_text(&format!("{:3}", state.owned_count()), 16 * t, 6 * t, fg, fb);

    // Side menu items (PokedexMenuItemsText): DATA/CRY/AREA/QUIT at (16,10..13).
    let items = ["DATA", "CRY", "AREA", "QUIT"];
    for (i, item) in items.iter().enumerate() {
        draw_text(ui_label(item, is_zh), 16 * t, (10 + i) as u32 * t, fg, fb);
    }

    // The side-menu cursor arrow, at (15, 10+item) (wTopMenuItemX=15, Y=10).
    if state.mode() == PokedexScreenMode::SideMenu {
        draw_text("▶", 15 * t, (10 + state.side_menu_cursor() as u32) * t, fg, fb);
    }
}

/// The AREA page (`LoadTownMap_Nest` in engine/items/town_map.asm): the Kanto
/// town map with a nest-icon marker on every map whose grass/water tables
/// contain the species (`DisplayWildLocations`), the player's location marker,
/// and the header "<NAME>'s NEST". When the species is found nowhere (or only
/// in Cerulean Cave, whose nest the original skips) the page shows
/// "AREA UNKNOWN" in a box instead of the markers.
///
/// The town map's RLE tilemap includes its own border tiles, so — like the
/// asm's `LoadTownMap`, which draws `TextBoxBorder` and then overwrites the
/// same area with the RLE tiles — no separate frame is drawn.
pub fn draw_dex_area(
    state: &PokedexScreenState,
    current_map: MapId,
    is_zh: bool,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    fb.clear(Rgba::WHITE);
    let fg = Rgba::BLACK;
    let t = TILE_SIZE;

    let areas = state.area_maps();
    let name = if is_zh {
        pokered_data::lang_data::species_name(state.cursor_species(), true).to_string()
    } else {
        pokered_data::lang_data::species_name(state.cursor_species(), false).to_uppercase()
    };

    if let Some(ref mut rm) = res {
        // 1. The 20×18 town map (border baked into the RLE tilemap).
        if let Ok(sheet) = rm.load_town_map("town_map") {
            let ts = sheet.tileset.clone();
            for (i, &tile) in decode_town_map_tilemap().iter().enumerate() {
                let tx = (i % TOWN_MAP_WIDTH) as u32;
                let ty = (i / TOWN_MAP_WIDTH) as u32;
                blit_single_tile(fb, &ts, tile as usize, tx * t, ty * t, &GRAYSCALE_PALETTE);
            }
        }

        if areas.is_empty() {
            // DisplayWildLocations: no nest icons → "AREA UNKNOWN" text box at
            // (1,7), 2 rows × 15 cols; text at (2,9).
            draw_text_box(fb, t, 7 * t, 15, 2, fg);
            let label = ui_label("AREA UNKNOWN", is_zh);
            draw_text(&format!(" {label}"), 2 * t, 9 * t, fg, fb);
        } else {
            // 2. Nest icons on every habitat (MonNestIcon sprite, 8×8).
            if let Ok(nest) = rm.load_town_map("mon_nest_icon") {
                let nts = nest.tileset.clone();
                for map in &areas {
                    if let Some((x, y, _)) = town_map_position(*map) {
                        blit_single_tile(
                            fb,
                            &nts,
                            0,
                            (x as u32) * t,
                            (y as u32) * t,
                            &GRAYSCALE_PALETTE,
                        );
                    }
                }
            }
            // 3. Player marker at the current map (DrawPlayerOrBirdSprite with
            // OAM base tile $0 — the PLAYER sprite, 16×16, centered on the
            // landmark like the bird: OAM coords x*8+24/y*8+24 minus the 4px
            // 16×16 offset → top-left at x*8+4, y*8+4). gfx/player/red.png
            // is the battle back sprite — the OVERWORLD sheet is
            // gfx/sprites/red.png (16×96); its Down-standing frame (tiles
            // 0-3) is the town-map player sprite.
            if let Ok(player) = rm.load_sprite("red") {
                let pts = player.tileset.clone();
                let player_pal = Palette::new(&[
                    Rgba::TRANSPARENT,
                    GRAYSCALE_PALETTE.colors[1],
                    GRAYSCALE_PALETTE.colors[2],
                    GRAYSCALE_PALETTE.colors[3],
                ]);
                if let Some((x, y, _)) = town_map_position(current_map) {
                    let bx = (x as u32) * t + t / 2;
                    let by = (y as u32) * t + t / 2;
                    blit_single_tile(fb, &pts, 0, bx, by, &player_pal);
                    blit_single_tile(fb, &pts, 1, bx + t, by, &player_pal);
                    blit_single_tile(fb, &pts, 2, bx, by + t, &player_pal);
                    blit_single_tile(fb, &pts, 3, bx + t, by + t, &player_pal);
                }
            }
        }
    } else {
        // No asset manager (tests / minimal frontends): text fallback — the
        // habitat list, mirroring the TUI approximation.
        if areas.is_empty() {
            draw_text(ui_label("AREA UNKNOWN", is_zh), 2 * t, 8 * t, fg, fb);
        } else {
            for (i, map) in areas.iter().enumerate().take(7) {
                draw_text(map_name_for_map(*map, is_zh), 2 * t, (4 + i as u32) * t, fg, fb);
            }
        }
    }

    // Header: "<NAME>'s NEST" at (1,0) — over the top border, as the asm
    // prints it (GetMonName + MonsNestText).
    let suffix = ui_label("'s NEST", is_zh);
    draw_text(&format!("{name}{suffix}"), t, 0, fg, fb);
}

/// The Pokéball mark the original puts next to owned Pokémon (tile $72).
/// Drawn programmatically: circle outline + equator + center button.
fn draw_pokeball_mark(x: u32, y: u32, fb: &mut FrameBuffer) {
    const BALL: [u8; 8] = [
        0b0011_1100,
        0b0110_0110,
        0b1100_0011,
        0b1111_1111,
        0b1111_1111,
        0b1100_0011,
        0b0110_0110,
        0b0011_1100,
    ];
    for (dy, bits) in BALL.iter().enumerate() {
        for dx in 0..8u32 {
            if bits & (0x80 >> dx) != 0 {
                let px = x + dx;
                let py = y + dy as u32;
                if px < fb.width() && py < fb.height() {
                    fb.set_pixel(px, py, Rgba::BLACK);
                }
            }
        }
    }
}

/// Flatten a species' flavor-text pages into display lines (3 per page; '#'
/// expands to "POKé" per the disassembly's charmap convention).
fn flavor_lines(entry: &PokedexEntry) -> Vec<String> {
    entry
        .flavor_text_pages
        .iter()
        .flat_map(|page| page.split('\n'))
        .map(|line| line.replace('#', "POKé"))
        .collect()
}

/// Display width of a char in half-width tiles: CJK glyphs render full-width
/// (2 tiles) in the Fusion Pixel font, everything else 1. Range-based mirror
/// of the renderer's glyph-table classification — same convention as the
/// battle-text wrapping in core.
fn char_tile_width(c: char) -> usize {
    let cp = c as u32;
    let wide = (0x1100..=0x115F).contains(&cp)
        || (0x2010..=0x2027).contains(&cp) // …, quotes, dashes as full-width punct
        || (0x2E80..=0xA4CF).contains(&cp) // CJK radicals, punct, kana, CJK unified
        || (0xAC00..=0xD7A3).contains(&cp) // Hangul
        || (0xF900..=0xFAFF).contains(&cp) // CJK compat ideographs
        || (0xFE30..=0xFE4F).contains(&cp) // CJK compat forms
        || (0xFF00..=0xFF60).contains(&cp) // full-width forms
        || (0xFFE0..=0xFFE6).contains(&cp)
        || (0x20000..=0x3FFFD).contains(&cp);
    usize::from(wide) + 1
}

/// Width of the entry-description box in half-width tile units (18 tiles →
/// 9 full-width chars per line).
const ENTRY_LINE_WIDTH_TILES: usize = 18;

/// No line may START with one of these closers (kinsoku): pull it back to
/// the previous line instead.
const NO_LINE_START: &[char] = &['」', '』', '）', '、', '。', '，', '！', '？', '…', '：', '；'];

/// Wrap one Chinese flavor-text page into lines of at most
/// [`ENTRY_LINE_WIDTH_TILES`] tile units. Each page maps to its own screen
/// (like the original's one-screen-per-page entries), so the wrapped page
/// must stay within the 3 display rows — guaranteed for the shipped data by
/// `all_zh_pages_fit_three_lines` in pokered-data.
fn wrap_zh_page(page: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut width = 0usize;
    for c in page.chars() {
        let w = char_tile_width(c);
        if width + w > ENTRY_LINE_WIDTH_TILES {
            if current.chars().count() > 1 && NO_LINE_START.contains(&c) {
                // Break one char earlier so the closer doesn't start a line.
                let last = current.chars().last().unwrap();
                let popped_w = char_tile_width(last);
                current.pop();
                lines.push(std::mem::take(&mut current));
                current.push(last);
                current.push(c);
                width = popped_w + w;
                continue;
            }
            lines.push(std::mem::take(&mut current));
            width = 0;
        }
        current.push(c);
        width += w;
    }
    lines.push(current);
    lines
}

/// Chinese flavor-text pages flattened into display lines. Each page fills
/// exactly one screen (3 rows, short pages padded blank) so the drawn page
/// count stays in parity with the English pages the entry state machine
/// paginates by — a wrapped page exceeding 3 rows is a data bug.
fn flavor_lines_zh(entry: &PokedexEntry) -> Vec<String> {
    let mut out = Vec::new();
    for page in entry.flavor_text_pages_zh {
        let mut lines = wrap_zh_page(page);
        assert!(
            lines.len() <= 3,
            "{:?}: zh page wraps to {} rows (max 3): {:?}",
            entry.species,
            lines.len(),
            page
        );
        lines.resize(3, String::new());
        out.extend(lines);
    }
    out
}

/// Draw one species' entry (`ShowPokedexDataInternal`): the framed data view,
/// then the flipped front sprite in the reserved 7×7 area at tile (1,1). The
/// sprite must be blitted AFTER the widget: `FrameBufferPainter::draw_text_box`
/// fills the frame interior white, which would erase a sprite blitted first
/// (the sprite's white pixels are a no-op over the white interior, so drawing
/// it last is safe). When `owned` is false the widget prints the
/// "?′??″ / ???lb" placeholders and no description. Returns the description
/// page count.
pub fn draw_entry_for_species(
    sp: Species,
    page: usize,
    owned: bool,
    is_zh: bool,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) -> usize {
    fb.clear(Rgba::WHITE);
    let pal = &GRAYSCALE_PALETTE;
    let t = TILE_SIZE;

    let Some(entry) = pokered_data::pokedex::get_pokedex_entry(sp) else {
        draw_text("No data.", t, t * 3, Rgba::BLACK, fb);
        return 1;
    };

    let display_name = if is_zh {
        pokered_data::lang_data::species_name(sp, true).to_string()
    } else {
        pokered_data::lang_data::species_name(sp, false).to_uppercase()
    };
    let weight = format!("{:.1}", entry.weight_pounds());
    let lines = if is_zh {
        flavor_lines_zh(entry)
    } else {
        flavor_lines(entry)
    };
    let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let view = pokered_ui::menus::pokedex::PokedexEntryView {
        display_name: &display_name,
        category: entry.category_for(is_zh),
        dex_num: sp as u16,
        height_ft: entry.height_feet,
        height_in: entry.height_inches,
        weight_lb: &weight,
        description: &line_refs,
        owned,
    };
    let total_pages = {
        let mut painter = FrameBufferPainter::new(fb);
        let mut ui = Ui::new(&mut painter);
        pokered_ui::menus::pokedex::draw(&view, page, &POKEDEX_DEFAULT_LAYOUT, &mut ui)
    };

    // Sprite on top of the (white) frame interior — see the doc comment.
    if let Some(ref mut rm) = res {
        let sprite_name = species_to_sprite_name(&sp.pascal_name());
        if let Ok(cached) = rm.load_pokemon_front(&sprite_name) {
            let ts = cached.tileset.clone();
            let sprite_w = cached.source_size.0;
            let sprite_h = cached.source_size.1;
            let tiles_w = sprite_w / t;
            let tiles_h = sprite_h / t;
            let area_x = t;
            let area_y = t;
            let x_offset = ((7 - tiles_w + 1) / 2) * t;
            let y_offset = (7 - tiles_h) * t;
            for idx in 0..ts.len() {
                let tx = (idx as u32) % tiles_w;
                let ty = (idx as u32) / tiles_w;
                let flipped_tx = tiles_w - 1 - tx;
                blit_single_tile_flipped(
                    fb,
                    &ts,
                    idx,
                    area_x + x_offset + flipped_tx * t,
                    area_y + y_offset + ty * t,
                    pal,
                    true,
                );
            }
        }
    }

    total_pages
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render_config::RenderConfig;
    use pokered_core::pokemon::pokedex::Pokedex;
    use pokered_core::pokedex_screen::PokedexScreenInput;
    use pokered_data::wild_data::GameVersion;

    fn populated_dex() -> Pokedex {
        let mut dex = Pokedex::new();
        for n in [1u8, 4, 7, 25, 150] {
            dex.set_seen(Species::from_index_id(n));
        }
        for n in [1u8, 25] {
            dex.set_owned(Species::from_index_id(n));
        }
        dex
    }

    fn ink_pixels(fb: &FrameBuffer) -> usize {
        let mut count = 0;
        for y in 0..fb.height() {
            for x in 0..fb.width() {
                if fb.get_pixel(x, y) != Some(Rgba::WHITE) {
                    count += 1;
                }
            }
        }
        count
    }

    fn new_fb() -> FrameBuffer {
        FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE)
    }

    #[test]
    fn list_screen_draws_rows_counts_and_side_menu() {
        let state = PokedexScreenState::new(populated_dex(), GameVersion::Red);
        let mut fb = new_fb();
        draw_pokedex_screen(&state, MapId::PalletTown, false, &mut None, &mut fb);
        assert!(ink_pixels(&fb) > 500, "list screen should draw content");

        // Write a PNG for manual inspection (regenerated each test run).
        let path = std::env::temp_dir().join("pokedex_list_test.png");
        fb.save_png(&path).expect("save list png");
    }

    #[test]
    fn entry_screen_draws_owned_and_unowned() {
        let mut state = PokedexScreenState::new(populated_dex(), GameVersion::Red);
        // Cursor 1 (Bulbasaur, owned) → side menu → DATA → entry.
        state.update_frame(PokedexScreenInput {
            a: true,
            ..Default::default()
        });
        assert_eq!(state.mode(), PokedexScreenMode::SideMenu);
        state.update_frame(PokedexScreenInput {
            a: true,
            ..Default::default()
        });
        assert_eq!(state.mode(), PokedexScreenMode::Entry);
        let mut fb = new_fb();
        draw_pokedex_screen(&state, MapId::PalletTown, false, &mut None, &mut fb);
        assert!(ink_pixels(&fb) > 500, "owned entry should draw content");
        let path = std::env::temp_dir().join("pokedex_entry_owned_test.png");
        fb.save_png(&path).expect("save owned entry png");

        // Page 2 of the flavor text.
        state.update_frame(PokedexScreenInput {
            a: true,
            ..Default::default()
        });
        assert_eq!(state.mode(), PokedexScreenMode::Entry);
        assert_eq!(state.entry_page(), 1);

        // Back to the list, then cursor 4 (Charmander, seen-not-owned) →
        // placeholder entry, 1 page.
        state.update_frame(PokedexScreenInput {
            b: true,
            ..Default::default()
        });
        assert_eq!(state.mode(), PokedexScreenMode::List);
        for _ in 0..3 {
            state.update_frame(PokedexScreenInput {
                down: true,
                ..Default::default()
            });
        }
        assert_eq!(state.cursor(), 4);
        state.update_frame(PokedexScreenInput {
            a: true,
            ..Default::default()
        });
        assert_eq!(state.mode(), PokedexScreenMode::SideMenu);
        state.update_frame(PokedexScreenInput {
            a: true,
            ..Default::default()
        });
        assert_eq!(state.mode(), PokedexScreenMode::Entry);
        let mut fb2 = new_fb();
        draw_pokedex_screen(&state, MapId::PalletTown, false, &mut None, &mut fb2);
        assert!(ink_pixels(&fb2) > 300, "unowned entry should draw content");
        let path2 = std::env::temp_dir().join("pokedex_entry_unowned_test.png");
        fb2.save_png(&path2).expect("save unowned entry png");
    }

    #[test]
    fn side_menu_draws_arrow_on_selected_option() {
        let mut state = PokedexScreenState::new(populated_dex(), GameVersion::Red);
        state.update_frame(PokedexScreenInput {
            a: true,
            ..Default::default()
        });
        assert_eq!(state.mode(), PokedexScreenMode::SideMenu);
        // Move the menu cursor to CRY (row 1).
        state.update_frame(PokedexScreenInput {
            down: true,
            ..Default::default()
        });
        let mut fb = new_fb();
        draw_pokedex_screen(&state, MapId::PalletTown, false, &mut None, &mut fb);
        assert!(ink_pixels(&fb) > 500, "side menu should draw content");
        // The arrow tile (15, 11) holds ink; the empty row above (15, 10)
        // stays white — the arrow marks CRY, not DATA.
        let t = TILE_SIZE;
        let arrow_tile_has_ink = (0..t).any(|dy| {
            (0..t).any(|dx| fb.get_pixel(15 * t + dx, 11 * t + dy) != Some(Rgba::WHITE))
        });
        let data_row_clean = (0..t).all(|dy| {
            (0..t).all(|dx| fb.get_pixel(15 * t + dx, 10 * t + dy) == Some(Rgba::WHITE))
        });
        assert!(arrow_tile_has_ink, "arrow should be at (15,11) for CRY");
        assert!(data_row_clean, "no arrow at (15,10) when CRY is selected");
        let path = std::env::temp_dir().join("pokedex_side_menu_test.png");
        fb.save_png(&path).expect("save side menu png");
    }

    #[test]
    fn area_page_draws_nest_fallback_text() {
        // No ResourceManager in tests → the text fallback path. Cursor 1 is
        // Bulbasaur: no wild areas in Red → "AREA UNKNOWN" page.
        let mut state = PokedexScreenState::new(populated_dex(), GameVersion::Red);
        state.update_frame(PokedexScreenInput {
            a: true,
            ..Default::default()
        });
        // DATA → CRY → AREA.
        state.update_frame(PokedexScreenInput {
            down: true,
            ..Default::default()
        });
        state.update_frame(PokedexScreenInput {
            down: true,
            ..Default::default()
        });
        state.update_frame(PokedexScreenInput {
            a: true,
            ..Default::default()
        });
        assert_eq!(state.mode(), PokedexScreenMode::Area);
        assert!(state.area_maps().is_empty(), "Bulbasaur → AREA UNKNOWN");
        let mut fb = new_fb();
        draw_pokedex_screen(&state, MapId::PalletTown, false, &mut None, &mut fb);
        assert!(ink_pixels(&fb) > 100, "area page should draw content");
        let path = std::env::temp_dir().join("pokedex_area_test.png");
        fb.save_png(&path).expect("save area png");
    }

    /// Every zh flavor page wraps into at most the 3 rows of its screen, so
    /// the drawn page count stays in parity with the English pages the entry
    /// state machine paginates by (`entry_total_pages`).
    #[test]
    fn all_zh_pages_wrap_within_three_rows() {
        for entry in &pokered_data::pokedex::POKEDEX_ENTRIES[..151] {
            for (i, page) in entry.flavor_text_pages_zh.iter().enumerate() {
                let lines = wrap_zh_page(page);
                assert!(
                    lines.len() <= 3,
                    "{:?}: zh page {} wraps to {} rows: {:?}",
                    entry.species,
                    i,
                    lines.len(),
                    lines
                );
            }
            assert_eq!(
                flavor_lines_zh(entry).len(),
                3 * entry.flavor_text_pages_zh.len(),
                "{:?}: padded zh lines must fill one screen per page",
                entry.species
            );
        }
    }

    /// Wrapped zh lines never exceed the 18-tile entry box, and no line
    /// starts with closing punctuation (kinsoku).
    #[test]
    fn zh_wrap_respects_width_and_kinsoku() {
        let width = |s: &str| s.chars().map(char_tile_width).sum::<usize>();
        let long = "尾鳍舒展如优雅的舞裙，因此被称为水中女王，游动时姿态十分优雅。";
        for line in wrap_zh_page(long) {
            assert!(width(&line) <= 18, "line too wide: {line:?}");
            assert!(
                line.is_empty() || !NO_LINE_START.contains(&line.chars().next().unwrap()),
                "line starts with a closer: {line:?}"
            );
        }
        // A closer landing exactly on the row boundary pulls the preceding
        // char down with it instead of starting the next row.
        let lines = wrap_zh_page("一二三四五六七八九，再写九个字。");
        assert_eq!(lines[0], "一二三四五六七八", "break pulled back: {lines:?}");
        assert!(lines[1].starts_with("九，"), "closer kept: {lines:?}");
    }

    /// With zh selected the entry draws the Chinese category and flavor text:
    /// the rendered frame differs from the English entry, and page parity
    /// matches the English page count the state machine drives.
    #[test]
    fn zh_entry_draws_chinese_text() {
        let mut fb_en = new_fb();
        let en_pages =
            draw_entry_for_species(Species::Bulbasaur, 0, true, false, &mut None, &mut fb_en);
        let mut fb_zh = new_fb();
        let zh_pages =
            draw_entry_for_species(Species::Bulbasaur, 0, true, true, &mut None, &mut fb_zh);
        let diff = (0..fb_zh.width())
            .flat_map(|x| (0..fb_zh.height()).map(move |y| (x, y)))
            .filter(|&(x, y)| fb_zh.get_pixel(x, y) != fb_en.get_pixel(x, y))
            .count();
        assert!(diff > 200, "zh entry must render differently ({diff} px)");
        let entry = pokered_data::pokedex::get_pokedex_entry(Species::Bulbasaur).unwrap();
        assert_eq!(zh_pages, entry.flavor_text_pages.len());
    }

}


#[cfg(test)]
mod area_marker_visual {
    use super::*;
    use dotzuki_engine::render_config::RenderConfig;
    use pokered_renderer::palette::GbColor;
    use pokered_core::pokemon::pokedex::Pokedex;
    use pokered_core::pokedex_screen::{PokedexScreenInput, PokedexScreenState};
    use pokered_data::species::Species;
    use pokered_data::wild_data::GameVersion;

    /// AREA page with real assets: the player sprite marker (DrawPlayerOrBirdSprite)
    /// must add ink around the current map's landmark beyond the plain map tile.
    #[test]
    fn area_page_draws_player_sprite_marker() {
        let mut dex = Pokedex::new();
        for n in [1u8, 4, 25, 129, 130] {
            dex.set_seen(Species::from_index_id(n));
        }
        let mut state = PokedexScreenState::new(dex, GameVersion::Red);
        // Cursor 25 = Pikachu (wild in Viridian Forest) → DATA → CRY → AREA.
        for _ in 0..24 {
            state.update_frame(PokedexScreenInput {
                down: true,
                ..Default::default()
            });
        }
        state.update_frame(PokedexScreenInput {
            a: true,
            ..Default::default()
        });
        state.update_frame(PokedexScreenInput {
            down: true,
            ..Default::default()
        });
        state.update_frame(PokedexScreenInput {
            down: true,
            ..Default::default()
        });
        state.update_frame(PokedexScreenInput {
            a: true,
            ..Default::default()
        });
        assert_eq!(state.mode(), PokedexScreenMode::Area);
        assert!(!state.area_maps().is_empty(), "Pikachu has habitats");
        let mut res = pokered_renderer::resource::AssetRoot::auto_detect()
            .ok()
            .map(pokered_renderer::resource::ResourceManager::new);
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        // Cinnabar Island: far from all Pikachu habitats, so the 16×16
        // marker window at its landmark is the player sprite alone.
        draw_pokedex_screen(&state, MapId::CinnabarIsland, false, &mut res, &mut fb);
        let (x, y, _) = town_map_position(MapId::CinnabarIsland).expect("cinnabar");
        let bx = (x as u32) * TILE_SIZE + TILE_SIZE / 2;
        let by = (y as u32) * TILE_SIZE + TILE_SIZE / 2;
        // DrawPlayerOrBirdSprite with OAM base tile $0 = the PLAYER sprite:
        // the marker window must match gfx/sprites/red.png's first frame
        // (the Down-standing pose), palette-mapped like the renderer.
        let cached = res.as_mut().unwrap().load_sprite("red").expect("red sheet");
        let ts = cached.tileset.clone();
        let player_pal = Palette::new(&[
            Rgba::TRANSPARENT,
            GRAYSCALE_PALETTE.colors[1],
            GRAYSCALE_PALETTE.colors[2],
            GRAYSCALE_PALETTE.colors[3],
        ]);
        // The landmark's own map tile shows through the sprite's transparent
        // pixels, so only the sprite's INK pixels are compared.
        let mut ink_pixels = 0u32;
        for dy in 0..16u32 {
            for dx in 0..16u32 {
                let tile = ts.get((dy / TILE_SIZE * 2 + dx / TILE_SIZE) as usize);
                let color = player_pal.color(GbColor::from_u8(
                    tile.pixels[(dy % TILE_SIZE) as usize][(dx % TILE_SIZE) as usize],
                ));
                if color.a == 0 {
                    continue;
                }
                ink_pixels += 1;
                assert_eq!(
                    fb.get_pixel(bx + dx, by + dy),
                    Some(color),
                    "player marker ink pixel at {dx},{dy} matches red.png frame 0"
                );
            }
        }
        assert!(
            ink_pixels > 60,
            "the player sprite frame is mostly ink ({ink_pixels} px)"
        );
        let path = std::env::temp_dir().join("pokedex_area_marker_test.png");
        fb.save_png(&path).expect("save area png");
    }
}
