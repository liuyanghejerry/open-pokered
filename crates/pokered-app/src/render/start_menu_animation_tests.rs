//! START leaves hTileAnimations enabled; party selection explicitly clears it.
use super::*;
use dotzuki_app::{GbButton, InputState};
use dotzuki_engine::render_config::RenderConfig;
use pokered_renderer::Rgba;

#[test]
fn start_menu_animates_water_without_moving_actors_and_keeps_retained_render_equal() {
    let mut game = PokemonGame::new(GameVersion::Red);
    game.audio = None;
    game.state.config.language = Lang::En;
    game.overworld = pokered_core::overworld::OverworldScreen::new(
        MapId::ViridianCity, None, pokered_data::impl_traits::PokemonRedData);
    game.overworld.state.player.x = 14;
    game.overworld.state.player.y = 28;
    assert_eq!(game.overworld.tile_anim.kind(),
        pokered_core::overworld::presentation::TileAnimKind::WaterFlower);
    game.handle_transition(GameScreen::StartMenu);
    let position = (game.overworld.state.player.x, game.overworld.state.player.y);
    let actors: Vec<_> = game.overworld.npc_states.iter().map(|n| (n.x, n.y, n.walk_counter, n.delay_counter)).collect();
    let field_frame = game.overworld.frame_counter;
    let mut session = RenderSession::new();
    let mut retained = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| {};
    let mut water_images = Vec::new();
    for frame in 0..126 {
        let mut input = InputState::new();
        // Exercise both stationary and moving menu cursors while water changes.
        if frame % 30 == 0 { input.press(GbButton::Down); }
        game.update(&input);
        assert_eq!(game.state.screen, GameScreen::StartMenu);
        assert_eq!((game.overworld.state.player.x, game.overworld.state.player.y), position);
        assert_eq!(game.overworld.frame_counter, field_frame, "menu must not run field scripts");
        assert_eq!(game.overworld.npc_states.iter().map(|n| (n.x, n.y, n.walk_counter, n.delay_counter)).collect::<Vec<_>>(), actors);
        session.render(&mut game, &mut retained, &mut scroll);
        let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        game.draw(&mut full);
        assert_eq!(retained.packed(), full.packed(), "retained menu frame {frame}");
        let mut water = Vec::new();
        for y in 0..60 { for x in 0..32 { water.push(full.get_pixel(x, y)); } }
        if !water_images.contains(&water) { water_images.push(water); }
    }
    assert!(water_images.len() > 1, "visible water must keep animating under START");

    // Selecting POKEMON pauses tile animation in the original's
    // DisplayPartyMenu / HandlePartyMenuInput; it must not inherit this tick.
    game.save_data.party.add(pokered_core::pokemon::stats::create_pokemon(
        pokered_data::species::Species::Bulbasaur, 12, [0x9a, 0x78]).unwrap()).unwrap();
    game.handle_transition(GameScreen::PartyScreen);
    let before = serde_json::to_value(&game.overworld.tile_anim).unwrap();
    for _ in 0..84 { game.update(&InputState::new()); }
    assert_eq!(serde_json::to_value(&game.overworld.tile_anim).unwrap(), before);
}
