//! Per-menu mock data for the layout preview (`render_layout`).
//!
//! Each `fill_mock_*` fills the [`DataContext`] bindings a menu's compiled
//! layout expects, so the editor can render realistic previews without the
//! game running. Moved verbatim from the engine's `jrpg-web` during the
//! engine/game repo split — this data is pokered-flavoured by definition.

use jrpg_renderer::layout_engine::types::{DataContext, DataValue};


// ── Mock data ────────────────────────────────────────────────────────────

pub(crate) fn fill_mock_data(ctx: &mut DataContext, menu_name: &str, mock_state_id: u32) {
    match menu_name {
        "dialog" => fill_mock_dialog(ctx, mock_state_id),
        "yes_no" => fill_mock_yes_no(ctx, mock_state_id),
        "menu" | "main" => fill_mock_main(ctx, mock_state_id),
        "start" => fill_mock_start(ctx, mock_state_id),
        "battle_text" => fill_mock_battle_text(ctx),
        "pokedex" => fill_mock_pokedex(ctx, mock_state_id),
        "pokedex_list" => fill_mock_pokedex_list(ctx),
        "oak_speech" => fill_mock_oak_speech(ctx),
        "battle_main" => fill_mock_battle_main(ctx),
        "battle_party" => fill_mock_battle_party(ctx),
        "save" => fill_mock_save(ctx),
        "options" => fill_mock_options(ctx),
        "naming" => fill_mock_naming(ctx),
        "bag" | "battle_bag" => fill_mock_bag(ctx),
        "battle_move" => fill_mock_moves(ctx),
        "party" => fill_mock_party(ctx),
        "stats" => fill_mock_stats(ctx),
        "mart" => fill_mock_mart(ctx),
        "title" => fill_mock_title(ctx),
        _ => {}
    }
}

fn fill_mock_dialog(ctx: &mut DataContext, mock_state_id: u32) {
    let text = match mock_state_id {
        0 => "Hello there! Welcome to the world of adventure.",
        1 => "This is a longer dialog with more text to demonstrate word wrapping across multiple lines in the text box display.",
        2 => "Short message.",
        _ => "Hello there! Welcome to the world of adventure.",
    };
    ctx.set("text", text);
}

fn fill_mock_yes_no(ctx: &mut DataContext, mock_state_id: u32) {
    ctx.set("question", "Would you like to proceed?");
    ctx.set("yes_text", "YES");
    ctx.set("no_text", "NO");
    ctx.set("cursor", (mock_state_id % 2) as i64);
    // Cursor arrow row: YES at ty=10, NO at ty=12 in yes_no.json.
    ctx.set("cursor_y", 10i64 + 2 * (mock_state_id % 2) as i64);
}

/// Title-screen main menu (NEW GAME / OPTION, or CONTINUE / NEW GAME / OPTION).
fn fill_mock_main(ctx: &mut DataContext, mock_state_id: u32) {
    ctx.set("title", "");
    match mock_state_id {
        // No save file: NEW GAME / OPTION
        0 => ctx.set("items", vec!["NEW GAME".into(), "OPTION".into()]),
        // Save present: CONTINUE / NEW GAME / OPTION
        _ => ctx.set("items", vec![
            "CONTINUE".into(), "NEW GAME".into(), "OPTION".into(),
        ]),
    }
    ctx.set("cursor", 0i64);
}

/// In-game START menu (right-aligned POKéDEX … EXIT box).
fn fill_mock_start(ctx: &mut DataContext, mock_state_id: u32) {
    ctx.set("title", "");
    match mock_state_id {
        // Minimal menu (no Pokédex / no party yet)
        1 => ctx.set("items", vec![
            "ITEM".into(), "ASH".into(), "SAVE".into(), "OPTION".into(), "EXIT".into(),
        ]),
        // Full menu
        _ => ctx.set("items", vec![
            "POKéDEX".into(), "POKéMON".into(), "ITEM".into(),
            "ASH".into(), "SAVE".into(), "OPTION".into(), "EXIT".into(),
        ]),
    }
    ctx.set("cursor", 0i64);
}

fn fill_mock_battle_text(ctx: &mut DataContext) {
    ctx.set("text", "Wild PIKACHU appeared!");
}

