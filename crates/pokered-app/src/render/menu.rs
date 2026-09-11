use crate::alloc_prelude::*;
use pokered_core::game_state::Lang;
use pokered_core::items::{BuyMenuState, BuyResult, MartPhase, MartState, SellMenuState, SellResult};
use pokered_core::main_menu::MainMenuState;
use pokered_core::options_menu::OptionsMenuState;
use pokered_core::party_screen::{PartyScreenPhase, PartyScreenState};
use pokered_core::save_menu::{SaveMenuState, YesNoChoice};
use pokered_core::start_menu::StartMenuState;
use pokered_core::stats_screen::{StatsPage, StatsScreenState};
use pokered_data::mon_party_icons::{icon_for_species, IconKind};
use pokered_data::impl_traits::PokemonRenderData;
use pokered_data::lang_data;
use pokered_data::ui_layout::schema::{MART_CONFIRM_LAYOUT, MART_MAIN_MENU_LAYOUT, MART_QUANTITY_LAYOUT, MART_RESULT_DIALOG_LAYOUT, MAIN_DEFAULT_LAYOUT, START_DEFAULT_LAYOUT, OPTIONS_DEFAULT_LAYOUT, SAVE_DEFAULT_LAYOUT, SAVE_ASK_PROMPT_LAYOUT, PARTY_DEFAULT_LAYOUT, PARTY_ENTRY_LAYOUT, STATS_PAGE1_LAYOUT, STATS_PAGE2_LAYOUT, BAG_DEFAULT_LAYOUT};
use pokered_renderer::mon_icon::{draw_mon_icon, load_mon_icon_tiles, IconFrame};
use pokered_renderer::palette::GRAYSCALE_SPRITE_PALETTE;
use pokered_renderer::party_hp_bar::draw_party_hp_bar;
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, TILE_SIZE};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{menus, InkColor, Painter, TilePos, TileRect, Ui};
use pokered_core::bag_screen::{BagPhase, BagScreenState};

use super::{blit_tileset, species_to_sprite_name};

pub fn draw_main_menu(state: &MainMenuState, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let mut ui = Ui::new(&mut painter);
    menus::main::draw(state, &MAIN_DEFAULT_LAYOUT, &mut ui, lang);
}

/// Repaint only the changed cursor cells of an already-rendered title menu.
#[cfg(any(test, target_os = "none"))]
pub fn redraw_main_menu_cursor(
    previous_cursor: usize,
    current_cursor: usize,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    menus::main::redraw_cursor(previous_cursor, current_cursor, &mut painter, lang);
}

pub fn draw_start_menu(state: &StartMenuState, player_name: &str, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let mut ui = Ui::new(&mut painter);
    menus::start::draw(state, player_name, &START_DEFAULT_LAYOUT, &mut ui, lang);
}

/// Repaint only the changed cursor cells of an already-rendered START menu.
#[cfg(any(test, target_os = "none"))]
pub fn redraw_start_menu_cursor(
    item_count: usize,
    previous_cursor: usize,
    current_cursor: usize,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    menus::start::redraw_cursor(
        item_count,
        previous_cursor,
        current_cursor,
        &START_DEFAULT_LAYOUT,
        &mut painter,
    );
}

pub fn draw_options_menu(state: &OptionsMenuState, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let mut ui = Ui::new(&mut painter);
    menus::options::draw(state, &OPTIONS_DEFAULT_LAYOUT, &mut ui, lang);
}

/// Return the absolute tile position of the options screen's visible cursor.
#[cfg(target_os = "none")]
pub fn options_menu_cursor_position(state: &OptionsMenuState, lang: Lang) -> (u32, u32) {
    let pos = menus::options::cursor_position(state, &OPTIONS_DEFAULT_LAYOUT, lang);
    (pos.tx, pos.ty)
}

/// Repaint only the changed cursor cells of an already-rendered options screen.
#[cfg(any(test, target_os = "none"))]
pub fn redraw_options_menu_cursor(
    previous: (u32, u32),
    current: (u32, u32),
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    menus::options::redraw_cursor(
        pokered_ui::TilePos::new(previous.0, previous.1),
        pokered_ui::TilePos::new(current.0, current.1),
        &mut painter,
        lang,
    );
}

pub fn draw_save_menu(state: &SaveMenuState, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let mut ui = Ui::new(&mut painter);
    menus::save::draw(state, &SAVE_DEFAULT_LAYOUT, &SAVE_ASK_PROMPT_LAYOUT, &mut ui, lang);
}

/// Repaint only the changed YES/NO cursor cells of an already-rendered save prompt.
#[cfg(any(test, target_os = "none"))]
pub fn redraw_save_menu_cursor(
    previous: YesNoChoice,
    current: YesNoChoice,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    menus::save::redraw_cursor(previous, current, &mut painter);
}

