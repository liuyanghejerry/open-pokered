//! Menu layout configuration for Pokémon Red/Blue.
//!
//! Provides [`MenuConfig`] statics for each of the 18 standard menus,
//! derived from the layout definitions in `ui_layouts/*.json`.

use std::sync::LazyLock;

use dotzuki_engine::menu::{BorderStyle, CursorStyle, MenuConfig};
use dotzuki_engine::render::TileRect;

/// Standard Pokémon menu border using the default Game Boy border tiles
/// (tile indices 192–200).
pub fn pokemon_border_style() -> BorderStyle {
    BorderStyle::default()
}

fn cursor_default() -> CursorStyle {
    CursorStyle::default()
}

// Bag item list — `bag.json` variant "default" child "list".
static BAG_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(0, 3, 20, 14),
    Some(BorderStyle::default()),
    TileRect::new(1, 4, 18, 12),
    cursor_default(),
));

// Battle bag item list — `battle_bag.json` variant "default" child "list".
static BATTLE_BAG_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(4, 10, 16, 6),
    Some(BorderStyle::default()),
    TileRect::new(5, 11, 14, 4),
    cursor_default(),
));

// Battle main action (FIGHT/PKMN/ITEM/RUN) — `battle_main.json` variant "default" child "box_0".
static BATTLE_MAIN_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(8, 12, 12, 6),
    Some(BorderStyle::default()),
    TileRect::new(9, 13, 10, 4),
    cursor_default(),
));

// Battle move selection — `battle_move.json` variant "default" child "box_0".
static BATTLE_MOVE_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(4, 12, 16, 6),
    Some(BorderStyle::default()),
    TileRect::new(5, 13, 14, 4),
    cursor_default(),
));

// Battle party switch — `battle_party.json` variant "default" child "box_0".
static BATTLE_PARTY_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(1, 13, 18, 5),
    Some(BorderStyle::default()),
    TileRect::new(2, 14, 16, 3),
    cursor_default(),
));

// Battle text dialog — `battle_text.json` variant "default" child "box_0".
static BATTLE_TEXT_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(0, 12, 20, 6),
    Some(BorderStyle::default()),
    TileRect::new(1, 13, 18, 4),
    cursor_default(),
));

// Generic dialog box — `dialog.json` variant "default" child "box_0". No border.
static DIALOG_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(0, 12, 20, 6),
    None,
    TileRect::new(0, 12, 20, 6),
    cursor_default(),
));

// Main overworld menu (POKéDEX/POKéMON/ITEM) — `main.json` variant "default" child "menu".
static MAIN_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(0, 0, 13, 16),
    Some(BorderStyle::default()),
    TileRect::new(1, 1, 11, 14),
    cursor_default(),
));

// Poké Mart (BUY/SELL/QUIT) — `mart.json` variant "main_menu" child "menu_box".
static MART_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(0, 0, 7, 8),
    Some(BorderStyle::default()),
    TileRect::new(1, 1, 5, 6),
    cursor_default(),
));

// Naming screen input box — `naming.json` variant "default" child "box_0".
static NAMING_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(0, 5, 20, 9),
    Some(BorderStyle::default()),
    TileRect::new(1, 6, 18, 7),
    cursor_default(),
));

// Oak's speech dialog — `oak_speech.json` variant "text_phase" child "dialog_box".
static OAK_SPEECH_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(0, 12, 20, 6),
    Some(BorderStyle::default()),
    TileRect::new(1, 13, 18, 4),
    cursor_default(),
));

// Options screen first group (TEXT SPEED) — `options.json` variant "default" child "box_0".
static OPTIONS_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(0, 0, 20, 5),
    Some(BorderStyle::default()),
    TileRect::new(1, 1, 18, 3),
    cursor_default(),
));

// Party screen — `party.json` variant "default" child "region_0". Full-screen, no border.
static PARTY_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(0, 0, 20, 18),
    None,
    TileRect::new(0, 0, 20, 18),
    cursor_default(),
));

