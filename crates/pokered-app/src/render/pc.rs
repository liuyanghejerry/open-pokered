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

/// Translate a main-menu label produced by `pc_screen::main_menu_labels`
/// ("BILL's PC", "<NAME>'s PC", "PROF.OAK's PC", "#MON LEAGUE", "LOG OFF").
fn zh_main_menu_label(label: &str) -> String {
    match label {
        "BILL's PC" => "正辉的电脑".to_string(),
        "SOMEONE's PC" => "某个人的电脑".to_string(),
        "PROF.OAK's PC" => "大木博士的电脑".to_string(),
        "#MON LEAGUE" => "宝可梦联盟".to_string(),
        "LOG OFF" => lang_data::ui_label("LOG OFF", true).to_string(),
        _ => match label.strip_suffix("'s PC") {
            Some(name) => format!("{}的电脑", name),
            None => label.to_string(),
        },
    }
}

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

/// Exact static line → Chinese for the messages `pokered_core::pc_screen`
/// produces (and the rating texts it shares with the Hall of Fame stats
/// page). Unknown lines pass through unchanged.
const PC_LINE_ZH: &[(&str, &str)] = &[
    ("Switch on!", "开机！"),
    ("the PC.", "电脑。"),
    ("PC.", "电脑。"),
    ("Accessed BILL's", "访问了正辉的"),
    ("Accessed someone's", "访问了某个人的"),
    ("Accessed my PC.", "访问了自己的电脑。"),
    ("Accessed PROF.", "访问了大木博士的"),
    ("OAK's PC.", "电脑。"),
    ("Accessed #DEX", "访问了图鉴"),
    ("Rating System.", "评价系统。"),
    ("LEAGUE's site.", "联盟的站点。"),
    ("Accessed the HALL", "访问了名人堂"),
    ("OF FAME List.", "名单。"),
    ("What? There are", "什么？这里"),
    ("no #MON here!", "没有宝可梦！"),
    ("You can't take", "你不能带走"),
    ("any more #MON.", "更多的宝可梦。"),
    ("Deposit #MON", "请先存放"),
    ("first.", "宝可梦。"),
    ("You can't deposit", "你不能存放"),
    ("the last #MON!", "最后的宝可梦！"),
    ("Oops! This Box is", "哎呀！这个盒子"),
    ("full of #MON.", "装满了宝可梦。"),
    ("taken out.", "取出来了。"),
    ("released outside.", "放生了。"),
    ("There is nothing", "没有存放"),
    ("stored.", "任何东西。"),
    ("You have nothing", "你没有可"),
    ("to deposit.", "存放的东西。"),
    ("That's too impor-", "这太重要了，"),
    ("tant to toss!", "不能扔掉！"),
    ("No room left to", "没有空间"),
    ("store items.", "存放道具。"),
    ("You can't carry", "你带不了"),
    ("any more items.", "更多的道具。"),
    ("stored via PC.", "已存入电脑。"),
    ("Withdrew", "取出了"),
    ("Threw away", "扔掉了"),
    ("Closed link to", "已断开与大木"),
    ("PROF.OAK's PC.", "博士电脑的连线。"),
    ("#DEX comp-", "图鉴完成"),
    ("letion is:", "度："),
    ("PROF.OAK's", "大木博士"),
    ("Rating:", "评价："),
    // Professor Oak's #DEX rating texts (pokedex_rating.asm table).
    ("You still have", "你还有很多"),
    ("lots to do.", "要做的事。"),
    ("Look for #MON", "去草丛里"),
    ("in grassy areas!", "找宝可梦吧！"),
    ("You're on the", "你正走在"),
    ("right track!", "正确的路上！"),
    ("Get a FLASH HM", "去我的助手"),
    ("from my AIDE!", "那里拿闪光！"),
    ("You still need", "你还需要"),
    ("more #MON!", "更多宝可梦！"),
    ("Try to catch", "试着捕捉"),
    ("other species!", "其他种类！"),
    ("Good, you're", "不错，你"),
    ("trying hard!", "很努力！"),
    ("Get an ITEMFINDER", "去我的助手"),
    ("from my AIDE!", "那里拿探宝器！"),
    ("Looking good!", "看起来不错！"),
    ("Go find my AIDE", "去找我的助手"),
    ("when you get 50!", "凑满50只时！"),
    ("You finally got at", "你终于凑满"),
    ("least 50 species!", "至少50只了！"),
    ("Be sure to get", "记得去拿"),
    ("EXP.ALL from my", "我助手那里的"),
    ("AIDE!", "学习装置！"),
    ("Ho! This is geting", "哦！越来越"),
    ("even better!", "好了！"),
    ("Very good!", "非常好！"),
    ("Go fish for some", "去钓一些"),
    ("marine #MON!", "水里的宝可梦！"),
    ("Wonderful!", "太棒了！"),
    ("Do you like to", "你喜欢"),
    ("collect things?", "收集东西吗？"),
    ("I'm impressed!", "我很佩服！"),
    ("It must have been", "这一定"),
    ("difficult to do!", "很难做到！"),
    ("least 100 species!", "至少100只了！"),
    ("I can't believe", "真不敢相信"),
    ("how good you are!", "你有多厉害！"),
    ("You even have the", "你甚至拥有"),
    ("evolved forms of", "宝可梦的"),
    ("#MON! Super!", "进化形态！厉害！"),
    ("Excellent! Trade", "太棒了！和"),
    ("with friends to", "朋友交换"),
    ("get some more!", "得到更多吧！"),
    ("Outstanding!", "太出色了！"),
    ("You've become a", "你已经成了"),
    ("real pro at this!", "真正的高手！"),
    ("I have nothing", "我已经"),
    ("left to say!", "无话可说了！"),
    ("You're the", "你就是"),
    ("authority now!", "权威了！"),
    ("Your #DEX is", "你的图鉴"),
    ("entirely complete!", "完全完成了！"),
    ("Congratulations!", "恭喜你！"),
    // Change-box confirmation (renderer-built, routed through the same map).
    ("When you change a", "更换宝可梦盒子时，"),
    ("#MON BOX, data", "数据会被"),
    ("will be saved.", "保存。"),
    ("Is that okay?", "这样可以吗？"),
    // Oak's rating prompt.
    ("Want to get your", "想让大木博士"),
    ("#DEX rated?", "评价图鉴吗？"),
];