/// Draws the party screen with real Pokémon icons composited on top of the
/// text UI.
///
/// `resources` is optional so this function still renders a usable screen
/// in headless tests / contexts where no `ResourceManager` is available.
/// In that case icon space is left blank.
///
/// `frame_counter` is the free-running game-loop frame index, used to
/// animate the *selected* mon's icon between Frame1 and Frame2 (matching
/// the original game's `AnimatePartyMon` behavior).
pub fn draw_party_screen(
    state: &PartyScreenState,
    resources: Option<&mut ResourceManager>,
    frame_counter: u64,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    {
        let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
        let mut ui = Ui::new(&mut painter);
        menus::party::draw_entries(state, &PARTY_DEFAULT_LAYOUT, &mut ui, lang);
    }

    if let Some(rm) = resources {
        let cursor = state.cursor();
        const ICON_X_PX: u32 = 8;
        let layout = &pokered_data::ui_layout::schema::PARTY_ENTRY_LAYOUT;
        let row_height = layout.cursors[0].row_step * TILE_SIZE;
        let hp_bar = layout
            .dynamic_labels
            .iter()
            .find(|(key, _)| *key == "hp_bar")
            .map(|(_, label)| label)
            .expect("party layout has an HP bar anchor");

        for (i, pokemon) in state.party().iter().enumerate() {
            let kind = icon_for_species(pokemon.species);
            let frame = if i == cursor {
                IconFrame::from_counter(frame_counter, 16)
            } else {
                IconFrame::Frame1
            };
            match load_mon_icon_tiles(rm, kind, frame) {
                Ok(tiles) => {
                    let y = (i as u32) * row_height;
                    draw_mon_icon(fb, tiles, ICON_X_PX, y, &GRAYSCALE_SPRITE_PALETTE);
                }
                Err(e) => {
                    log::warn!(
                        "party screen: failed to load icon for {:?}: {}",
                        pokemon.species,
                        e
                    );
                }
            }

            let hp_bar_y = (i as u32) * row_height + hp_bar.ty * TILE_SIZE + 4;
            if let Err(e) = draw_party_hp_bar(
                fb,
                rm,
                hp_bar.tx * TILE_SIZE,
                hp_bar_y,
                pokemon.hp,
                pokemon.max_hp,
            ) {
                log::warn!(
                    "party screen: failed to draw HP bar for {:?}: {}",
                    pokemon.species,
                    e
                );
            }
        }
    }
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    menus::party::draw_overlay(state, &mut Ui::new(&mut painter), lang);
}

#[cfg(any(test, target_os = "none"))]
fn clear_top_level_party_icon_at(party_index: usize, fb: &mut FrameBuffer) {
    const ICON_X_PX: u32 = 8;
    const ICON_SIZE_PX: u32 = 16;
    let row_height = PARTY_ENTRY_LAYOUT.cursors[0].row_step * TILE_SIZE;
    let icon_y = party_index as u32 * row_height;
    fb.fill_rect(
        ICON_X_PX,
        icon_y,
        ICON_SIZE_PX,
        ICON_SIZE_PX,
        pokered_renderer::Rgba::WHITE,
    );
}

#[cfg(any(test, target_os = "none"))]
fn draw_top_level_party_icon_at(
    state: &PartyScreenState,
    party_index: usize,
    frame: IconFrame,
    resources: Option<&mut ResourceManager>,
    fb: &mut FrameBuffer,
) {
    const ICON_X_PX: u32 = 8;
    let row_height = PARTY_ENTRY_LAYOUT.cursors[0].row_step * TILE_SIZE;
    let icon_y = party_index as u32 * row_height;
    let (Some(pokemon), Some(rm)) = (state.party_member(party_index), resources) else {
        return;
    };
    let kind = icon_for_species(pokemon.species);
    if let Ok(tiles) = load_mon_icon_tiles(rm, kind, frame) {
        draw_mon_icon(
            fb,
            tiles,
            ICON_X_PX,
            icon_y,
            &GRAYSCALE_SPRITE_PALETTE,
        );
    }
}

/// Repaint the selected icon when its 16-frame animation phase changes.
#[cfg(any(test, target_os = "none"))]
pub fn redraw_top_level_party_icon(
    state: &PartyScreenState,
    frame_counter: u64,
    resources: Option<&mut ResourceManager>,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    clear_top_level_party_icon_at(state.cursor(), fb);
    draw_top_level_party_icon_at(
        state,
        state.cursor(),
        IconFrame::from_counter(frame_counter, 16),
        resources,
        fb,
    );

    // The switch-target hint can cover the bottom party row. The complete
    // renderer draws overlays after icons, so restore that ordering here too.
    if matches!(state.phase(), PartyScreenPhase::SwitchTarget { .. }) {
        let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
        menus::party::draw_overlay(state, &mut Ui::new(&mut painter), lang);
    }
}

/// Repaint the two affected list rows when the selected party member moves.
#[cfg(any(test, target_os = "none"))]
pub fn redraw_top_level_party_selection(
    state: &PartyScreenState,
    previous_cursor: usize,
    frame_counter: u64,
    mut resources: Option<&mut ResourceManager>,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let current_cursor = state.cursor();
    let cursor_defs = PARTY_ENTRY_LAYOUT.cursors.as_ref();
    let cursor_position = |row: usize| {
        TilePos::new(
            cursor_defs[0].tx,
            cursor_defs[0].base_ty + row as u32 * cursor_defs[0].row_step,
        )
    };
    let previous_position = cursor_position(previous_cursor);
    let current_position = cursor_position(current_cursor);
    let source_index = match state.phase() {
        PartyScreenPhase::SwitchTarget { source_index } => Some(source_index),
        _ => None,
    };

    clear_top_level_party_icon_at(previous_cursor, fb);
    clear_top_level_party_icon_at(current_cursor, fb);
    {
        let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
        for position in [previous_position, current_position] {
            painter.draw_pixel_rect(
                position.tx * TILE_SIZE,
                position.ty * TILE_SIZE,
                TILE_SIZE,
                TILE_SIZE + 1,
                pokered_ui::Rgba::INK_WHITE,
            );
        }
        if source_index == Some(previous_cursor) && previous_cursor != current_cursor {
            let source = &cursor_defs[1];
            painter.draw_glyph(
                previous_position,
                source.glyph,
                pokered_ui::Rgba::INK_DARK_GRAY,
            );
        }
        let selected = &cursor_defs[0];
        painter.draw_glyph(
            current_position,
            selected.glyph,
            pokered_ui::Rgba::INK_BLACK,
        );
    }

    draw_top_level_party_icon_at(
        state,
        previous_cursor,
        IconFrame::Frame1,
        resources.as_deref_mut(),
        fb,
    );
    draw_top_level_party_icon_at(
        state,
        current_cursor,
        IconFrame::from_counter(frame_counter, 16),
        resources,
        fb,
    );

    if matches!(state.phase(), PartyScreenPhase::SwitchTarget { .. }) {
        let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
        menus::party::draw_overlay(state, &mut Ui::new(&mut painter), lang);
    }
}

