//! Reproduction test: after OAK's PARCEL is delivered (EVENT_OAK_GOT_PARCEL),
//! talking to the Viridian Mart clerk must open the shop — not repeat the
//! pre-delivery "Say hi to PROF.OAK" line.

use pokered_core::overworld::{Direction, OverworldInput, OverworldScreen};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn none() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn press_a() -> OverworldInput {
    OverworldInput::new(false, false, false, false, true, false, false, false)
}

/// Talk to the clerk (npc 1 at (0,5), across the counter) from `from`,
/// mashing A to get through dialogue; return whether the shop opened.
fn talk_to_clerk(x: u16, y: u16, facing: Direction) -> (bool, Vec<String>) {
    let mut screen = OverworldScreen::new(MapId::ViridianMart, None, PokemonRedData);
    screen.set_flag_live("EVENT_GOT_OAKS_PARCEL", true);
    screen.set_flag_live("EVENT_OAK_GOT_PARCEL", true);
    screen.run_on_load();
    screen.state.player.x = x;
    screen.state.player.y = y;
    screen.state.player.facing = facing;

    let mut dialogues: Vec<String> = Vec::new();
    let mut shop_opened = false;
    for frame in 0..600 {
        let input = if frame % 10 == 0 { press_a() } else { none() };
        screen.update_frame(input);
        if let Some(dlg) = &screen.pending_dialogue {
            let text = dlg
                .pages()
                .iter()
                .flat_map(|p| [p.line1, p.line2])
                .collect::<Vec<_>>()
                .join(" ");
            if dialogues.last().map(|s| s.as_str()) != Some(text.as_str()) {
                dialogues.push(text);
            }
        }
        if screen.pending_shop.is_some() {
            shop_opened = true;
            break;
        }
    }
    (shop_opened, dialogues)
}

#[test]
fn clerk_opens_shop_after_parcel_delivered() {
    // The cutscene walks the player to (3,5) facing left; the counter tile
    // sits between the player and the clerk at (0,5).
    let (opened, dialogues) = talk_to_clerk(2, 5, Direction::Left);
    assert!(
        opened,
        "shop must open after parcel delivery; dialogues seen: {dialogues:?}"
    );
}

/// The exact item list ViridianMart's scene passes to `openShop` must build
/// a real shop stock. Regression: `MartStock::from_strings` parses via
/// strum `EnumString` (exact PascalCase variant names), so SCREAMING_SNAKE
/// asm const names failed and the app logged "unknown item id", skipping
/// the shop — the mart never opened even after the parcel was delivered.
#[test]
fn viridian_mart_stock_resolves_from_script_names() {
    let items = vec![
        "POKE_BALL".to_string(),
        "ANTIDOTE".to_string(),
        "PARLYZ_HEAL".to_string(),
        "BURN_HEAL".to_string(),
    ];
    let stock = pokered_core::items::shop_stock_from_script_names(&items)
        .expect("ViridianMart stock must resolve");
    assert_eq!(stock.len(), 4);
    assert_eq!(stock.get(0), Some(pokered_data::items::ItemId::PokeBall));
}
