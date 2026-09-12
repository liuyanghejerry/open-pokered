//! Renderer for the PC storage screens (Bill's PC, the player's item PC, and
//! PROF.OAK's #DEX rating).
//!
//! GB-style presentation over the same primitives as the elevator/menu
//! renderers: text boxes with `>` cursors, single/double-spaced lists, YES/NO
//! popups. All logic lives in `pokered_core::pc_screen`.

use crate::alloc_prelude::*;
use pokered_core::battle::state::Pokemon;
use pokered_core::game_state::Lang;
use pokered_core::pc_screen::{ItemListMode, MonListMode, PcPhase, PcScreen, PC_LIST_VISIBLE_ROWS};
use pokered_core::save::SaveData;
use pokered_data::lang_data;
use pokered_renderer::embedded_font::{draw_text, measure_text};
use pokered_renderer::palette::GRAYSCALE_SPRITE_PALETTE;
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::{blit_tileset, draw_text_box, species_to_sprite_name};
use crate::render::battle_i18n::zh_name;
use pokered_data::ui_text::{zh_main_menu_label, zh_pc_line};

const BG: Rgba = Rgba::WHITE;
const FG: Rgba = Rgba::BLACK;

const T: u32 = 8; // tile size in pixels

/// Repaint the two cursor cells used by PC menus and lists.
///
/// This is deliberately limited to the marker column: every PC cursor is a
/// plain `>` on the white list/menu background, and the adjacent label never
/// overlaps this 5x10 pixel cell. The proportional font advances `>` by five
/// pixels; clearing a full tile would erase the first letter in the compact
/// box chooser, which intentionally has no separating space.
#[cfg(any(test, target_os = "none"))]
pub fn redraw_pc_cursor(
    previous: (u32, u32),
    current: (u32, u32),
    fb: &mut FrameBuffer,
) {
    for (x, y) in [previous, current] {
        for py in y..(y + 10).min(fb.height()) {
            for px in x..(x + measure_text(">")).min(fb.width()) {
                fb.set_pixel(px, py, BG);
            }
        }
    }
    draw_text(">", current.0, current.1, FG, fb);
}

fn item_name(id: pokered_data::items::ItemId, is_zh: bool) -> String {
    if is_zh {
        lang_data::item_name(id, true).to_string()
    } else {
        pokered_data::item_data::get_item_data(id)
            .map(|d| d.name.to_string())
            .unwrap_or_else(|| "???".to_string())
    }
}

fn mon_row(mon: &Pokemon) -> String {
    let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
    format!("{} :L{}", mon.display_name(&mut name_buf), mon.level)
}

/// Bottom text box holding up to `lines` lines (max 5), plus the current
/// message page of the Message phase. Lines are routed through
/// [`zh_pc_line`] so the English messages produced by `pokered_core::pc_screen`
/// are translated at display time only.
fn draw_message(lines: &[String], fb: &mut FrameBuffer, is_zh: bool) {
    let shown: Vec<String> = lines.iter().take(5)
        .flat_map(|line| {
            let text = if is_zh { zh_pc_line(line) } else { line.clone() };
            if is_zh { wrap_message(&text) } else { vec![text] }
        }).collect();
    let pitch = if is_zh { 12 } else { T };
    let height = if is_zh {
        (shown.len().max(1) as u32 * pitch).div_ceil(T)
    } else { shown.len().max(1) as u32 + 1 };
    let by = 144u32.saturating_sub((height + 2) * T);
    draw_text_box(fb, 0, by, 18, height, FG);
    for (i, line) in shown.iter().enumerate() {
        draw_text(line, T, by + T + i as u32 * pitch, FG, fb);
    }
}

// Wrap translated lines by glyph width, including mixed Chinese/Latin names.
fn wrap_message(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for ch in text.chars() {
        let mut next = line.clone();
        next.push(ch);
        if !line.is_empty() && measure_text(&next) > 18 * T {
            lines.push(core::mem::take(&mut line));
        }
        line.push(ch);
    }
    lines.push(line);
    lines
}

