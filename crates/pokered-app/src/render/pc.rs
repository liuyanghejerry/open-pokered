//! Renderer for the PC storage screens (Bill's PC, the player's item PC, and
//! PROF.OAK's #DEX rating).
//!
//! GB-style presentation over the same primitives as the elevator/menu
//! renderers: text boxes with `>` cursors, single/double-spaced lists, YES/NO
//! popups. All logic lives in `pokered_core::pc_screen`.

use pokered_core::battle::state::Pokemon;
use pokered_core::game_state::Lang;
use pokered_core::pc_screen::{ItemListMode, MonListMode, PcPhase, PcScreen, PC_LIST_VISIBLE_ROWS};
use pokered_core::save::SaveData;
use pokered_data::lang_data;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::palette::GRAYSCALE_SPRITE_PALETTE;
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::{blit_tileset, draw_text_box, species_to_sprite_name};
use crate::render::battle_i18n::zh_name;
use pokered_data::ui_text::{zh_main_menu_label, zh_pc_line};

const BG: Rgba = Rgba::WHITE;
const FG: Rgba = Rgba::BLACK;

const T: u32 = 8; // tile size in pixels

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
    let rows = lines.len().clamp(1, 5) as u32;
    let bh = rows + 1; // interior tiles: lines at 8px pitch
    let by = 144 - (bh + 2) * T;
    draw_text_box(fb, 0, by, 18, bh, FG);
    for (i, line) in lines.iter().take(5).enumerate() {
        let shown = if is_zh { zh_pc_line(line) } else { line.clone() };
        draw_text(&shown, T, by + (1 + i as u32) * T, FG, fb);
    }
}

/// YES/NO popup on the right side (original: TWO_OPTION_MENU at hlcoord 14,7).
fn draw_yes_no(selected_yes: bool, fb: &mut FrameBuffer, is_zh: bool) {
    let bx = 14 * T;
    let by = 7 * T;
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
    fb: &mut FrameBuffer,
) {
    let bh = visible as u32 + 1;
    draw_text_box(fb, bx, by, bw, bh, FG);
    for (row, (i, label)) in rows
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .enumerate()
    {
        let y = by + (1 + row as u32) * T;
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
            draw_list(0, 0, 18, 8, &rows, cursor, scroll, fb);
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
            let bx = 11 * T;
            draw_text_box(fb, bx, 0, 7, 12, FG);
            for i in 0..12usize {
                let y = (1 + i as u32) * T;
                let marker = if i == pc.box_cursor() { ">" } else { " " };
                let name = if is_zh {
                    format!("盒子{:>2}", i + 1)
                } else {
                    format!("BOX{:>2}", i + 1)
                };
                draw_text(&format!("{}{}", marker, name), bx + T, y, FG, fb);
                let non_empty = save
                    .pc_storage
                    .get_box(i)
                    .map(|b| !b.is_empty())
                    .unwrap_or(false);
                if non_empty {
                    // pokeball-ish marker dot at the row's right edge
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
            draw_list(0, 0, 18, 8, &rows, cursor, scroll, fb);
            match pc.phase() {
                PcPhase::ItemQuantity => {
                    // "How many?" + the running quantity (players_pc.asm
                    // DisplayChooseQuantityMenu).
                    let name = rows.get(cursor).cloned().unwrap_or_default();
                    let prompt = if is_zh { "几个？" } else { "How many?" };
                    draw_message(&[prompt.to_string()], fb, is_zh);
                    let bx = 13 * T;
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
