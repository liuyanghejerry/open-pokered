use pokered_core::battle::state::{Pokemon, StatusCondition};
use pokered_core::game_state::Lang;
use pokered_core::overworld::hm_effects;
use pokered_core::party_screen::{PartyScreenPhase, PartyScreenState};
use pokered_data::lang_data;
use pokered_data::ui_layout::schema::{
    PartyDefaultLayout, PARTY_ACTION_MENU_LAYOUT, PARTY_ENTRY_LAYOUT,
    PARTY_SWITCH_HINT_LAYOUT,
};

use crate::engine::{Frame, InkColor, Painter, TileRect, Ui};

const NAME_MAX_LEN: usize = 10;

fn status_code(status: &StatusCondition) -> &'static str {
    match status {
        StatusCondition::None => "",
        StatusCondition::Sleep(_) => "SLP",
        StatusCondition::Poison => "PSN",
        StatusCondition::Burn => "BRN",
        StatusCondition::Freeze => "FRZ",
        StatusCondition::Paralysis => "PAR",
    }
}

pub fn draw<P: Painter>(state: &PartyScreenState, layout: &PartyDefaultLayout, ui: &mut Ui<P>, lang: Lang) {
    let is_zh = lang == Lang::Zh;
    ui.clear(InkColor::White);

    let party = state.party();
    if party.is_empty() {
        let default_region = &layout.region_0;
        ui.text_box(default_region.rect, default_region.color, false, |frame| {
            for label in default_region.labels.iter() {
                frame.label(label.tx, label.ty, lang_data::ui_label(&label.text, is_zh), label.color);
            }
        });
        return;
    }

    let cursor = state.cursor();
    let phase = state.phase();
    let entry_layout = &PARTY_ENTRY_LAYOUT;
    let cursors = entry_layout.cursors.as_ref();

    let source_index = match phase {
        PartyScreenPhase::SwitchTarget { source_index } => Some(source_index),
        _ => None,
    };

    ui.text_box(layout.region_1.rect, layout.region_1.color, false, |frame| {
        for (i, pokemon) in party.iter().enumerate() {
            let row = i as u32 * cursors[0].row_step;
            let is_cursor = i == cursor;
            let is_source = source_index == Some(i);

            if is_cursor {
                let c = &cursors[0];
                let cy = c.base_ty + row;
                frame.cursor_glyph_at(c.tx, cy, c.glyph, c.color);
            } else if is_source {
                let c = &cursors[1];
                let cy = c.base_ty + row;
                frame.cursor_glyph_at(c.tx, cy, c.glyph, c.color);
            }

            draw_entry(frame, pokemon, row, entry_layout);
        }
    });

    match phase {
        PartyScreenPhase::Browsing => {}
        PartyScreenPhase::ActionMenu { cursor: menu_cursor } => {
            draw_action_menu(ui, state, menu_cursor, is_zh);
        }
        PartyScreenPhase::SwitchTarget { .. } => {
            draw_switch_hint(ui, is_zh);
        }
        PartyScreenPhase::ChooseMove { cursor: move_cursor } => {
            draw_move_choice(ui, state, move_cursor, is_zh);
        }
    }
}

fn draw_entry<P: Painter>(
    frame: &mut Frame<'_, P>,
    pokemon: &Pokemon,
    row: u32,
    layout: &pokered_data::ui_layout::schema::PartyEntryLayout,
) {
    let dl = layout.dynamic_labels.as_ref();

    let name_dl = dl.iter().find_map(|(k, v)| if k == "name" { Some(v) } else { None });
    let level_dl = dl.iter().find_map(|(k, v)| if k == "level" { Some(v) } else { None });
    let status_dl = dl.iter().find_map(|(k, v)| if k == "status" { Some(v) } else { None });
    let hp_val_dl = dl.iter().find_map(|(k, v)| if k == "hp_value" { Some(v) } else { None });

    let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
    let name = pokemon.display_name(&mut name_buf);
    let display_name: &str = if name.len() > NAME_MAX_LEN { &name[..NAME_MAX_LEN] } else { &name };
    if let Some(dl) = name_dl {
        frame.label(dl.tx, dl.ty + row, display_name, dl.color);
    }

    let lvl_str = format!(":L{}", pokemon.level);
    if let Some(dl) = level_dl {
        frame.label(dl.tx, dl.ty + row, &lvl_str, dl.color);
    }

    let code = status_code(&pokemon.status);
    if !code.is_empty() {
        if let Some(dl) = status_dl {
            frame.label(dl.tx, dl.ty + row, code, dl.color);
        }
    }

    let hp_str = format!("{}/{}", pokemon.hp, pokemon.max_hp);
    if let Some(dl) = hp_val_dl {
        frame.label(dl.tx, dl.ty + row, &hp_str, dl.color);
    }
}