/// YES/NO popup on the right side (original: TWO_OPTION_MENU at hlcoord 14,7).
fn draw_yes_no(selected_yes: bool, fb: &mut FrameBuffer, is_zh: bool) {
    let bx = 14 * T;
    let by = if is_zh { T } else { 7 * T };
    draw_text_box(fb, bx, by, 4, 4, FG);
    let cy = if selected_yes { 1 } else { 3 };
    draw_text(">", bx + T, by + cy * T, FG, fb);
    draw_text(lang_data::ui_label("YES", is_zh), bx + 2 * T, by + T, FG, fb);
    draw_text(lang_data::ui_label("NO", is_zh), bx + 2 * T, by + 3 * T, FG, fb);
}

/// A boxed, scrollable selection list. `rows` are the label lines; `cancel`
/// appends a CANCEL row. Returns nothing; purely visual.
fn draw_list(
    bx: u32,
    by: u32,
    bw: u32,
    visible: usize,
    rows: &[String],
    cursor: usize,
    scroll: usize,
    is_zh: bool,
    fb: &mut FrameBuffer,
) {
    let pitch = if is_zh { 12 } else { T };
    let bh = if is_zh { (visible as u32 * pitch).div_ceil(T) } else { visible as u32 + 1 };
    draw_text_box(fb, bx, by, bw, bh, FG);
    for (row, (i, label)) in rows
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .enumerate()
    {
        let y = by + T + row as u32 * pitch;
        let marker = if i == cursor { ">" } else { " " };
        draw_text(&format!("{} {}", marker, label), bx + T, y, FG, fb);
    }
}

/// Scrolling window start that keeps `cursor` visible (same policy as the
/// bag screen's clamp_scroll).
fn follow_scroll(cursor: usize, rows: usize, visible: usize) -> usize {
    if rows <= visible {
        return 0;
    }
    cursor
        .saturating_sub(visible / 2)
        .min(rows - visible)
}

/// Current mon list (party for DEPOSIT, current box otherwise) + CANCEL.
fn mon_rows(pc: &PcScreen, save: &SaveData, is_zh: bool) -> Vec<String> {
    let mut rows: Vec<String> = match pc.mon_mode() {
        MonListMode::Deposit => save.party.iter().map(mon_row).collect(),
        MonListMode::Withdraw | MonListMode::Release => save
            .pc_storage
            .current_box()
            .iter()
            .map(mon_row)
            .collect(),
    };
    rows.push(lang_data::ui_label("CANCEL", is_zh).to_string());
    rows
}

/// Current item list (bag for DEPOSIT, PC storage otherwise) + CANCEL.
fn item_rows(pc: &PcScreen, save: &SaveData, is_zh: bool) -> Vec<String> {
    let src: Vec<(pokered_data::items::ItemId, u32)> = match pc.item_mode() {
        ItemListMode::Deposit => save.game_data.bag.items(),
        ItemListMode::Withdraw | ItemListMode::Toss => save.game_data.pc_items.items(),
    };
    let mut rows: Vec<String> = src
        .iter()
        .map(|(id, qty)| {
            if id.is_key_item() {
                item_name(*id, is_zh)
            } else {
                format!("{} x{:02}", item_name(*id, is_zh), qty)
            }
        })
        .collect();
    rows.push(lang_data::ui_label("CANCEL", is_zh).to_string());
    rows
}

const BILLS_LABELS: [&str; 5] = [
    "WITHDRAW #MON",
    "DEPOSIT #MON",
    "RELEASE #MON",
    "CHANGE BOX",
    "SEE YA!",
];

const PLAYERS_LABELS: [&str; 4] = ["WITHDRAW ITEM", "DEPOSIT ITEM", "TOSS ITEM", "LOG OFF"];