fn fill_mock_pokedex(ctx: &mut DataContext, mock_state_id: u32) {
    match mock_state_id {
        0 => {
            ctx.set("name", "PIKACHU");
            ctx.set("species", "MOUSE");
            ctx.set("dex_num", 25i64);
            ctx.set("height_ft", 1i64);
            ctx.set("height_in", 4i64);
            ctx.set("weight_tenths", 132i64);
            ctx.set("description", "When several of these POKéMON gather, their electricity could build and cause lightning storms.");
            ctx.set("description_lines", DataValue::List(vec![
                "When several of these POKéMON".into(),
                "gather, their electricity could".into(),
                "build and cause lightning storms.".into(),
            ]));
            ctx.set("has_more_pages", false);
            ctx.set("weight_lbs", "13.2 lbs");
        }
        1 => {
            ctx.set("name", "MEWTWO");
            ctx.set("species", "GENETIC");
            ctx.set("dex_num", 150i64);
            ctx.set("height_ft", 6i64);
            ctx.set("height_in", 7i64);
            ctx.set("weight_tenths", 2690i64);
            ctx.set("description", "It was created by a scientist after years of horrific gene splicing and DNA engineering experiments.");
            ctx.set("description_lines", DataValue::List(vec![
                "It was created by a scientist".into(),
                "after years of horrific gene".into(),
                "splicing and DNA engineering".into(),
                "experiments.".into(),
            ]));
            ctx.set("has_more_pages", true);
            ctx.set("weight_lbs", "269.0 lbs");
        }
        _ => fill_mock_pokedex(ctx, 0),
    }
}

fn fill_mock_oak_speech(ctx: &mut DataContext) {
    ctx.set("text", "Hello there! Welcome to the world of POKéMON! My name is OAK! People call me the POKéMON PROF!");
}

fn fill_mock_battle_main(ctx: &mut DataContext) {
    ctx.set("options", vec!["FIGHT".into(), "BAG".into(), "PKMN".into(), "RUN".into()]);
    ctx.set("cursor", 0i64);
    ctx.set("cursor_tx", 0i64);
    ctx.set("cursor_ty", 0i64);
}

fn fill_mock_battle_party(ctx: &mut DataContext) {
    ctx.set("cursor", 0i64);
    // party data is list-driven; layout references {party_members} list variable
    ctx.set("title", "Choose a POKéMON");
    ctx.set("party_members", DataValue::List(vec![
        "BULBASAUR 36/36".into(),
        "PIDGEY 31/31".into(),
        "RATTATA 24/24".into(),
    ]));
}

fn fill_mock_save(ctx: &mut DataContext) {
    ctx.set("title", "SAVE");
    ctx.set("cursor", 0i64);
    ctx.set("player_name", "ASH");
    ctx.set("play_time", "12:34");
    ctx.set("badges", 4i64);
    ctx.set("seen_count", 50i64);
    ctx.set("save_message", "Save completed!");
}

fn fill_mock_options(ctx: &mut DataContext) {
    ctx.set("title", "OPTIONS");
    ctx.set("cursor", 0i64);
    ctx.set("text_speed_options", vec!["SLOW".into(), "MEDIUM".into(), "FAST".into()]);
    ctx.set("animation_options", vec!["ON".into(), "OFF".into()]);
    ctx.set("style_options", vec!["NORMAL".into(), "INVERT".into()]);
    // Cursor sits just left of the selected value on each row.
    ctx.set("cursor_0_tx", 6i64);  // TEXT SPEED = MEDIUM
    ctx.set("cursor_1_tx", 1i64);  // BATTLE ANIMATION = ON
    ctx.set("cursor_2_tx", 1i64);  // BATTLE STYLE = SHIFT
    ctx.set("cursor_3_tx", 1i64);
}

fn fill_mock_naming(ctx: &mut DataContext) {
    ctx.set("title", "YOUR NAME?");
    ctx.set("name", "ASH");
    ctx.set("cursor_pos", 3i64);
    ctx.set("prompt_text", "Your name?");
    ctx.set("entered_name", "ASH");
    ctx.set("char_cursor_x", 48i64);
    ctx.set("gender_symbol", "♂");
}

fn fill_mock_bag(ctx: &mut DataContext) {
    ctx.set("title", "BAG");
    ctx.set("cursor", 0i64);
    // Quantities are bare numbers; the bag layout's qty column adds the "x" prefix.
    ctx.set("bag_items", DataValue::List(vec![
        DataValue::List(vec!["POKE BALL".into(), "5".into()]),
        DataValue::List(vec!["POTION".into(), "3".into()]),
        DataValue::List(vec!["ANTIDOTE".into(), "2".into()]),
    ]));
}