// Pokédex screen — `pokedex.json` variant "default" child "frame".
static POKEDEX_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(0, 0, 20, 18),
    Some(BorderStyle::default()),
    TileRect::new(1, 1, 18, 16),
    cursor_default(),
));

// Save screen player-info box — `save.json` variant "default" child "box_0".
static SAVE_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(4, 0, 15, 10),
    Some(BorderStyle::default()),
    TileRect::new(5, 1, 13, 8),
    cursor_default(),
));

// Start/title-screen menu (NEW GAME/OPTION) — `start.json` variant "default" child "menu".
static START_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(10, 0, 10, 16),
    Some(BorderStyle::default()),
    TileRect::new(11, 1, 8, 14),
    cursor_default(),
));

// Pokémon stats page 1 left pane — `stats.json` variant "page1" child "box_0".
static STATS_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(0, 8, 10, 10),
    Some(BorderStyle::default()),
    TileRect::new(1, 9, 8, 8),
    cursor_default(),
));

// Yes/No confirmation prompt — `yes_no.json` variant "default" child "box_0". No border.
static YES_NO_CONFIG: LazyLock<MenuConfig> = LazyLock::new(|| MenuConfig::new(
    TileRect::new(11, 8, 9, 6),
    None,
    TileRect::new(11, 8, 9, 6),
    cursor_default(),
));

/// Return the [`MenuConfig`] for a named Pokémon menu, or `None` if unknown.
pub fn pokemon_menu_config(menu_name: &str) -> Option<MenuConfig> {
    match menu_name {
        "bag" => Some(BAG_CONFIG.clone()),
        "battle_bag" => Some(BATTLE_BAG_CONFIG.clone()),
        "battle_main" => Some(BATTLE_MAIN_CONFIG.clone()),
        "battle_move" => Some(BATTLE_MOVE_CONFIG.clone()),
        "battle_party" => Some(BATTLE_PARTY_CONFIG.clone()),
        "battle_text" => Some(BATTLE_TEXT_CONFIG.clone()),
        "dialog" => Some(DIALOG_CONFIG.clone()),
        "main" => Some(MAIN_CONFIG.clone()),
        "mart" => Some(MART_CONFIG.clone()),
        "naming" => Some(NAMING_CONFIG.clone()),
        "oak_speech" => Some(OAK_SPEECH_CONFIG.clone()),
        "options" => Some(OPTIONS_CONFIG.clone()),
        "party" => Some(PARTY_CONFIG.clone()),
        "pokedex" => Some(POKEDEX_CONFIG.clone()),
        "save" => Some(SAVE_CONFIG.clone()),
        "start" => Some(START_CONFIG.clone()),
        "stats" => Some(STATS_CONFIG.clone()),
        "yes_no" => Some(YES_NO_CONFIG.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_18_menus_are_found() {
        for name in [
            "bag", "battle_bag", "battle_main", "battle_move", "battle_party",
            "battle_text", "dialog", "main", "mart", "naming", "oak_speech",
            "options", "party", "pokedex", "save", "start", "stats", "yes_no",
        ] {
            assert!(pokemon_menu_config(name).is_some(), "missing config for '{name}'");
        }
    }

    #[test]
    fn unknown_menu_returns_none() {
        assert!(pokemon_menu_config("nonexistent").is_none());
    }

    #[test]
    fn bordered_menus_have_inset_content() {
        let cfg = pokemon_menu_config("bag").unwrap();
        assert!(cfg.border.is_some());
        assert_eq!(cfg.content.tx, cfg.area.tx + 1);
        assert_eq!(cfg.content.ty, cfg.area.ty + 1);
        assert_eq!(cfg.content.tw, cfg.area.tw - 2);
        assert_eq!(cfg.content.th, cfg.area.th - 2);
    }

    #[test]
    fn borderless_menus_have_matching_area_and_content() {
        for name in ["dialog", "party", "yes_no"] {
            let cfg = pokemon_menu_config(name).unwrap();
            assert!(cfg.border.is_none(), "{name} should be borderless");
            assert_eq!(cfg.area, cfg.content, "{name}: content should match area");
        }
    }
}