/// Double-spaced vertical menu (the original PC menus use 2-tile rows).
fn draw_menu(bx: u32, by: u32, bw: u32, labels: &[String], cursor: usize, fb: &mut FrameBuffer) {
    let bh = labels.len() as u32 * 2;
    draw_text_box(fb, bx, by, bw, bh, FG);
    for (i, label) in labels.iter().enumerate() {
        let y = by + (1 + i as u32 * 2) * T;
        let marker = if i == cursor { ">" } else { " " };
        draw_text(&format!("{} {}", marker, label), bx + T, y, FG, fb);
    }
}

// `zh_main_menu_label` / `zh_pc_line` (+ the PC_LINE_ZH table) moved to
// `pokered_data::ui_text` so the TUI frontend shares them; re-imported below.

/// "BOX No.N" indicator (bills_pc.asm:149-169).
fn draw_box_no(save: &SaveData, fb: &mut FrameBuffer, is_zh: bool) {
    let bx = 9 * T;
    let by = 14 * T;
    draw_text_box(fb, bx, by, 8, 1, FG);
    let n = save.pc_storage.current_box_index() + 1;
    let text = if is_zh {
        format!("盒子{}号", n)
    } else {
        format!("BOX No.{}", n)
    };
    draw_text(&text, bx + T, by + T, FG, fb);
}