fn fill_mock_moves(ctx: &mut DataContext) {
    ctx.set("title", "MOVES");
    ctx.set("cursor", 0i64);
    ctx.set("moves", DataValue::List(vec![
        "TACKLE".into(),
        "GROWL".into(),
        "LEECH SEED".into(),
        "VINE WHIP".into(),
    ]));
}

fn fill_mock_party(ctx: &mut DataContext) {
    ctx.set("title", "PARTY");
    ctx.set("cursor", 0i64);
    ctx.set("show_empty", false);
    // Per-entry mock data (party.json references mon{N}_* with per-entry
    // show_entry{N} visibility). Mock a 3-Pokémon party.
    let mons = [
        ("BULBASAUR", 12, "24/24"),
        ("CHARMANDER", 14, "27/27"),
        ("SQUIRTLE", 13, "26/26"),
        ("PIDGEY", 10, "19/19"),
        ("RATTATA", 8, "15/15"),
        ("PIKACHU", 16, "31/31"),
    ];
    for (i, (name, level, hp)) in mons.iter().enumerate() {
        let n = i + 1;
        ctx.set(&format!("mon{n}_name"), *name);
        ctx.set(&format!("mon{n}_level"), *level as i64);
        ctx.set(&format!("mon{n}_status"), "");
        ctx.set(&format!("mon{n}_hp"), *hp);
        // First three slots filled.
        ctx.set(&format!("show_entry{n}"), n <= 3);
    }
}

fn fill_mock_stats(ctx: &mut DataContext) {
    ctx.set("name", "MONSTER A");
    ctx.set("level", 25i64);
    ctx.set("species", "MONSTER");
    ctx.set("hp", 60i64);
    ctx.set("max_hp", 60i64);
    ctx.set("attack", 55i64);
    ctx.set("defense", 40i64);
    ctx.set("speed", 90i64);
    ctx.set("special", 50i64);
    ctx.set("hp_bar_tiles", DataValue::List(vec![
        DataValue::TileId(0x60),
        DataValue::TileId(0x62),
        DataValue::TileId(0x63),
        DataValue::TileId(0x63),
        DataValue::TileId(0x64),
    ]));
    ctx.set("status", "OK");
    ctx.set("ot_name", "ASH");
    ctx.set("dex_num", 1i64);
    ctx.set("type1", "GRASS");
    ctx.set("type2", "POISON");
    ctx.set("trainer_id", 12345i64);
    ctx.set("page1", true);
}

fn fill_mock_mart(ctx: &mut DataContext) {
    ctx.set("greeting", "Welcome! How may I help you?");
    ctx.set("balance", 9999i64);
    ctx.set("cursor", 0i64);
    ctx.set("shop_items", DataValue::List(vec![
        DataValue::List(vec!["POTION".into(), "300".into()]),
        DataValue::List(vec!["ANTIDOTE".into(), "100".into()]),
        DataValue::List(vec!["POKE BALL".into(), "200".into()]),
    ]));
    ctx.set("sell_items", DataValue::List(vec![
        DataValue::List(vec!["POTION".into(), "150".into()]),
        DataValue::List(vec!["ANTIDOTE".into(), "50".into()]),
    ]));
    ctx.set("item_name", "POTION");
    ctx.set("quantity", 1i64);
    ctx.set("unit_price", 300i64);
    ctx.set("confirm_message", "Buy POTION for $300?");
    ctx.set("result_message", "Bought POTION!");
    ctx.set("cursor_y", 0i64);
}

fn fill_mock_pokedex_list(ctx: &mut DataContext) {
    ctx.set("pokedex_entries", DataValue::List(vec![
        DataValue::List(vec!["001".into(), "BULBASAUR".into()]),
        DataValue::List(vec!["002".into(), "IVYSAUR".into()]),
        DataValue::List(vec!["003".into(), "VENUSAUR".into()]),
        DataValue::List(vec!["004".into(), "CHARMANDER".into()]),
        DataValue::List(vec!["025".into(), "PIKACHU".into()]),
    ]));
    ctx.set("seen_count", 25i64);
    ctx.set("owned_count", 10i64);
    ctx.set("selected_name", "PIKACHU");
    ctx.set("selected_num", 25i64);
    ctx.set("sprite_index", 24i64);
    ctx.set("has_selected", true);
}

fn fill_mock_title(ctx: &mut DataContext) {
    ctx.set("title_text", "POKéMON RED");
    ctx.set("copyright", "©1995 NINTENDO");
    ctx.set("show_blink", true);
}
