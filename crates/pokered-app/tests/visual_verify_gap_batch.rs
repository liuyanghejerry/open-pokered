//! Visual verification for the 2026-09 fidelity-gap batch — the surfaces that
//! change on-screen output:
//!
//! 1. Bench guys (tx_pre hidden text events): A on the Pewter Pokécenter
//!    bench tile now shows the "Yawn! When JIGGLYPUFF sings..." text.
//! 2. Wall TOWN MAP (bookshelf tile table `House` $3D): "A TOWN MAP." text
//!    and the TownMap screen hand-off.
//! 3. FLY arrival bird (EnterMapAnim `.flyAnimation`): the bird sprite
//!    glides in instead of the spin-in.
//! 4. Trainer sight engagement (TalkToTrainer): the BEFORE-battle text now
//!    displays after the "!" walk-up (the fight fires once it closes).
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_gap_batch -- --nocapture
//!
//! PNGs land under docs/screenshots/2026-09-gaps/ (committed PR evidence).

use dotzuki_engine::render_config::RenderConfig;
use pokered_app::render::draw_overworld;
use pokered_core::game_state::Lang;
use pokered_core::overworld::{Direction, OverworldInput, OverworldScreen};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;
use pokered_renderer::{resource::ResourceManager, FrameBuffer, Rgba};

fn maps_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pokered-data/maps")
}

fn out_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/screenshots/2026-09-gaps")
}

fn input(a: bool) -> OverworldInput {
    OverworldInput::new(false, false, false, false, a, false, false, false)
}

fn new_rm() -> Option<ResourceManager> {
    let root = pokered_renderer::resource::AssetRoot::auto_detect()
        .expect("cannot auto-detect gfx/ asset root — run scripts/fetch-gfx.sh first");
    Some(ResourceManager::new(root))
}

fn render_and_save(screen: &mut OverworldScreen, rm: &mut Option<ResourceManager>, path: &std::path::Path) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_overworld(screen, rm, &mut fb, Lang::default());
    let mut img = image::RgbaImage::new(fb.width(), fb.height());
    for y in 0..fb.height() {
        for x in 0..fb.width() {
            if let Some(color) = fb.get_pixel(x, y) {
                let c = color.to_array();
                img.put_pixel(x, y, image::Rgba(c));
            }
        }
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    img.save(path).expect("failed to save PNG");
    eprintln!("saved: {}", path.display());
}

fn render_bench_zh(screen: &mut OverworldScreen, rm: &mut Option<ResourceManager>, path: &std::path::Path) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_overworld(screen, rm, &mut fb, Lang::Zh);
    let mut img = image::RgbaImage::new(fb.width(), fb.height());
    for y in 0..fb.height() {
        for x in 0..fb.width() {
            if let Some(color) = fb.get_pixel(x, y) {
                let c = color.to_array();
                img.put_pixel(x, y, image::Rgba(c));
            }
        }
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    img.save(path).unwrap();
    eprintln!("saved: {}", path.display());
}

fn settle_dialogue(screen: &mut OverworldScreen, press_a_every: u32, frames: u32) -> bool {
    let mut saw_dialogue = false;
    for frame in 0..frames {
        let a = frame % press_a_every == 0;
        screen.update_frame(input(a));
        if screen.pending_dialogue.is_some() {
            saw_dialogue = true;
        }
    }
    saw_dialogue
}

/// 1. Bench guy: stand right of the Pewter Pokécenter bench (0,4) facing
/// LEFT and press A — the "Yawn! When JIGGLYPUFF sings..." text shows.
#[test]
fn render_bench_guy_text() {
    let mut rm = new_rm();
    let mut screen =
        OverworldScreen::new(MapId::PewterPokecenter, Some(maps_dir()), PokemonRedData);
    screen.state.player.x = 1;
    screen.state.player.y = 4;
    screen.state.player.facing = Direction::Left;

    assert!(
        settle_dialogue(&mut screen, 40, 400),
        "bench guy text never appeared"
    );
    render_bench_zh(&mut screen, &mut rm, &out_dir().join("bench-guy-after.png"));
}

/// 2. Wall TOWN MAP in BluesHouse: face up at the bookshelf row (House $3D),
/// press A — "A TOWN MAP." shows and the TownMap hand-off is queued.
#[test]
fn render_wall_town_map_text() {
    let mut rm = new_rm();
    let mut screen = OverworldScreen::new(MapId::BluesHouse, Some(maps_dir()), PokemonRedData);
    // The wall map hangs at (3,0) (House tile $3D).
    screen.state.player.x = 3;
    screen.state.player.y = 1;
    screen.state.player.facing = Direction::Up;

    let mut saw_town_map_pending = false;
    for frame in 0..400 {
        let a = frame % 40 == 0;
        screen.update_frame(input(a));
        if screen.pending_town_map {
            saw_town_map_pending = true;
            break;
        }
    }
    assert!(saw_town_map_pending, "wall town map never triggered");
    // Let the typewriter reveal the text before capturing.
    for _ in 0..40 {
        screen.update_frame(input(false));
    }
    render_and_save(
        &mut screen,
        &mut rm,
        &out_dir().join("wall-town-map-after.png"),
    );
}

/// 3. FLY arrival bird: fly to Pallet Town and render a mid-flight frame —
/// the bird sprite is on screen gliding toward the landing spot, and the
/// player is hidden until it lands.
#[test]
fn render_fly_bird_mid_arrival() {
    let mut rm = new_rm();
    let mut screen = OverworldScreen::new(MapId::Route1, Some(maps_dir()), PokemonRedData);
    screen.fly_warp_to(MapId::PalletTown, 5, 6);

    // Step through the fade-out/warp-commit/fade-in until the bird anim is
    // running, then stop mid-flight for the capture.
    let mut captured = false;
    for _ in 0..400 {
        screen.update_frame(input(false));
        if let Some(fly) = screen.enter_map_fly_anim.as_ref() {
            if fly.frame >= 15 {
                captured = true;
                break;
            }
        }
    }
    assert!(captured, "fly arrival bird never reached mid-flight");
    assert!(screen.enter_map_fly_anim.as_ref().unwrap().is_done() == false);
    render_and_save(
        &mut screen,
        &mut rm,
        &out_dir().join("fly-bird-after.png"),
    );
}

/// 4. Trainer sight engagement: stand on the Viridian Forest Bug Catcher's
/// sight line — after the "!" walk-up the BEFORE-battle text displays (the
/// fight fires once it closes).
#[test]
fn render_trainer_prebattle_text() {
    let mut rm = new_rm();
    let mut screen =
        OverworldScreen::new(MapId::ViridianForest, Some(maps_dir()), PokemonRedData);
    screen.state.player.x = 29;
    screen.state.player.y = 33;

    let mut engaged = false;
    for _ in 0..240 {
        screen.update_frame(input(false));
        if screen.pending_dialogue.is_some() {
            engaged = true;
            break;
        }
    }
    assert!(engaged, "sight trainer's before-battle text never showed");
    // Let the typewriter reveal the text before capturing.
    for _ in 0..40 {
        screen.update_frame(input(false));
    }
    render_and_save(
        &mut screen,
        &mut rm,
        &out_dir().join("trainer-prebattle-text-after.png"),
    );
}

