//! Visual verification for the story-script fixes that change on-screen
//! output:
//!
//! 1. MtMoon B2F fossils: taking a fossil must hide BOTH fossil sprites
//!    (hideObjectByName bindings were missing, so the item balls stayed on
//!    the floor forever).
//! 2. ViridianGym Giovanni: after the post-battle farewell he must leave the
//!    gym (hideObjectByName("VIRIDIAN_GYM_GIOVANNI") was a silent no-op).
//! 3. PewterPokecenter Jigglypuff: while MUSIC_JIGGLYPUFF_SONG plays, the
//!    sprite spins DOWN -> LEFT -> UP -> RIGHT every 24 frames (orig
//!    spinMovementLoop); it used to stand frozen facing the player.
//!
//! Each test drives the real on-disk `.scene` with A taps and renders the
//! relevant frame via the actual game renderer. Run with:
//!   cargo test -p pokered-app --test visual_verify_story_fixes -- --nocapture
//!
//! PNGs land in the cargo cwd (crates/pokered-app); committed evidence lives
//! under docs/screenshots/.

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

fn new_rm() -> Option<ResourceManager> {
    let root = pokered_renderer::resource::AssetRoot::auto_detect()
        .expect("cannot auto-detect gfx/ asset root — run scripts/fetch-gfx.sh first");
    Some(ResourceManager::new(root))
}

fn render_and_save(screen: &mut OverworldScreen, rm: &mut Option<ResourceManager>, path: &str) {
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
    img.save(path).expect("failed to save PNG");
    eprintln!("saved: {path}");
}

fn npc_by_text_id(screen: &OverworldScreen, text_id: u8) -> &pokered_core::overworld::npc_movement::NpcRuntimeState {
    screen
        .npc_states
        .iter()
        .find(|n| n.text_id == text_id)
        .expect("NPC with text id missing on this map")
}

/// Talk to the Dome Fossil (npc 6), answer YES, and render the room after the
/// storyline: both fossil sprites must be gone from the floor.
#[test]
fn render_fossils_hidden_after_pickup() {
    let mut rm = new_rm();
    let mut screen =
        OverworldScreen::new(MapId::MtMoonB2F, Some(maps_dir()), PokemonRedData);
    // Dome fossil npc 6 at (12,6); stand right below it facing up.
    screen.state.player.x = 12;
    screen.state.player.y = 7;
    screen.state.player.facing = Direction::Up;

    let mut dialogue_started = false;
    let mut settled = 0;
    for frame in 0..6000 {
        let a = frame % 40 == 0;
        screen.update_frame(input(a));
        if screen.pending_dialogue.is_some() || screen.pending_choice.is_some() {
            dialogue_started = true;
        }
        // Wait for the full storyline (both hide calls) to finish: once the
        // boxes are gone, let any trailing effects run their course.
        if dialogue_started && screen.pending_dialogue.is_none() && screen.pending_choice.is_none()
        {
            settled += 1;
            if settled > 200 {
                break;
            }
        }
    }
    assert!(dialogue_started, "fossil talk never started");

    render_and_save(&mut screen, &mut rm, "fossil_floor_after_pickup.png");
    assert!(
        !npc_by_text_id(&screen, 6).visible && !npc_by_text_id(&screen, 7).visible,
        "both fossil sprites must be hidden after taking the Dome Fossil"
    );
}

/// With the gym-beaten flags preset, talk to Giovanni and render the gym
/// after his farewell: he must be gone (hidden), not standing at (2,1).
#[test]
fn render_giovanni_hidden_after_farewell() {
    let mut rm = new_rm();
    let mut screen =
        OverworldScreen::new(MapId::ViridianGym, Some(maps_dir()), PokemonRedData);
    screen.set_flag_live("EVENT_BEAT_VIRIDIAN_GYM_GIOVANNI", true);
    screen.set_flag_live("EVENT_GOT_TM27", true);
    screen.state.player.x = 2;
    screen.state.player.y = 2;
    screen.state.player.facing = Direction::Up;

    let mut dialogue_started = false;
    let mut settled = 0;
    for frame in 0..6000 {
        let a = frame % 40 == 0;
        screen.update_frame(input(a));
        if screen.pending_dialogue.is_some() || screen.pending_choice.is_some() {
            dialogue_started = true;
        }
        // Farewell text, then let the trailing hide effect land.
        if dialogue_started && screen.pending_dialogue.is_none() && screen.pending_choice.is_none()
        {
            settled += 1;
            if settled > 200 {
                break;
            }
        }
    }
    assert!(dialogue_started, "Giovanni farewell never started");

    render_and_save(&mut screen, &mut rm, "viridian_gym_after_farewell.png");
    assert!(
        !npc_by_text_id(&screen, 1).visible,
        "Giovanni must be hidden after his farewell"
    );
}

/// While MUSIC_JIGGLYPUFF_SONG plays, the Jigglypuff must be mid-spin. Both
/// runs step the exact same frame count after the intro box closes, so the
/// before/after captures are phase-identical.
#[test]
fn render_jigglypuff_mid_spin() {
    let mut rm = new_rm();
    let mut screen =
        OverworldScreen::new(MapId::PewterPokecenter, Some(maps_dir()), PokemonRedData);
    // Fairy npc 3 at (1,3); stand to its right facing left.
    screen.state.player.x = 2;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Left;

    let mut dialogue_opened = false;
    let mut closed_frame: Option<u32> = None;
    let mut facing_at = std::collections::HashMap::new();
    for frame in 0..6000u32 {
        let a = frame % 40 == 0;
        screen.update_frame(input(a));

        if screen.pending_dialogue.is_some() {
            dialogue_opened = true;
        } else if dialogue_opened && closed_frame.is_none() {
            closed_frame = Some(frame);
        }
        if let Some(closed) = closed_frame {
            let offset = frame - closed;
            if offset == 100 || offset == 160 {
                facing_at.insert(offset, npc_by_text_id(&screen, 3).facing);
                let path = format!("jigglypuff_mid_song_frame{offset}.png");
                render_and_save(&mut screen, &mut rm, &path);
            }
            if offset == 160 {
                break;
            }
        }
    }
    assert!(closed_frame.is_some(), "intro dialogue never closed");

    // Mid-song the spin must have progressed between the two captures. On a
    // broken build the sprite stays frozen facing the player at both.
    assert_ne!(
        facing_at[&100], facing_at[&160],
        "Jigglypuff must be mid-spin (facing changes) during its song, got {:?} at both captures",
        facing_at[&100]
    );
}