/// Translate one message line produced by `pokered_core::pc_screen` (or the
/// renderer's own confirmations) at display time. Exact lines hit
/// [`PC_LINE_ZH`]; dynamic templates (names/numbers embedded) are handled
/// below; anything unknown passes through unchanged.
pub(crate) fn zh_pc_line(line: &str) -> String {
    if let Some(zh) = PC_LINE_ZH.iter().find(|(en, _)| *en == line).map(|(_, zh)| *zh) {
        return zh.to_string();
    }
    // "<NAME> turned on" (PC boot).
    if let Some(name) = line.strip_suffix(" turned on") {
        return format!("{}打开了", zh_name(name));
    }
    // "<NAME> was ..." — deposit / release results.
    if let Some(name) = line.strip_suffix(" was") {
        return format!("{}被", zh_name(name));
    }
    // "<NAME> is ..." — withdraw result.
    if let Some(name) = line.strip_suffix(" is") {
        return format!("{}被", zh_name(name));
    }
    // "stored in Box N."
    if let Some(n) = line.strip_prefix("stored in Box ") {
        return format!("存入了盒子{}。", n);
    }
    // "Got <NAME>."
    if let Some(name) = line.strip_prefix("Got ") {
        return format!("得到了{}", zh_name(name));
    }
    // "Bye <NAME>!"
    if let Some(name) = line.strip_prefix("Bye ").and_then(|s| s.strip_suffix('!')) {
        return format!("再见了{}！", zh_name(name));
    }
    // "<N> #MON seen" / "<N> #MON owned" (Oaks rating summary).
    if let Some(n) = line.strip_suffix(" #MON seen") {
        return format!("已见{}只", n);
    }
    if let Some(n) = line.strip_suffix(" #MON owned") {
        return format!("拥有{}只", n);
    }
    // "<ITEM>." — the trailing period line of a withdraw/toss result.
    if let Some(name) = line.strip_suffix('.') {
        return format!("{}。", zh_name(name));
    }
    line.to_string()
}
