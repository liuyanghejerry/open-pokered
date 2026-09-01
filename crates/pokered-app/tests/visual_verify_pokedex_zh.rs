//! Visual verification test for the Chinese Pokédex ENTRY view.
//!
//! Drives the list → side menu → DATA → entry state machine, then renders
//! entry screens with `is_zh = true` (Bulbasaur page 1 + page 2, and
//! Caterpie — a >5-char category exercising the label width shift). Saves
//! PNGs to docs/screenshots/ for the PR's before/after comparison.
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_pokedex_zh -- --nocapture

use pokered_app::render::draw_pokedex_screen;
use pokered_core::pokemon::pokedex::Pokedex;
use pokered_core::pokedex_screen::{PokedexScreenInput, PokedexScreenState};
use pokered_data::maps::MapId;
use pokered_data::species::Species;
use pokered_data::wild_data::GameVersion;
use dotzuki_engine::render_config::RenderConfig;
use pokered_renderer::{resource::ResourceManager, FrameBuffer, Rgba};

fn owned(species: Species) -> PokedexScreenState {
    let mut dex = Pokedex::new();
    // Row 1 must be seen so the cursor can open the side menu from the list.
    dex.set_seen(Species::Bulbasaur);
    dex.set_seen(species);
    dex.set_owned(species);
    let mut state = PokedexScreenState::new(dex, GameVersion::Red);
    // Walk the cursor down to the target row (edge-detected per frame).
    for _ in 1..species as u16 {
        state.update_frame(PokedexScreenInput {
            down: true,
            ..Default::default()
        });
    }
    state
}

/// list → side menu → DATA → entry.
fn to_entry(state: &mut PokedexScreenState) {
    use pokered_core::pokedex_screen::{PokedexScreenAction, PokedexScreenMode};
    for _ in 0..2 {
        assert_eq!(
            state.update_frame(PokedexScreenInput {
                a: true,
                ..Default::default()
            }),
            PokedexScreenAction::Active
        );
    }
    assert_eq!(state.mode(), PokedexScreenMode::Entry, "must reach entry");
}

fn save(fb: &FrameBuffer, name: &str) {
    let img = image::RgbaImage::from_fn(fb.width(), fb.height(), |x, y| {
        let c = fb.get_pixel(x, y).unwrap_or(Rgba::WHITE);
        image::Rgba(c.to_array())
    });
    let path = format!("../../docs/screenshots/{name}");
    img.save(&path).expect("save png");
    eprintln!("Saved: {path}");
}

fn render_zh(state: &PokedexScreenState, name: &str) {
    let mut rm = pokered_renderer::resource::AssetRoot::auto_detect()
        .map(ResourceManager::new)
        .ok();
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_pokedex_screen(state, MapId::PalletTown, true, &mut rm, &mut fb);
    save(&fb, name);
}

#[test]
fn render_pokedex_entry_zh_bulbasaur_and_caterpie() {
    // Bulbasaur page 1 (妙蛙种子 / 种子宝可梦).
    let mut state = owned(Species::Bulbasaur);
    to_entry(&mut state);
    render_zh(&state, "pokedex-entry-bulbasaur-zh.png");

    // Page 2 of the flavor text.
    state.update_frame(PokedexScreenInput {
        a: true,
        ..Default::default()
    });
    render_zh(&state, "pokedex-entry-bulbasaur-zh-page2.png");

    // Caterpie: 6-char category 毛毛虫宝可梦 shifts the label left.
    let mut state = owned(Species::Caterpie);
    to_entry(&mut state);
    render_zh(&state, "pokedex-entry-caterpie-zh.png");
}
