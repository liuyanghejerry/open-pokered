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

fn render_and_save(screen: &mut OverworldScreen, rm: &mut Option<ResourceManager>, path: &str) -> FrameBuffer {
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
    fb
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

/// Drive the real scene, audio sequencer and renderer together. Jigglypuff
/// changes its displayed ImageIndex during the song, not its logical facing.
/// This is an OverworldScreen/audio/render integration, not a CLI playthrough.
#[test]
fn render_jigglypuff_mid_spin() {
    use pokered_audio::{music_data::MusicId, output::AudioOutput};
    use pokered_core::overworld::screen::OverworldAudioRequest;

    let mut rm = new_rm();
    let mut screen =
        OverworldScreen::new(MapId::PewterPokecenter, Some(maps_dir()), PokemonRedData);
    // Fairy npc 3 at (1,3); stand to its right facing left.
    screen.state.player.x = 2;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Left;

    // The core alone has no music device. Feed its sample from the same real,
    // unmodified sequencer used by App; never force the song-playing boolean.
    let audio = AudioOutput::new_pcm();
    audio.play_music(MusicId::POKECENTER);
    let mut pcm = vec![0.; 1470];
    let mut audible_song = false;
    let mut stop_requests = 0;
    let mut song_requests = 0;
    let mut restore_requests = 0;
    let mut song_start = None;
    let mut captures = Vec::new();
    let mut finished = false;
    for frame in 0..6000u32 {
        audio.update_frame();
        screen.script_sfx_playing = audio.is_sfx_playing();
        screen.jigglypuff_song_playing = audio.last_music_id() == Some(MusicId::JIGGLYPUFF_SONG)
            && (audio.is_music_channel_playing(0) || audio.is_music_channel_playing(1));
        // One ordinary interaction after settling; no ACK is needed to finish
        // this caller's inner DONE or its final DoNotWait text close.
        screen.update_frame(input(frame == 2 || frame == 3));
        for request in screen.audio_requests.drain(..) {
            match request {
                OverworldAudioRequest::PlaySound { sound_id } => {
                    assert_eq!(sound_id, "SFX_STOP_ALL_MUSIC");
                    stop_requests += 1;
                    audio.stop_all();
                }
                OverworldAudioRequest::PlayMusic { music_id } => {
                    assert_eq!(music_id, "MUSIC_JIGGLYPUFF_SONG");
                    song_requests += 1;
                    audio.clear_saved_music_states();
                    audio.play_music(MusicId::JIGGLYPUFF_SONG);
                }
                OverworldAudioRequest::RestoreMapMusic { map } => {
                    assert_eq!(map, MapId::PewterPokecenter);
                    restore_requests += 1;
                    audio.clear_saved_music_states();
                    audio.play_music(MusicId::POKECENTER);
                }
                other => panic!("unexpected Jigglypuff audio request: {other:?}"),
            }
        }

        let effect = screen.active_script_effect_value();
        let phase = effect.as_ref().and_then(|effect| {
            assert_eq!(effect["effect"], "JigglypuffSong");
            effect["state"]["phase"].as_str()
        });
        if phase == Some("SongSpin") {
            let offset = frame - *song_start.get_or_insert(frame);
            assert_eq!(audio.last_music_id(), Some(MusicId::JIGGLYPUFF_SONG));
            assert_eq!(npc_by_text_id(&screen, 3).facing, Direction::Right);
            assert!(screen.retain_field_text);
            assert!(screen.displayed_field_dialogue().is_some());
            audio.render_pcm(&mut pcm);
            audible_song |= pcm.iter().any(|sample| sample.abs() > 1e-7);

            // Full 24-frame rotations, plus two frames for shadow/hardware OAM
            // publication. Both captures must still be inside the actual song.
            if offset == 26 || offset == 50 {
                assert!(audio.is_music_channel_playing(0) || audio.is_music_channel_playing(1));
                assert_eq!(effect.as_ref().unwrap()["state"]["frames_remaining"], 22);
                let slot = screen.npc_states.iter().position(|npc| npc.text_id == 3).unwrap();
                let pose = screen.npc_presentation.pose(MapId::PewterPokecenter, slot).unwrap();
                assert!(pose.visible);
                assert_eq!(pose.image.facing, if offset == 26 { Direction::Down } else { Direction::Left });
                let path = format!("jigglypuff_mid_song_frame{offset}.png");
                let rendered = render_and_save(&mut screen, &mut rm, &path);
                let mut pixels = Vec::new();
                for y in pose.y..pose.y + 16 {
                    for x in pose.x..pose.x + 16 {
                        pixels.push(rendered.get_pixel(x as u32, y as u32).unwrap());
                    }
                }
                assert!(pixels.iter().any(|pixel| *pixel != pixels[0]), "NPC crop must not be blank");
                captures.push((pose.x, pose.y, pixels));
            }
        }
        if phase == Some("PostSongWait") {
            assert!(!audio.is_music_channel_playing(0) && !audio.is_music_channel_playing(1));
            assert!(screen.retain_field_text);
        }
        if restore_requests == 1 && screen.script_engine_idle() && effect.is_none() {
            assert!(screen.displayed_field_dialogue().is_none());
            assert_eq!(audio.last_music_id(), Some(MusicId::POKECENTER));
            finished = true;
            break;
        }
    }
    assert!(finished, "real song must finish and close automatically");
    assert!(audible_song, "unmodified song must emit PCM");
    assert_eq!((stop_requests, song_requests, restore_requests), (1, 1, 1));
    assert_eq!(captures.len(), 2, "both rotating song frames must be rendered");
    assert_eq!((captures[0].0, captures[0].1), (captures[1].0, captures[1].1));
    assert_ne!(captures[0].2, captures[1].2, "displayed NPC pixels must rotate");
}
