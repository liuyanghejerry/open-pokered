//! WASM preview shim for pokered-ui layouts.
//!
//! Consumed by `tools/game-editor` (Vue) to render menu layouts inside the browser
//! while the user edits the layout JSON. The Rust side is stateless: every edit
//! ships the full JSON; we parse, render, and return RGBA bytes.
//!
//! ## Stage 2.7 status
//!
//! All 15 menus are fully implemented with at least 2 mock state variants each,
//! selectable via `mock_state_id` (u32). The **mart** menu is the first to honor
//! live `layout_json` overrides; all other menus still use their compiled-in
//! static layout (forward-compat stubs).
//!
//! - **`layout_json` for mart**: parsed as `ScreenLayout`, extracts the
//!   `"main_menu"` variant, and converts it to `MartMainMenuLayout`. Falls
//!   back to the static const when empty or malformed.
//! - **`layout_json` for all other menus**: accepted but ignored (forward-compat).
//! - **`mock_state_id`** selects a pre-canned mock state per menu. Unrecognized
//!   values silently fall back to the default state (0).
//! - Unknown menu names return an empty `Vec<u8>`.

use wasm_bindgen::prelude::*;

use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::impl_traits::PokemonRenderData;
use pokered_core::game_state::Lang;
use pokered_renderer::{FrameBuffer, RenderConfig, Rgba};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::Ui;

// ── core state types ─────────────────────────────────────────────────
use pokered_core::battle::menu::{BagMenuState, BattleMenuInput, BattleMenuState, MoveMenuState, MoveSlot};
use pokered_core::battle::state::Pokemon;
use pokered_core::game_state::SaveFileSummary;
use pokered_core::main_menu::MainMenuState;
use pokered_core::naming_screen::{NamingScreenState, NamingScreenType};
use pokered_core::options_menu::{BattleAnimation, BattleStyle, GameOptions, OptionsMenuState, TextSpeed};
use pokered_core::party_screen::PartyScreenState;
use pokered_core::pokemon::stats::create_pokemon;
use pokered_core::save_menu::{SaveMenuState, SaveScreenInfo};
use pokered_core::start_menu::StartMenuState;
use pokered_core::stats_screen::StatsScreenState;

// ── helpers ──────────────────────────────────────────────────────────

/// Create a new framebuffer, render inside it via a FrameBufferPainter + Ui,
/// and return the raw RGBA pixels.
fn render_with<F>(draw_fn: F, lang: Lang) -> Vec<u8>
where
    F: FnOnce(&mut Ui<FrameBufferPainter>),
{
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    {
        let mut painter =
            FrameBufferPainter::new(&mut fb)
                .with_lang(lang);
        let mut ui = Ui::new(&mut painter);
        draw_fn(&mut ui);
    }
    fb.data
}

/// Build a minimal Pokemon for mock state construction,
/// using the canonical create_pokemon helper from pokered_core.
fn mock_mon(species: Species, level: u8) -> Pokemon {
    create_pokemon(species, level, [0xFF, 0xFF]).expect("valid DV bytes")
}

/// Default save info for mock states that display the save screen.
fn default_save_info() -> SaveScreenInfo {
    SaveScreenInfo {
        player_name: "RED".into(),
        num_badges: 0,
        pokedex_owned: 5,
        play_time_hours: 12,
        play_time_minutes: 34,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Public entry point (wasm-bindgen export)
// ═══════════════════════════════════════════════════════════════════════

/// Render a single menu layout to an RGBA framebuffer.
///
/// # Arguments
/// * `menu_name` — one of the 15 supported menu identifiers (e.g. `"bag"`, `"stats"`).
/// * `layout_json` — full ScreenLayout JSON from the editor. **Mart** honors this
///   parameter; all other menus treat it as forward-compat (ignored).
/// * `mock_state_id` — selects a pre-canned mock state. Unrecognized values fall back to 0 (default).
///
/// # Returns
/// `Vec<u8>` of length 92160 (160×144×4 RGBA8) for supported menus, empty `Vec` for unknown menus.
#[wasm_bindgen]
pub fn render_layout(menu_name: &str, layout_json: &str, mock_state_id: u32, lang: u32) -> Vec<u8> {
    // Stage 2.7: layout_json is honored by the mart menu; all others treat it as
    // forward-compat (ignored). Once other menus gain JSON-parameterized draw_*
    // functions, this parameter will be parsed and passed through.
    let lang = if lang == 1 { Lang::Zh } else { Lang::En };

    match menu_name {
        "bag" => render_bag(mock_state_id, layout_json),
        "battle_bag" => render_battle_bag(mock_state_id, layout_json),
        "battle_main" => render_battle_main(mock_state_id, layout_json),
        "battle_move" => render_battle_move(mock_state_id, layout_json),
        "battle_party" => render_battle_party(mock_state_id, layout_json),
        "battle_text" => render_battle_text(mock_state_id, layout_json, lang),
        "dialog" => render_dialog(mock_state_id, layout_json, lang),
        "main" => render_main(mock_state_id, layout_json),
        "mart" => render_mart(mock_state_id, layout_json),
        "naming" => render_naming(mock_state_id, layout_json),
        "options" => render_options(mock_state_id, layout_json),
        "party" => render_party(mock_state_id, layout_json),
        "save" => render_save(mock_state_id, layout_json),
        "start" => render_start(mock_state_id, layout_json),
        "stats" => render_stats(mock_state_id, layout_json),
        "pokedex" => render_pokedex(mock_state_id, layout_json),
        "yes_no" => render_yes_no(mock_state_id, layout_json),
        "oak_speech" => render_oak_speech(mock_state_id, layout_json),
        _ => Vec::new(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Per-menu render helpers
// ═══════════════════════════════════════════════════════════════════════

// ── bag ───────────────────────────────────────────────────────────────
fn render_bag(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    let (items, cursor): (Vec<(ItemId, u8)>, usize) = match mock_state_id {
        0 => (
            vec![
                (ItemId::PokeBall, 5),
                (ItemId::Potion, 3),
                (ItemId::Antidote, 2),
            ],
            0,
        ),
        1 => (vec![], 0), // empty
        2 => (
            vec![
                (ItemId::PokeBall, 5),
                (ItemId::Potion, 3),
                (ItemId::SuperPotion, 1),
                (ItemId::Antidote, 2),
                (ItemId::ParlyzHeal, 1),
                (ItemId::Awakening, 1),
                (ItemId::BurnHeal, 1),
                (ItemId::IceHeal, 1),
            ],
            0,
        ),
        3 => (
            vec![
                (ItemId::PokeBall, 5),
                (ItemId::Potion, 3),
                (ItemId::Antidote, 2),
            ],
            2, // cursor at last item
        ),
        _ => return render_bag(0, layout_json), // fallback to default
    };

    let layout = resolve_bag_layout(layout_json);

    render_with(|ui| {
        let rd = PokemonRenderData::new(false);
        pokered_ui::menus::bag::draw(&items, cursor, &layout, ui, &rd);
    }, Lang::default())
}

fn resolve_bag_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::BagDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_bag_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::BagDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_bag_static()
}

fn clone_bag_static() -> pokered_data::ui_layout::schema::BagDefaultLayout {
    let s = &pokered_data::ui_layout::schema::BAG_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::BagDefaultLayout {
        box_0: s.box_0.clone(),
        list: s.list.clone(),
    }
}

// ── battle_bag ────────────────────────────────────────────────────────
fn render_battle_bag(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    let items: Vec<(ItemId, u8)> = match mock_state_id {
        0 => vec![
            (ItemId::Potion, 3),
            (ItemId::SuperPotion, 1),
            (ItemId::Antidote, 2),
        ],
        1 => vec![], // empty
        2 => vec![
            (ItemId::Potion, 3),
            (ItemId::SuperPotion, 1),
            (ItemId::HyperPotion, 1),
            (ItemId::FullHeal, 2),
            (ItemId::Revive, 1),
            (ItemId::PokeDoll, 1),
        ],
        _ => return render_battle_bag(0, layout_json),
    };

    let layout = resolve_battle_bag_layout(layout_json);
    let state = BagMenuState::new(items);
    render_with(|ui| {
        let rd = PokemonRenderData::new(false);
        pokered_ui::menus::battle_bag::draw(&state, &layout, ui, &rd);
    }, Lang::default())
}

fn resolve_battle_bag_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::BattleBagDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_battle_bag_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::BattleBagDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_battle_bag_static()
}

fn clone_battle_bag_static() -> pokered_data::ui_layout::schema::BattleBagDefaultLayout {
    let s = &pokered_data::ui_layout::schema::BATTLE_BAG_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::BattleBagDefaultLayout {
        list: s.list.clone(),
    }
}

// ── battle_main ───────────────────────────────────────────────────────
fn render_battle_main(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    let mut state = BattleMenuState::new();
    match mock_state_id {
        0 => {} // default: FIGHT selected (row=0, col=0)
        1 => {
            // cursor at RUN (bottom-right)
            state.update_frame(BattleMenuInput {
                down: true,
                right: true,
                ..BattleMenuInput::none()
            });
        }
        _ => {} // fallback
    }

    let layout = resolve_battle_main_layout(layout_json);

    render_with(|ui| {
        pokered_ui::menus::battle_main::draw(&state, &layout, ui, Lang::default());
    }, Lang::default())
}

fn resolve_battle_main_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::BattleMainDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_battle_main_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::BattleMainDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_battle_main_static()
}

fn clone_battle_main_static() -> pokered_data::ui_layout::schema::BattleMainDefaultLayout {
    let s = &pokered_data::ui_layout::schema::BATTLE_MAIN_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::BattleMainDefaultLayout {
        base: s.base.clone(),
        box_0: s.box_0.clone(),
        cursor: s.cursor,
    }
}

// ── battle_move ───────────────────────────────────────────────────────
fn render_battle_move(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    let moves: Vec<MoveSlot> = match mock_state_id {
        0 => vec![
            MoveSlot { move_id: MoveId::Tackle, current_pp: 35, max_pp: 35, is_disabled: false },
            MoveSlot { move_id: MoveId::Growl, current_pp: 40, max_pp: 40, is_disabled: false },
            MoveSlot { move_id: MoveId::LeechSeed, current_pp: 10, max_pp: 10, is_disabled: false },
            MoveSlot { move_id: MoveId::VineWhip, current_pp: 25, max_pp: 25, is_disabled: false },
        ],
        1 => vec![
            MoveSlot { move_id: MoveId::Scratch, current_pp: 35, max_pp: 35, is_disabled: false },
            MoveSlot { move_id: MoveId::Growl, current_pp: 40, max_pp: 40, is_disabled: false },
        ],
        2 => vec![
            MoveSlot { move_id: MoveId::Tackle, current_pp: 0, max_pp: 35, is_disabled: false },
            MoveSlot { move_id: MoveId::Growl, current_pp: 40, max_pp: 40, is_disabled: true },
        ],
        _ => return render_battle_move(0, layout_json),
    };

    let layout = resolve_battle_move_layout(layout_json);
    let state = MoveMenuState::new(moves);
    render_with(|ui| {
        let rd = PokemonRenderData::new(false);
        pokered_ui::menus::battle_move::draw(&state, &layout, ui, &rd);
    }, Lang::default())
}

fn resolve_battle_move_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::BattleMoveDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_battle_move_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::BattleMoveDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_battle_move_static()
}

fn clone_battle_move_static() -> pokered_data::ui_layout::schema::BattleMoveDefaultLayout {
    let s = &pokered_data::ui_layout::schema::BATTLE_MOVE_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::BattleMoveDefaultLayout {
        base: s.base.clone(),
        box_0: s.box_0.clone(),
        box_1: s.box_1.clone(),
        list_default: s.list_default.clone(),
    }
}

// ── battle_party ─────────────────────────────────────────────────────
fn render_battle_party(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    let (party, cursor): (Vec<Pokemon>, usize) = match mock_state_id {
        0 => (
            vec![
                mock_mon(Species::Bulbasaur, 12),
                mock_mon(Species::Pidgey, 10),
                mock_mon(Species::Rattata, 8),
            ],
            0,
        ),
        1 => (
            vec![
                mock_mon(Species::Charizard, 50),
                mock_mon(Species::Pidgeot, 42),
                mock_mon(Species::Raichu, 45),
                mock_mon(Species::Gyarados, 44),
                mock_mon(Species::Alakazam, 48),
                mock_mon(Species::Snorlax, 40),
            ],
            0,
        ),
        2 => (vec![], 0), // empty party (draw returns early)
        _ => return render_battle_party(0, layout_json),
    };

    let layout = resolve_battle_party_layout(layout_json);

    render_with(|ui| {
        pokered_ui::menus::battle_party::draw(&party, cursor, &layout, ui, false);
    }, Lang::default())
}

fn resolve_battle_party_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::BattlePartyDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_battle_party_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::BattlePartyDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_battle_party_static()
}

