//! Pokédex list/side menu/entry/area + trainer card rendering for the
//! terminal frontend. Text-mode mirrors of the native screens
//! (engine/menus/pokedex.asm, engine/items/town_map.asm `LoadTownMap_Nest`,
//! start_sub_menus.asm DrawTrainerInfo). The AREA page is a text list of
//! habitat names (the native app draws the actual town map with nest icons).

use pokered_core::pokedex_screen::{PokedexScreenMode, PokedexScreenState, LIST_ROWS};
use pokered_data::lang_data::ui_label;
use pokered_data::species::Species;
use pokered_renderer::embedded_font::{draw_text, fill_tile};
use pokered_renderer::palette::GRAYSCALE_PALETTE;
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::{blit_single_tile_flipped, species_to_sprite_name};

pub fn draw_pokedex_screen(
    state: &PokedexScreenState,
    is_zh: bool,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    match state.mode() {
        PokedexScreenMode::List | PokedexScreenMode::SideMenu => draw_list(state, is_zh, fb),
        PokedexScreenMode::Entry => draw_entry(state, res, fb),
        PokedexScreenMode::Area => draw_area(state, is_zh, fb),
    }
}

fn draw_list(state: &PokedexScreenState, is_zh: bool, fb: &mut FrameBuffer) {
    fb.clear(Rgba::WHITE);
    let fg = Rgba::BLACK;
    let t = TILE_SIZE;

    // Right column dividers (asm: horizontal line at (15,8), vertical at
    // (14,1..9)).
    for col in 15..20 {
        fill_tile(col * t, 8 * t, fg, fb);
    }
    for row in 1..10 {
        fill_tile(14 * t, row * t, fg, fb);
    }

    draw_text("CONTENTS", t, t, fg, fb);

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
            let mark = if state.is_owned(n) { "*" } else { " " };
            draw_text(mark, t * 4, y, fg, fb);
            let name =
                pokered_data::lang_data::species_name(Species::from_index_id(n as u8), false)
                    .to_uppercase();
            draw_text(&name, name_x, y, fg, fb);
        } else {
            draw_text("----------", name_x, y, fg, fb);
        }
    }

    let sel_row = state.cursor() - 1 - scroll;
    draw_text("▶", 0, (3 + sel_row * 2) as u32 * t, fg, fb);

    draw_text("SEEN", 16 * t, 2 * t, fg, fb);
    draw_text(&format!("{:3}", state.seen_count()), 16 * t, 3 * t, fg, fb);
    draw_text("OWN", 16 * t, 5 * t, fg, fb);
    draw_text(&format!("{:3}", state.owned_count()), 16 * t, 6 * t, fg, fb);

    // Side menu items (PokedexMenuItemsText): DATA/CRY/AREA/QUIT at (16,10..13).
    let items = ["DATA", "CRY", "AREA", "QUIT"];
    for (i, item) in items.iter().enumerate() {
        draw_text(ui_label(item, is_zh), 16 * t, (10 + i) as u32 * t, fg, fb);
    }
    if state.mode() == PokedexScreenMode::SideMenu {
        draw_text("▶", 15 * t, (10 + state.side_menu_cursor() as u32) * t, fg, fb);
    }
}

/// The AREA page (`LoadTownMap_Nest`): text rendition — the TUI has no
/// town-map renderer, so it prints "<NAME>'s NEST" and the habitat names,
/// or "AREA UNKNOWN" (`DisplayWildLocations`).
fn draw_area(state: &PokedexScreenState, is_zh: bool, fb: &mut FrameBuffer) {
    fb.clear(Rgba::WHITE);
    let fg = Rgba::BLACK;
    let t = TILE_SIZE;

    let name = pokered_data::lang_data::species_name(state.cursor_species(), false).to_uppercase();
    let suffix = ui_label("'s NEST", is_zh);
    draw_text(&format!("{name}{suffix}"), t, t, fg, fb);

    let areas = state.area_maps();
    if areas.is_empty() {
        draw_text(ui_label("AREA UNKNOWN", is_zh), 2 * t, 8 * t, fg, fb);
    } else {
        for (i, map) in areas.iter().enumerate().take(7) {
            draw_text(
                pokered_data::map_names::map_name_for_map(*map, is_zh),
                2 * t,
                (3 + i as u32) * t,
                fg,
                fb,
            );
        }
    }
}