pub fn draw_pc(
    pc: &PcScreen,
    save: &SaveData,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let is_zh = lang == Lang::Zh;
    fb.clear(BG);
    match pc.phase() {
        PcPhase::Message => {
            let start = pc.message_page() * 4;
            let page: Vec<String> = pc
                .message_lines()
                .iter()
                .skip(start)
                .take(4)
                .cloned()
                .collect();
            draw_message(&page, fb, is_zh);
        }
        PcPhase::MainMenu => {
            let labels: Vec<String> = pc
                .main_menu_labels()
                .iter()
                .map(|s| if is_zh { zh_main_menu_label(s) } else { s.clone() })
                .collect();
            draw_menu(0, 0, 13, &labels, pc.main_menu().cursor(), fb);
        }
        PcPhase::BillsMenu => {
            let labels: Vec<String> = BILLS_LABELS
                .iter()
                .map(|s| lang_data::ui_label(s, is_zh).to_string())
                .collect();
            draw_menu(0, 0, 12, &labels, pc.bills_menu().cursor(), fb);
            draw_box_no(save, fb, is_zh);
        }
        PcPhase::MonList | PcPhase::MonAction | PcPhase::ReleaseConfirm => {
            let rows = mon_rows(pc, save, is_zh);
            let cursor = pc.mon_cursor();
            let scroll = follow_scroll(cursor, rows.len(), 8);
            draw_list(0, 0, 18, 8, &rows, cursor, scroll, is_zh, fb);
            match pc.phase() {
                PcPhase::MonAction => {
                    let first = match pc.mon_mode() {
                        MonListMode::Withdraw => "WITHDRAW",
                        MonListMode::Deposit => "DEPOSIT",
                        MonListMode::Release => "RELEASE",
                    };
                    let labels: Vec<String> = [first, "STATS", "CANCEL"]
                        .iter()
                        .map(|s| lang_data::ui_label(s, is_zh).to_string())
                        .collect();
                    draw_menu(10 * T, 8 * T, 8, &labels, pc.mon_action_cursor(), fb);
                }
                PcPhase::ReleaseConfirm => {
                    let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
                    let name = save
                        .pc_storage
                        .current_box()
                        .get(cursor)
                        .map(|m| m.display_name(&mut name_buf))
                        .unwrap_or("");
                    // "Once released, {NAME} is gone forever. OK?"
                    // (_OnceReleasedText)
                    if is_zh {
                        draw_message(
                            &[
                                format!("一旦放生，{}就", name),
                                "永远消失了。".to_string(),
                                "可以吗？".to_string(),
                            ],
                            fb,
                            is_zh,
                        );
                    } else {
                        draw_message(
                            &[
                                "Once released,".to_string(),
                                format!("{} is", name),
                                "gone forever. OK?".to_string(),
                            ],
                            fb,
                            is_zh,
                        );
                    }
                    draw_yes_no(pc.yes_selected(), fb, is_zh);
                }
                _ => {}
            }
        }
        PcPhase::ChangeBoxConfirm => {
            // "When you change a #MON BOX, data will be saved. Is that okay?"
            // (_WhenYouChangeBoxText)
            draw_message(
                &[
                    "When you change a".to_string(),
                    "#MON BOX, data".to_string(),
                    "will be saved.".to_string(),
                    String::new(),
                    "Is that okay?".to_string(),
                ],
                fb,
                is_zh,
            );
            draw_yes_no(pc.yes_selected(), fb, is_zh);
        }
        PcPhase::BoxList => {
            // "Choose a #MON BOX." header + the 12 box names; a filled
            // marker stands in for the original's pokeball tile next to
            // non-empty boxes (save.asm DisplayChangeBoxMenu:487-498).
            draw_text_box(fb, 0, 0, 9, 3, FG);
            let (h1, h2) = if is_zh {
                ("选择盒子。", "")
            } else {
                ("Choose a", "#MON BOX.")
            };
            draw_text(h1, T, T, FG, fb);
            if !h2.is_empty() {
                draw_text(h2, T, 3 * T, FG, fb);
            }
            if is_zh {
                for col in 0..2 {
                    draw_text_box(fb, col * 10 * T, 4 * T, 8, 12, FG);
                }
            } else {
                draw_text_box(fb, 11 * T, 0, 7, 12, FG);
            }
            for i in 0..12usize {
                let (bx, y) = if is_zh {
                    ((i / 6) as u32 * 10 * T, (5 + (i % 6) as u32 * 2) * T)
                } else { (11 * T, (1 + i as u32) * T) };
                let marker = if i == pc.box_cursor() { ">" } else { " " };
                let name = if is_zh { format!("盒子{:>2}", i + 1) }
                    else { format!("BOX{:>2}", i + 1) };
                draw_text(&format!("{}{}", marker, name), bx + T, y, FG, fb);
                if save.pc_storage.get_box(i).is_ok_and(|b| !b.is_empty()) {
                    for dy in 0..4u32 {
                        for dx in 0..4u32 {
                            fb.set_pixel(bx + 7 * T + 2 + dx, y + 2 + dy, FG);
                        }
                    }
                }
            }
        }
        PcPhase::ItemMenu => {
            let labels: Vec<String> = PLAYERS_LABELS
                .iter()
                .map(|s| lang_data::ui_label(s, is_zh).to_string())
                .collect();
            draw_menu(0, 0, 13, &labels, pc.players_menu().cursor(), fb);
        }
        PcPhase::ItemList | PcPhase::ItemQuantity | PcPhase::TossConfirm => {
            let rows = item_rows(pc, save, is_zh);
            let cursor = pc.item_list_cursor();
            let scroll = follow_scroll(cursor, rows.len(), PC_LIST_VISIBLE_ROWS.max(8));
            draw_list(0, 0, 18, 8, &rows, cursor, scroll, is_zh, fb);
            match pc.phase() {
                PcPhase::ItemQuantity => {
                    // "How many?" + the running quantity (players_pc.asm
                    // DisplayChooseQuantityMenu).
                    let name = rows.get(cursor).cloned().unwrap_or_default();
                    let prompt = if is_zh { "几个？" } else { "How many?" };
                    draw_message(&[prompt.to_string()], fb, is_zh);
                    let bx = 12 * T;
                    let by = 10 * T;
                    draw_text_box(fb, bx, by, 6, 1, FG);
                    draw_text(&format!("x{:02}", pc.item_qty()), bx + T, by + T, FG, fb);
                    let _ = name;
                }
                PcPhase::TossConfirm => {
                    // "Is it OK to toss {ITEM}?" (_IsItOKToTossItemText)
                    let name = save
                        .game_data
                        .pc_items
                        .get(cursor)
                        .map(|(id, _)| item_name(id, is_zh))
                        .unwrap_or_default();
                    if is_zh {
                        draw_message(&[format!("要扔掉{}吗？", name)], fb, is_zh);
                    } else {
                        draw_message(
                            &[
                                "Is it OK to toss".to_string(),
                                format!("{}?", name),
                            ],
                            fb,
                            is_zh,
                        );
                    }
                    draw_yes_no(pc.yes_selected(), fb, is_zh);
                }
                _ => {}
            }
        }
        PcPhase::OaksConfirm => {
            // "Want to get your #DEX rated?" (_GetDexRatedText)
            draw_message(
                &[
                    "Want to get your".to_string(),
                    "#DEX rated?".to_string(),
                ],
                fb,
                is_zh,
            );
            draw_yes_no(pc.yes_selected(), fb, is_zh);
        }
        PcPhase::LeagueHoF => {
            draw_league_hof(pc, resources, fb, is_zh);
        }
    }
}