/// Repaint only the changed cursor cells of an action/choose-move overlay.
#[cfg(any(test, target_os = "none"))]
pub fn redraw_top_level_party_overlay_cursor(
    state: &PartyScreenState,
    previous_cursor: u8,
    current_cursor: u8,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let Some(previous) = menus::party::overlay_cursor_position(state, previous_cursor, lang) else {
        return;
    };
    let Some(current) = menus::party::overlay_cursor_position(state, current_cursor, lang) else {
        return;
    };
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    painter.draw_pixel_rect(
        previous.tx * TILE_SIZE,
        previous.ty * TILE_SIZE,
        TILE_SIZE,
        TILE_SIZE + 1,
        pokered_ui::Rgba::INK_WHITE,
    );
    painter.draw_glyph(current, '▶', pokered_ui::Rgba::INK_BLACK);
}

/// Draw the stats/details screen. Renders the text UI (name, level, HP
/// bar + numbers, status, stats box, types, moves, EXP) via the
/// pokered-ui stats renderer, then composites the mon front sprite (page 1)
/// on top when `resources` is available.
pub fn draw_stats_screen(
    state: &StatsScreenState,
    resources: Option<&mut ResourceManager>,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    {
        let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
        let mut ui = Ui::new(&mut painter);
        menus::stats::draw(state, &STATS_PAGE1_LAYOUT, &STATS_PAGE2_LAYOUT, &mut ui, lang, &PokemonRenderData::new(lang == Lang::Zh));
    }

    let Some(rm) = resources else {
        return;
    };

    let pokemon = state.pokemon();

    let species_display = format!("{}", pokemon.species);
    let sprite_name = species_to_sprite_name(&species_display);
    let drew_front = if let Ok(cached) = rm.load_pokemon_front(&sprite_name) {
        let ts = cached.tileset.clone();
        let w_tiles = cached.source_size.0 / TILE_SIZE;
        let max_w = 7u32;
        let x_off = ((max_w.saturating_sub(w_tiles)) / 2) * TILE_SIZE;
        let px = TILE_SIZE + x_off;
        // A full-size front sprite is 56 px tall. Start it at y=0 so it
        // stays above the dex-number row (y=56); the old 4 px offset
        // let its bottom tiles overwrite the number drawn by the UI.
        let py = match state.page() {
            StatsPage::Stats => 0,
            StatsPage::Moves => TILE_SIZE / 2,
        };
        blit_tileset(fb, &ts, px, py, w_tiles, &GRAYSCALE_SPRITE_PALETTE);
        true
    } else {
        false
    };

    if !drew_front {
        let kind = icon_for_species(pokemon.species);
        if let Ok(tiles) = load_mon_icon_tiles(rm, kind, IconFrame::Frame1) {
            draw_mon_icon(fb, tiles, 8, 0, &GRAYSCALE_SPRITE_PALETTE);
        }
    }
}

