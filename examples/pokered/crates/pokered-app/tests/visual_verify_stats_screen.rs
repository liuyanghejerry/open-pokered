//! Visual verification test for the STATS screen (page 1).
//!
//! Renders STATS page 1 for several Pokémon at varying levels and HP,
//! exercising the full pipeline (draw_stats_screen → menus::stats::draw +
//! sprite blit + HP bar overlay). Saves PNGs to the current directory.
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_stats_screen -- --nocapture

use pokered_app::render::draw_stats_screen;
use pokered_core::battle::state::{Pokemon, StatusCondition};
use pokered_core::pokemon::stats::{create_pokemon, create_pokemon_with_moves};
use pokered_core::stats_screen::{StatsPage, StatsScreenState};
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use jrpg_engine::render_config::RenderConfig;
use pokered_renderer::{resource::ResourceManager, FrameBuffer, Rgba};

fn create_resource_manager() -> ResourceManager {
    let root = pokered_renderer::resource::AssetRoot::auto_detect()
        .expect("Cannot auto-detect asset root (gfx/). Run from the workspace project.");
    ResourceManager::new(root)
}

fn save_frame(fb: &FrameBuffer, filename: &str) {
    let img = image::RgbaImage::from_fn(fb.width(), fb.height(), |x, y| {
        let c = fb.get_pixel(x, y).unwrap_or(Rgba::WHITE);
        image::Rgba(c.to_array())
    });
    img.save(filename).expect("Failed to save PNG");
    eprintln!("Saved: {filename}");
}

fn make_mon(species: Species, level: u8, hp_ratio: f32, status: StatusCondition) -> Pokemon {
    let mut mon = create_pokemon(species, level, [0xFF, 0xFF]).unwrap();
    mon.hp = (mon.max_hp as f32 * hp_ratio).round() as u16;
    mon.status = status;
    mon
}

fn render(state: &StatsScreenState, name: &str) {
    let mut rm = create_resource_manager();
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_stats_screen(state, Some(&mut rm), &mut fb, pokered_core::game_state::Lang::En);
    save_frame(&fb, name);
}

#[test]
fn render_stats_page1_bulbasaur_full_hp() {
    let mon = make_mon(Species::Bulbasaur, 5, 1.0, StatusCondition::None);
    let state = StatsScreenState::new(mon);
    render(&state, "stats_page1_bulbasaur_l5.png");
}

#[test]
fn render_stats_page1_charizard_high_level() {
    let mon = make_mon(Species::Charizard, 36, 0.65, StatusCondition::None);
    let state = StatsScreenState::new(mon);
    render(&state, "stats_page1_charizard_l36.png");
}

#[test]
fn render_stats_page1_mewtwo_low_hp() {
    let mon = make_mon(Species::Mewtwo, 70, 0.08, StatusCondition::Poison);
    let state = StatsScreenState::new(mon);
    render(&state, "stats_page1_mewtwo_l70.png");
}

#[test]
fn render_stats_page1_pikachu() {
    let mon = make_mon(Species::Pikachu, 18, 0.5, StatusCondition::Paralysis);
    let state = StatsScreenState::new(mon);
    render(&state, "stats_page1_pikachu_l18.png");
}

fn page2_state(mon: Pokemon) -> StatsScreenState {
    let mut state = StatsScreenState::new(mon);
    state.page = StatsPage::Moves;
    state
}

#[test]
fn render_stats_page2_charizard_full_moveset() {
    let mon = create_pokemon_with_moves(
        Species::Charizard,
        36,
        [0xFF, 0xFF],
        [
            MoveId::Flamethrower,
            MoveId::Slash,
            MoveId::Earthquake,
            MoveId::Fly,
        ],
    )
    .unwrap();
    render(&page2_state(mon), "stats_page2_charizard_l36.png");
}

#[test]
fn render_stats_page2_bulbasaur_partial_moves() {
    let mon = create_pokemon(Species::Bulbasaur, 5, [0xFF, 0xFF]).unwrap();
    render(&page2_state(mon), "stats_page2_bulbasaur_l5.png");
}

#[test]
fn render_stats_page2_mewtwo_high_level() {
    let mon = create_pokemon_with_moves(
        Species::Mewtwo,
        70,
        [0xFF, 0xFF],
        [
            MoveId::PsychicM,
            MoveId::IceBeam,
            MoveId::Thunderbolt,
            MoveId::Recover,
        ],
    )
    .unwrap();
    render(&page2_state(mon), "stats_page2_mewtwo_l70.png");
}
