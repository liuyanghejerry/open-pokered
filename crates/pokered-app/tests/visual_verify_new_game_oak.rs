//! Visual + logic regression: on a fresh NEW GAME, Professor Oak must NOT be
//! standing in Pallet Town. In the original he starts hidden
//! (InitializeToggleableObjectsFlags sets TOGGLE_PALLET_TOWN_OAK OFF) until
//! the north-exit interception shows him for the escort cutscene.
//!
//! The menu-driven NEW GAME flow used to build the overworld without seeding
//! the freshly-reset save's `toggleable_object_flags` into the screen, so the
//! all-zero constructor default made `apply_hidden_object_flags` pass 3
//! actively re-show Oak on the first warp into Pallet Town.
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_new_game_oak -- --nocapture
//! The PNG lands in the cargo cwd (crates/pokered-app); committed evidence
//! lives under docs/screenshots/.

use dotzuki_app::InputState;
use dotzuki_engine::render_config::RenderConfig;
use pokered_app::render::draw_overworld;
use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::{GameScreen, Lang, MainMenuChoice};
use pokered_core::overworld::{Direction, PendingWarp, WarpFadeState};
use pokered_core::save::SaveData;
use pokered_data::toggleable_objects::{is_object_hidden, toggle_id_to_bit_index};
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
use pokered_renderer::{resource::ResourceManager, FrameBuffer, Rgba};

/// Drive the real menu path: MainMenu → OakSpeech (resets the in-memory save)
/// → Overworld (builds the new-game overworld at RedsHouse2F).
fn new_game_overworld() -> PokemonGame {
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red,
        None,
        None,
        None,
        false,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    game.save_data = SaveData::new();
    game.main_menu.last_choice = Some(MainMenuChoice::NewGame);
    game.state.screen = GameScreen::MainMenu;
    game.handle_transition(GameScreen::OakSpeech);
    game.handle_transition(GameScreen::Overworld);
    game
}

/// Walk out the front door: the real warp path into Pallet Town (the door mat
/// exit lands on the walk-out tile below the door at (5,5)), including the
/// warp-time `apply_hidden_object_flags` pass that re-showed Oak.
fn leave_house(game: &mut PokemonGame) {
    game.overworld.pending_warp = Some(PendingWarp {
        dest_map: MapId::PalletTown,
        dest_x: 5,
        dest_y: 5,
        save_last_map: false,
        arrival_spin: false,
    });
    game.overworld.warp_fade_state = WarpFadeState::FadingOut { frames_remaining: 1 };
    let input = InputState::new();
    for _ in 0..400 {
        game.update(&input);
        if game.overworld.pending_warp.is_none()
            && game.overworld.warp_fade_state == WarpFadeState::Idle
        {
            break;
        }
    }
    assert_eq!(game.overworld.state.current_map, MapId::PalletTown);
    game.overworld.state.player.facing = Direction::Down;
    for _ in 0..30 {
        game.update(&input);
    }
}

#[test]
fn new_game_seeds_toggleable_object_flags() {
    let game = new_game_overworld();
    let bit = toggle_id_to_bit_index("PALLET_TOWN_OBJ_1").unwrap();
    assert!(
        is_object_hidden(game.overworld.toggleable_object_flags(), bit),
        "NEW GAME must seed the fresh save's toggleable object flags into the \
         overworld (PALLET_TOWN Oak starts hidden)"
    );
}

#[test]
fn oak_hidden_in_pallet_town_after_leaving_house() {
    let mut game = new_game_overworld();
    leave_house(&mut game);
    let oak_visible = game
        .overworld
        .npc_states
        .iter()
        .find(|n| n.home_x == 8 && n.home_y == 5)
        .expect("Oak NPC missing on Pallet Town")
        .visible;

    let mut rm = Some(ResourceManager::new(
        pokered_renderer::resource::AssetRoot::auto_detect()
            .expect("cannot auto-detect gfx/ asset root — run scripts/fetch-gfx.sh first"),
    ));
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_overworld(&mut game.overworld, &mut rm, &mut fb, Lang::En);
    let mut img = image::RgbaImage::new(fb.width(), fb.height());
    for y in 0..fb.height() {
        for x in 0..fb.width() {
            if let Some(c) = fb.get_pixel(x, y) {
                img.put_pixel(x, y, image::Rgba(c.to_array()));
            }
        }
    }
    img.save("pallet_oak_new_game.png").expect("failed to save PNG");
    eprintln!("saved: pallet_oak_new_game.png");

    assert!(
        !oak_visible,
        "Oak must be hidden in Pallet Town on a fresh NEW GAME (he appears \
         only for the north-exit interception)"
    );
}