fn clone_battle_party_static() -> pokered_data::ui_layout::schema::BattlePartyDefaultLayout {
    let s = &pokered_data::ui_layout::schema::BATTLE_PARTY_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::BattlePartyDefaultLayout {
        box_0: s.box_0.clone(),
        cursor: s.cursor,
    }
}

// ── battle_text ──────────────────────────────────────────────────────
fn render_battle_text(mock_state_id: u32, layout_json: &str, lang: Lang) -> Vec<u8> {
    let (text, show_arrow): (&str, bool) = match mock_state_id {
        0 => if lang == Lang::Zh {
            ("妙蛙种子\n要做什么？", true)
        } else {
            ("What should\nBULBASAUR do?", true)
        },
        1 => if lang == Lang::Zh {
            ("对手的皮卡丘\n使用了十万伏特！", false)
        } else {
            ("Enemy PIKACHU used\nTHUNDERSHOCK!", false)
        },
        2 => if lang == Lang::Zh {
            ("效果拔群！", true)
        } else {
            ("It's super\neffective!", true)
        },
        _ => return render_battle_text(0, layout_json, lang),
    };

    let layout = resolve_battle_text_layout(layout_json);

    render_with(|ui| {
        pokered_ui::menus::battle_text::draw(text, show_arrow, &layout, ui, lang);
    }, lang)
}

fn resolve_battle_text_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::BattleTextDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_battle_text_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::BattleTextDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_battle_text_static()
}

fn clone_battle_text_static() -> pokered_data::ui_layout::schema::BattleTextDefaultLayout {
    let s = &pokered_data::ui_layout::schema::BATTLE_TEXT_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::BattleTextDefaultLayout {
        box_0: s.box_0.clone(),
        cursor: s.cursor,
    }
}

// ── dialog ────────────────────────────────────────────────────────────
fn render_dialog(mock_state_id: u32, layout_json: &str, lang: Lang) -> Vec<u8> {
    let (text, show_arrow): (&str, bool) = match mock_state_id {
        0 => if lang == Lang::Zh {
            ("欢迎来到宝可梦\n的世界！", true)
        } else {
            ("Hello there! Welcome\nto the world of POKeMON!", true)
        },
        1 => if lang == Lang::Zh {
            ("大木：这是我的孙子。\n他从你小时候就是你对手了。", false)
        } else {
            ("OAK: This is my grandson. He's been your rival since you were a baby.", false)
        },
        2 => if lang == Lang::Zh {
            ("收到！", false)
        } else {
            ("Got it!", false)
        },
        _ => return render_dialog(0, layout_json, lang),
    };

    let layout = resolve_dialog_layout(layout_json);

    render_with(|ui| {
        pokered_ui::menus::dialog::draw(text, show_arrow, &layout, ui, lang);
    }, lang)
}

fn resolve_dialog_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::DialogDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_dialog_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::DialogDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_dialog_static()
}

fn clone_dialog_static() -> pokered_data::ui_layout::schema::DialogDefaultLayout {
    let s = &pokered_data::ui_layout::schema::DIALOG_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::DialogDefaultLayout {
        box_0: s.box_0.clone(),
        cursor: s.cursor,
    }
}

// ── main ──────────────────────────────────────────────────────────────
fn render_main(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    let state = match mock_state_id {
        0 => MainMenuState::new(None), // no save: NEW GAME / OPTION
        1 => MainMenuState::new(Some(SaveFileSummary {
            player_name: b"RED".to_vec(),
            badges: 2,
            pokedex_owned: 30,
            play_time_hours: 5,
            play_time_minutes: 15,
            play_time_seconds: 0,
        })),
        _ => return render_main(0, layout_json),
    };

    let layout = resolve_main_layout(layout_json);

    render_with(|ui| {
        pokered_ui::menus::main::draw(&state, &layout, ui, Lang::default());
    }, Lang::default())
}

