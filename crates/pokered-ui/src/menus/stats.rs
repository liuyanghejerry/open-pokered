use dotzuki_engine::render_data::RenderData;
use pokered_core::battle::experience::growth::exp_for_level;
use pokered_core::battle::state::{Pokemon, StatusCondition};
use pokered_core::game_state::Lang;
use pokered_core::stats_screen::{StatsPage, StatsScreenState};
use pokered_data::lang_data;
use pokered_data::moves::MoveId;
use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::Species;
use pokered_data::ui_layout::schema::{get_screen_v2_json, StatsPage1Layout, StatsPage2Layout};

use crate::engine::{InkColor, Painter, Ui};
use crate::v2::{self, DataContext};

const NAME_MAX_LEN: usize = 10;

pub fn draw<P: Painter>(
    state: &StatsScreenState,
    _page1_layout: &StatsPage1Layout,
    page2_layout: &StatsPage2Layout,
    ui: &mut Ui<P>,
    lang: Lang,
    render_data: &dyn RenderData<Move = MoveId, Item = pokered_data::items::ItemId, Species = Species>,
) {
    match state.page() {
        // Page 1 (stats) — migrated to the v2 layout engine (uses the hp_bar
        // primitive). Page 2 (moves) still uses the v1 renderer.
        StatsPage::Stats => draw_page1_v2(state.pokemon(), ui, lang, render_data),
        StatsPage::Moves => {
            ui.clear(InkColor::White);
            draw_page2(state.pokemon(), page2_layout, ui, render_data);
        }
    }
}

fn draw_page1_v2<P: Painter>(
    mon: &Pokemon,
    ui: &mut Ui<P>,
    lang: Lang,
    render_data: &dyn RenderData<Move = MoveId, Item = pokered_data::items::ItemId, Species = Species>,
) {
    let Some(json) = get_screen_v2_json("stats") else {
        ui.clear(InkColor::White);
        return;
    };
    let Some(layout) = v2::parse_screen(json) else {
        ui.clear(InkColor::White);
        return;
    };

    let is_zh = lang == Lang::Zh;
    let name = display_name(mon, render_data);
    let name = if name.len() > NAME_MAX_LEN {
        name[..NAME_MAX_LEN].to_string()
    } else {
        name
    };

    let mut ctx = DataContext::new();
    ctx.set("name", name);
    ctx.set("level", mon.level as i64);
    ctx.set("hp", mon.hp as i64);
    ctx.set("max_hp", mon.max_hp as i64);
    ctx.set("status", status_code(&mon.status));
    ctx.set("dex_num", mon.species as u8 as i64);
    ctx.set("attack", mon.attack as i64);
    ctx.set("defense", mon.defense as i64);
    ctx.set("speed", mon.speed as i64);
    ctx.set("special", mon.special as i64);
    ctx.set("type1", lang_data::type_name(mon.type1, is_zh));
    // The original only shows TYPE2 when it differs from TYPE1.
    ctx.set(
        "type2",
        if mon.type1 != mon.type2 {
            lang_data::type_name(mon.type2, is_zh)
        } else {
            ""
        },
    );
    // Trainer ID № / OT name for the right column.
    ctx.set("id", mon.ot_id as i64);
    let mut ot_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
    ctx.set("ot", pokered_core::battle::state::decode_name(&mon.ot_name, &mut ot_buf));
    // Visibility keys the layout uses to pick the right-column arrangement:
    // CJK glyphs are taller than one tile row, so the zh variant cannot stack
    // values under labels the way the Latin layout does.
    ctx.set("is_zh", is_zh);
    ctx.set("is_en", !is_zh);
    ctx.set("__lang", v2::lang_code(lang));

    v2::render_screen(&layout, &ctx, ui.painter());
}

fn draw_page2<P: Painter>(
    mon: &Pokemon,
    layout: &StatsPage2Layout,
    ui: &mut Ui<P>,
    render_data: &dyn RenderData<Move = MoveId, Item = pokered_data::items::ItemId, Species = Species>,
) {
    ui.text_box(layout.region_0.rect, layout.region_0.color, false, |frame| {
        for label in layout.region_0.labels.iter() {
            frame.label(label.tx, label.ty, &label.text, label.color);
        }

        let name = display_name(mon, render_data);
        let name_display: &str = if name.len() > NAME_MAX_LEN { &name[..NAME_MAX_LEN] } else { &name };
        frame.label(9, 1, name_display, InkColor::Black);

        frame.pixel_rect(19 * 8, 3 * 8, 1, 8, InkColor::Black);

        frame.label(12, 4, &format!("{:>7}", mon.total_exp), InkColor::Black);

        let exp_needed = calc_exp_to_level_up(mon);
        frame.label(5, 6, &format!("{:>7}", exp_needed), InkColor::Black);
        let target_level = if mon.level >= 100 { 100 } else { mon.level + 1 };
        frame.label(15, 6, &format!(":L{:<3}", target_level), InkColor::Black);
    });

    // ── Moves box ──────────────────────────────────────────────────
    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |frame| {
        for slot in 0..4 {
            // Each move takes two interior rows — name on top, PP below —
            // filling the box's 8 interior rows exactly. (One row lower and
            // the last PP line would sit on the bottom border.)
            let name_row = (slot * 2) as u32;
            let pp_row = (slot * 2 + 1) as u32;
            let move_id = mon.moves[slot];
            if move_id == MoveId::None {
                frame.label(1, name_row, "-", InkColor::DarkGray);
                frame.label(10, pp_row, "--", InkColor::DarkGray);
                continue;
            }
            frame.label(1, name_row, render_data.move_name(move_id), InkColor::Black);
            frame.label(10, pp_row, "PP", InkColor::Black);
            let pp_current = mon.pp[slot];
            let pp_max = calc_max_pp(move_id, mon.pp_ups[slot], render_data);
            frame.label(
                13,
                pp_row,
                &format!("{:>2}/{:>2}", pp_current, pp_max),
                InkColor::Black,
            );
        }
    });
}

fn display_name(
    mon: &Pokemon,
    render_data: &dyn RenderData<Move = MoveId, Item = pokered_data::items::ItemId, Species = Species>,
) -> String {
    let mut buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
    if mon.has_nickname() {
        return mon.display_name(&mut buf).to_string();
    }
    render_data.species_name(mon.species).to_string()
}

fn status_code(status: &StatusCondition) -> &'static str {
    match status {
        StatusCondition::None => "OK",
        StatusCondition::Sleep(_) => "SLP",
        StatusCondition::Poison => "PSN",
        StatusCondition::Burn => "BRN",
        StatusCondition::Freeze => "FRZ",
        StatusCondition::Paralysis => "PAR",
    }
}

fn calc_max_pp(
    move_id: MoveId,
    pp_ups: u8,
    render_data: &dyn RenderData<Move = MoveId, Item = pokered_data::items::ItemId, Species = Species>,
) -> u8 {
    let (base, _) = render_data.move_pp(move_id);
    let ups = pp_ups.min(3);
    let bonus = (base as u16 * ups as u16 / 5) as u8;
    base + bonus
}

fn calc_exp_to_level_up(mon: &Pokemon) -> u32 {
    if mon.level >= 100 {
        return 0;
    }
    match get_base_stats(mon.species) {
        Some(bs) => {
            let total_for_next = exp_for_level(bs.growth_rate, mon.level + 1);
            total_for_next.saturating_sub(mon.total_exp)
        }
        None => 0,
    }
}
