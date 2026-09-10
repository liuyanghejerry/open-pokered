//! The overworld Start menu (party view / bag / save), the scene-opened
//! shop (`openShop`), and the game-over whiteout.
//!
//! These live in a child module of [`super`] so the `impl RunnerGame` blocks
//! below can drive the runner's private state directly (the battle module's
//! pattern of a self-contained screen doesn't fit: the menu owns the
//! runner's party/inventory/money between battles).
//!
//! # Start menu
//!
//! Start in the overworld opens a pause menu (B/Start closes; the overworld
//! is frozen underneath): **Party** (read-only member details from the
//! party-table records plus the persistent HP/MP/status), **Bag** (the
//! inventory; records with a positive heal field can be used on a living,
//! not-full member — capped at max HP, count decremented; fainted members
//! are NOT revived), **Save** (writes the save file immediately, always
//! allowed from the menu), **Close**.
//!
//! # Shops
//!
//! `ScriptCommand::OpenShop { items }` (from `game.openShop([...])`) suspends
//! the scene and opens a shop whose root offers **Buy / Sell / Exit** (B on a
//! list returns to the root; B/Exit on the root resumes the scene with
//! `Void`). Buy: the given items with their record `price` (default 0) and
//! the player's money; A buys (money −= price, inventory += 1, unaffordable
//! entries are marked and rejected). Sell: the player's inventory entries
//! with a positive count, each at `floor(price / 2)` (items priced 0 sell
//! for 0 — allowed); A sells one (money += , count −= 1).
//!
//! # Money
//!
//! The runner owns a `u32`, initialized from the manifest's `shop.startMoney`
//! (default 100) and persisted in the v3 save. The currency label
//! (`shop.currency`, default "G") shows in the shop UI and the Bag.
//!
//! # Whiteout
//!
//! A lost battle arms the whiteout ([`RunnerGame::start_whiteout`]), which
//! fires when the scene that received `"lose"` finishes: a brief blackout, a
//! "<Name> collapsed…" line, then the whole party is healed to full (status
//! cleared) and the player returns to the entry map's spawn — flags,
//! inventory and money kept. Map-less projects only heal. There is no
//! heal-point system yet: the respawn is always the entry spawn.

use std::collections::VecDeque;

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::overworld::types::Direction;
use dotzuki_engine::render::{FrameBuffer, Rgba, TileRect, Ui};
use dotzuki_engine_script::command::CommandResult;
use dotzuki_engine_script::engine::ScriptEngine;
use dotzuki_renderer::input::{GbButton, InputState};
use dotzuki_ui::widgets::flex_menu::{draw_flex_menu, FlexMenuState};
use dotzuki_ui::FrameBufferPainter;

use super::{draw_textbox, find_spawn, paginate, Mode, RunnerGame, SCREEN_H, SCREEN_W};
use crate::battle::{
    draw_panel, exp_to_next, get_num, get_str, growth_stat, read_record, record_ids, text,
    PartyMemberState, BASIC_ATTACK_NAME,
};
use crate::manifest::{DEFAULT_CURRENCY, DEFAULT_SKILLS_FIELD};
use crate::project::LoadedProject;

/// Blackout frames before the whiteout message shows.
const WHITEOUT_BLACKOUT_FRAMES: u32 = 30;

// ── mode payloads ─────────────────────────────────────────────────────────────

/// Which Start-menu view is showing.
pub enum MenuView {
    /// The root entries: Party / Bag / Save / Close.
    Root,
    /// The read-only party detail list.
    Party,
    /// The inventory list.
    Bag,
    /// Picking the party member a bag item is used on.
    BagTarget { item: String },
    /// A one-line message ("Game saved.", "It won't have any effect.", …);
    /// any button dismisses back to `back`.
    Note { text: String, back: NoteBack },
}

/// Where a dismissed [`MenuView::Note`] returns to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteBack {
    Root,
    Bag,
}

/// Live Start-menu state.
pub struct MenuState {
    pub view: MenuView,
    pub cursor: usize,
}

impl MenuState {
    pub fn new() -> Self {
        Self {
            view: MenuView::Root,
            cursor: 0,
        }
    }
}

/// One shop shelf entry (resolved from the item records at open time).
#[derive(Debug, Clone)]
pub struct ShopItem {
    /// Item record id.
    pub id: String,
    /// Display name (record `name`, else the id).
    pub name: String,
    /// Record `price` (default 0).
    pub price: u32,
}

/// Which shop view is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopView {
    /// The root entries: Buy / Sell / Exit.
    Root,
    /// The shelf (buy).
    Buy,
    /// The player's sellable inventory.
    Sell,
}

