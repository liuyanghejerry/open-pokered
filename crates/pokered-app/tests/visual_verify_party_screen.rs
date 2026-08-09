//! Visual verification test for the party screen.
//!
//! Creates a PartyScreenState with various Pokemon and renders using the
//! actual game rendering pipeline (draw_party_screen). Saves output as PNG.
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_party_screen -- --nocapture

use pokered_app::render::draw_party_screen;
use pokered_core::battle::state::{Pokemon, StatusCondition};
use pokered_core::party_screen::PartyScreenState;
use pokered_core::pokemon::stats::create_pokemon;
use pokered_data::species::Species;
use jrpg_engine::render_config::RenderConfig;
use pokered_renderer::{FrameBuffer, Rgba};

fn save_frame(fb: &FrameBuffer, filename: &str) {
    let img = image::RgbaImage::from_fn(fb.width(), fb.height(), |x, y| {
        let c = fb.get_pixel(x, y).unwrap_or(pokered_renderer::Rgba::WHITE);
        image::Rgba(c.to_array())
    });
    img.save(filename).expect("Failed to save PNG");
    eprintln!("Saved: {filename}");
}

/// Create a Pokemon with custom HP and status
fn create_custom_pokemon(species: Species, level: u8, hp_ratio: f32, status: StatusCondition) -> Pokemon {
    let mut mon = create_pokemon(species, level, [0xFF, 0xFF]).unwrap();
    mon.hp = (mon.max_hp as f32 * hp_ratio) as u16;
    mon.status = status;
    mon
}

#[test]
fn render_party_screen_various_states() {
    // Create a party with various Pokemon showing different states
    let party = vec![
        // Full HP, no status
        create_custom_pokemon(Species::Charizard, 36, 1.0, StatusCondition::None),
        // High HP, poisoned
        create_custom_pokemon(Species::Blastoise, 42, 0.85, StatusCondition::Poison),
        // Medium HP, burned
        create_custom_pokemon(Species::Venusaur, 40, 0.6, StatusCondition::Burn),
        // Low HP, paralyzed
        create_custom_pokemon(Species::Pikachu, 18, 0.3, StatusCondition::Paralysis),
        // Very low HP, frozen
        create_custom_pokemon(Species::Mewtwo, 70, 0.1, StatusCondition::Freeze),
        // Fainted (0 HP), asleep
        create_custom_pokemon(Species::Snorlax, 50, 0.0, StatusCondition::Sleep(3)),
    ];

    // Test with different cursor positions
    for cursor_pos in 0..party.len() {
        let state = PartyScreenState::new(party.clone());
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);

        // Manually set cursor position by simulating input
        let mut state = state;
        for _ in 0..cursor_pos {
            state.update_frame(pokered_core::party_screen::PartyScreenInput {
                down: true,
                up: false,
                a: false,
                b: false,
            });
        }

        draw_party_screen(&state, None, 0, &mut fb, pokered_core::game_state::Lang::En);
        save_frame(&fb, &format!("party_screen_cursor_{}.png", cursor_pos));
    }
}

#[test]
fn render_party_screen_single_pokemon() {
    let party = vec![
        create_custom_pokemon(Species::Bulbasaur, 5, 1.0, StatusCondition::None),
    ];

    let state = PartyScreenState::new(party);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_party_screen(&state, None, 0, &mut fb, pokered_core::game_state::Lang::En);
    save_frame(&fb, "party_screen_single.png");
}

#[test]
fn render_party_screen_empty() {
    let state = PartyScreenState::new(vec![]);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_party_screen(&state, None, 0, &mut fb, pokered_core::game_state::Lang::En);
    save_frame(&fb, "party_screen_empty.png");
}

#[test]
fn render_party_screen_full_hp() {
    let party = vec![
        create_custom_pokemon(Species::Charmander, 5, 1.0, StatusCondition::None),
        create_custom_pokemon(Species::Squirtle, 5, 1.0, StatusCondition::None),
        create_custom_pokemon(Species::Bulbasaur, 5, 1.0, StatusCondition::None),
    ];

    let state = PartyScreenState::new(party);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_party_screen(&state, None, 0, &mut fb, pokered_core::game_state::Lang::En);
    save_frame(&fb, "party_screen_full_hp.png");
}

#[test]
fn render_party_screen_low_hp() {
    let party = vec![
        create_custom_pokemon(Species::Charmander, 5, 0.1, StatusCondition::None),
        create_custom_pokemon(Species::Squirtle, 5, 0.05, StatusCondition::Poison),
        create_custom_pokemon(Species::Bulbasaur, 5, 0.0, StatusCondition::None),
    ];

    let state = PartyScreenState::new(party);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_party_screen(&state, None, 0, &mut fb, pokered_core::game_state::Lang::En);
    save_frame(&fb, "party_screen_low_hp.png");
}