fn resolve_main_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::MainDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_main_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::MainDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_main_static()
}

fn clone_main_static() -> pokered_data::ui_layout::schema::MainDefaultLayout {
    let s = &pokered_data::ui_layout::schema::MAIN_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::MainDefaultLayout {
        menu: s.menu.clone(),
    }
}

// ── mart ──────────────────────────────────────────────────────────────
fn render_mart(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    match mock_state_id {
        0 | 1 => {
            let (idx, money): (usize, u32) = match mock_state_id {
                0 => (0, 12_345),
                1 => (2, 999),  // lower money, different cursor position
                _ => unreachable!(),
            };
            let layout = resolve_mart_layout(layout_json);
            render_with(|ui| {
                pokered_ui::menus::mart::draw_main_with_money(idx, money, &layout, ui, Lang::default());
            }, Lang::default())
        }
        2 => {
            let layout = resolve_mart_result_dialog_layout(layout_json);
            render_with(|ui| {
                pokered_ui::menus::mart::draw_result_dialog(
                    &["Here you are!", "Thank you!"],
                    &layout,
                    ui,
                );
            }, Lang::default())
        }
        3 => {
            let layout = resolve_mart_confirm_layout(layout_json);
            render_with(|ui| {
                use pokered_ui::menus::mart::ConfirmChoice;
                pokered_ui::menus::mart::draw_confirm(Lang::default(),
                    "Buy item for $300?",
                    ConfirmChoice::Yes,
                    &layout,
                    ui,
                );
            }, Lang::default())
        }
        4 => {
            let layout = resolve_mart_quantity_layout(layout_json);
            render_with(|ui| {
                pokered_ui::menus::mart::draw_quantity(
                    "POTION", 5, 300, 1500, 5000, &layout, pokered_core::game_state::Lang::En, ui,
                );
            }, Lang::default())
        }
        5 => {
            let layout = resolve_mart_buy_items_with_money_layout(layout_json);
            let items: Vec<pokered_data::items::ItemId> = vec![
                pokered_data::items::ItemId::Potion,
                pokered_data::items::ItemId::Antidote,
                pokered_data::items::ItemId::ParlyzHeal,
                pokered_data::items::ItemId::Awakening,
            ];
            render_with(|ui| {
                let rd = PokemonRenderData::new(false);
                pokered_ui::menus::mart::draw_buy_items_with_money(
                    &items, 1, 0, 3000, &layout, ui, pokered_core::game_state::Lang::En, &rd,
                );
            }, Lang::default())
        }
        6 => {
            let layout = resolve_mart_sell_items_with_money_layout(layout_json);
            let owned: Vec<(pokered_data::items::ItemId, u32)> = vec![
                (pokered_data::items::ItemId::Potion, 5),
                (pokered_data::items::ItemId::Antidote, 2),
                (pokered_data::items::ItemId::ParlyzHeal, 3),
            ];
            render_with(|ui| {
                let rd = PokemonRenderData::new(false);
                pokered_ui::menus::mart::draw_sell_items_with_money(
                    &owned, 1, 0, 3000, &layout, ui, pokered_core::game_state::Lang::En, &rd,
                );
            }, Lang::default())
        }
        _ => render_mart(0, layout_json),
    }
}

/// Resolve a `MartMainMenuLayout` from a JSON string, falling back to the
/// static `MART_MAIN_MENU_LAYOUT` const when the json is empty or malformed.
fn resolve_mart_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::MartMainMenuLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_mart_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("main_menu") {
                if let Some(layout) = schema::MartMainMenuLayout::from_main_menu_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_mart_static()
}

fn clone_mart_static() -> pokered_data::ui_layout::schema::MartMainMenuLayout {
    let s = &pokered_data::ui_layout::schema::MART_MAIN_MENU_LAYOUT;
    pokered_data::ui_layout::schema::MartMainMenuLayout {
        menu_box: s.menu_box.clone(),
        money_box: s.money_box.clone(),
        cursor: s.cursor,
        dynamic_labels: s.dynamic_labels.clone(),
    }
}