fn draw_action_menu<P: Painter>(ui: &mut Ui<P>, state: &PartyScreenState, menu_cursor: u8, is_zh: bool) {
    let field_moves = state.selected_field_moves();
    let n = field_moves.len() as u32;

    // Menu entries: usable field moves first (Gen-1 order), then
    // STATS / SWITCH / CANCEL — mirrors DisplayFieldMoveMonMenu.
    let mut items: Vec<&str> = field_moves
        .iter()
        .map(|m| lang_data::move_name(*m, is_zh))
        .collect();
    items.push(lang_data::ui_label("STATS", is_zh));
    items.push(lang_data::ui_label("SWITCH", is_zh));
    items.push(lang_data::ui_label("CANCEL", is_zh));

    if n == 0 {
        // No field moves: the fixed 3-entry box from the layout file.
        let box_def = &PARTY_ACTION_MENU_LAYOUT.box_0;
        ui.text_box(box_def.rect, box_def.color, true, |frame| {
            frame.menu_list(0, 0, &items, menu_cursor as usize, 2, InkColor::Black);
        });
        return;
    }

    // With field moves the original grows the box 2 rows per move and shifts
    // it left when a long name (STRENGTH/TELEPORT) is listed
    // (FieldMoveDisplayData "leftmost tile", text_box.asm).
    let leftmost = field_moves
        .iter()
        .filter_map(|m| hm_effects::field_move_menu_leftmost(*m))
        .min()
        .unwrap_or(0x0C) as u32;
    let base = &PARTY_ACTION_MENU_LAYOUT.box_0.rect;
    let rect = TileRect::new(
        leftmost - 1,
        base.ty - 2 * n,
        8,
        base.th + 2 * n,
    );
    ui.text_box(rect, InkColor::Black, true, |frame| {
        frame.menu_list(0, 0, &items, menu_cursor as usize, 2, InkColor::Black);
    });
}

fn draw_switch_hint<P: Painter>(ui: &mut Ui<P>, is_zh: bool) {
    let box_def = &PARTY_SWITCH_HINT_LAYOUT.box_0;
    ui.text_box(box_def.rect, box_def.color, true, |frame| {
        for label in box_def.labels.iter() {
            frame.label(label.tx, label.ty, lang_data::ui_label(&label.text, is_zh), label.color);
        }
    });
}

/// "Which move should be forgotten?" — the selected mon's known moves plus a
/// CANCEL row (TM/HM teaching when the moveset is full). Rendered like the
/// action menu: a right-side box grown to fit the entries.
fn draw_move_choice<P: Painter>(ui: &mut Ui<P>, state: &PartyScreenState, move_cursor: u8, is_zh: bool) {
    let moves = state.selected_known_moves();
    let mut items: Vec<&str> = moves
        .iter()
        .map(|m| lang_data::move_name(*m, is_zh))
        .collect();
    items.push(lang_data::ui_label("CANCEL", is_zh));

    let extra_rows = (items.len() as u32).saturating_sub(3);
    let base = &PARTY_ACTION_MENU_LAYOUT.box_0.rect;
    let rect = TileRect::new(
        base.tx,
        base.ty - 2 * extra_rows,
        base.tw,
        base.th + 2 * extra_rows,
    );
    ui.text_box(rect, InkColor::Black, true, |frame| {
        frame.menu_list(0, 0, &items, move_cursor as usize, 2, InkColor::Black);
    });
}