pub fn draw_mart(state: &MartState, player_money: u32, bag_items: &[(pokered_data::items::ItemId, u32)], fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let mut ui = Ui::new(&mut painter);
    // The buy/sell list boxes show 5 entries at the 2-row CJK pitch
    // (interior rows 1/3/5/7/9 of a 12-tall box). The core tracks a bare
    // cursor, so window the scroll offset here to keep it on-screen.
    const LIST_VISIBLE: usize = 5;
    let list_scroll = |cursor: usize, count: usize| -> usize {
        cursor.saturating_sub(LIST_VISIBLE - 1).min(count.saturating_sub(LIST_VISIBLE))
    };
    match &state.phase {
        MartPhase::MainMenu { cursor } => {
            menus::mart::draw_main_with_money(cursor.position(), player_money, &MART_MAIN_MENU_LAYOUT, &mut ui, lang);
        }
        MartPhase::Buy(bs) => match bs {
            BuyMenuState::SelectItem { cursor } => {
                menus::mart::draw_buy_items_with_money(
                    state.inventory.items(),
                    *cursor,
                    list_scroll(*cursor, state.inventory.items().len()),
                    player_money,
                    &pokered_data::ui_layout::schema::MART_BUY_ITEMS_WITH_MONEY_LAYOUT,
                    &mut ui,
                    lang,
                    &PokemonRenderData::new(lang == Lang::Zh),
                );
            }
            BuyMenuState::Quantity { item_index, quantity } => {
                if let Some(item_id) = state.inventory.get(*item_index) {
                    if let Some(data) = pokered_data::item_data::get_item_data(item_id) {
                        let total = data.price as u32 * *quantity as u32;
                        let item_name = if lang == Lang::Zh {
                            pokered_data::lang_data::item_name(item_id, true)
                        } else {
                            data.name
                        };
                        menus::mart::draw_quantity(item_name, *quantity, data.price as u32, total, player_money, &MART_QUANTITY_LAYOUT, lang, &mut ui);
                    }
                }
            }
            BuyMenuState::Confirm { item_index, quantity, selected } => {
                if let Some(item_id) = state.inventory.get(*item_index) {
                    if let Some(data) = pokered_data::item_data::get_item_data(item_id) {
                        let total = data.price as u32 * *quantity as u32;
                        let item_name = if lang == Lang::Zh {
                            pokered_data::lang_data::item_name(item_id, true)
                        } else {
                            data.name
                        };
                        let msg = if lang == Lang::Zh {
                            format!("{} ×{} ${}.00\n总共${}.00。可以吗？", item_name, quantity, total, total)
                        } else {
                            format!("{} ×{} ${}.00\nThat'll be ${}.00. OK?", item_name, quantity, total, total)
                        };
                        let choice = match selected {
                            pokered_core::items::ConfirmChoice::Yes => menus::mart::ConfirmChoice::Yes,
                            pokered_core::items::ConfirmChoice::No => menus::mart::ConfirmChoice::No,
                        };
                        menus::mart::draw_confirm(lang, &msg, choice, &MART_CONFIRM_LAYOUT, &mut ui);
                    }
                }
            }
            BuyMenuState::Result { dialogue, .. } => {
                let lines = buy_result_lines(dialogue, lang == Lang::Zh);
                menus::mart::draw_result_dialog(&lines, &MART_RESULT_DIALOG_LAYOUT, &mut ui);
            }
        },
        MartPhase::Sell(ss) => match ss {
            SellMenuState::SelectItem { cursor } => {
                // CANCEL is the entry after the last bag item.
                let entries = bag_items.len() + 1;
                menus::mart::draw_sell_items_with_money(
                    bag_items,
                    *cursor,
                    list_scroll(*cursor, entries),
                    player_money,
                    &pokered_data::ui_layout::schema::MART_SELL_ITEMS_WITH_MONEY_LAYOUT,
                    &mut ui,
                    lang,
                    &PokemonRenderData::new(lang == Lang::Zh),
                );
            }
            SellMenuState::Quantity { item_index, quantity, max_quantity } => {
                if let Some((item_id, _owned)) = bag_items.get(*item_index) {
                    if let Some(data) = pokered_data::item_data::get_item_data(*item_id) {
                        let price = (data.price as u32) / 2;
                        let total = price * *quantity as u32;
                        let item_name = if lang == Lang::Zh {
                            pokered_data::lang_data::item_name(*item_id, true)
                        } else {
                            data.name
                        };
                        menus::mart::draw_quantity(item_name, *quantity, price, total, player_money, &MART_QUANTITY_LAYOUT, lang, &mut ui);
                    }
                }
                let _ = max_quantity;
            }
            SellMenuState::Confirm { item_index, quantity, max_quantity, selected } => {
                if let Some((item_id, _owned)) = bag_items.get(*item_index) {
                    if let Some(data) = pokered_data::item_data::get_item_data(*item_id) {
                        let price = (data.price as u32) / 2;
                        let total = price * *quantity as u32;
                        let item_name = if lang == Lang::Zh {
                            pokered_data::lang_data::item_name(*item_id, true)
                        } else {
                            data.name
                        };
                        let msg = if lang == Lang::Zh {
                            format!("{} ×{} ${}.00\n我可以支付${}.00。\n可以吗？", item_name, quantity, total, total)
                        } else {
                            format!("{} ×{} ${}.00\nI can pay ${}.00.\nOK?", data.name, quantity, total, total)
                        };
                        let choice = match selected {
                            pokered_core::items::ConfirmChoice::Yes => menus::mart::ConfirmChoice::Yes,
                            pokered_core::items::ConfirmChoice::No => menus::mart::ConfirmChoice::No,
                        };
                        menus::mart::draw_confirm(lang, &msg, choice, &MART_CONFIRM_LAYOUT, &mut ui);
                    }
                }
                let _ = max_quantity;
            }
            SellMenuState::Result { dialogue, .. } => {
                let lines = sell_result_lines(dialogue, lang == Lang::Zh);
                menus::mart::draw_result_dialog(&lines, &MART_RESULT_DIALOG_LAYOUT, &mut ui);
            }
        },
        MartPhase::Exiting => {}
    }
}

fn buy_result_lines(result: &BuyResult, is_zh: bool) -> Vec<&'static str> {
    if is_zh {
        match result {
            BuyResult::Success { .. } => vec!["谢谢惠顾！", "欢迎再来！"],
            BuyResult::NotEnoughMoney => vec!["你的钱", "不够！"],
            BuyResult::BagFull => vec!["你的包包满了！"],
            BuyResult::InvalidItem => vec!["那个道具", "不存在！"],
        }
    } else {
        match result {
            BuyResult::Success { .. } => vec!["Thank you!", "Come again!"],
            BuyResult::NotEnoughMoney => vec!["You don't have", "enough money!"],
            BuyResult::BagFull => vec!["Your bag is full!"],
            BuyResult::InvalidItem => vec!["That item doesn't", "exist!"],
        }
    }
}

