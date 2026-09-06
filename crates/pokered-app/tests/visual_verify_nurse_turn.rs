//! Visual verification for the Pokécenter nurse turn during the heal.
//!
//! Drives the REAL PewterPokecenter `talkNurse` scene (on-disk `.scene`
//! files) with A taps, answers YES, waits for the healing-machine overlay,
//! then renders the mid-heal frame with the actual game renderer. The nurse
//! must be facing LEFT (toward the machine) in the captured frame — the
//! original pokes the image index $18, which decodes ($18 & $f = $8) to
//! StandingLeft in SpriteFacingAndAnimationTable. She used to keep facing
//! the player because `faceNpc("1", ...)` resolved its numeric id against
//! the 0-based object index (hers is 0) and turned the second NPC instead.
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_nurse_turn -- --nocapture
//!
//! The PNG lands in docs/screenshots/nurse_heal_during_machine.png.

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

fn input(a: bool) -> OverworldInput {
    OverworldInput::new(false, false, false, false, a, false, false, false)
}

#[test]
fn render_nurse_facing_machine_during_heal() {
    // Cwd-relative like the other visual_verify tests (cargo test runs from
    // crates/pokered-app); committed evidence lives in docs/screenshots/.
    let out_path = std::path::PathBuf::from("nurse_heal_during_machine.png");

    let root = pokered_renderer::resource::AssetRoot::auto_detect()
        .expect("cannot auto-detect gfx/ asset root — run scripts/fetch-gfx.sh first");
    let mut rm = Some(ResourceManager::new(root));

    let mut screen =
        OverworldScreen::new(MapId::PewterPokecenter, Some(maps_dir()), PokemonRedData);
    screen.state.player.x = 3;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Up;
    // Machine ball count comes straight from party_count; 3 keeps the
    // capture short while still showing balls on the machine.
    screen.party_count = 3;

    let mut captured = false;
    let mut nurse_facing = None;
    for frame in 0..6000 {
        // A rising edge every 40 frames: advance dialogue, confirm YES.
        let a = frame % 40 == 0;
        screen.update_frame(input(a));

        if let Some(machine) = &screen.pending_healing_machine {
            if machine.pokeballs_visible == 3 && machine.frames_remaining < 30 {
                nurse_facing = Some(screen.npc_states[0].facing);
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                draw_overworld(&mut screen, &mut rm, &mut fb, Lang::default());
                save_frame(&fb, &out_path);
                captured = true;
                break;
            }
        }
    }

    assert!(
        captured,
        "never reached the mid-heal machine frame (3 balls out)"
    );
    assert_eq!(
        nurse_facing,
        Some(Direction::Left),
        "nurse must face the machine (left) while it runs"
    );
}

fn save_frame(fb: &FrameBuffer, path: &std::path::Path) {
    let mut img = image::RgbaImage::new(fb.width(), fb.height());
    for y in 0..fb.height() {
        for x in 0..fb.width() {
            if let Some(color) = fb.get_pixel(x, y) {
                let c = color.to_array();
                img.put_pixel(x, y, image::Rgba(c));
            }
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).expect("create screenshots dir");
    }
    img.save(path).expect("failed to save PNG");
    eprintln!("saved: {}", path.display());
}