/// #MON LEAGUE HoF viewer (LeaguePCShowMon, engine/menus/league_pc.asm:
/// 78-113): the recorded mon's front pic, "HALL OF FAME No. X" and the
/// nickname / LEVEL / TYPE1 / TYPE2 info (`HoFDisplayMonInfo`).
fn draw_league_hof(pc: &PcScreen, resources: &mut Option<ResourceManager>, fb: &mut FrameBuffer, is_zh: bool) {
    let Some((team_no, view)) = pc.league_hof_mon() else {
        return;
    };
    // The recorded mon's front pic at hlcoord 12,5 (league_pc.asm:95-100).
    if let Some(rm) = resources.as_mut() {
        let sprite = species_to_sprite_name(&format!("{}", view.species));
        if let Ok(cached) = rm.load_pokemon_front(&sprite) {
            let ts = cached.tileset.clone();
            let w_tiles = cached.source_size.0 / TILE_SIZE;
            blit_tileset(fb, &ts, 12 * T, 5 * T, w_tiles, &GRAYSCALE_SPRITE_PALETTE);
        }
    }
    let hof_no = if is_zh {
        format!("名人堂第{:>3}号", team_no)
    } else {
        format!("HALL OF FAME No.{:>3}", team_no)
    };
    draw_text(&hof_no, T, 15 * T, FG, fb);
    draw_text(&view.nickname, T, T, FG, fb);
    draw_text(&format!("{} :L{}", lang_data::ui_label("LEVEL/", is_zh), view.level), T, 3 * T, FG, fb);
    if let Some(stats) = pokered_data::pokemon_data::get_base_stats(view.species) {
        draw_text(
            &format!(
                "{} {}",
                lang_data::ui_label("TYPE1/", is_zh),
                pokered_data::lang_data::type_name(stats.type1, is_zh)
            ),
            T,
            5 * T,
            FG,
            fb,
        );
        if stats.type1 != stats.type2 {
            draw_text(
                &format!(
                    "{} {}",
                    lang_data::ui_label("TYPE2/", is_zh),
                    pokered_data::lang_data::type_name(stats.type2, is_zh)
                ),
                T,
                7 * T,
                FG,
                fb,
            );
        }
    }
}

// The exact `PC_LINE_ZH` table and `zh_pc_line` moved to `pokered_data::ui_text`
// (shared with the TUI); imported at the top of this file.

#[cfg(test)]
mod layout_tests {
    use super::*;
    use dotzuki_engine::render_config::RenderConfig;
    use pokered_core::main_menu::MenuInput;
    use pokered_core::pc_screen::{PcContext, PcEntry, PcOpenContext};
    use pokered_core::pokemon::stats::create_pokemon;
    use pokered_data::items::ItemId;
    use pokered_data::species::Species;

    const A: MenuInput = MenuInput {
        up: false,
        down: false,
        a: true,
        b: false,
    };
    const UP: MenuInput = MenuInput {
        up: true,
        down: false,
        a: false,
        b: false,
    };
    const DOWN: MenuInput = MenuInput {
        up: false,
        down: true,
        a: false,
        b: false,
    };

    fn open_context(has_pokedex: bool) -> PcOpenContext {
        PcOpenContext {
            has_pokedex,
            met_bill: true,
            beaten_league: false,
            player_name: "RED".into(),
            hof_teams: Vec::new(),
        }
    }

