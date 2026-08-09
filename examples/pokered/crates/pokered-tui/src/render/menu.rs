use pokered_core::game_state::Lang;
use pokered_core::items::{BuyMenuState, BuyResult, MartPhase, MartState, SellMenuState, SellResult};
use pokered_core::main_menu::MainMenuState;
use pokered_core::options_menu::OptionsMenuState;
use pokered_core::party_screen::PartyScreenState;
use pokered_core::save_menu::SaveMenuState;
use pokered_core::start_menu::StartMenuState;
use pokered_core::stats_screen::StatsScreenState;
use pokered_core::bag_screen::{BagPhase, BagScreenState};
use pokered_data::impl_traits::PokemonRenderData;
use pokered_data::ui_layout::schema::{MART_CONFIRM_LAYOUT, MART_MAIN_MENU_LAYOUT, MART_QUANTITY_LAYOUT, MART_RESULT_DIALOG_LAYOUT, MAIN_DEFAULT_LAYOUT, START_DEFAULT_LAYOUT, OPTIONS_DEFAULT_LAYOUT, SAVE_DEFAULT_LAYOUT, SAVE_ASK_PROMPT_LAYOUT, PARTY_DEFAULT_LAYOUT, STATS_PAGE1_LAYOUT, STATS_PAGE2_LAYOUT, BAG_DEFAULT_LAYOUT};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::FrameBuffer;
use pokered_ui::backends::framebuffer::FrameBufferPainter;
use pokered_ui::{menus, InkColor, TileRect, Ui};

pub fn draw_main_menu(state: &MainMenuState, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    menus::main::draw(state, &MAIN_DEFAULT_LAYOUT, &mut Ui::new(&mut painter), lang);
}

pub fn draw_start_menu(state: &StartMenuState, player_name: &str, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    menus::start::draw(state, player_name, &START_DEFAULT_LAYOUT, &mut Ui::new(&mut painter), lang);
}

pub fn draw_options_menu(state: &OptionsMenuState, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    menus::options::draw(state, &OPTIONS_DEFAULT_LAYOUT, &mut Ui::new(&mut painter), lang);
}

pub fn draw_save_menu(state: &SaveMenuState, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    menus::save::draw(state, &SAVE_DEFAULT_LAYOUT, &SAVE_ASK_PROMPT_LAYOUT, &mut Ui::new(&mut painter), lang);
}

/// TUI variant of the party screen: same signature as the app version but
/// `_resources` and `_frame_counter` are intentionally unused — the
/// terminal backend has no GPU surface to composite Pokémon icons onto
/// and therefore no need for animation frame selection.
pub fn draw_party_screen(
    state: &PartyScreenState,
    _resources: Option<&mut ResourceManager>,
    _frame_counter: u64,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    menus::party::draw(state, &PARTY_DEFAULT_LAYOUT, &mut Ui::new(&mut painter), lang);
}

pub fn draw_stats_screen(state: &StatsScreenState, fb: &mut FrameBuffer, lang: Lang) {
    let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
    menus::stats::draw(state, &STATS_PAGE1_LAYOUT, &STATS_PAGE2_LAYOUT, &mut Ui::new(&mut painter), lang, &PokemonRenderData::new(false));
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
                let lines: Vec<String> = match dialogue {
                    BuyResult::Success { .. } => {
                        if lang == Lang::Zh { vec!["谢谢惠顾！".into(), "欢迎再来！".into()] } else { vec!["Thank you!".into(), "Come again!".into()] }
                    }
                    BuyResult::NotEnoughMoney => {
                        if lang == Lang::Zh { vec!["你的钱".into(), "不够啊！".into()] } else { vec!["You don't have".into(), "enough money!".into()] }
                    }
                    BuyResult::BagFull => {
                        if lang == Lang::Zh { vec!["你的包包满了！".into()] } else { vec!["Your bag is full!".into()] }
                    }
                    BuyResult::InvalidItem => {
                        if lang == Lang::Zh { vec!["没有这个".into(), "道具！".into()] } else { vec!["That item doesn't".into(), "exist!".into()] }
                    }
                };
                let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
                menus::mart::draw_result_dialog(&refs, &MART_RESULT_DIALOG_LAYOUT, &mut ui);
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
                            format!("{} ×{} ${}.00\n我给你${}.00。\n可以吗？", item_name, quantity, total, total)
                        } else {
                            format!("{} ×{} ${}.00\nI can pay ${}.00.\nOK?", item_name, quantity, total, total)
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
                let lines: Vec<String> = match dialogue {
                    SellResult::Success { .. } => {
                        if lang == Lang::Zh { vec!["谢谢惠顾！".into(), "欢迎再来！".into()] } else { vec!["Thank you!".into(), "Come again!".into()] }
                    }
                    SellResult::Unsellable => {
                        if lang == Lang::Zh { vec!["这个我不收。".into()] } else { vec!["I can't buy".into(), "that item.".into()] }
                    }
                    SellResult::NotInBag => {
                        if lang == Lang::Zh { vec!["你没有这个".into(), "道具！".into()] } else { vec!["You don't have".into(), "that item!".into()] }
                    }
                    SellResult::InvalidItem => {
                        if lang == Lang::Zh { vec!["没有这个".into(), "道具！".into()] } else { vec!["That item doesn't".into(), "exist!".into()] }
                    }
                };
                let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
                menus::mart::draw_result_dialog(&refs, &MART_RESULT_DIALOG_LAYOUT, &mut ui);
            }
        },
        MartPhase::Exiting => {}
    }
}

/// Overworld ITEM bag (Start menu → ITEM): the item list, plus a USE / TOSS /
/// CANCEL menu or the TOSS-quantity prompt when an item is selected.
/// Mirrors the native app's render/menu.rs::draw_bag (same pokered-ui widgets).
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
                for (i, opt) in ["USE", "TOSS", "CANCEL"].iter().enumerate() {
                    frame.label(2, 1 + i as u32 * 2, opt, InkColor::Black);
                }
                if let Some(c) = &BAG_DEFAULT_LAYOUT.list.cursor {
                    frame.cursor_glyph_at(1, 1 + cursor as u32 * 2, c.glyph, c.color);
                }
            });
        }
        BagPhase::TossQuantity { qty } => {
            ui.text_box(TileRect::new(4, 12, 15, 4), InkColor::Black, true, |frame| {
                frame.label(2, 1, "TOSS HOW MANY?", InkColor::Black);
                frame.label(2, 2, &format!("x{:02}", qty), InkColor::Black);
            });
        }
        BagPhase::Browsing => {}
    }
}