/// Live shop state: the suspended scene engine plus the shelf.
pub struct ShopState {
    pub engine: ScriptEngine,
    pub items: Vec<ShopItem>,
    pub cursor: usize,
    /// Transient message line ("Bought a Potion!" / "Not enough money…").
    pub note: Option<String>,
    /// The current view (root / buy / sell).
    pub view: ShopView,
}

impl ShopState {
    pub fn new(engine: ScriptEngine, items: Vec<ShopItem>) -> Self {
        Self {
            engine,
            items,
            cursor: 0,
            note: None,
            view: ShopView::Root,
        }
    }
}

/// The game-over whiteout phase machine: blackout first, then the message.
pub struct WhiteoutState {
    /// Blackout frames left before the message shows.
    pub blackout: u32,
    /// The message pages (A advances; draining them lands the whiteout).
    pub pages: VecDeque<String>,
}

// ── bilingual labels ──────────────────────────────────────────────────────────

/// The small en/zh label table for the menu/shop/whiteout UI.
#[derive(Debug, Clone, Copy)]
struct Labels {
    party: &'static str,
    bag: &'static str,
    save: &'static str,
    close: &'static str,
    saved: &'static str,
    no_effect: &'static str,
    empty: &'static str,
    no_party: &'static str,
    use_on_whom: &'static str,
    skills: &'static str,
    money: &'static str,
    buy: &'static str,
    sell: &'static str,
    exit: &'static str,
    buy_prompt: &'static str,
    sell_prompt: &'static str,
    bought: &'static str,
    sold: &'static str,
    no_money: &'static str,
    nothing_for_sale: &'static str,
    nothing_to_sell: &'static str,
}

const LABELS_EN: Labels = Labels {
    party: "Party",
    bag: "Bag",
    save: "Save",
    close: "Close",
    saved: "Game saved.",
    no_effect: "It won't have any effect.",
    empty: "(empty)",
    no_party: "No party members.",
    use_on_whom: "Use on whom?",
    skills: "Skills",
    money: "Money",
    buy: "Buy",
    sell: "Sell",
    exit: "Exit",
    buy_prompt: "What would you like?",
    sell_prompt: "What will you sell?",
    bought: "Bought",
    sold: "Sold",
    no_money: "Not enough money…",
    nothing_for_sale: "Nothing for sale.",
    nothing_to_sell: "Nothing to sell.",
};

const LABELS_ZH: Labels = Labels {
    party: "队伍",
    bag: "背包",
    save: "存档",
    close: "关闭",
    saved: "已保存。",
    no_effect: "没有任何效果。",
    empty: "（空）",
    no_party: "还没有伙伴。",
    use_on_whom: "给谁使用？",
    skills: "技能",
    money: "金钱",
    buy: "购买",
    sell: "出售",
    exit: "离开",
    buy_prompt: "要点什么？",
    sell_prompt: "要卖什么？",
    bought: "买到了",
    sold: "卖出了",
    no_money: "金钱不足……",
    nothing_for_sale: "没有商品。",
    nothing_to_sell: "没有可出售的物品。",
};

fn labels(lang: &str) -> Labels {
    if lang == "zh" {
        LABELS_ZH
    } else {
        LABELS_EN
    }
}

/// "<name> recovered <n> HP!" (interpolated, so it can't sit in `Labels`).
fn recovered_line(lang: &str, name: &str, n: u32) -> String {
    if lang == "zh" {
        format!("{name} 恢复了 {n} 点 HP！")
    } else {
        format!("{name} recovered {n} HP!")
    }
}

/// "Bought a <name>!" shop note.
fn bought_line(lang: &str, name: &str) -> String {
    if lang == "zh" {
        format!("{}{name}！", labels(lang).bought)
    } else {
        format!("{} a {name}!", labels(lang).bought)
    }
}

/// "Sold a <name>!" shop note.
fn sold_line(lang: &str, name: &str) -> String {
    if lang == "zh" {
        format!("{}{name}！", labels(lang).sold)
    } else {
        format!("{} a {name}!", labels(lang).sold)
    }
}

/// "<name> collapsed…" whiteout line.
fn collapsed_line(lang: &str, name: &str) -> String {
    if lang == "zh" {
        format!("{name} 倒下了……")
    } else {
        format!("{name} collapsed…")
    }
}

// ── record-derived info (items / party) ───────────────────────────────────────

/// The menu/shop view of one item record: display name, heal amount
/// (usability gate) and price. Without a `battle.items` block every item is
/// an unusable, priceless id.
#[derive(Debug, Clone)]
pub struct ItemInfo {
    pub name: String,
    pub heal: u32,
    pub price: u32,
}

