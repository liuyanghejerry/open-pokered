use pokered_core::battle::state::{Pokemon, StatusCondition};
use pokered_core::game_state::Lang;
use pokered_core::overworld::hm_effects;
use pokered_core::party_screen::{PartyScreenPhase, PartyScreenState};
use pokered_data::lang_data;
use pokered_data::ui_layout::schema::{
    PartyDefaultLayout, PARTY_ACTION_MENU_LAYOUT, PARTY_ENTRY_LAYOUT,
    PARTY_SWITCH_HINT_LAYOUT,
};

use crate::engine::{InkColor, Painter, TileRect, Ui};

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

pub fn draw<P: Painter>(
    state: &PartyScreenState,
    layout: &PartyDefaultLayout,
    ui: &mut Ui<P>,
    lang: Lang,
) {
    draw_entries(state, layout, ui, lang);
    draw_overlay(state, ui, lang);
}

/// Draw the list before a frontend composites its party icons and HP bars.
pub fn draw_entries<P: Painter>(
    state: &PartyScreenState,
    layout: &PartyDefaultLayout,
    ui: &mut Ui<P>,
    lang: Lang,
) {
    let is_zh = lang == Lang::Zh;
    ui.clear(InkColor::White);

    let party = state.party();
    if party.is_empty() {
        let default_region = &layout.region_0;
        ui.text_box(default_region.rect, default_region.color, false, |frame| {
            for label in default_region.labels.iter() {
                frame.label(
                    label.tx,
                    label.ty,
                    lang_data::ui_label(&label.text, is_zh),
                    label.color,
                );
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

    ui.text_box(
        layout.region_1.rect,
        layout.region_1.color,
        false,
        |frame| {
            for (i, _) in party.iter().enumerate() {
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
            }
        },
    );
    for (i, pokemon) in party.iter().enumerate() {
        draw_entry(
            ui.painter(),
            pokemon,
            i as u32 * cursors[0].row_step,
            entry_layout,
        );
    }
}

/// Draw menus last so they cover entries, including frontend-rendered sprites.
pub fn draw_overlay<P: Painter>(state: &PartyScreenState, ui: &mut Ui<P>, lang: Lang) {
    let is_zh = lang == Lang::Zh;
    match state.phase() {
        PartyScreenPhase::Browsing => {}
        PartyScreenPhase::ActionMenu {
            cursor: menu_cursor,
        } => {
            draw_action_menu(ui, state, menu_cursor, is_zh);
        }
        PartyScreenPhase::SwitchTarget { .. } => {
            draw_switch_hint(ui, is_zh);
        }
        PartyScreenPhase::ChooseMove {
            cursor: move_cursor,
        } => {
            draw_move_choice(ui, state, move_cursor, is_zh);
        }
        PartyScreenPhase::MoveChoiceNotice => {
            ui.text_box(TileRect::new(0, 12, 20, 6), InkColor::Black, true, |frame| {
                for (row, line) in state
                    .move_choice_notice()
                    .unwrap_or("")
                    .lines()
                    .take(4)
                    .enumerate()
                {
                    frame.label(1, 1 + row as u32, line, InkColor::Black);
                }
            });
        }
        PartyScreenPhase::ItemHpRestore => {}
        PartyScreenPhase::ItemUseNotice { .. } => {
            ui.text_box(TileRect::new(0, 12, 20, 6), InkColor::Black, true, |frame| {
                for (row, line) in state.item_use_notice().unwrap_or("").lines().take(4).enumerate() {
                    frame.label(1, 1 + row as u32, line, InkColor::Black);
                }
            });
        }
    }
}

fn draw_entry<P: Painter>(
    painter: &mut P,
    pokemon: &Pokemon,
    row: u32,
    layout: &pokered_data::ui_layout::schema::PartyEntryLayout,
) {
    let dl = layout.dynamic_labels.as_ref();

    let name_dl = dl
        .iter()
        .find_map(|(k, v)| if k == "name" { Some(v) } else { None });
    let level_dl = dl
        .iter()
        .find_map(|(k, v)| if k == "level" { Some(v) } else { None });
    let status_dl = dl
        .iter()
        .find_map(|(k, v)| if k == "status" { Some(v) } else { None });
    let hp_val_dl = dl
        .iter()
        .find_map(|(k, v)| if k == "hp_value" { Some(v) } else { None });

    let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
    let name = pokemon.display_name(&mut name_buf);
    if let Some(dl) = name_dl {
        // Leave a full tile before status/level, including for wide nicknames.
        let name_right = if pokemon.status != StatusCondition::None {
            88
        } else {
            120
        };
        let mut display_name = String::new();
        for ch in name.chars().take(NAME_MAX_LEN) {
            let next = format!("{display_name}{ch}");
            if dl.tx * 8 + painter.measure_text_px(&next) > name_right {
                break;
            }
            display_name.push(ch);
        }
        painter.draw_text_px(dl.tx * 8, (dl.ty + row) * 8, &display_name, dl.color.into());
    }

    let lvl_str = format!("Lv{}", pokemon.level);
    if let Some(dl) = level_dl {
        let right = (dl.tx + 5) * 8 - 8;
        painter.draw_text_px(
            right - painter.measure_text_px(&lvl_str),
            (dl.ty + row) * 8,
            &lvl_str,
            dl.color.into(),
        );
    }

    let code = status_code(&pokemon.status);
    if !code.is_empty() {
        if let Some(dl) = status_dl {
            painter.draw_text_px(dl.tx * 8, (dl.ty + row) * 8, code, dl.color.into());
        }
    }

    let hp_str = format!("{}/{}", pokemon.hp, pokemon.max_hp);
    if let Some(dl) = hp_val_dl {
        let right = (dl.tx + 7) * 8 - 8;
        // Fusion Pixel ink is taller than 8px: use 12px between text baselines.
        painter.draw_text_px(
            right - painter.measure_text_px(&hp_str),
            (dl.ty + row) * 8 + 4,
            &hp_str,
            dl.color.into(),
        );
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
    let width = menu_width(&items).max(base.tx + base.tw - (leftmost - 1));
    let rect = TileRect::new(
        base.tx + base.tw - width,
        base.ty - 2 * n,
        width,
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
    // Original learn_move.asm:123: the move-choice menu is its OWN box at
    // column 4 with an interior 14 tiles wide — the narrow action-menu box
    // truncated LEECH SEED / POISONPOWDER past the border (audit:
    // cut-forget-menu.png).
    let rect = TileRect::new(
        4,
        base.ty - 2 * extra_rows,
        16,
        base.th + 2 * extra_rows,
    );
    ui.text_box(rect, InkColor::Black, true, |frame| {
        frame.menu_list(0, 0, &items, move_cursor as usize, 2, InkColor::Black);
    });
}

#[cfg(test)]
mod move_choice_tests {
    use super::*;
    use crate::engine::{TilePos, Rgba};
    use pokered_core::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;

    #[derive(Debug, Default)]
    struct Rec {
        ops: Vec<Op>,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Op {
        Box(TileRect, Rgba),
        Text(TilePos, String),
        Cursor(TilePos),
    }

    impl Painter for Rec {
        fn clear(&mut self, _color: Rgba) {}
        fn draw_text_box(&mut self, rect: TileRect, color: Rgba) {
            self.ops.push(Op::Box(rect, color));
        }
        fn draw_text(&mut self, pos: TilePos, text: &str, _color: Rgba) {
            self.ops.push(Op::Text(pos, text.to_string()));
        }
        fn draw_glyph(&mut self, pos: TilePos, _glyph: char, _color: Rgba) {
            self.ops.push(Op::Cursor(pos));
        }
        fn draw_pixel_rect(&mut self, _x: u32, _y: u32, _w: u32, _h: u32, _c: Rgba) {}
        fn draw_gb_tile(&mut self, _pos: TilePos, _tile_id: u8, _fallback: &str, _color: Rgba) {}
    }

    /// The forget-menu box matches the original (learn_move.asm:123): column 4,
    /// interior 14 wide — and every move name fits inside it.
    #[test]
    fn forget_menu_box_wide_enough_for_long_move_names() {
        let mon = create_pokemon_with_moves(
            Species::Venusaur,
            50,
            [0xFF, 0xFF],
            [
                MoveId::LeechSeed,
                MoveId::Poisonpowder,
                MoveId::SleepPowder,
                MoveId::RazorLeaf,
            ],
        )
        .unwrap();
        let state = PartyScreenState::new(vec![mon]);
        let mut rec = Rec::default();
        let mut ui = Ui::new(&mut rec);
        draw_move_choice(&mut ui, &state, 0, false);

        let rect = rec
            .ops
            .iter()
            .find_map(|op| match op {
                Op::Box(r, _) => Some(*r),
                _ => None,
            })
            .expect("forget menu box drawn");
        assert_eq!(rect.tx, 4, "box starts at column 4");
        assert_eq!(rect.tw, 16, "interior 14 + two borders");
        assert!(rect.ty + rect.th <= 18, "box bottom stays on screen");

        // No rendered text may cross the right border of the interior.
        let interior_right = rect.tx + rect.tw - 1;
        for op in &rec.ops {
            if let Op::Text(pos, text) = op {
                assert!(
                    pos.tx + text.chars().count() as u32 <= interior_right,
                    "text {text:?} at {} crosses the interior right edge {}",
                    pos.tx,
                    interior_right
                );
            }
        }
    }
}

// One tile per glyph, plus the cursor column and both borders.
fn menu_width(items: &[&str]) -> u32 {
    items.iter().map(|text| text.chars().count() as u32).max().unwrap_or(0) + 3
}