fn sell_result_lines(result: &SellResult, is_zh: bool) -> Vec<&'static str> {
    if is_zh {
        match result {
            SellResult::Success { .. } => vec!["谢谢惠顾！", "欢迎再来！"],
            SellResult::Unsellable => vec!["我不能买", "那个道具。"],
            SellResult::NotInBag => vec!["你没有", "那个道具！"],
            SellResult::InvalidItem => vec!["那个道具", "不存在！"],
        }
    } else {
        match result {
            SellResult::Success { .. } => vec!["Thank you!", "Come again!"],
            SellResult::Unsellable => vec!["I can't buy", "that item."],
            SellResult::NotInBag => vec!["You don't have", "that item!"],
            SellResult::InvalidItem => vec!["That item doesn't", "exist!"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_core::bag_screen::{BagScreenInput, BagScreenState};
    use pokered_core::game_state::SaveFileSummary;
    use pokered_core::start_menu::{StartMenuInput, StartMenuState};
    use pokered_core::options_menu::{
        BattleAnimation, BattleStyle, GameOptions, OptionsMenuState, OptionsRow, TextSpeed,
    };
    use pokered_core::save_menu::{SavePhase, SaveScreenInfo};
    use pokered_core::party_screen::{PartyScreenInput, PartyScreenState};
    use pokered_core::pokemon::stats::create_pokemon;
    use pokered_data::items::ItemId;
    use pokered_data::species::Species;
    use pokered_renderer::resource::AssetRoot;
    use pokered_renderer::Rgba;

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

    #[test]
    fn main_menu_cursor_repaint_matches_a_fresh_menu_for_every_transition() {
        let save = SaveFileSummary {
            player_name: b"RED".to_vec(),
            badges: 0,
            pokedex_owned: 0,
            play_time_hours: 0,
            play_time_minutes: 0,
            play_time_seconds: 0,
            player_id: 0,
        };
        for language in [Lang::En, Lang::Zh] {
            for save_summary in [None, Some(save.clone())] {
                let state = MainMenuState::new(save_summary);
                for previous_cursor in 0..state.item_count() {
                    for current_cursor in 0..state.item_count() {
                        if previous_cursor == current_cursor {
                            continue;
                        }
                        let mut previous = state.clone();
                        previous.cursor = previous_cursor;
                        let mut current = state.clone();
                        current.cursor = current_cursor;
                        let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);

                        let mut actual = FrameBuffer::new(config, Rgba::BLACK);
                        draw_main_menu(&previous, &mut actual, language);
                        redraw_main_menu_cursor(
                            previous_cursor,
                            current_cursor,
                            &mut actual,
                            language,
                        );

                        let mut expected = FrameBuffer::new(config, Rgba::BLACK);
                        draw_main_menu(&current, &mut expected, language);
                        assert_framebuffers_equal(&actual, &expected);
                    }
                }
            }
        }
    }

    #[test]
    fn start_menu_cursor_repaint_matches_a_fresh_menu_for_every_transition() {
        let input_down = StartMenuInput {
            up: false,
            down: true,
            a: false,
            b: false,
            start: false,
        };
        for language in [Lang::En, Lang::Zh] {
            for (has_pokedex, has_pokemon) in [(false, false), (true, true)] {
                let state = StartMenuState::new(has_pokedex, has_pokemon, false);
                for previous_cursor in 0..state.item_count() {
                    for current_cursor in 0..state.item_count() {
                        if previous_cursor == current_cursor {
                            continue;
                        }
                        let mut previous = state.clone();
                        for _ in 0..previous_cursor {
                            previous.update_frame(input_down);
                        }
                        let mut current = state.clone();
                        for _ in 0..current_cursor {
                            current.update_frame(input_down);
                        }
                        let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);

                        let mut actual = FrameBuffer::new(config, Rgba::BLACK);
                        draw_start_menu(&previous, "RED", &mut actual, language);
                        redraw_start_menu_cursor(
                            state.item_count(),
                            previous_cursor,
                            current_cursor,
                            &mut actual,
                            language,
                        );

                        let mut expected = FrameBuffer::new(config, Rgba::BLACK);
                        draw_start_menu(&current, "RED", &mut expected, language);
                        assert_framebuffers_equal(&actual, &expected);
                    }
                }
            }
        }
    }

    #[test]
    fn options_cursor_repaint_matches_a_fresh_menu_for_every_visible_transition() {
        let state = |row, text_speed, battle_animation, battle_style| {
            let mut state = OptionsMenuState::new(GameOptions {
                text_speed,
                battle_animation,
                battle_style,
            });
            state.row = row;
            state
        };
        let states = [
            state(
                OptionsRow::TextSpeed,
                TextSpeed::Fast,
                BattleAnimation::On,
                BattleStyle::Shift,
            ),
            state(
                OptionsRow::TextSpeed,
                TextSpeed::Medium,
                BattleAnimation::On,
                BattleStyle::Shift,
            ),
            state(
                OptionsRow::TextSpeed,
                TextSpeed::Slow,
                BattleAnimation::On,
                BattleStyle::Shift,
            ),
            state(
                OptionsRow::BattleAnimation,
                TextSpeed::Medium,
                BattleAnimation::On,
                BattleStyle::Shift,
            ),
            state(
                OptionsRow::BattleAnimation,
                TextSpeed::Medium,
                BattleAnimation::Off,
                BattleStyle::Shift,
            ),
            state(
                OptionsRow::BattleStyle,
                TextSpeed::Medium,
                BattleAnimation::On,
                BattleStyle::Shift,
            ),
            state(
                OptionsRow::BattleStyle,
                TextSpeed::Medium,
                BattleAnimation::On,
                BattleStyle::Set,
            ),
            state(
                OptionsRow::Cancel,
                TextSpeed::Medium,
                BattleAnimation::On,
                BattleStyle::Shift,
            ),
        ];
        let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);

        for language in [Lang::En, Lang::Zh] {
            for previous in &states {
                for current in &states {
                    let previous_pos = menus::options::cursor_position(
                        previous,
                        &OPTIONS_DEFAULT_LAYOUT,
                        language,
                    );
                    let current_pos = menus::options::cursor_position(
                        current,
                        &OPTIONS_DEFAULT_LAYOUT,
                        language,
                    );
                    if previous_pos == current_pos {
                        continue;
                    }

                    let mut actual = FrameBuffer::new(config, Rgba::BLACK);
                    draw_options_menu(previous, &mut actual, language);
                    redraw_options_menu_cursor(
                        (previous_pos.tx, previous_pos.ty),
                        (current_pos.tx, current_pos.ty),
                        &mut actual,
                        language,
                    );

                    let mut expected = FrameBuffer::new(config, Rgba::BLACK);
                    draw_options_menu(current, &mut expected, language);
                    assert_framebuffers_equal(&actual, &expected);
                }
            }
        }
    }

    #[test]
    fn save_cursor_repaint_matches_a_fresh_prompt_in_both_directions() {
        for language in [Lang::En, Lang::Zh] {
            for phase in [SavePhase::AskSave, SavePhase::ConfirmOverwrite] {
                for (previous, current) in [
                    (YesNoChoice::Yes, YesNoChoice::No),
                    (YesNoChoice::No, YesNoChoice::Yes),
                ] {
                    let make_state = |cursor| SaveMenuState {
                        phase: phase.clone(),
                        cursor,
                        info: SaveScreenInfo {
                            player_name: "RED".into(),
                            num_badges: 3,
                            pokedex_owned: 42,
                            play_time_hours: 12,
                            play_time_minutes: 34,
                        },
                        has_previous_save: false,
                        is_different_player: false,
                        sfx_event: pokered_core::save_menu::SaveSfxEvent::None,
                    };
                    let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);

                    let mut actual = FrameBuffer::new(config, Rgba::BLACK);
                    draw_save_menu(&make_state(previous), &mut actual, language);
                    redraw_save_menu_cursor(previous, current, &mut actual, language);

                    let mut expected = FrameBuffer::new(config, Rgba::BLACK);
                    draw_save_menu(&make_state(current), &mut expected, language);
                    assert_framebuffers_equal(&actual, &expected);
                }
            }
        }
    }

    fn top_level_party() -> PartyScreenState {
        PartyScreenState::new(vec![
            create_pokemon(Species::Bulbasaur, 20, [0xFF, 0xFF]).unwrap(),
            create_pokemon(Species::Charmander, 20, [0xFF, 0xFF]).unwrap(),
            create_pokemon(Species::Squirtle, 20, [0xFF, 0xFF]).unwrap(),
            create_pokemon(Species::Pikachu, 20, [0xFF, 0xFF]).unwrap(),
            create_pokemon(Species::Pidgey, 20, [0xFF, 0xFF]).unwrap(),
            create_pokemon(Species::Rattata, 20, [0xFF, 0xFF]).unwrap(),
        ])
    }

    fn party_input(up: bool, down: bool, a: bool) -> PartyScreenInput {
        PartyScreenInput {
            up,
            down,
            a,
            b: false,
        }
    }

    #[test]
    fn top_level_party_icon_repaint_matches_fresh_draws_in_browsing_and_switch_hint() {
        let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
        let root = AssetRoot::auto_detect().expect("test graphics");
        let mut resources = ResourceManager::new(root);

        for language in [Lang::En, Lang::Zh] {
            let browsing = top_level_party();
            let mut actual = FrameBuffer::new(config, Rgba::BLACK);
            draw_party_screen(&browsing, Some(&mut resources), 0, &mut actual, language);
            redraw_top_level_party_icon(
                &browsing,
                16,
                Some(&mut resources),
                &mut actual,
                language,
            );
            let mut expected = FrameBuffer::new(config, Rgba::BLACK);
            draw_party_screen(&browsing, Some(&mut resources), 16, &mut expected, language);
            assert_framebuffers_equal(&actual, &expected);

            // Select the bottom row, then enter SWITCH so its icon overlaps
            // the hint overlay. Incremental animation must restore the hint.
            let mut switching = top_level_party();
            for _ in 0..5 {
                switching.update_frame(party_input(false, true, false));
            }
            switching.update_frame(party_input(false, false, true));
            switching.update_frame(party_input(false, true, false));
            switching.update_frame(party_input(false, false, true));
            assert!(matches!(
                switching.phase(),
                PartyScreenPhase::SwitchTarget { source_index: 5 }
            ));

            let mut actual = FrameBuffer::new(config, Rgba::BLACK);
            draw_party_screen(&switching, Some(&mut resources), 0, &mut actual, language);
            redraw_top_level_party_icon(
                &switching,
                16,
                Some(&mut resources),
                &mut actual,
                language,
            );
            let mut expected = FrameBuffer::new(config, Rgba::BLACK);
            draw_party_screen(&switching, Some(&mut resources), 16, &mut expected, language);
            assert_framebuffers_equal(&actual, &expected);
        }
    }

    #[test]
    fn top_level_party_selection_repaint_matches_fresh_draws() {
        let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
        let root = AssetRoot::auto_detect().expect("test graphics");
        let mut resources = ResourceManager::new(root);

        for language in [Lang::En, Lang::Zh] {
            for previous_cursor in 0..6 {
                for down in [false, true] {
                    if (!down && previous_cursor == 0) || (down && previous_cursor == 5) {
                        continue;
                    }
                    let mut state = top_level_party();
                    for _ in 0..previous_cursor {
                        state.update_frame(party_input(false, true, false));
                    }
                    let mut actual = FrameBuffer::new(config, Rgba::BLACK);
                    draw_party_screen(&state, Some(&mut resources), 0, &mut actual, language);
                    state.update_frame(party_input(!down, down, false));
                    redraw_top_level_party_selection(
                        &state,
                        previous_cursor,
                        16,
                        Some(&mut resources),
                        &mut actual,
                        language,
                    );

                    let mut expected = FrameBuffer::new(config, Rgba::BLACK);
                    draw_party_screen(&state, Some(&mut resources), 16, &mut expected, language);
                    assert_framebuffers_equal(&actual, &expected);
                }
            }

            // In SWITCH mode the old source row changes from the selected
            // arrow to the diamond marker when the cursor leaves it.
            let mut switching = top_level_party();
            switching.update_frame(party_input(false, false, true));
            switching.update_frame(party_input(false, true, false));
            switching.update_frame(party_input(false, false, true));
            let mut actual = FrameBuffer::new(config, Rgba::BLACK);
            draw_party_screen(&switching, Some(&mut resources), 0, &mut actual, language);
            switching.update_frame(party_input(false, true, false));
            redraw_top_level_party_selection(
                &switching,
                0,
                16,
                Some(&mut resources),
                &mut actual,
                language,
            );
            let mut expected = FrameBuffer::new(config, Rgba::BLACK);
            draw_party_screen(&switching, Some(&mut resources), 16, &mut expected, language);
            assert_framebuffers_equal(&actual, &expected);
        }
    }

    #[test]
    fn top_level_party_overlay_cursor_repaint_matches_fresh_draws() {
        let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
        let root = AssetRoot::auto_detect().expect("test graphics");
        let mut resources = ResourceManager::new(root);

        for language in [Lang::En, Lang::Zh] {
            let mut action = top_level_party();
            action.update_frame(party_input(false, false, true));
            let mut actual = FrameBuffer::new(config, Rgba::BLACK);
            draw_party_screen(&action, Some(&mut resources), 0, &mut actual, language);
            action.update_frame(party_input(false, true, false));
            redraw_top_level_party_icon(
                &action,
                16,
                Some(&mut resources),
                &mut actual,
                language,
            );
            redraw_top_level_party_overlay_cursor(&action, 0, 1, &mut actual, language);
            let mut expected = FrameBuffer::new(config, Rgba::BLACK);
            draw_party_screen(&action, Some(&mut resources), 16, &mut expected, language);
            assert_framebuffers_equal(&actual, &expected);

            let mut choose = PartyScreenState::new_for_move_choice(
                top_level_party().party().to_vec(),
                0,
            );
            let mut actual = FrameBuffer::new(config, Rgba::BLACK);
            draw_party_screen(&choose, Some(&mut resources), 0, &mut actual, language);
            choose.update_frame(party_input(false, true, false));
            redraw_top_level_party_icon(
                &choose,
                16,
                Some(&mut resources),
                &mut actual,
                language,
            );
            redraw_top_level_party_overlay_cursor(&choose, 0, 1, &mut actual, language);
            let mut expected = FrameBuffer::new(config, Rgba::BLACK);
            draw_party_screen(&choose, Some(&mut resources), 16, &mut expected, language);
            assert_framebuffers_equal(&actual, &expected);
        }
    }

    fn bag_at(cursor: usize) -> BagScreenState {
        let mut state = BagScreenState::new(vec![
            (ItemId::Potion, 5),
            (ItemId::Antidote, 2),
            (ItemId::PokeBall, 12),
            (ItemId::PokeFlute, 1),
            (ItemId::Bicycle, 1),
            (ItemId::Potion, 8),
            (ItemId::Antidote, 4),
            (ItemId::PokeBall, 20),
        ]);
        for _ in 0..cursor {
            state.update_frame(BagScreenInput {
                down: true,
                ..BagScreenInput::none()
            });
        }
        state
    }

    #[test]
    fn top_level_bag_list_cursor_repaint_matches_a_fresh_bag() {
        for language in [Lang::En, Lang::Zh] {
            for (previous_cursor, down) in [(0, true), (1, false), (4, true), (5, false)] {
                let mut state = bag_at(previous_cursor);
                let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
                let mut actual = FrameBuffer::new(config, Rgba::BLACK);
                draw_bag(&state, &mut actual, language);
                let previous = top_level_bag_cursor_position(state.items().len(), state.cursor());

                state.update_frame(BagScreenInput {
                    up: !down,
                    down,
                    ..BagScreenInput::none()
                });
                let current = top_level_bag_cursor_position(state.items().len(), state.cursor());
                assert_eq!(
                    top_level_bag_viewport_offset(state.items().len(), previous_cursor),
                    top_level_bag_viewport_offset(state.items().len(), state.cursor()),
                );
                redraw_top_level_bag_cursor(previous, current, &mut actual, language);

                let mut expected = FrameBuffer::new(config, Rgba::BLACK);
                draw_bag(&state, &mut expected, language);
                assert_framebuffers_equal(&actual, &expected);
            }
        }
    }

    #[test]
    fn top_level_bag_swap_and_action_cursors_match_fresh_draws() {
        for language in [Lang::En, Lang::Zh] {
            let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);

            let mut swap = bag_at(0);
            swap.update_frame(BagScreenInput {
                select: true,
                ..BagScreenInput::none()
            });
            let mut actual = FrameBuffer::new(config, Rgba::BLACK);
            draw_bag(&swap, &mut actual, language);
            let previous = top_level_bag_cursor_position(swap.items().len(), swap.cursor());
            swap.update_frame(BagScreenInput {
                down: true,
                ..BagScreenInput::none()
            });
            let current = top_level_bag_cursor_position(swap.items().len(), swap.cursor());
            redraw_top_level_bag_cursor(previous, current, &mut actual, language);
            let mut expected = FrameBuffer::new(config, Rgba::BLACK);
            draw_bag(&swap, &mut expected, language);
            assert_framebuffers_equal(&actual, &expected);

            let mut action = bag_at(0);
            action.update_frame(BagScreenInput {
                a: true,
                ..BagScreenInput::none()
            });
            let mut actual = FrameBuffer::new(config, Rgba::BLACK);
            draw_bag(&action, &mut actual, language);
            action.update_frame(BagScreenInput {
                down: true,
                ..BagScreenInput::none()
            });
            redraw_top_level_bag_action_cursor(0, 1, &mut actual, language);
            let mut expected = FrameBuffer::new(config, Rgba::BLACK);
            draw_bag(&action, &mut expected, language);
            assert_framebuffers_equal(&actual, &expected);
        }
    }

    #[test]
    fn top_level_bag_quantity_repaint_matches_a_fresh_draw() {
        for language in [Lang::En, Lang::Zh] {
            let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
            let mut state = bag_at(0);
            state.update_frame(BagScreenInput {
                a: true,
                ..BagScreenInput::none()
            });
            state.update_frame(BagScreenInput {
                down: true,
                ..BagScreenInput::none()
            });
            state.update_frame(BagScreenInput {
                a: true,
                ..BagScreenInput::none()
            });

            let mut actual = FrameBuffer::new(config, Rgba::BLACK);
            draw_bag(&state, &mut actual, language);
            state.update_frame(BagScreenInput {
                up: true,
                ..BagScreenInput::none()
            });
            redraw_top_level_bag_quantity(1, 2, &mut actual, language);

            let mut expected = FrameBuffer::new(config, Rgba::BLACK);
            draw_bag(&state, &mut expected, language);
            assert_framebuffers_equal(&actual, &expected);

            // Crossing the two/three-digit boundary must also erase the
            // trailing digit when moving back from x100 to x99.
            let mut state = BagScreenState::new(vec![(ItemId::Potion, 100)]);
            state.update_frame(BagScreenInput {
                a: true,
                ..BagScreenInput::none()
            });
            state.update_frame(BagScreenInput {
                down: true,
                ..BagScreenInput::none()
            });
            state.update_frame(BagScreenInput {
                a: true,
                ..BagScreenInput::none()
            });
            for _ in 1..100 {
                state.update_frame(BagScreenInput {
                    up: true,
                    ..BagScreenInput::none()
                });
            }
            let mut actual = FrameBuffer::new(config, Rgba::BLACK);
            draw_bag(&state, &mut actual, language);
            state.update_frame(BagScreenInput {
                down: true,
                ..BagScreenInput::none()
            });
            redraw_top_level_bag_quantity(100, 99, &mut actual, language);
            let mut expected = FrameBuffer::new(config, Rgba::BLACK);
            draw_bag(&state, &mut expected, language);
            assert_framebuffers_equal(&actual, &expected);
        }
    }
}