/// Resolve an item id against the manifest's `battle.items` table.
pub fn item_info(project: &LoadedProject, id: &str) -> ItemInfo {
    let fallback = || ItemInfo {
        name: id.to_string(),
        heal: 0,
        price: 0,
    };
    let Some(items) = project
        .manifest()
        .battle
        .as_ref()
        .and_then(|b| b.items.as_ref())
    else {
        return fallback();
    };
    let Some(dir) = project.table_dir_rel(&items.table) else {
        return fallback();
    };
    match read_record(project.files().as_ref(), &dir, id) {
        Ok(rec) => ItemInfo {
            name: get_str(&rec, "name").unwrap_or(id).to_string(),
            heal: get_num(&rec, &items.heal_field).unwrap_or(0),
            price: get_num(&rec, "price").unwrap_or(0),
        },
        Err(e) => {
            log::warn!("item record '{id}': {e:#}");
            fallback()
        }
    }
}

/// One party member as the menu displays it: base stats from the record
/// (growth-applied when the manifest has a `battle.levels` block), current
/// HP/MP/status from the runner's persistent state.
#[derive(Debug, Clone)]
pub struct MemberInfo {
    pub id: String,
    pub name: String,
    pub hp: u32,
    pub max_hp: u32,
    pub mp: u32,
    pub max_mp: u32,
    pub status: Option<String>,
    /// Current level (1 when levels are off).
    pub level: u8,
    /// EXP progress toward the next level (0 when levels are off).
    pub exp: u32,
    /// EXP needed for the next level (`Some` only with a `levels` block).
    pub exp_to_next: Option<u32>,
    pub attack: u32,
    pub defense: u32,
    pub speed: u32,
    pub element: Option<String>,
    /// Skill display names (record ids resolved through the skills table).
    pub skills: Vec<String>,
}

/// The party as the menu displays it: every record of the party table
/// (sorted by id), stats via the manifest's `battle` field mapping — with
/// the level-growth multiplier when a `levels` block is present (level from
/// the persistent state, else the record's `levelField`, default 1) — and
/// current HP/MP/status from `state` (full when no battle/save produced one
/// yet). Empty when the project has no battle section / party table.
pub fn party_info(project: &LoadedProject, state: Option<&[PartyMemberState]>) -> Vec<MemberInfo> {
    let Some(battle) = project.manifest().battle.as_ref() else {
        return Vec::new();
    };
    let Some(party_ref) = battle.party.as_ref() else {
        return Vec::new();
    };
    let Some(dir) = project.table_dir_rel(&party_ref.table) else {
        return Vec::new();
    };
    let stats = battle.stats.clone().unwrap_or_default();
    let skills_field = battle
        .skills
        .as_ref()
        .map(|s| s.field.clone())
        .unwrap_or_else(|| DEFAULT_SKILLS_FIELD.to_string());
    let skills_dir = battle
        .skills
        .as_ref()
        .and_then(|s| project.table_dir_rel(&s.table));
    let mut out = Vec::new();
    for id in record_ids(project.files().as_ref(), &dir) {
        let Ok(rec) = read_record(project.files().as_ref(), &dir, &id) else {
            continue;
        };
        let cur = state.and_then(|s| s.iter().find(|m| m.id == id));
        // v2-c: the persistent level wins; the record's levelField is the
        // fallback (default 1 ⇒ ×1, numerically identical to v1).
        let (level, exp, exp_to_next) = match &battle.levels {
            Some(levels) => {
                let level = cur.map(|m| m.level.max(1)).unwrap_or_else(|| {
                    get_num(&rec, &levels.level_field).unwrap_or(1).min(255) as u8
                });
                (
                    level,
                    cur.map(|m| m.exp).unwrap_or(0),
                    Some(exp_to_next(levels.curve.base, levels.curve.exponent, level)),
                )
            }
            None => (1, 0, None),
        };
        let grown = |raw: u32| match &battle.levels {
            Some(levels) => growth_stat(raw, level, levels.growth),
            None => raw,
        };
        let max_hp = grown(get_num(&rec, &stats.hp).unwrap_or(1));
        let max_mp = grown(
            battle
                .resource
                .as_deref()
                .and_then(|f| get_num(&rec, f))
                .unwrap_or(0),
        );
        out.push(MemberInfo {
            name: get_str(&rec, "name").unwrap_or(&id).to_string(),
            hp: cur.map(|m| m.hp.min(max_hp)).unwrap_or(max_hp),
            max_hp,
            mp: cur.map(|m| m.mp.min(max_mp)).unwrap_or(max_mp),
            max_mp,
            status: cur.and_then(|m| m.status.clone()),
            level,
            exp,
            exp_to_next,
            attack: grown(get_num(&rec, &stats.attack).unwrap_or(1)),
            defense: grown(get_num(&rec, &stats.defense).unwrap_or(1)),
            speed: grown(get_num(&rec, &stats.speed).unwrap_or(1)),
            element: get_str(&rec, "element").map(str::to_string),
            skills: skill_names(
                &rec,
                &skills_field,
                project.files().as_ref(),
                skills_dir.as_deref(),
            ),
            id,
        });
    }
    out
}

