//! Cable Club link UI overlay: drawn over the frozen overworld while the
//! in-room link flow is modal (text boxes, the peer-request yes/no prompt,
//! and the trade party-select list).
//!
//! Mirrors the original screens: `CableClub_TextBoxBorder` boxes for
//! "Just a moment." / "Waiting...!" / "PLEASE WAIT!"
//! (engine/link/cable_club.asm:15-18, engine/link/print_waiting_text.asm),
//! the `_WillBeTradedText` + TRADE_CANCEL_MENU confirm
//! (engine/link/cable_club.asm:714-740), and the `TradeCenter_SelectMon`
//! party list (cable_club.asm:635-680) — v1 shows only the LOCAL list (the
//! wire protocol carries no party metadata until `TradeComplete`; the
//! original exchanged full parties before the menu).

use pokered_core::game_state::Lang;
use pokered_core::party_select::PartySelectState;
use pokered_data::lang_data;
use pokered_data::lang_data::species_name;
use pokered_data::ui_layout::schema::{DIALOG_DEFAULT_LAYOUT, YES_NO_DEFAULT_LAYOUT};
use pokered_renderer::FrameBuffer;
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::menus;
use pokered_ui::Ui;

use crate::link::cable_club::CableClubFlow;

/// Display-layer translation for the fixed link-flow box texts (the strings
/// themselves live in the app's `cable_club` driver; this only affects what
/// the player sees).
fn zh_link_text(text: &str) -> String {
    match text {
        "Just a moment." => "请稍等。".to_string(),
        "Waiting...!" => "正在等待……！".to_string(),
        "PLEASE WAIT!" => "请稍候！".to_string(),
        "Trade completed!" => "交换完成！".to_string(),
        "Too bad! The trade\nwas canceled!" => "太可惜了！交换\n被取消了！".to_string(),
        "The link was\ncanceled." => "联机被\n取消了。".to_string(),
        "Start a link\nbattle?" => "开始联机\n对战？".to_string(),
        "Start a link\ntrade?" => "开始联机\n交换？".to_string(),
        _ => {
            // "<NAME> will\nbe traded." (trade confirm)
            if let Some(name) = text.strip_suffix(" will\nbe traded.") {
                return format!("{}将\n被交换。", name);
            }
            text.to_string()
        }
    }
}

/// Draw the link flow overlay (no-op when the flow has nothing to show).
pub fn draw_link_flow(flow: &CableClubFlow, fb: &mut FrameBuffer, is_zh: bool) {
    let language = if is_zh { Lang::Zh } else { Lang::En };

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

    if let Some(sel) = flow.party_select() {
        draw_trade_party_list(sel, fb, language, is_zh);
    }
}

fn draw_dialog(text: &str, fb: &mut FrameBuffer, language: Lang) {
    let mut painter = FrameBufferPainter::new(fb);
    let mut ui = Ui::new(&mut painter);
    menus::dialog::draw(text, false, &DIALOG_DEFAULT_LAYOUT, &mut ui, language);
}

/// The trade selection list: the local party with a cursor, plus a CANCEL
/// row — the original's `TradeCenter_DrawPartyLists` +
/// `TradeCenter_DrawCancelBox` (engine/link/cable_club.asm:635-680,
/// 601-612), local side only (see module docs).
fn draw_trade_party_list(
    sel: &PartySelectState,
    fb: &mut FrameBuffer,
    language: Lang,
    is_zh: bool,
) {
    let party = sel.party();
    let cursor = sel.cursor();

    let mut lines: Vec<String> = party
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
            let name = if m.has_nickname() {
                m.display_name(&mut name_buf).to_string()
            } else {
                species_name(m.species, is_zh).to_string()
            };
            let marker = if i == cursor { "▶" } else { " " };
            format!("{}{}", marker, name)
        })
        .collect();
    lines.push(format!(
        "{}{}",
        if party.is_empty() { "▶" } else { " " },
        lang_data::ui_label("CANCEL", is_zh)
    ));

    let mut painter = FrameBufferPainter::new(fb);
    let mut ui = Ui::new(&mut painter);
    let text = lines.join("\n");
    menus::dialog::draw(&text, false, &DIALOG_DEFAULT_LAYOUT, &mut ui, language);
}