/// Overworld ITEM bag (Start menu → ITEM): the item list, plus a USE / TOSS /
/// CANCEL menu or the TOSS-quantity prompt when an item is selected.
pub fn draw_bag(state: &BagScreenState, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let mut ui = Ui::new(&mut painter);
    let rd = PokemonRenderData::new(false);
    menus::bag::draw(state.items(), state.cursor(), &BAG_DEFAULT_LAYOUT, &mut ui, &rd);

    match state.phase() {
        BagPhase::SwapFrom { row } => {
            // ▷ marker on the swap row (SwapItemsInMenu's select cursor),
            // drawn through a borderless overlay box at the list's left edge.
            let y = row.saturating_sub(state.scroll()) as u32;
            ui.text_box(
                TileRect::new(
                    BAG_DEFAULT_LAYOUT.list.rect.tx.saturating_sub(1),
                    BAG_DEFAULT_LAYOUT.list.rect.ty + 1 + y,
                    1,
                    1,
                ),
                InkColor::White,
                false,
                |frame| {
                    frame.label(0, 0, "▷", InkColor::Black);
                },
            );
        }
        BagPhase::ActionMenu { cursor } => {
            ui.text_box(TileRect::new(11, 10, 9, 8), InkColor::Black, true, |frame| {
                let is_zh = lang == Lang::Zh;
                for (i, opt) in ["USE", "TOSS", "CANCEL"].iter().enumerate() {
                    frame.label(2, 1 + i as u32 * 2, lang_data::ui_label(opt, is_zh), InkColor::Black);
                }
                if let Some(c) = &BAG_DEFAULT_LAYOUT.list.cursor {
                    frame.cursor_glyph_at(1, 1 + cursor as u32 * 2, c.glyph, c.color);
                }
            });
        }
        BagPhase::TossQuantity { qty } => {
            ui.text_box(TileRect::new(4, 11, 15, 7), InkColor::Black, true, |frame| {
                let prompt = if lang == Lang::Zh { "扔掉几个？" } else { "TOSS HOW MANY?" };
                frame.label(2, 1, prompt, InkColor::Black);
                frame.label(2, 3, &format!("x{:02}", qty), InkColor::Black);
            });
        }
        BagPhase::Browsing => {}
    }
}