/// A record's skill display names: the configured skills field's ids,
/// resolved through the skills table (raw id when the table/record is
/// missing); the built-in Attack when the list is empty.
fn skill_names(
    rec: &serde_json::Value,
    field: &str,
    files: &dyn crate::vfs::ProjectFiles,
    skills_dir: Option<&str>,
) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(ids) = rec.get(field).and_then(|v| v.as_array()) {
        for id in ids.iter().filter_map(|v| v.as_str()) {
            let name = skills_dir
                .and_then(|dir| read_record(files, dir, id).ok())
                .and_then(|r| get_str(&r, "name").map(str::to_string))
                .unwrap_or_else(|| id.to_string());
            names.push(name);
        }
    }
    if names.is_empty() {
        names.push(BASIC_ATTACK_NAME.to_string());
    }
    names
}

// ── RunnerGame: shared bits ───────────────────────────────────────────────────

impl RunnerGame {
    /// The currency label (manifest `shop.currency`, default "G").
    pub(crate) fn currency(&self) -> &str {
        self.project
            .manifest()
            .shop
            .as_ref()
            .map(|s| s.currency.as_str())
            .unwrap_or(DEFAULT_CURRENCY)
    }

    /// Resolve the item ids an `openShop` command names into shelf entries
    /// (unknown ids open as name=id, price 0 — never a scene deadlock).
    pub(crate) fn build_shop_items(&self, ids: &[String]) -> Vec<ShopItem> {
        ids.iter()
            .map(|id| {
                let info = item_info(&self.project, id);
                ShopItem {
                    id: id.clone(),
                    name: info.name,
                    price: info.price,
                }
            })
            .collect()
    }

    /// Materialize the persistent inventory from the manifest's starting
    /// counts the first time menu/shop code needs it.
    fn ensure_inventory(&mut self) {
        if self.inventory.is_none() {
            self.inventory = Some(
                self.project
                    .manifest()
                    .battle
                    .as_ref()
                    .and_then(|b| b.items.as_ref())
                    .map(|i| i.starting.clone())
                    .unwrap_or_default(),
            );
        }
    }

    /// Materialize the persistent party state (full HP/MP, no status) the
    /// first time menu code needs it (e.g. a Bag heal before any battle).
    /// Level/exp seed from the records (the `levelField`, default 1 / 0).
    fn ensure_party_state(&mut self) {
        if self.party_state.is_some() {
            return;
        }
        let members = party_info(&self.project, None);
        if members.is_empty() {
            return;
        }
        self.party_state = Some(
            members
                .into_iter()
                .map(|m| PartyMemberState {
                    id: m.id,
                    hp: m.max_hp,
                    mp: m.max_mp,
                    status: None,
                    level: m.level,
                    exp: m.exp,
                })
                .collect(),
        );
    }