fn resolve_mart_result_dialog_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::MartResultDialogLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_mart_result_dialog_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("result_dialog") {
                if let Some(layout) = schema::MartResultDialogLayout::from_result_dialog_variant(&variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_mart_result_dialog_static()
}

fn clone_mart_result_dialog_static() -> pokered_data::ui_layout::schema::MartResultDialogLayout {
    let s = &pokered_data::ui_layout::schema::MART_RESULT_DIALOG_LAYOUT;
    pokered_data::ui_layout::schema::MartResultDialogLayout {
        result_box: s.result_box.clone(),
        dynamic_labels: s.dynamic_labels.clone(),
    }
}

fn resolve_mart_confirm_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::MartConfirmLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_mart_confirm_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("confirm") {
                if let Some(layout) = schema::MartConfirmLayout::from_confirm_variant(&variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_mart_confirm_static()
}

fn clone_mart_confirm_static() -> pokered_data::ui_layout::schema::MartConfirmLayout {
    let s = &pokered_data::ui_layout::schema::MART_CONFIRM_LAYOUT;
    pokered_data::ui_layout::schema::MartConfirmLayout {
        message_region: s.message_region.clone(),
        choice_box: s.choice_box.clone(),
        cursor: s.cursor,
    }
}

fn resolve_mart_quantity_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::MartQuantityLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_mart_quantity_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("quantity") {
                if let Some(layout) = schema::MartQuantityLayout::from_quantity_variant(&variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_mart_quantity_static()
}

fn clone_mart_quantity_static() -> pokered_data::ui_layout::schema::MartQuantityLayout {
    let s = &pokered_data::ui_layout::schema::MART_QUANTITY_LAYOUT;
    pokered_data::ui_layout::schema::MartQuantityLayout {
        detail_box: s.detail_box.clone(),
        money_box: s.money_box.clone(),
        dynamic_labels: s.dynamic_labels.clone(),
    }
}

fn resolve_mart_buy_items_with_money_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::MartBuyItemsWithMoneyLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_mart_buy_items_with_money_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("buy_items_with_money") {
                if let Some(layout) = schema::MartBuyItemsWithMoneyLayout::from_buy_items_with_money_variant(&variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_mart_buy_items_with_money_static()
}

fn clone_mart_buy_items_with_money_static() -> pokered_data::ui_layout::schema::MartBuyItemsWithMoneyLayout {
    let s = &pokered_data::ui_layout::schema::MART_BUY_ITEMS_WITH_MONEY_LAYOUT;
    pokered_data::ui_layout::schema::MartBuyItemsWithMoneyLayout {
        list_box: s.list_box.clone(),
        money_box: s.money_box.clone(),
        cursor: s.cursor,
        dynamic_labels: s.dynamic_labels.clone(),
    }
}

fn resolve_mart_sell_items_with_money_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::MartSellItemsWithMoneyLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_mart_sell_items_with_money_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("sell_items_with_money") {
                if let Some(layout) = schema::MartSellItemsWithMoneyLayout::from_sell_items_with_money_variant(&variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_mart_sell_items_with_money_static()
}

fn clone_mart_sell_items_with_money_static() -> pokered_data::ui_layout::schema::MartSellItemsWithMoneyLayout {
    let s = &pokered_data::ui_layout::schema::MART_SELL_ITEMS_WITH_MONEY_LAYOUT;
    pokered_data::ui_layout::schema::MartSellItemsWithMoneyLayout {
        list_box: s.list_box.clone(),
        money_box: s.money_box.clone(),
        cursor: s.cursor,
        dynamic_labels: s.dynamic_labels.clone(),
    }
}

// ── naming ────────────────────────────────────────────────────────────
fn render_naming(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    let state = match mock_state_id {
        0 => NamingScreenState::new(NamingScreenType::Player), // empty name
        1 => NamingScreenState::new(NamingScreenType::Pokemon), // empty name, different title
        2 => {
            let s = NamingScreenState::new(NamingScreenType::Player);
            s
        }
        _ => return render_naming(0, layout_json),
    };

    let layout = resolve_naming_layout(layout_json);

    render_with(|ui| {
        pokered_ui::menus::naming::draw(&state, &layout, ui, false);
    }, Lang::default())
}

fn resolve_naming_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::NamingDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_naming_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::NamingDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_naming_static()
}

fn clone_naming_static() -> pokered_data::ui_layout::schema::NamingDefaultLayout {
    let s = &pokered_data::ui_layout::schema::NAMING_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::NamingDefaultLayout {
        box_0: s.box_0.clone(),
        region_0: s.region_0.clone(),
    }
}

// ── options ───────────────────────────────────────────────────────────
fn render_options(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    let options = match mock_state_id {
        0 => GameOptions {
            text_speed: TextSpeed::Medium,
            battle_animation: BattleAnimation::On,
            battle_style: BattleStyle::Shift,
        },
        1 => GameOptions {
            text_speed: TextSpeed::Fast,
            battle_animation: BattleAnimation::Off,
            battle_style: BattleStyle::Set,
        },
        _ => return render_options(0, layout_json),
    };

    let layout = resolve_options_layout(layout_json);
    let state = OptionsMenuState::new(options);
    render_with(|ui| {
        pokered_ui::menus::options::draw(&state, &layout, ui, Lang::default());
    }, Lang::default())
}

fn resolve_options_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::OptionsDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_options_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::OptionsDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_options_static()
}

fn clone_options_static() -> pokered_data::ui_layout::schema::OptionsDefaultLayout {
    let s = &pokered_data::ui_layout::schema::OPTIONS_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::OptionsDefaultLayout {
        box_0: s.box_0.clone(),
        box_1: s.box_1.clone(),
        box_2: s.box_2.clone(),
        region_0: s.region_0.clone(),
        cursors: s.cursors.clone(),
        enum_position_map: s.enum_position_map.clone(),
    }
}

// ── party ─────────────────────────────────────────────────────────────
fn render_party(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    let party: Vec<Pokemon> = match mock_state_id {
        0 => vec![
            mock_mon(Species::Charmander, 14),
            mock_mon(Species::Spearow, 11),
            mock_mon(Species::NidoranM, 9),
        ],
        1 => vec![
            mock_mon(Species::Blastoise, 55),
            mock_mon(Species::Arcanine, 48),
            mock_mon(Species::Jolteon, 47),
            mock_mon(Species::Lapras, 45),
            mock_mon(Species::Dragonite, 55),
            mock_mon(Species::Mewtwo, 70),
        ],
        2 => vec![], // empty: draw shows fallback text
        _ => return render_party(0, layout_json),
    };

    let layout = resolve_party_layout(layout_json);
    let state = PartyScreenState::new(party);
    render_with(|ui| {
        pokered_ui::menus::party::draw(&state, &layout, ui, Lang::default());
    }, Lang::default())
}

fn resolve_party_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::PartyDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_party_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::PartyDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_party_static()
}

fn clone_party_static() -> pokered_data::ui_layout::schema::PartyDefaultLayout {
    let s = &pokered_data::ui_layout::schema::PARTY_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::PartyDefaultLayout {
        region_0: s.region_0.clone(),
        region_1: s.region_1.clone(),
    }
}

// ── save ──────────────────────────────────────────────────────────────
fn render_save(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    let (info, has_prev, different) = match mock_state_id {
        0 => (default_save_info(), false, false), // first-time save → AskSave
        1 => (default_save_info(), true, false),  // overwrite → ConfirmOverwrite
        2 => (
            SaveScreenInfo {
                player_name: "BLUE".into(),
                num_badges: 5,
                pokedex_owned: 70,
                play_time_hours: 30,
                play_time_minutes: 15,
            },
            true,
            false,
        ),
        _ => return render_save(0, layout_json),
    };

    let layout = resolve_save_layout(layout_json);
    let ask_layout = resolve_save_ask_prompt_layout(layout_json);
    let state = SaveMenuState::new(info, has_prev, different);
    render_with(|ui| {
        pokered_ui::menus::save::draw(&state, &layout, &ask_layout, ui, Lang::default());
    }, Lang::default())
}

fn resolve_save_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::SaveDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_save_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::SaveDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_save_static()
}

fn clone_save_static() -> pokered_data::ui_layout::schema::SaveDefaultLayout {
    let s = &pokered_data::ui_layout::schema::SAVE_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::SaveDefaultLayout {
        box_0: s.box_0.clone(),
        box_1: s.box_1.clone(),
        box_2: s.box_2.clone(),
    }
}

fn resolve_save_ask_prompt_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::SaveAskPromptLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_save_ask_prompt_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("ask_prompt") {
                if let Some(layout) = schema::SaveAskPromptLayout::from_ask_prompt_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_save_ask_prompt_static()
}

fn clone_save_ask_prompt_static() -> pokered_data::ui_layout::schema::SaveAskPromptLayout {
    let s = &pokered_data::ui_layout::schema::SAVE_ASK_PROMPT_LAYOUT;
    pokered_data::ui_layout::schema::SaveAskPromptLayout {
        box_0: s.box_0.clone(),
        box_1: s.box_1.clone(),
        cursor: s.cursor,
        enum_position_map: s.enum_position_map.clone(),
    }
}

// ── start ─────────────────────────────────────────────────────────────
fn render_start(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    let (has_pokedex, has_pokemon, is_link) = match mock_state_id {
        0 => (true, true, false),   // full menu: POKeDEX + POKeMON + ITEM + NAME + SAVE + OPTION + EXIT
        1 => (false, false, false), // minimal: ITEM + NAME + SAVE + OPTION + EXIT
        2 => (true, true, true),    // link mode: SAVE→RESET
        _ => return render_start(0, layout_json),
    };

    let layout = resolve_start_layout(layout_json);
    let state = StartMenuState::new(has_pokedex, has_pokemon, is_link);
    let player_name = if mock_state_id == 2 { "RED" } else { "ASH" };
    render_with(|ui| {
        pokered_ui::menus::start::draw(&state, player_name, &layout, ui, pokered_core::game_state::Lang::En);
    }, Lang::default())
}

fn resolve_start_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::StartDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_start_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::StartDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_start_static()
}

fn clone_start_static() -> pokered_data::ui_layout::schema::StartDefaultLayout {
    let s = &pokered_data::ui_layout::schema::START_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::StartDefaultLayout {
        menu: s.menu.clone(),
    }
}

// ── stats ─────────────────────────────────────────────────────────────
fn render_stats(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    let mut state = StatsScreenState::new(mock_mon(Species::Bulbasaur, 25));
    match mock_state_id {
        0 => {} // page 1: stats
        1 => {
            // toggle to page 2: moves
            use pokered_core::stats_screen::StatsScreenInput;
            state.update(StatsScreenInput { a: true, b: false });
        }
        _ => {} // fallback to page 1
    }

    let (page1, page2) = resolve_stats_layouts(layout_json);

    render_with(|ui| {
        let rd = PokemonRenderData::new(false);
        pokered_ui::menus::stats::draw(&state, &page1, &page2, ui, pokered_core::game_state::Lang::default(), &rd);
    }, Lang::default())
}

fn resolve_stats_layouts(
    layout_json: &str,
) -> (
    pokered_data::ui_layout::schema::StatsPage1Layout,
    pokered_data::ui_layout::schema::StatsPage2Layout,
) {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return (clone_stats_page1_static(), clone_stats_page2_static());
    }

    let parsed = serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json);
    match parsed {
        Ok(screen) => {
            let page1 = screen
                .variants
                .get("page1")
                .and_then(|v| schema::StatsPage1Layout::from_page1_variant(v))
                .unwrap_or_else(clone_stats_page1_static);
            let page2 = screen
                .variants
                .get("page2")
                .and_then(|v| schema::StatsPage2Layout::from_page2_variant(v))
                .unwrap_or_else(clone_stats_page2_static);
            (page1, page2)
        }
        Err(_) => (clone_stats_page1_static(), clone_stats_page2_static()),
    }
}