#[cfg(any(test, target_os = "none"))]
pub fn top_level_bag_viewport_offset(item_count: usize, cursor: usize) -> usize {
    menus::bag::viewport_offset(item_count, cursor, &BAG_DEFAULT_LAYOUT)
}

#[cfg(any(test, target_os = "none"))]
pub fn top_level_bag_cursor_position(item_count: usize, cursor: usize) -> TilePos {
    menus::bag::cursor_position(item_count, cursor, &BAG_DEFAULT_LAYOUT)
}

/// Repaint only the changed list cursor cells of an already-rendered bag.
#[cfg(any(test, target_os = "none"))]
pub fn redraw_top_level_bag_cursor(
    previous: TilePos,
    current: TilePos,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    menus::bag::redraw_cursor(previous, current, &mut painter);
}

/// Repaint only the changed USE/TOSS/CANCEL cursor cells.
#[cfg(any(test, target_os = "none"))]
pub fn redraw_top_level_bag_action_cursor(
    previous: u8,
    current: u8,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let position = |cursor| TilePos::new(13, 12 + cursor as u32 * 2);
    let old = position(previous);
    painter.draw_pixel_rect(old.tx * 8, old.ty * 8, 8, 9, pokered_ui::Rgba::INK_WHITE);
    painter.draw_glyph(position(current), '▶', pokered_ui::Rgba::INK_BLACK);
}

/// Repaint only the changed `xNN` value of the toss-quantity prompt.
#[cfg(any(test, target_os = "none"))]
pub fn redraw_top_level_bag_quantity(
    previous: u32,
    current: u32,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let position = TilePos::new(7, 15);
    let text_width = |qty| format!("x{:02}", qty).chars().count() as u32 * 8;
    painter.draw_pixel_rect(
        position.tx * 8,
        position.ty * 8,
        text_width(previous).max(text_width(current)),
        10,
        pokered_ui::Rgba::INK_WHITE,
    );
    painter.draw_text(
        position,
        &format!("x{:02}", current),
        pokered_ui::Rgba::INK_BLACK,
    );
}
