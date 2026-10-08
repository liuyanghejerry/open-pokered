//! Cable Club link UI overlay: drawn over the frozen overworld while the
//! in-room link flow is modal (text boxes, the peer-request yes/no prompt,
//! and the trade party-select list).
//!
//! Mirrors the original screens: `CableClub_TextBoxBorder` boxes for
//! "Just a moment." / "Waiting...!" / "PLEASE WAIT!"
//! (engine/link/cable_club.asm:15-18, engine/link/print_waiting_text.asm),
//! the `_WillBeTradedText` + TRADE_CANCEL_MENU confirm
//! (engine/link/cable_club.asm:714-740), and the `TradeCenter_SelectMon`
//! party lists and peer stats available before confirmation.

use crate::alloc_prelude::*;
use pokered_core::game_state::Lang;
use pokered_data::lang_data;
use pokered_data::lang_data::species_name;
use pokered_data::ui_layout::schema::{DIALOG_DEFAULT_LAYOUT, YES_NO_DEFAULT_LAYOUT};
use pokered_renderer::FrameBuffer;
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::menus;
use pokered_ui::Ui;
use pokered_ui::{Painter, Rgba, TilePos};

use crate::link::cable_club::CableClubFlow;

/// Display-layer translation for the fixed link-flow box texts (the strings
/// themselves live in the app's `cable_club` driver; this only affects what
/// the player sees).
fn zh_link_text(text: &str) -> String {
    match text {
        "the link, we have\nto save the game." => "必须保存游戏。".to_string(),
        "Just a moment." => "请稍等。".to_string(),
        "Waiting...!" => "正在等待……！".to_string(),
        "PLEASE WAIT!" => "请稍候！".to_string(),
        "OK, please wait\njust a moment." => "好的，请稍等\n片刻。".to_string(),
        "Trade completed!" => "交换完成！".to_string(),
        "Too bad! The trade\nwas canceled!" => "太可惜了！交换被取消了！".to_string(),
        "The link was\ncanceled." => "联机被取消了。".to_string(),
        "Start a link\nbattle?" => "开始联机对战？".to_string(),
        "Start a link\ntrade?" => "开始联机交换？".to_string(),
        _ => {
            if let Some((local, remote)) = text.split_once(" and\n") {
                if let Some(remote) = remote.strip_suffix(" will be traded.") {
                    return format!("{}与\n{}将交换。", local, remote);
                }
            }
            text.to_string()
        }
    }
}

/// Draw the link flow overlay (no-op when the flow has nothing to show).
pub fn draw_link_flow(
    flow: &CableClubFlow,
    fb: &mut FrameBuffer,
    is_zh: bool,
    resources: Option<&mut pokered_renderer::resource::ResourceManager>,
) {
    let language = if is_zh { Lang::Zh } else { Lang::En };

    let post_trade_clear = matches!(flow.phase(), crate::link::cable_club::CableClubPhase::TradeSync
        | crate::link::cable_club::CableClubPhase::TradeSyncDelay { .. }
        | crate::link::cable_club::CableClubPhase::TradeCompletionDelay { .. }
        | crate::link::cable_club::CableClubPhase::TradeCompleted);
    if post_trade_clear {
        FrameBufferPainter::new(fb).clear(Rgba::INK_WHITE);
    }
    if let Some(stats) = flow.stats() {
        if stats.entry_frame() == Some(0) { draw_trade_party_list(flow, fb, language, is_zh); }
        else { super::draw_stats_screen(stats, resources, fb, language); }
        return;
    }
    if !post_trade_clear && flow.party_select().is_some() {
        draw_trade_party_list(flow, fb, language, is_zh);
    }

    if let Some(selected) = flow.reception_menu() {
        let mut painter = FrameBufferPainter::new(fb).with_lang(language);
        painter.clear(Rgba::INK_WHITE);
        for (i, text) in (if is_zh { ["交换中心", "竞技场", "取消"] } else { ["TRADE CENTER", "COLOSSEUM", "CANCEL"] }).iter().enumerate() {
            painter.draw_text(TilePos::new(3, 5 + i as u32 * 2), text, Rgba::INK_BLACK);
            if selected as usize == i { painter.draw_glyph(TilePos::new(2, 5 + i as u32 * 2), '▶', Rgba::INK_BLACK); }
        }
        return;
    }

    if let Some((title, selected)) = flow.prompt() {
        // Yes/no prompt (peer battle/trade request, or the trade confirm):
        // a text box with the question plus the YES/NO menu — the original's
        // text + two-option menu pairing (TRADE_CANCEL_MENU).
        let title = if is_zh { zh_link_text(&title) } else { title };
        draw_dialog(&title, fb, language);
        let mut painter = FrameBufferPainter::new(fb);
        let mut ui = Ui::new(&mut painter);
        let (yes, no) = (
            lang_data::ui_label("YES", is_zh).to_string(),
            lang_data::ui_label("NO", is_zh).to_string(),
        );
        menus::yes_no::draw(&[yes, no], selected as u32, &YES_NO_DEFAULT_LAYOUT, &mut ui);
        return;
    }

    if let Some(text) = flow.text_box() {
        let shown = if is_zh { zh_link_text(&text) } else { text };
        draw_dialog(&shown, fb, language);
        return;
    }
}