fn clone_stats_page1_static() -> pokered_data::ui_layout::schema::StatsPage1Layout {
    let s = &pokered_data::ui_layout::schema::STATS_PAGE1_LAYOUT;
    pokered_data::ui_layout::schema::StatsPage1Layout {
        box_0: s.box_0.clone(),
        region_0: s.region_0.clone(),
        prim_0: s.prim_0.clone(),
        bracket_0: s.bracket_0.clone(),
        bracket_1: s.bracket_1.clone(),
    }
}

fn clone_stats_page2_static() -> pokered_data::ui_layout::schema::StatsPage2Layout {
    let s = &pokered_data::ui_layout::schema::STATS_PAGE2_LAYOUT;
    pokered_data::ui_layout::schema::StatsPage2Layout {
        box_0: s.box_0.clone(),
        region_0: s.region_0.clone(),
        list_page2: s.list_page2.clone(),
    }
}

// ── pokedex ────────────────────────────────────────────────────────────
fn render_pokedex(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    use pokered_ui::menus::pokedex::{draw, PokedexEntryView};

    let (entry, page): (PokedexEntryView<'_>, usize) = match mock_state_id {
        0 => (
            PokedexEntryView {
                display_name: "BULBASAUR",
                category: "SEED",
                dex_num: 1,
                height_ft: 2,
                height_in: 4,
                weight_lb: "15.2",
                description: &[
                    "A strange seed was",
                    "planted on its back",
                    "at birth. The plant",
                    "sprouts and grows",
                    "with this POKeMON.",
                ],
                owned: true,
            },
            0,
        ),
        1 => (
            PokedexEntryView {
                display_name: "MEWTWO",
                category: "GENETIC",
                dex_num: 150,
                height_ft: 6,
                height_in: 7,
                weight_lb: "269.0",
                description: &[
                    "It was created by",
                    "a scientist after",
                    "years of horrific",
                    "gene splicing and",
                    "DNA engineering",
                    "experiments.",
                ],
                owned: true,
            },
            0,
        ),
        _ => return render_pokedex(0, layout_json),
    };

    let layout = resolve_pokedex_layout(layout_json);

    render_with(|ui| {
        draw(&entry, page, &layout, ui);
    }, Lang::default())
}

fn resolve_pokedex_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::PokedexDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_pokedex_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::PokedexDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_pokedex_static()
}

fn clone_pokedex_static() -> pokered_data::ui_layout::schema::PokedexDefaultLayout {
    let s = &pokered_data::ui_layout::schema::POKEDEX_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::PokedexDefaultLayout {
        frame: s.frame.clone(),
        cursor: s.cursor,
    }
}

// ── yes_no ─────────────────────────────────────────────────────────────
fn render_yes_no(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    use pokered_ui::menus::yes_no::draw;

    let (options, selected): (Vec<String>, u32) = match mock_state_id {
        0 => (vec!["YES".into(), "NO".into()], 0),
        1 => (vec!["YES".into(), "NO".into()], 1),
        2 => (vec!["HEAL".into(), "CANCEL".into()], 0),
        _ => return render_yes_no(0, layout_json),
    };

    let layout = resolve_yes_no_layout(layout_json);

    render_with(|ui| {
        draw(&options, selected, &layout, ui);
    }, Lang::default())
}

fn resolve_yes_no_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::YesNoDefaultLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_yes_no_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("default") {
                if let Some(layout) = schema::YesNoDefaultLayout::from_default_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_yes_no_static()
}

fn clone_yes_no_static() -> pokered_data::ui_layout::schema::YesNoDefaultLayout {
    let s = &pokered_data::ui_layout::schema::YES_NO_DEFAULT_LAYOUT;
    pokered_data::ui_layout::schema::YesNoDefaultLayout {
        box_0: s.box_0.clone(),
        cursor: s.cursor,
    }
}

// ── oak_speech ─────────────────────────────────────────────────────────
fn render_oak_speech(mock_state_id: u32, layout_json: &str) -> Vec<u8> {
    match mock_state_id {
        0 => {
            let layout = resolve_oak_speech_text_phase_layout(layout_json);
            render_with(|ui| {
                pokered_ui::menus::oak_speech::draw_text_phase(
                    "Hello there!",
                    "Welcome to the",
                    true,
                    &layout,
                    ui,
                );
            }, Lang::default())
        }
        1 => {
            let layout = resolve_oak_speech_text_phase_layout(layout_json);
            render_with(|ui| {
                pokered_ui::menus::oak_speech::draw_text_phase(
                    "Eevee! No!",
                    "Come back!",
                    false,
                    &layout,
                    ui,
                );
            }, Lang::default())
        }
        2 => {
            let layout = resolve_oak_speech_name_choice_layout(layout_json);
            render_with(|ui| {
                pokered_ui::menus::oak_speech::draw_name_choice(
                    &["NEW NAME", "RED", "ASH", "BLUE"],
                    0,
                    "Your name?",
                    &layout,
                    ui,
                );
            }, Lang::default())
        }
        _ => return render_oak_speech(0, layout_json),
    }
}

fn resolve_oak_speech_text_phase_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::OakSpeechTextPhaseLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_oak_speech_text_phase_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("text_phase") {
                if let Some(layout) = schema::OakSpeechTextPhaseLayout::from_text_phase_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_oak_speech_text_phase_static()
}

fn clone_oak_speech_text_phase_static() -> pokered_data::ui_layout::schema::OakSpeechTextPhaseLayout {
    let s = &pokered_data::ui_layout::schema::OAK_SPEECH_TEXT_PHASE_LAYOUT;
    pokered_data::ui_layout::schema::OakSpeechTextPhaseLayout {
        dialog_box: s.dialog_box.clone(),
        cursor: s.cursor,
    }
}

fn resolve_oak_speech_name_choice_layout(
    layout_json: &str,
) -> pokered_data::ui_layout::schema::OakSpeechNameChoiceLayout {
    use pokered_data::ui_layout::schema;

    if layout_json.is_empty() {
        return clone_oak_speech_name_choice_static();
    }

    match serde_json::from_str::<pokered_data::ui_layout::ScreenLayout>(layout_json) {
        Ok(screen) => {
            if let Some(variant) = screen.variants.get("name_choice") {
                if let Some(layout) = schema::OakSpeechNameChoiceLayout::from_name_choice_variant(variant) {
                    return layout;
                }
            }
        }
        Err(_) => {}
    }

    clone_oak_speech_name_choice_static()
}