    /// Inventory entries with a positive count, sorted by item id.
    fn bag_entries(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .inventory
            .as_ref()
            .map(|inv| {
                inv.iter()
                    .filter(|(_, count)| **count > 0)
                    .map(|(id, _)| id.clone())
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        ids
    }
}

// ── RunnerGame: the Start menu ────────────────────────────────────────────────

impl RunnerGame {
    /// One frame of the Start menu.
    pub(crate) fn update_menu(&mut self, mut state: MenuState, input: &InputState) {
        let l = labels(&self.lang);
        let cancelled =
            input.is_just_pressed(GbButton::B) || input.is_just_pressed(GbButton::Start);
        let confirmed = input.is_just_pressed(GbButton::A);
        let view = std::mem::replace(&mut state.view, MenuView::Root);
        state.view = match view {
            MenuView::Root => {
                move_cursor(input, &mut state.cursor, 4);
                if cancelled {
                    self.mode = Mode::Overworld;
                    return;
                }
                if !confirmed {
                    MenuView::Root
                } else {
                    let cursor = state.cursor;
                    state.cursor = 0;
                    match self.pick_root(cursor) {
                        Some(view) => view,
                        None => {
                            self.mode = Mode::Overworld;
                            return;
                        }
                    }
                }
            }
            MenuView::Party => {
                if confirmed || cancelled {
                    state.cursor = 0;
                    MenuView::Root
                } else {
                    MenuView::Party
                }
            }
            MenuView::Bag => {
                if cancelled {
                    state.cursor = 0;
                    MenuView::Root
                } else {
                    let entries = self.bag_entries();
                    if entries.is_empty() {
                        MenuView::Bag
                    } else {
                        move_cursor(input, &mut state.cursor, entries.len());
                        if confirmed {
                            let id = entries[state.cursor % entries.len()].clone();
                            if item_info(&self.project, &id).heal > 0 {
                                self.ensure_party_state();
                                state.cursor = 0;
                                MenuView::BagTarget { item: id }
                            } else {
                                MenuView::Note {
                                    text: l.no_effect.to_string(),
                                    back: NoteBack::Bag,
                                }
                            }
                        } else {
                            MenuView::Bag
                        }
                    }
                }
            }
            MenuView::BagTarget { item } => {
                if cancelled {
                    MenuView::Bag
                } else {
                    let members = party_info(&self.project, self.party_state.as_deref());
                    if members.is_empty() {
                        MenuView::Bag
                    } else {
                        move_cursor(input, &mut state.cursor, members.len());
                        if confirmed {
                            let member = members[state.cursor % members.len()].clone();
                            self.use_bag_item(&item, &member)
                        } else {
                            MenuView::BagTarget { item }
                        }
                    }
                }
            }
            MenuView::Note { text, back } => {
                if confirmed || cancelled {
                    state.cursor = 0;
                    match back {
                        NoteBack::Root => MenuView::Root,
                        NoteBack::Bag => MenuView::Bag,
                    }
                } else {
                    MenuView::Note { text, back }
                }
            }
        };
        self.mode = Mode::Menu(state);
    }

    /// The root-entry pick: `None` = Close.
    fn pick_root(&mut self, cursor: usize) -> Option<MenuView> {
        let l = labels(&self.lang);
        match cursor {
            0 => Some(MenuView::Party),
            1 => {
                self.ensure_inventory();
                Some(MenuView::Bag)
            }
            2 => {
                // Saving is always allowed from the menu, even where the
                // automatic stable-state saves are off.
                self.write_save_now();
                Some(MenuView::Note {
                    text: l.saved.to_string(),
                    back: NoteBack::Root,
                })
            }
            _ => None,
        }
    }

    /// Apply a bag item to a party member: heal (capped at max), decrement
    /// the count. Fainted or full-HP members reject with the classic line.
    fn use_bag_item(&mut self, item: &str, member: &MemberInfo) -> MenuView {
        let l = labels(&self.lang);
        if member.hp == 0 || member.hp >= member.max_hp {
            return MenuView::Note {
                text: l.no_effect.to_string(),
                back: NoteBack::Bag,
            };
        }
        let healed = item_info(&self.project, item)
            .heal
            .min(member.max_hp - member.hp);
        if let Some(party) = &mut self.party_state {
            if let Some(p) = party.iter_mut().find(|p| p.id == member.id) {
                p.hp += healed;
            }
        }
        self.ensure_inventory();
        if let Some(inv) = &mut self.inventory {
            let count = inv.entry(item.to_string()).or_insert(0);
            *count = count.saturating_sub(1);
        }
        MenuView::Note {
            text: recovered_line(&self.lang, &member.name, healed),
            back: NoteBack::Bag,
        }
    }

    /// The rows the menu currently displays (see [`RunnerGame::menu_lines`]).
    pub(crate) fn menu_lines_for(&self, state: &MenuState) -> Vec<String> {
        let l = labels(&self.lang);
        match &state.view {
            MenuView::Root => [
                l.party.to_string(),
                l.bag.to_string(),
                l.save.to_string(),
                l.close.to_string(),
            ]
            .to_vec(),
            MenuView::Party => {
                let members = party_info(&self.project, self.party_state.as_deref());
                if members.is_empty() {
                    return vec![l.no_party.to_string()];
                }
                let mut lines = Vec::new();
                for m in members {
                    // Level/EXP show only with a `battle.levels` block — the
                    // no-levels rows stay byte-identical to v1.
                    let mut row = match m.exp_to_next {
                        Some(_) => format!(
                            "{} Lv {} HP {}/{} MP {}/{}",
                            m.name, m.level, m.hp, m.max_hp, m.mp, m.max_mp
                        ),
                        None => format!(
                            "{} HP {}/{} MP {}/{}",
                            m.name, m.hp, m.max_hp, m.mp, m.max_mp
                        ),
                    };
                    if let Some(status) = &m.status {
                        row.push_str(&format!(" ({status})"));
                    }
                    lines.push(row);
                    let mut stats = format!("ATK {} DEF {} SPD {}", m.attack, m.defense, m.speed);
                    if let Some(element) = &m.element {
                        stats.push_str(&format!(" {element}"));
                    }
                    lines.push(stats);
                    if let Some(need) = m.exp_to_next {
                        lines.push(format!("EXP {}/{need}", m.exp));
                    }
                    lines.push(format!("{}: {}", l.skills, m.skills.join(", ")));
                }
                lines
            }
            MenuView::Bag => {
                let entries = self.bag_entries();
                if entries.is_empty() {
                    return vec![l.empty.to_string()];
                }
                entries
                    .iter()
                    .map(|id| {
                        let count = self
                            .inventory
                            .as_ref()
                            .and_then(|inv| inv.get(id))
                            .copied()
                            .unwrap_or(0);
                        format!("{} ×{count}", item_info(&self.project, id).name)
                    })
                    .collect()
            }
            MenuView::BagTarget { .. } => {
                let members = party_info(&self.project, self.party_state.as_deref());
                if members.is_empty() {
                    return vec![l.no_party.to_string()];
                }
                members
                    .iter()
                    .map(|m| {
                        let row = format!("{} HP {}/{}", m.name, m.hp, m.max_hp);
                        if m.hp == 0 || m.hp >= m.max_hp {
                            format!("× {row}")
                        } else {
                            row
                        }
                    })
                    .collect()
            }
            MenuView::Note { text, .. } => vec![text.clone()],
        }
    }

    /// Draw the Start menu over the frozen overworld.
    pub(crate) fn draw_menu(&self, fb: &mut FrameBuffer, state: &MenuState) {
        let l = labels(&self.lang);
        match &state.view {
            MenuView::Root | MenuView::Bag | MenuView::BagTarget { .. } => {
                if !matches!(state.view, MenuView::Root) {
                    // Money header above the list (shop parity).
                    text(
                        fb,
                        &format!("{}: {} {}", l.money, self.money, self.currency()),
                        16,
                        12,
                        Rgba::rgb(0xF0, 0xF0, 0xF0),
                    );
                }
                let rows = self.menu_lines_for(state);
                draw_corner_menu(fb, &rows, state.cursor);
                if let MenuView::BagTarget { .. } = &state.view {
                    draw_textbox(fb, l.use_on_whom);
                }
            }
            MenuView::Party => {
                draw_panel(fb, 8, 8, (SCREEN_W - 16) as u32, (SCREEN_H - 16) as u32);
                for (i, line) in self.menu_lines_for(state).iter().enumerate() {
                    text(
                        fb,
                        line,
                        20,
                        20 + i as u32 * 12,
                        Rgba::rgb(0xF0, 0xF0, 0xF0),
                    );
                }
            }
            MenuView::Note { text, .. } => draw_textbox(fb, text),
        }
    }
}

// ── RunnerGame: the shop ──────────────────────────────────────────────────────

impl RunnerGame {
    /// One frame of the shop: the root offers Buy / Sell / Exit (B on a
    /// list returns to the root; B/Exit on the root exits and resumes the
    /// suspended scene with `Void`). A buys on the shelf (unaffordable
    /// entries rejected) and sells one on the Sell list.
    pub(crate) fn update_shop(&mut self, mut state: ShopState, input: &InputState) {
        if state.note.is_some() {
            if input.is_just_pressed(GbButton::A) || input.is_just_pressed(GbButton::B) {
                state.note = None;
            }
            self.mode = Mode::Shop(Box::new(state));
            return;
        }
        let cancelled = input.is_just_pressed(GbButton::B);
        let confirmed = input.is_just_pressed(GbButton::A);
        match state.view {
            ShopView::Root => {
                move_cursor(input, &mut state.cursor, 3);
                if cancelled || (confirmed && state.cursor % 3 == 2) {
                    // B or Exit: close the shop, resume the scene.
                    let mut engine = state.engine;
                    let cmd = self.signal(&mut engine, CommandResult::Void);
                    self.pump(engine, cmd, false);
                    return;
                }
                if confirmed {
                    match state.cursor % 3 {
                        0 => state.view = ShopView::Buy,
                        _ => {
                            self.ensure_inventory();
                            state.view = ShopView::Sell;
                        }
                    }
                    state.cursor = 0;
                }
            }
            ShopView::Buy => {
                if cancelled {
                    state.view = ShopView::Root;
                    state.cursor = 0;
                } else if !state.items.is_empty() {
                    move_cursor(input, &mut state.cursor, state.items.len());
                    if confirmed {
                        let item = state.items[state.cursor % state.items.len()].clone();
                        if self.money >= item.price {
                            self.money -= item.price;
                            self.ensure_inventory();
                            if let Some(inv) = &mut self.inventory {
                                *inv.entry(item.id.clone()).or_insert(0) += 1;
                            }
                            state.note = Some(bought_line(&self.lang, &item.name));
                        } else {
                            state.note = Some(labels(&self.lang).no_money.to_string());
                        }
                    }
                }
            }
            ShopView::Sell => {
                if cancelled {
                    state.view = ShopView::Root;
                    state.cursor = 0;
                } else {
                    let entries = self.sell_entries();
                    if !entries.is_empty() {
                        move_cursor(input, &mut state.cursor, entries.len());
                        if confirmed {
                            let entry = entries[state.cursor % entries.len()].clone();
                            self.money = self.money.saturating_add(entry.price);
                            if let Some(inv) = &mut self.inventory {
                                let count = inv.entry(entry.id.clone()).or_insert(0);
                                *count = count.saturating_sub(1);
                                if *count == 0 {
                                    inv.remove(&entry.id);
                                }
                            }
                            state.note = Some(sold_line(&self.lang, &entry.name));
                        }
                    }
                }
            }
        }
        self.mode = Mode::Shop(Box::new(state));
    }

    /// The Sell list: the inventory entries with a positive count (sorted by
    /// item id), each at `floor(price / 2)` — items priced 0 sell for 0.
    fn sell_entries(&self) -> Vec<SellEntry> {
        self.bag_entries()
            .into_iter()
            .map(|id| {
                let info = item_info(&self.project, &id);
                let count = self
                    .inventory
                    .as_ref()
                    .and_then(|inv| inv.get(&id))
                    .copied()
                    .unwrap_or(0);
                SellEntry {
                    id,
                    name: info.name,
                    price: info.price / 2,
                    count,
                }
            })
            .collect()
    }

    /// The rows the shop displays (see [`RunnerGame::shop_lines`]).
    pub(crate) fn shop_lines_for(&self, state: &ShopState) -> Vec<String> {
        let mut rows = self.shop_rows(state);
        if let Some(note) = &state.note {
            rows.push(note.clone());
        }
        rows
    }

    /// The rows of the current view only (no note): the root entries, the
    /// shelf ("Potion 20 G", unaffordable prefixed ×), or the Sell list
    /// ("Potion ×3 10 G").
    fn shop_rows(&self, state: &ShopState) -> Vec<String> {
        let l = labels(&self.lang);
        match state.view {
            ShopView::Root => [l.buy.to_string(), l.sell.to_string(), l.exit.to_string()].to_vec(),
            ShopView::Buy => {
                if state.items.is_empty() {
                    return vec![l.nothing_for_sale.to_string()];
                }
                let currency = self.currency();
                state
                    .items
                    .iter()
                    .map(|item| {
                        let row = format!("{} {} {}", item.name, item.price, currency);
                        if item.price > self.money {
                            format!("× {row}")
                        } else {
                            row
                        }
                    })
                    .collect()
            }
            ShopView::Sell => {
                let entries = self.sell_entries();
                if entries.is_empty() {
                    return vec![l.nothing_to_sell.to_string()];
                }
                let currency = self.currency();
                entries
                    .iter()
                    .map(|entry| {
                        format!(
                            "{} ×{} {} {}",
                            entry.name, entry.count, entry.price, currency
                        )
                    })
                    .collect()
            }
        }
    }

    /// Draw the shop over the frozen overworld: money header, the current
    /// view's list, prompt/note textbox.
    pub(crate) fn draw_shop(&self, fb: &mut FrameBuffer, state: &ShopState) {
        let l = labels(&self.lang);
        text(
            fb,
            &format!("{}: {} {}", l.money, self.money, self.currency()),
            16,
            12,
            Rgba::rgb(0xF0, 0xF0, 0xF0),
        );
        let rows = self.shop_rows(state);
        draw_corner_menu(fb, &rows, state.cursor);
        match &state.note {
            Some(note) => draw_textbox(fb, note),
            None => match state.view {
                ShopView::Sell => draw_textbox(fb, l.sell_prompt),
                _ => draw_textbox(fb, l.buy_prompt),
            },
        }
    }
}

/// One Sell-list entry: an inventory stack and its `floor(price / 2)` value.
#[derive(Debug, Clone)]
struct SellEntry {
    id: String,
    name: String,
    price: u32,
    count: u32,
}

// ── RunnerGame: the whiteout ──────────────────────────────────────────────────

impl RunnerGame {
    /// Open the whiteout: blackout, then the "<Name> collapsed…" line. The
    /// heal + respawn land when the message is dismissed ([`apply_whiteout`]).
    pub(crate) fn start_whiteout(&mut self) {
        let name = match self.party_state.as_ref().and_then(|p| p.first()) {
            Some(first) => party_info(&self.project, self.party_state.as_deref())
                .into_iter()
                .find(|m| m.id == first.id)
                .map(|m| m.name)
                .unwrap_or_else(|| first.id.clone()),
            None if self.lang == "zh" => "你".to_string(),
            None => "You".to_string(),
        };
        self.mode = Mode::Whiteout(WhiteoutState {
            blackout: WHITEOUT_BLACKOUT_FRAMES,
            pages: paginate(&collapsed_line(&self.lang, &name)),
        });
    }

    /// One frame of the whiteout: count the blackout down, page the message
    /// on A/B, land the heal + respawn when it drains.
    pub(crate) fn update_whiteout(&mut self, mut state: WhiteoutState, input: &InputState) {
        if state.blackout > 0 {
            state.blackout -= 1;
            self.mode = Mode::Whiteout(state);
            return;
        }
        if input.is_just_pressed(GbButton::A) || input.is_just_pressed(GbButton::B) {
            state.pages.pop_front();
        }
        if state.pages.is_empty() {
            self.apply_whiteout();
        } else {
            self.mode = Mode::Whiteout(state);
        }
    }

    /// Land the whiteout: heal the whole party to full (status cleared) and
    /// return to the entry map's spawn — flags, inventory and money kept.
    /// Map-less projects only heal.
    fn apply_whiteout(&mut self) {
        self.heal_party_to_full();
        if let Ok(entry) = self.project.entry_map() {
            let on_entry = self.map.as_ref().is_some_and(|m| m.id() == entry);
            if on_entry {
                let map = self.map.as_ref().expect("checked above");
                let spawn = find_spawn(map);
                self.actor.place(spawn.0, spawn.1, Direction::Down);
                self.center_camera();
                self.camera.update(0.0);
            } else if let Ok(map) = self.project.load_map(&entry) {
                let spawn = find_spawn(&map);
                if !self.enter_map(&entry, spawn, Direction::Down) {
                    log::warn!("whiteout: entry map '{entry}' failed to load — staying put");
                }
            }
        }
        self.mode = if self.map.is_some() {
            Mode::Overworld
        } else {
            Mode::Idle
        };
        // A stable state again (the losing scene long finished): persist the
        // healed party/position when automatic saves are on.
        self.write_save();
        log::info!("whiteout: party healed, player returned to the entry spawn");
    }

    /// Set every party member to full HP/MP from the records and clear
    /// status. With a `levels` block the pools heal to the GROWN maxima at
    /// the member's current level. No-op without a battle section/party
    /// state (a whiteout only follows a lost battle, which needs both).
    fn heal_party_to_full(&mut self) {
        let Some(battle) = self.project.manifest().battle.as_ref() else {
            return;
        };
        let Some(party_ref) = battle.party.as_ref() else {
            return;
        };
        let Some(dir) = self.project.table_dir_rel(&party_ref.table) else {
            return;
        };
        let stats = battle.stats.clone().unwrap_or_default();
        let resource = battle.resource.clone();
        let levels = battle.levels.clone();
        let Some(party) = &mut self.party_state else {
            return;
        };
        for member in party.iter_mut() {
            if let Ok(rec) = read_record(self.project.files().as_ref(), &dir, &member.id) {
                let grown = |raw: u32| match &levels {
                    Some(levels) => growth_stat(raw, member.level.max(1), levels.growth),
                    None => raw,
                };
                member.hp = grown(get_num(&rec, &stats.hp).unwrap_or(1));
                member.mp = grown(
                    resource
                        .as_deref()
                        .and_then(|f| get_num(&rec, f))
                        .unwrap_or(0),
                );
                member.status = None;
            }
        }
    }
}

// ── drawing helpers ───────────────────────────────────────────────────────────

/// A flex-menu box anchored top-right (the classic pause-menu corner),
/// sized to its rows.
fn draw_corner_menu(fb: &mut FrameBuffer, items: &[String], cursor: usize) {
    let n = items.len() as u32;
    if n == 0 {
        return;
    }
    let max_len = items.iter().map(|o| o.chars().count()).max().unwrap_or(1) as u32;
    // +4: left/right border, cursor column, one padding column.
    let w = (max_len + 4).clamp(8, 24);
    let h = n + 2;
    let tx = (40 - w) as i32;
    let config = MenuConfig::new(
        TileRect::new(tx.max(0) as u32, 1, w, h),
        None,
        TileRect::new(tx.max(0) as u32 + 1, 2, w - 2, n),
        Default::default(),
    );
    let state = FlexMenuState {
        cursor,
        scroll_offset: 0,
    };
    let mut painter = FrameBufferPainter::new(fb);
    let mut ui = Ui::new(&mut painter);
    draw_flex_menu(items, &[config], &state, items.len(), &mut ui);
}

/// Move a cursor over `n` entries with Up/Down.
fn move_cursor(input: &InputState, cursor: &mut usize, n: usize) {
    let n = n.max(1);
    if input.is_just_pressed(GbButton::Up) {
        *cursor = (*cursor + n - 1) % n;
    } else if input.is_just_pressed(GbButton::Down) {
        *cursor = (*cursor + 1) % n;
    }
}