fn draw_dialog(text: &str, fb: &mut FrameBuffer, language: Lang) {
    let mut painter = FrameBufferPainter::new(fb);
    let mut ui = Ui::new(&mut painter);
    menus::dialog::draw_paginated(text, false, &DIALOG_DEFAULT_LAYOUT, &mut ui, language);
}

/// Both parties remain visible while choosing; left/right switches the list.
fn draw_trade_party_list(flow: &CableClubFlow, fb: &mut FrameBuffer, language: Lang, is_zh: bool) {
    let Some(sel) = flow.party_select() else {
        return;
    };
    let mut painter = FrameBufferPainter::new(fb).with_lang(language);
    painter.clear(Rgba::INK_WHITE);
    let (local_name, remote_name) = flow.trainer_names();
    for (party, top, label, cursor) in [
        (
            sel.party(),
            0,
            if !local_name.is_empty() {
                local_name
            } else if is_zh {
                "我方"
            } else {
                "YOUR PARTY"
            },
            (flow.peer_cursor().is_none() && !flow.cancel_selected()).then_some(sel.cursor()),
        ),
        (
            flow.remote_party(),
            8,
            if !remote_name.is_empty() {
                remote_name
            } else if is_zh {
                "对方"
            } else {
                "PARTNER"
            },
            flow.peer_cursor(),
        ),
    ] {
        painter.draw_text(TilePos::new(1, top), label, Rgba::INK_BLACK);
        for (i, mon) in party.iter().enumerate() {
            let mut buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
            let name = if mon.has_nickname() {
                mon.display_name(&mut buf)
            } else {
                species_name(mon.species, is_zh)
            };
            painter.draw_text(TilePos::new(2, top + 1 + i as u32), name, Rgba::INK_BLACK);
            if cursor == Some(i) {
                painter.draw_glyph(TilePos::new(1, top + 1 + i as u32), '▶', Rgba::INK_BLACK);
            }
        }
    }
    if let Some((_, trade_selected)) = flow.local_action() {
        painter.draw_text_box(pokered_ui::TileRect::new(0, 14, 20, 4), Rgba::INK_BLACK);
        painter.draw_text(TilePos::new(2, 16), if is_zh { "状态" } else { "STATS" }, Rgba::INK_BLACK);
        painter.draw_text(TilePos::new(12, 16), if is_zh { "交换" } else { "TRADE" }, Rgba::INK_BLACK);
        painter.draw_glyph(TilePos::new(if trade_selected { 11 } else { 1 }, 16), '▶', Rgba::INK_BLACK);
        return;
    }
    painter.draw_text_box(pokered_ui::TileRect::new(0, 15, 11, 3), Rgba::INK_BLACK);
    painter.draw_text(TilePos::new(2, 16), if is_zh { "取消" } else { "CANCEL" }, Rgba::INK_BLACK);
    if flow.cancel_selected() {
        painter.draw_glyph(TilePos::new(1, 16), '▶', Rgba::INK_BLACK);
    }
}