fn clone_oak_speech_name_choice_static() -> pokered_data::ui_layout::schema::OakSpeechNameChoiceLayout {
    let s = &pokered_data::ui_layout::schema::OAK_SPEECH_NAME_CHOICE_LAYOUT;
    pokered_data::ui_layout::schema::OakSpeechNameChoiceLayout {
        name_list: s.name_list.clone(),
        prompt_box: s.prompt_box.clone(),
        cursor: s.cursor,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Optional debug helpers
// ═══════════════════════════════════════════════════════════════════════

/// Optional: enable detailed panic messages in browser console.
/// Call once on JS-side init in dev builds.
#[cfg(feature = "debug-panic-hook")]
#[wasm_bindgen]
pub fn install_panic_hook() {
    console_error_panic_hook::set_once();
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // The preview renders the pokered screen at GB resolution (160×144); the
    // engine no longer hardcodes this (resolution is per-game), and the lib above
    // builds its framebuffer with `RenderConfig::new(160, 144)`, so pin the same
    // GB fixture here.
    const SCREEN_WIDTH: u32 = 160;
    const SCREEN_HEIGHT: u32 = 144;

    const EXPECTED_FRAMEBUFFER_LEN: usize =
        (SCREEN_WIDTH as usize) * (SCREEN_HEIGHT as usize) * 4;

    fn assert_valid_framebuffer(bytes: &[u8], menu: &str) {
        assert_eq!(
            bytes.len(),
            EXPECTED_FRAMEBUFFER_LEN,
            "{menu} should return full RGBA framebuffer"
        );
        let all_white = bytes
            .chunks(4)
            .all(|chunk| chunk == [0xFF, 0xFF, 0xFF, 0xFF]);
        assert!(
            !all_white,
            "framebuffer should contain non-white pixels from drawing {menu}"
        );
    }

    // ── existing tests from 2.1b ─────────────────────────────────────
    #[test]
    fn render_mart_returns_full_framebuffer() {
        let bytes = render_layout("mart", "{}", 0, 0);
        assert_valid_framebuffer(&bytes, "mart");
    }

    #[test]
    fn render_start_returns_full_framebuffer() {
        let bytes = render_layout("start", "{}", 0, 0);
        assert_valid_framebuffer(&bytes, "start");
    }

    #[test]
    fn unknown_menu_returns_empty_vec() {
        let bytes = render_layout("nonexistent", "{}", 0, 0);
        assert!(bytes.is_empty(), "unknown menu should return empty Vec");
    }

    #[test]
    fn empty_json_is_accepted() {
        let bytes = render_layout("mart", "", 0, 0);
        assert_valid_framebuffer(&bytes, "mart (empty json)");
    }

    // ── per-menu smoke tests ─────────────────────────────────────────
    #[test]
    fn render_bag_returns_full_framebuffer() {
        let bytes = render_layout("bag", "", 0, 0);
        assert_valid_framebuffer(&bytes, "bag");
    }

    #[test]
    fn render_battle_bag_returns_full_framebuffer() {
        let bytes = render_layout("battle_bag", "", 0, 0);
        assert_valid_framebuffer(&bytes, "battle_bag");
    }

    #[test]
    fn render_battle_main_returns_full_framebuffer() {
        let bytes = render_layout("battle_main", "", 0, 0);
        assert_valid_framebuffer(&bytes, "battle_main");
    }

    #[test]
    fn render_battle_move_returns_full_framebuffer() {
        let bytes = render_layout("battle_move", "", 0, 0);
        assert_valid_framebuffer(&bytes, "battle_move");
    }

    #[test]
    fn render_battle_party_returns_full_framebuffer() {
        let bytes = render_layout("battle_party", "", 0, 0);
        assert_valid_framebuffer(&bytes, "battle_party");
    }

    #[test]
    fn render_battle_text_returns_full_framebuffer() {
        let bytes = render_layout("battle_text", "", 0, 0);
        assert_valid_framebuffer(&bytes, "battle_text");
    }

    #[test]
    fn render_dialog_returns_full_framebuffer() {
        let bytes = render_layout("dialog", "", 0, 0);
        assert_valid_framebuffer(&bytes, "dialog");
    }

    #[test]
    fn render_main_returns_full_framebuffer() {
        let bytes = render_layout("main", "", 0, 0);
        assert_valid_framebuffer(&bytes, "main");
    }

    #[test]
    fn render_naming_returns_full_framebuffer() {
        let bytes = render_layout("naming", "", 0, 0);
        assert_valid_framebuffer(&bytes, "naming");
    }

    #[test]
    fn render_options_returns_full_framebuffer() {
        let bytes = render_layout("options", "", 0, 0);
        assert_valid_framebuffer(&bytes, "options");
    }

    #[test]
    fn render_party_returns_full_framebuffer() {
        let bytes = render_layout("party", "", 0, 0);
        assert_valid_framebuffer(&bytes, "party");
    }

    #[test]
    fn render_save_returns_full_framebuffer() {
        let bytes = render_layout("save", "", 0, 0);
        assert_valid_framebuffer(&bytes, "save");
    }

    #[test]
    fn render_stats_returns_full_framebuffer() {
        let bytes = render_layout("stats", "", 0, 0);
        assert_valid_framebuffer(&bytes, "stats");
    }

    // ── mock state variant tests ─────────────────────────────────────
    #[test]
    fn mock_state_variant_renders_non_white() {
        // Verify alternate mock_state_id values produce valid output.
        let menus = [
            ("bag", 1),
            ("battle_bag", 2),
            ("battle_main", 1),
            ("battle_move", 1),
            ("battle_party", 1),
            ("battle_text", 1),
            ("dialog", 1),
            ("main", 1),
            ("mart", 1),
            ("naming", 1),
            ("options", 1),
            ("party", 1),
            ("save", 1),
            ("start", 1),
            ("stats", 1),
        ];
        for (menu, state_id) in &menus {
            let bytes = render_layout(menu, "", *state_id, 0);
            assert_eq!(
                bytes.len(),
                EXPECTED_FRAMEBUFFER_LEN,
                "alternate mock state for {menu} should return full framebuffer"
            );
        }
    }

    #[test]
    fn unknown_mock_state_falls_back_to_default() {
        // A huge or nonsense mock_state_id should silently fall back to default (0).
        let bytes = render_layout("bag", "", 9999, 0);
        assert_eq!(
            bytes.len(),
            EXPECTED_FRAMEBUFFER_LEN,
            "unknown mock_state_id should fall back and render successfully"
        );
        // Same for another menu
        let bytes2 = render_layout("stats", "", 99, 0);
        assert_eq!(bytes2.len(), EXPECTED_FRAMEBUFFER_LEN);
    }

    #[test]
    fn mart_json_round_trip_changes_framebuffer() {
        // Render with empty json — uses the static const.
        let bytes_static = render_layout("mart", "", 0, 0);
        assert_valid_framebuffer(&bytes_static, "mart (empty json)");

        // Obtain the canonical JSON from the build-time registry.
        let canonical_json =
            pokered_data::ui_layout::schema::get_layout_json("mart")
                .expect("get_layout_json('mart') must be Some");

        // Parse-and-render via render_layout — should produce identical bytes.
        let bytes_parsed = render_layout("mart", canonical_json, 0, 0);
        assert_valid_framebuffer(&bytes_parsed, "mart (parsed json)");
        assert_eq!(
            bytes_static, bytes_parsed,
            "serialize/parse round-trip must be lossless"
        );

        // Mutate the layout: move the menu box one tile right.
        let mut json_val: serde_json::Value =
            serde_json::from_str(canonical_json).expect("canonical JSON must be valid");
        json_val["variants"]["main_menu"]["children"][0]["rect"]["tx"] =
            serde_json::Value::Number(serde_json::Number::from(1));
        let mutated_json = serde_json::to_string(&json_val)
            .expect("serialize mutated layout");

        let bytes_mutated = render_layout("mart", &mutated_json, 0, 0);
        assert_valid_framebuffer(&bytes_mutated, "mart (mutated json)");
        assert_ne!(
            bytes_static, bytes_mutated,
            "moving the menu box must produce a visibly different framebuffer"
        );
    }

    #[test]
    fn all_menus_default_render_valid() {
        let menus = [
            "bag",
            "battle_bag",
            "battle_main",
            "battle_move",
            "battle_party",
            "battle_text",
            "dialog",
            "main",
            "mart",
            "naming",
            "oak_speech",
            "options",
            "party",
            "pokedex",
            "save",
            "start",
            "stats",
            "yes_no",
        ];
        for menu in &menus {
            let bytes = render_layout(menu, "", 0, 0);
            assert_eq!(
                bytes.len(),
                EXPECTED_FRAMEBUFFER_LEN,
                "{menu}: should return full RGBA framebuffer"
            );
        }
    }

    // ── flex container tests ──────────────────────────────────────────

    #[test]
    fn main_flex_auto_height_adapts_to_items() {
        // Mock state 0: no save → 2 items (NEW GAME / OPTION)
        // Mock state 1: save → 7 items (POKéDEX / POKéMON / ITEM / NAME / SAVE / OPTION / EXIT)
        // Auto height should produce visually different framebuffers.
        let bytes_few = render_layout("main", "", 0, 0);
        let bytes_many = render_layout("main", "", 1, 0);
        assert_valid_framebuffer(&bytes_few, "main (few items)");
        assert_valid_framebuffer(&bytes_many, "main (many items)");
        assert_ne!(
            bytes_few, bytes_many,
            "auto-height container with different item counts must differ"
        );
    }

    #[test]
    fn start_flex_auto_size_adapts_to_items() {
        // Mock state 1: minimal (no POKéDEX, no POKéMON) → fewer items
        // Mock state 0: full → more items
        // Auto width/height should differ.
        let bytes_minimal = render_layout("start", "", 1, 0);
        let bytes_full = render_layout("start", "", 0, 0);
        assert_valid_framebuffer(&bytes_minimal, "start (minimal)");
        assert_valid_framebuffer(&bytes_full, "start (full)");
        assert_ne!(
            bytes_minimal, bytes_full,
            "auto-size container with different item counts must differ"
        );
    }

    #[test]
    fn main_flex_json_round_trip() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("main")
            .expect("get_layout_json('main')");
        let bytes_static = render_layout("main", "", 1, 0);
        let bytes_parsed = render_layout("main", canonical, 1, 0);
        assert_valid_framebuffer(&bytes_parsed, "main (parsed)");
        assert_eq!(
            bytes_static, bytes_parsed,
            "flex layout json round-trip must be lossless"
        );
    }

    #[test]
    fn start_flex_json_round_trip() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("start")
            .expect("get_layout_json('start')");
        let bytes_static = render_layout("start", "", 0, 0);
        let bytes_parsed = render_layout("start", canonical, 0, 0);
        assert_valid_framebuffer(&bytes_parsed, "start (parsed)");
        assert_eq!(
            bytes_static, bytes_parsed,
            "flex layout json round-trip must be lossless"
        );
    }

    // ── serde / schema tests (Layer 1) ────────────────────────────────

    #[test]
    fn serde_edgeinsets_partial_defaults_to_one() {
        let json = r#"{"top":5}"#;
        let e: pokered_data::ui_layout::schema::EdgeInsets =
            serde_json::from_str(json).expect("parse");
        assert_eq!(e.top, 5);
        assert_eq!(e.bottom, 1, "missing bottom");
        assert_eq!(e.left, 1, "missing left");
        assert_eq!(e.right, 1, "missing right");
    }

    #[test]
    fn serde_edgeinsets_empty_json_defaults_one() {
        let json = r#"{}"#;
        let e: pokered_data::ui_layout::schema::EdgeInsets =
            serde_json::from_str(json).expect("parse");
        assert_eq!((e.top, e.bottom, e.left, e.right), (1, 1, 1, 1));
    }

    #[test]
    fn serde_box_def_defaults() {
        use pokered_data::ui_layout::schema::{Align, BoxDef, Justify, SizeMode};
        let json = r#"{"id":"t","rect":{"tx":0,"ty":0,"tw":10,"th":10},"color":"Black"}"#;
        let b: BoxDef = serde_json::from_str(json).expect("parse");
        assert_eq!(b.gap, 1);
        assert_eq!(b.width_mode, SizeMode::Fixed);
        assert_eq!(b.height_mode, SizeMode::Fixed);
        assert_eq!(b.justify, Justify::Start);
        assert_eq!(b.align, Align::Start);
        assert_eq!(b.padding.top, 1);
        assert!(b.min_width.is_none());
    }

    #[test]
    fn serde_flex_variant_with_children() {
        let json = r#"{"children":[{"id":"b","rect":{"tx":0,"ty":0,"tw":5,"th":3},"color":"Black"},
          {"id":"f","rect":{"tx":0,"ty":4,"tw":10,"th":8},"color":"Black","layout":"flex"}]}"#;
        let v: pokered_data::ui_layout::VariantDef = serde_json::from_str(json).expect("parse");
        let children = v.children.as_ref().unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].id, "b");
        assert_eq!(children[1].id, "f");
    }

    /// Verify that mutating a flex field in JSON actually survives round-trip
    /// through ScreenLayout deserialization (the path used by resolve_*_layout).
    #[test]
    fn mutate_flex_gap_survives_screenlayout_roundtrip() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("main").unwrap();
        let mut json_val: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        json_val["variants"]["default"]["children"][0]["gap"] = serde_json::Value::Number(3.into());
        let mutated = serde_json::to_string(&json_val).unwrap();
        let screen: pokered_data::ui_layout::ScreenLayout =
            serde_json::from_str(&mutated).expect("ScreenLayout parse must succeed");
        let variant = screen.variants.get("default").expect("default variant exists");
        let children = variant.children.as_ref().expect("children field exists");
        let flex_child = children.first().expect("first child exists");
        assert_eq!(flex_child.gap, 3, "mutated gap=3 must survive round-trip");
    }

    // ── pixel-diff tests using hash comparison (Layers 3+4) ───────────

    fn framebuffer_hash(bytes: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for &b in bytes.iter() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    /// Regenerate golden hashes: run with `-- --nocapture`, copy output to GOLDEN_* constants.
    #[test]
    fn print_golden_hashes() {
        let menus: &[(&str, u32)] = &[
            ("main", 1), ("start", 0), ("dialog", 0), ("battle_move", 0),
            ("bag", 0), ("battle_bag", 0), ("pokedex", 0), ("yes_no", 0),
            ("oak_speech", 0), ("save", 0), ("options", 0), ("naming", 0),
            ("battle_main", 0), ("battle_party", 0),
        ];
        for &(name, mock) in menus {
            let bytes = render_layout(name, "", mock, 0);
            assert_valid_framebuffer(&bytes, name);
            println!("const GOLDEN_{:<20} = 0x{:016x};", name.to_uppercase(), framebuffer_hash(&bytes));
        }
    }

    // ── Layer 3: golden snapshot tests ────────────────────────────────
    // These hashes lock the rendered output. If a refactor changes visual
    // output, the test fails — confirm the change is intentional, then
    // regenerate with print_golden_hashes.

    // Regenerated after the schema_version 2 layout-engine render migration
    // (via print_golden_hashes). These lock the current v2 rendered output.
    // battle_move / options / naming / battle_main re-recorded after their
    // .gui migration plus the template-`visible` compiler fix (options ▶ now
    // only on the active row) and the 0xE1/0xE2 PKMN tile mapping.
    // NOTE: START/BAG/BATTLE_BAG/OPTIONS/NAMING re-recorded after the dotzuki-renderer
    // CJK-repertoire font bake (full Fusion Pixel set; ASCII now from the Latin
    // face). The v2 preview renders pokered menus through the shared engine font,
    // so these snapshots track that font. Verified the new render is clean/legible
    // (pokered-app screenshots of options + main-menu) — an intentional change.
    const GOLDEN_MAIN: u64                 = 0x2708152520481bec;
    const GOLDEN_START: u64                = 0x39c01e00fa9daf25;
    const GOLDEN_DIALOG: u64               = 0x00bca8cdd951c80c;
    const GOLDEN_BATTLE_MOVE: u64          = 0x32e6d7f437e8c7f5;
    const GOLDEN_BAG: u64                  = 0x11925ebd809db43d;
    const GOLDEN_BATTLE_BAG: u64           = 0xa590234630604f9c;
    const GOLDEN_POKEDEX: u64              = 0x8a3a2f5ef81eed6d;
    const GOLDEN_YES_NO: u64               = 0x0a8743f31c0acdad;
    const GOLDEN_OAK_SPEECH: u64           = 0x69da641919f996ec;
    const GOLDEN_SAVE: u64                 = 0xbfe24358695a190d;
    const GOLDEN_OPTIONS: u64              = 0x4d87ce12033ce29c;
    const GOLDEN_NAMING: u64               = 0x3816ff8f4a7ee334;
    const GOLDEN_BATTLE_MAIN: u64          = 0x3a3f636fd146abc5;
    const GOLDEN_BATTLE_PARTY: u64         = 0x41ce2a3d42dacacc;

    macro_rules! assert_golden {
        ($name:expr, $mock:expr, $golden:ident) => {
            let bytes = render_layout($name, "", $mock, 0);
            assert_valid_framebuffer(&bytes, $name);
            assert_eq!(
                framebuffer_hash(&bytes), $golden,
                concat!("golden snapshot failed for ", $name, " (mock=", stringify!($mock), ")")
            );
        };
    }

    #[test] fn golden_main()          { assert_golden!("main", 1, GOLDEN_MAIN); }
    #[test] fn golden_start()         { assert_golden!("start", 0, GOLDEN_START); }
    #[test] fn golden_dialog()        { assert_golden!("dialog", 0, GOLDEN_DIALOG); }
    #[test] fn golden_battle_move()   { assert_golden!("battle_move", 0, GOLDEN_BATTLE_MOVE); }
    #[test] fn golden_bag()           { assert_golden!("bag", 0, GOLDEN_BAG); }
    #[test] fn golden_battle_bag()    { assert_golden!("battle_bag", 0, GOLDEN_BATTLE_BAG); }
    #[test] fn golden_pokedex()       { assert_golden!("pokedex", 0, GOLDEN_POKEDEX); }
    #[test] fn golden_yes_no()        { assert_golden!("yes_no", 0, GOLDEN_YES_NO); }
    #[test] fn golden_oak_speech()    { assert_golden!("oak_speech", 0, GOLDEN_OAK_SPEECH); }
    #[test] fn golden_save()          { assert_golden!("save", 0, GOLDEN_SAVE); }
    #[test] fn golden_options()       { assert_golden!("options", 0, GOLDEN_OPTIONS); }
    #[test] fn golden_naming()        { assert_golden!("naming", 0, GOLDEN_NAMING); }
    #[test] fn golden_battle_main()   { assert_golden!("battle_main", 0, GOLDEN_BATTLE_MAIN); }
    #[test] fn golden_battle_party()  { assert_golden!("battle_party", 0, GOLDEN_BATTLE_PARTY); }

    // The main menu and options screens render through the v2 layout engine:
    // their draw() functions read the compiled `.gui` layout and ignore the
    // v1 layout-JSON override, so the old gap/padding/height/box-resize
    // override tests can no longer affect the output. State-driven variation
    // (different mock states must yield different pixels) replaces them.

    #[test]
    fn pixel_main_no_save_vs_save_differs() {
        // mock 0 = no save (2 items, short box), mock 1 = with save (taller
        // box via the v2 auto-height shim). The renders must differ.
        let a = render_layout("main", "", 0, 0);
        let b = render_layout("main", "", 1, 0);
        assert_valid_framebuffer(&a, "main no-save");
        assert_valid_framebuffer(&b, "main with-save");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "save presence must change pixels");
    }

    #[test]
    fn pixel_start_width_auto_vs_fixed_differs() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("start").unwrap();
        let mut json_val: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        json_val["variants"]["default"]["children"][0]["width"] = serde_json::Value::String("fixed".into());
        json_val["variants"]["default"]["children"][0]["rect"]["tw"] = serde_json::Value::Number(16.into());
        let mutated = serde_json::to_string(&json_val).unwrap();
        let a = render_layout("start", "", 1, 0);
        let b = render_layout("start", &mutated, 1, 0);
        assert_valid_framebuffer(&b, "start width=fixed");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "width auto→fixed(16) must change pixels");
    }

    #[test]
    fn pixel_start_min_height_clamps() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("start").unwrap();
        let mut json_val: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        json_val["variants"]["default"]["children"][0]["min_height"] = serde_json::Value::Number(14.into());
        let mutated = serde_json::to_string(&json_val).unwrap();
        let a = render_layout("start", "", 1, 0);
        let b = render_layout("start", &mutated, 1, 0);
        assert_valid_framebuffer(&b, "start min_h=14");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "min_height clamp must change pixels");
    }

    #[test]
    fn pixel_bag_gap_1_vs_0_differs() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("bag").unwrap();
        let mut json_val: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        json_val["variants"]["default"]["children"][1]["gap"] = serde_json::Value::Number(0.into());
        let mutated = serde_json::to_string(&json_val).unwrap();
        let a = render_layout("bag", "", 0, 0);
        let b = render_layout("bag", &mutated, 0, 0);
        assert_valid_framebuffer(&b, "bag gap=0");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "bag gap 1→0 must change pixels");
    }

    #[test]
    fn pixel_battle_bag_padding_top_1_vs_0_differs() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("battle_bag").unwrap();
        let mut json_val: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        json_val["variants"]["default"]["children"][0]["padding"]["top"] = serde_json::Value::Number(0.into());
        let mutated = serde_json::to_string(&json_val).unwrap();
        let a = render_layout("battle_bag", "", 0, 0);
        let b = render_layout("battle_bag", &mutated, 0, 0);
        assert_valid_framebuffer(&b, "battle_bag pad_top=0");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "battle_bag padding must change pixels");
    }

    #[test]
    fn pixel_save_ask_prompt_box_shift_differs() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("save").unwrap();
        let mut json_val: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        let children = &mut json_val["variants"]["ask_prompt"]["children"];
        children[0]["rect"]["tx"] = serde_json::Value::Number(2.into());
        children[0]["rect"]["ty"] = serde_json::Value::Number(13.into());
        let mutated = serde_json::to_string(&json_val).unwrap();
        let a = render_layout("save", "", 0, 0);
        let b = render_layout("save", &mutated, 0, 0);
        assert_valid_framebuffer(&b, "save ask_prompt shifted");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "ask_prompt box shift must change pixels");
    }

    // ── pixel-diff tests: non-flex (children-only) menus ────────────────

    #[test]
    fn pixel_dialog_box_tx_shift_differs() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("dialog").unwrap();
        let mut json_val: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        let children = &mut json_val["variants"]["default"]["children"];
        children[0]["rect"]["tx"] = serde_json::Value::Number(5.into());
        let a = render_layout("dialog", "", 0, 0);
        let b = render_layout("dialog", &serde_json::to_string(&json_val).unwrap(), 0, 0);
        assert_valid_framebuffer(&b, "dialog tx=5");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "box tx shift must change pixels");
    }

    #[test]
    fn pixel_battle_text_box_tw_resize_differs() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("battle_text").unwrap();
        let mut json_val: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        let children = &mut json_val["variants"]["default"]["children"];
        children[0]["rect"]["tw"] = serde_json::Value::Number(10.into());
        let a = render_layout("battle_text", "", 0, 0);
        let b = render_layout("battle_text", &serde_json::to_string(&json_val).unwrap(), 0, 0);
        assert_valid_framebuffer(&b, "battle_text tw=10");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "box tw resize must change pixels");
    }

    #[test]
    fn pixel_options_setting_values_differ() {
        // v2 options ignores the v1 layout override (see note above); instead
        // assert different option values (Medium/On/Shift vs Fast/Off/Set)
        // move the ▶ cursor and change pixels.
        let a = render_layout("options", "", 0, 0);
        let b = render_layout("options", "", 1, 0);
        assert_valid_framebuffer(&a, "options mock0");
        assert_valid_framebuffer(&b, "options mock1");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "option values must change pixels");
    }

    #[test]
    fn pixel_save_label_text_change_differs() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("save").unwrap();
        let mut json_val: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        let labels = &mut json_val["variants"]["default"]["children"][0]["labels"];
        labels[0]["text"] = serde_json::Value::String("XYZ".into());
        let a = render_layout("save", "", 0, 0);
        let b = render_layout("save", &serde_json::to_string(&json_val).unwrap(), 0, 0);
        assert_valid_framebuffer(&b, "save label=XYZ");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "label text change must change pixels");
    }

    #[test]
    fn pixel_yes_no_box_tx_change_differs() {
        // The yes/no cursor glyph is positioned from the box + selection at draw
        // time, not from the layout's `cursor.tx`, so mutate the box rect (the
        // field that actually drives the render) to prove layout JSON is honored.
        let canonical = pokered_data::ui_layout::schema::get_layout_json("yes_no").unwrap();
        let mut json_val: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        json_val["variants"]["default"]["children"][0]["rect"]["tx"] = serde_json::Value::Number(8.into());
        let a = render_layout("yes_no", "", 0, 0);
        let b = render_layout("yes_no", &serde_json::to_string(&json_val).unwrap(), 0, 0);
        assert_valid_framebuffer(&b, "yes_no box_tx=8");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "box tx change must change pixels");
    }

    #[test]
    fn pixel_naming_text_change_differs() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("naming").unwrap();
        let mut json_val: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        json_val["variants"]["default"]["children"][0]["rect"]["tx"] = serde_json::Value::Number(8.into());
        let a = render_layout("naming", "", 0, 0);
        let b = render_layout("naming", &serde_json::to_string(&json_val).unwrap(), 0, 0);
        assert_valid_framebuffer(&b, "naming tx=8");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "box tx shift must change pixels");
    }

    #[test]
    fn pixel_pokedex_box_ty_shift_differs() {
        let canonical = pokered_data::ui_layout::schema::get_layout_json("pokedex").unwrap();
        let mut json_val: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        let children = &mut json_val["variants"]["default"]["children"];
        children[0]["rect"]["ty"] = serde_json::Value::Number(4.into());
        let a = render_layout("pokedex", "", 0, 0);
        let b = render_layout("pokedex", &serde_json::to_string(&json_val).unwrap(), 0, 0);
        assert_valid_framebuffer(&b, "pokedex ty=4");
        assert_ne!(framebuffer_hash(&a), framebuffer_hash(&b), "box ty shift must change pixels");
    }
}