    fn update_pc(pc: &mut PcScreen, save: &mut SaveData, input: MenuInput) {
        let mut ctx = PcContext {
            party: &mut save.party,
            pc_storage: &mut save.pc_storage,
            bag: &mut save.game_data.bag,
            pc_items: &mut save.game_data.pc_items,
            pokedex: &save.game_data.pokedex,
        };
        let _ = pc.update_frame(input, &mut ctx);
    }

    fn skip_message(pc: &mut PcScreen, save: &mut SaveData) {
        while pc.phase() == PcPhase::Message {
            update_pc(pc, save, A);
        }
    }

    fn render_pc_state(pc: &PcScreen, save: &SaveData, language: Lang) -> FrameBuffer {
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::BLACK);
        let mut resources = None;
        draw_pc(pc, save, &mut resources, &mut fb, language);
        fb
    }

    fn assert_framebuffers_equal(actual: &FrameBuffer, expected: &FrameBuffer) {
        assert_eq!(actual.width(), expected.width());
        assert_eq!(actual.height(), expected.height());
        for y in 0..actual.height() {
            for x in 0..actual.width() {
                assert_eq!(
                    actual.get_pixel(x, y),
                    expected.get_pixel(x, y),
                    "framebuffer mismatch at ({x}, {y})",
                );
            }
        }
    }

    fn assert_cursor_repaint(
        previous: &PcScreen,
        current: &PcScreen,
        save: &SaveData,
        language: Lang,
        previous_position: (u32, u32),
        current_position: (u32, u32),
    ) {
        let mut actual = render_pc_state(previous, save, language);
        redraw_pc_cursor(previous_position, current_position, &mut actual);
        let expected = render_pc_state(current, save, language);
        assert_framebuffers_equal(&actual, &expected);
    }

    fn cursor_state(
        base: &PcScreen,
        save: &mut SaveData,
        cursor: usize,
    ) -> PcScreen {
        let mut state = base.clone();
        for _ in 0..cursor {
            update_pc(&mut state, save, DOWN);
        }
        state
    }

    #[test]
    fn translated_message_wrap_preserves_text_and_fits_box() {
        for text in ["一旦放生，CHARMANDER就永远消失了。可以吗？", "更换宝可梦盒子时，数据会被保存。", ""] {
            let lines = wrap_message(text);
            assert_eq!(lines.concat(), text);
            assert!(lines.iter().all(|line| measure_text(line) <= 144));
        }
    }

    #[test]
    fn chinese_list_leaves_clear_pixels_between_rows() {
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), BG);
        let rows = vec!["精灵球 x03".into(), "好伤药 x12".into()];
        draw_list(0, 0, 18, 8, &rows, 0, 0, true, &mut fb);
        // CJK ink includes the font baseline offset: the second row
        // begins at y=22, with clear scanlines after the first row.
        for y in 20..22 {
            for x in 8..152 {
                assert_eq!(fb.get_pixel(x, y), Some(BG), "rows touch at {x},{y}");
            }
        }
    }

    #[test]
    fn pc_menu_cursor_repaint_matches_full_redraw_for_every_transition() {
        for language in [Lang::En, Lang::Zh] {
            for (entry, phase, item_count) in [
                (PcEntry::PokemonCenter, PcPhase::MainMenu, 4),
                (PcEntry::BillsPc, PcPhase::BillsMenu, 5),
                (PcEntry::PlayersPc, PcPhase::ItemMenu, 4),
            ] {
                let mut save = SaveData::new();
                let mut base = PcScreen::new(entry, &open_context(true));
                skip_message(&mut base, &mut save);
                assert_eq!(base.phase(), phase);

                for previous_cursor in 0..item_count {
                    for current_cursor in 0..item_count {
                        if previous_cursor == current_cursor {
                            continue;
                        }
                        let previous = cursor_state(&base, &mut save, previous_cursor);
                        let current = cursor_state(&base, &mut save, current_cursor);
                        let position = |cursor| (8, (1 + cursor as u32 * 2) * T);
                        assert_cursor_repaint(
                            &previous,
                            &current,
                            &save,
                            language,
                            position(previous_cursor),
                            position(current_cursor),
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn pc_list_and_overlay_cursor_repaint_matches_full_redraw() {
        for language in [Lang::En, Lang::Zh] {
            let pitch = if language == Lang::Zh { 12 } else { T };

            let mut mon_save = SaveData::new();
            let mon = create_pokemon(Species::Bulbasaur, 9, [0x9a, 0x78]).unwrap();
            mon_save.pc_storage.current_box_mut().deposit(mon).unwrap();
            let mut mon_list = PcScreen::new(PcEntry::BillsPc, &open_context(false));
            skip_message(&mut mon_list, &mut mon_save);
            update_pc(&mut mon_list, &mut mon_save, A);
            assert_eq!(mon_list.phase(), PcPhase::MonList);

            let mon_cancel = cursor_state(&mon_list, &mut mon_save, 1);
            assert_cursor_repaint(
                &mon_list,
                &mon_cancel,
                &mon_save,
                language,
                (T, T),
                (T, T + pitch),
            );
            assert_cursor_repaint(
                &mon_cancel,
                &mon_list,
                &mon_save,
                language,
                (T, T + pitch),
                (T, T),
            );

            let mut mon_action = mon_list.clone();
            update_pc(&mut mon_action, &mut mon_save, A);
            assert_eq!(mon_action.phase(), PcPhase::MonAction);
            for previous_cursor in 0..3 {
                for current_cursor in 0..3 {
                    if previous_cursor == current_cursor {
                        continue;
                    }
                    let previous = cursor_state(&mon_action, &mut mon_save, previous_cursor);
                    let current = cursor_state(&mon_action, &mut mon_save, current_cursor);
                    let position = |cursor| (11 * T, (9 + cursor as u32 * 2) * T);
                    assert_cursor_repaint(
                        &previous,
                        &current,
                        &mon_save,
                        language,
                        position(previous_cursor),
                        position(current_cursor),
                    );
                }
            }

            let mut item_save = SaveData::new();
            item_save
                .game_data
                .pc_items
                .add_item(ItemId::Potion, 2)
                .unwrap();
            item_save
                .game_data
                .pc_items
                .add_item(ItemId::Antidote, 1)
                .unwrap();
            let mut item_list = PcScreen::new(PcEntry::PlayersPc, &open_context(false));
            skip_message(&mut item_list, &mut item_save);
            update_pc(&mut item_list, &mut item_save, A);
            assert_eq!(item_list.phase(), PcPhase::ItemList);

            for previous_cursor in 0..3 {
                for current_cursor in 0..3 {
                    if previous_cursor == current_cursor {
                        continue;
                    }
                    let previous = cursor_state(&item_list, &mut item_save, previous_cursor);
                    let current = cursor_state(&item_list, &mut item_save, current_cursor);
                    let position = |cursor| (T, T + cursor as u32 * pitch);
                    assert_cursor_repaint(
                        &previous,
                        &current,
                        &item_save,
                        language,
                        position(previous_cursor),
                        position(current_cursor),
                    );
                }
            }
        }
    }

    #[test]
    fn pc_confirmation_and_box_cursor_repaint_matches_full_redraw() {
        for language in [Lang::En, Lang::Zh] {
            let yes_no_position = |selected_yes| {
                let box_y = if language == Lang::Zh { T } else { 7 * T };
                (15 * T, box_y + if selected_yes { T } else { 3 * T })
            };

            let mut save = SaveData::new();
            let mut confirm = PcScreen::new(PcEntry::BillsPc, &open_context(false));
            skip_message(&mut confirm, &mut save);
            for _ in 0..3 {
                update_pc(&mut confirm, &mut save, DOWN);
            }
            update_pc(&mut confirm, &mut save, A);
            assert_eq!(confirm.phase(), PcPhase::ChangeBoxConfirm);
            let mut yes = confirm.clone();
            update_pc(&mut yes, &mut save, UP);
            assert_cursor_repaint(
                &confirm,
                &yes,
                &save,
                language,
                yes_no_position(false),
                yes_no_position(true),
            );
            assert_cursor_repaint(
                &yes,
                &confirm,
                &save,
                language,
                yes_no_position(true),
                yes_no_position(false),
            );

            update_pc(&mut yes, &mut save, A);
            assert_eq!(yes.phase(), PcPhase::BoxList);
            let box_position = |cursor: usize| {
                if language == Lang::Zh {
                    (
                        (cursor / 6) as u32 * 10 * T + T,
                        (5 + (cursor % 6) as u32 * 2) * T,
                    )
                } else {
                    (12 * T, (1 + cursor as u32) * T)
                }
            };
            for previous_cursor in 0..12 {
                for current_cursor in 0..12 {
                    if previous_cursor == current_cursor {
                        continue;
                    }
                    let previous = cursor_state(&yes, &mut save, previous_cursor);
                    let current = cursor_state(&yes, &mut save, current_cursor);
                    assert_cursor_repaint(
                        &previous,
                        &current,
                        &save,
                        language,
                        box_position(previous_cursor),
                        box_position(current_cursor),
                    );
                }
            }

            let mut release_save = SaveData::new();
            let mon = create_pokemon(Species::Pikachu, 5, [0x9a, 0x78]).unwrap();
            release_save
                .pc_storage
                .current_box_mut()
                .deposit(mon)
                .unwrap();
            let mut release = PcScreen::new(PcEntry::BillsPc, &open_context(false));
            skip_message(&mut release, &mut release_save);
            update_pc(&mut release, &mut release_save, DOWN);
            update_pc(&mut release, &mut release_save, DOWN);
            update_pc(&mut release, &mut release_save, A);
            update_pc(&mut release, &mut release_save, A);
            assert_eq!(release.phase(), PcPhase::ReleaseConfirm);
            let mut release_yes = release.clone();
            update_pc(&mut release_yes, &mut release_save, UP);
            assert_cursor_repaint(
                &release,
                &release_yes,
                &release_save,
                language,
                yes_no_position(false),
                yes_no_position(true),
            );

            let mut toss_save = SaveData::new();
            toss_save
                .game_data
                .pc_items
                .add_item(ItemId::Potion, 2)
                .unwrap();
            let mut toss = PcScreen::new(PcEntry::PlayersPc, &open_context(false));
            skip_message(&mut toss, &mut toss_save);
            update_pc(&mut toss, &mut toss_save, DOWN);
            update_pc(&mut toss, &mut toss_save, DOWN);
            update_pc(&mut toss, &mut toss_save, A);
            update_pc(&mut toss, &mut toss_save, A);
            update_pc(&mut toss, &mut toss_save, A);
            assert_eq!(toss.phase(), PcPhase::TossConfirm);
            let mut toss_yes = toss.clone();
            update_pc(&mut toss_yes, &mut toss_save, UP);
            assert_cursor_repaint(
                &toss,
                &toss_yes,
                &toss_save,
                language,
                yes_no_position(false),
                yes_no_position(true),
            );

            let mut oak_save = SaveData::new();
            let mut oak = PcScreen::new(PcEntry::PokemonCenter, &open_context(true));
            skip_message(&mut oak, &mut oak_save);
            update_pc(&mut oak, &mut oak_save, DOWN);
            update_pc(&mut oak, &mut oak_save, DOWN);
            update_pc(&mut oak, &mut oak_save, A);
            skip_message(&mut oak, &mut oak_save);
            assert_eq!(oak.phase(), PcPhase::OaksConfirm);
            let mut oak_yes = oak.clone();
            update_pc(&mut oak_yes, &mut oak_save, UP);
            assert_cursor_repaint(
                &oak,
                &oak_yes,
                &oak_save,
                language,
                yes_no_position(false),
                yes_no_position(true),
            );
        }
    }
}