fn draw_entry(state: &PokedexScreenState, res: &mut Option<ResourceManager>, fb: &mut FrameBuffer) {
    fb.clear(Rgba::WHITE);
    let fg = Rgba::BLACK;
    let t = TILE_SIZE;
    let sp = state.cursor_species();
    let owned = state.is_owned(state.cursor());

    // Front sprite (flipped, as in the original entry view).
    if let Some(ref mut rm) = res {
        let sprite_name = species_to_sprite_name(&sp.pascal_name());
        if let Ok(cached) = rm.load_pokemon_front(&sprite_name) {
            let ts = cached.tileset.clone();
            let tiles_w = cached.source_size.0 / t;
            let tiles_h = cached.source_size.1 / t;
            let x_offset = ((7 - tiles_w + 1) / 2) * t;
            let y_offset = (7 - tiles_h) * t;
            for idx in 0..ts.len() {
                let tx = (idx as u32) % tiles_w;
                let ty = (idx as u32) / tiles_w;
                blit_single_tile_flipped(
                    fb,
                    &ts,
                    idx,
                    t + x_offset + (tiles_w - 1 - tx) * t,
                    t + y_offset + ty * t,
                    &GRAYSCALE_PALETTE,
                    true,
                );
            }
        }
    }

    let name = pokered_data::lang_data::species_name(sp, false).to_uppercase();
    draw_text(&name, 9 * t, 2 * t, fg, fb);
    draw_text(&format!("No.{:03}", sp as u16), 2 * t, 8 * t, fg, fb);

    let Some(entry) = pokered_data::pokedex::get_pokedex_entry(sp) else {
        draw_text("No data.", t, 11 * t, fg, fb);
        return;
    };
    draw_text(entry.category, 9 * t, 4 * t, fg, fb);
    if owned {
        draw_text(
            &format!("HT  {}'{:02}\"", entry.height_feet, entry.height_inches),
            9 * t,
            6 * t,
            fg,
            fb,
        );
        draw_text(
            &format!("WT   {:.1}lb", entry.weight_pounds()),
            9 * t,
            8 * t,
            fg,
            fb,
        );
        // Flavor pages: 3 lines per page, page selected by the state machine.
        let lines: Vec<String> = entry
            .flavor_text_pages
            .iter()
            .flat_map(|p| p.split('\n'))
            .map(|l| l.replace('#', "POKé"))
            .collect();
        let page = state.entry_page();
        let start = page * 3;
        let end = lines.len().min(start + 3);
        for (i, line) in lines[start..end].iter().enumerate() {
            draw_text(line, t, (11 + i as u32 * 2) * t, fg, fb);
        }
    } else {
        // Seen-not-owned: the original prints placeholders and no description.
        draw_text("HT  ?'??\"", 9 * t, 6 * t, fg, fb);
        draw_text("WT   ???lb", 9 * t, 8 * t, fg, fb);
    }
}

pub fn draw_trainer_card(
    player_name: &str,
    money: u32,
    play_time_hours: u8,
    play_time_minutes: u8,
    obtained_badges: u8,
    fb: &mut FrameBuffer,
) {
    fb.clear(Rgba::WHITE);
    let fg = Rgba::BLACK;
    let t = TILE_SIZE;

    draw_text("NAME/", 2 * t, 2 * t, fg, fb);
    draw_text(&player_name.to_uppercase(), 7 * t, 2 * t, fg, fb);
    draw_text("MONEY/", 2 * t, 4 * t, fg, fb);
    draw_text(&format!("${}", money), 8 * t, 4 * t, fg, fb);
    draw_text("TIME/", 2 * t, 6 * t, fg, fb);
    draw_text(
        &format!("{}:{:02}", play_time_hours, play_time_minutes),
        9 * t,
        6 * t,
        fg,
        fb,
    );

    draw_text("BADGES", 6 * t, 9 * t, fg, fb);
    // Two rows of four; owned slots are bracketed.
    for i in 0..8u32 {
        let x = (2 + (i % 4) * 4) * t;
        let y = (11 + (i / 4) * 3) * t;
        if obtained_badges & (1 << i) != 0 {
            draw_text(&format!("[{}]", i + 1), x, y, fg, fb);
        } else {
            draw_text(&format!(" {} ", i + 1), x, y, fg, fb);
        }
    }
}
