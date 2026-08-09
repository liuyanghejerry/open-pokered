use pokered_core::game_state::Lang;
use pokered_core::items::{BuyMenuState, BuyResult, MartPhase, MartState, SellMenuState, SellResult};
use pokered_core::main_menu::MainMenuState;
use pokered_core::options_menu::OptionsMenuState;
use pokered_core::party_screen::PartyScreenState;
use pokered_core::save_menu::SaveMenuState;
use pokered_core::start_menu::StartMenuState;
use pokered_core::stats_screen::StatsScreenState;
use pokered_data::mon_party_icons::{icon_for_species, IconKind};
use pokered_data::impl_traits::PokemonRenderData;
use pokered_data::lang_data;
use pokered_data::ui_layout::schema::{MART_CONFIRM_LAYOUT, MART_MAIN_MENU_LAYOUT, MART_QUANTITY_LAYOUT, MART_RESULT_DIALOG_LAYOUT, MAIN_DEFAULT_LAYOUT, START_DEFAULT_LAYOUT, OPTIONS_DEFAULT_LAYOUT, SAVE_DEFAULT_LAYOUT, SAVE_ASK_PROMPT_LAYOUT, PARTY_DEFAULT_LAYOUT, STATS_PAGE1_LAYOUT, STATS_PAGE2_LAYOUT, BAG_DEFAULT_LAYOUT};
use pokered_renderer::mon_icon::{draw_mon_icon, load_mon_icon_tiles, IconFrame};
use pokered_renderer::palette::GRAYSCALE_SPRITE_PALETTE;
use pokered_renderer::party_hp_bar::draw_party_hp_bar;
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, TILE_SIZE};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{menus, InkColor, TileRect, Ui};
use pokered_core::bag_screen::{BagPhase, BagScreenState};

use super::{blit_tileset, species_to_sprite_name};

pub fn draw_main_menu(state: &MainMenuState, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let mut ui = Ui::new(&mut painter);
    menus::main::draw(state, &MAIN_DEFAULT_LAYOUT, &mut ui, lang);
}

pub fn draw_start_menu(state: &StartMenuState, player_name: &str, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let mut ui = Ui::new(&mut painter);
    menus::start::draw(state, player_name, &START_DEFAULT_LAYOUT, &mut ui, lang);
}

pub fn draw_options_menu(state: &OptionsMenuState, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let mut ui = Ui::new(&mut painter);
    menus::options::draw(state, &OPTIONS_DEFAULT_LAYOUT, &mut ui, lang);
}

pub fn draw_save_menu(state: &SaveMenuState, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let mut ui = Ui::new(&mut painter);
    menus::save::draw(state, &SAVE_DEFAULT_LAYOUT, &SAVE_ASK_PROMPT_LAYOUT, &mut ui, lang);
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
        menus::party::draw(state, &PARTY_DEFAULT_LAYOUT, &mut ui, lang);
    }

    let Some(rm) = resources else {
        return;
    };

    let cursor = state.cursor();
    const ICON_X_PX: u32 = 8;
    const HP_BAR_X_PX: u32 = 32;

    for (i, pokemon) in state.party().iter().enumerate() {
        let kind = icon_for_species(pokemon.species);
        let frame = if i == cursor {
            IconFrame::from_counter(frame_counter, 16)
        } else {
            IconFrame::Frame1
        };
        match load_mon_icon_tiles(rm, kind, frame) {
            Ok(tiles) => {
                let y = (i as u32) * 16;
                draw_mon_icon(fb, tiles, ICON_X_PX, y, &GRAYSCALE_SPRITE_PALETTE);
            }
            Err(e) => {
                tracing::warn!(
                    "party screen: failed to load icon for {:?}: {}",
                    pokemon.species,
                    e
                );
            }
        }

        let hp_bar_y = (i as u32) * 16 + 8;
        if let Err(e) =
            draw_party_hp_bar(fb, rm, HP_BAR_X_PX, hp_bar_y, pokemon.hp, pokemon.max_hp)
        {
            tracing::warn!(
                "party screen: failed to draw HP bar for {:?}: {}",
                pokemon.species,
                e
            );
        }
    }
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
        menus::stats::draw(state, &STATS_PAGE1_LAYOUT, &STATS_PAGE2_LAYOUT, &mut ui, lang, &PokemonRenderData::new(false));
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
        let h_tiles = cached.source_size.1 / TILE_SIZE;
        let _ = h_tiles;
        let max_w = 7u32;
        let x_off = ((max_w.saturating_sub(w_tiles)) / 2) * TILE_SIZE;
        let px = TILE_SIZE + x_off;
        let py = TILE_SIZE / 2;
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
    match &state.phase {
        MartPhase::MainMenu { cursor } => {
            menus::mart::draw_main_with_money(cursor.position(), player_money, &MART_MAIN_MENU_LAYOUT, &mut ui, lang);
        }
        MartPhase::Buy(bs) => match bs {
            BuyMenuState::SelectItem { cursor } => {
                menus::mart::draw_buy_items_with_money(
                    state.inventory.items(),
                    *cursor,
                    0,
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
                menus::mart::draw_sell_items_with_money(
                    bag_items,
                    *cursor,
                    0,
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

/// Overworld ITEM bag (Start menu → ITEM): the item list, plus a USE / TOSS /
/// CANCEL menu or the TOSS-quantity prompt when an item is selected.
pub fn draw_bag(state: &BagScreenState, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    let mut ui = Ui::new(&mut painter);
    let rd = PokemonRenderData::new(false);
    let items_u8: Vec<(pokered_data::items::ItemId, u8)> = state
        .items()
        .iter()
        .map(|(id, q)| (*id, (*q).min(99) as u8))
        .collect();
    menus::bag::draw(&items_u8, state.cursor(), &BAG_DEFAULT_LAYOUT, &mut ui, &rd);

    match state.phase() {
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
            ui.text_box(TileRect::new(4, 12, 15, 4), InkColor::Black, true, |frame| {
                let prompt = if lang == Lang::Zh { "扔掉几个？" } else { "TOSS HOW MANY?" };
                frame.label(2, 1, prompt, InkColor::Black);
                frame.label(2, 2, &format!("x{:02}", qty), InkColor::Black);
            });
        }
        BagPhase::Browsing => {}
    }
}
