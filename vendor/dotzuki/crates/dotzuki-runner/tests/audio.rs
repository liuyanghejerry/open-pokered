//! Integration tests for `dotzuki run` audio playback. CI has no audio device,
//! so these cover everything testable without one: library loading from a
//! fixture `data/audio/` tree, unknown-id handling, silent-mode no-panics
//! (missing dir / device disallowed), and scene audio commands resolving
//! through the pump with the scene VM continuing.
//!
//! Track JSON uses the dotzuki-audio `TrackDef` format (see
//! `dotzuki_audio::format`); ids are the `id` fields scenes pass to
//! `playMusic`/`playSound`.

use crate::alloc_prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use dotzuki_renderer::input::{GbButton, InputState};
use dotzuki_runner::{LoadedProject, RunnerAudio, RunnerGame, RunnerOptions};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

/// Unique temp directory, removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(test: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "dotzuki-runner-audio-{test}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TestDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}

/// A minimal valid music track (dotzuki-audio `TrackDef` JSON).
const THEME_JSON: &str = r#"{
  "id": "theme",
  "kind": "music",
  "tempo": 256,
  "channels": [
    {
      "hw": "pulse1",
      "commands": [
        { "type": "note_type", "speed": 12, "param": 197 },
        { "type": "octave", "value": 5 },
        { "type": "note", "pitch": 0, "length": 4 },
        { "type": "rest", "length": 4 },
        { "type": "sound_ret" }
      ]
    }
  ]
}"#;

/// A minimal valid SFX track.
const BEEP_JSON: &str = r#"{
  "id": "beep",
  "kind": "sfx",
  "channels": [
    {
      "hw": "pulse1",
      "commands": [
        { "type": "duty_cycle", "value": 2 },
        { "type": "sfx_square_note", "length": 4, "volume_envelope": 145, "frequency": 1984 },
        { "type": "sound_ret" }
      ]
    }
  ]
}"#;

/// Write the two fixture tracks under `<root>/data/audio/{music,sfx}/`.
fn write_audio_dir(root: &Path) {
    let music = root.join("data/audio/music");
    let sfx = root.join("data/audio/sfx");
    fs::create_dir_all(&music).unwrap();
    fs::create_dir_all(&sfx).unwrap();
    fs::write(music.join("theme.json"), THEME_JSON).unwrap();
    fs::write(sfx.join("beep.json"), BEEP_JSON).unwrap();
}

/// Write a 64×16 `tileset.png` — mirrors fixture.rs.
fn write_tileset(map_dir: &Path) {
    let tile = 16u32;
    let mut img = image::RgbaImage::new(tile * 4, tile);
    for i in 0..4 {
        for y in 0..tile {
            for x in 0..tile {
                img.put_pixel(i * tile + x, y, image::Rgba([0xFF, 0xFF, 0xFF, 0xFF]));
            }
        }
    }
    img.save(map_dir.join("tileset.png")).unwrap();
}

/// Copy the committed fixture into a temp dir and generate its tilesets.
fn demo_project(test: &str) -> (TestDir, PathBuf) {
    let tmp = TestDir::new(test);
    let root = tmp.path().join("demo");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo");
    copy_dir(&fixture, &root);
    write_tileset(&root.join("data/maps/Town"));
    write_tileset(&root.join("data/maps/Cave"));
    (tmp, root)
}

// ── library loading ─────────────────────────────────────────────────────────

#[test]
fn loads_tracks_recursively_from_data_audio() {
    let (_tmp, root) = demo_project("load");
    write_audio_dir(&root);

    let audio = RunnerAudio::new(&root.join("data"), false);
    assert_eq!(audio.track_count(), 2);
    assert!(audio.has_track("theme"));
    assert!(audio.has_track("beep"));
    assert!(!audio.has_track("nope"));
    assert!(!audio.has_output(), "no play command ⇒ no device init");
}

#[test]
fn missing_audio_dir_is_empty_and_silent() {
    let (_tmp, root) = demo_project("nodir");
    let mut audio = RunnerAudio::new(&root.join("data"), true);
    assert_eq!(audio.track_count(), 0);

    // Every command is a safe no-op; no device is ever initialised.
    audio.play_music("theme");
    audio.play_sound("beep");
    audio.stop_music();
    audio.fade_out_music();
    audio.update_frame();
    assert!(!audio.has_output());
}

#[test]
fn unknown_ids_are_noops_without_a_device() {
    let (_tmp, root) = demo_project("unknown");
    write_audio_dir(&root);
    let mut audio = RunnerAudio::new(&root.join("data"), false);

    // Unknown ids warn (once) and continue; repeated calls stay quiet no-ops.
    audio.play_music("nope");
    audio.play_music("nope");
    audio.play_sound("also-nope");
    assert!(
        !audio.has_output(),
        "unknown ids must not trigger device init"
    );
}

#[test]
fn device_disallowed_stays_silent_with_known_tracks() {
    let (_tmp, root) = demo_project("headless");
    write_audio_dir(&root);
    // allow_device = false — the headless contract: never touch cpal.
    let mut audio = RunnerAudio::new(&root.join("data"), false);

    audio.play_music("theme");
    audio.play_sound("beep");
    for _ in 0..90 {
        audio.update_frame(); // covers fade stepping too
    }
    audio.fade_out_music();
    audio.stop_music();
    assert!(!audio.has_output(), "headless must never open a device");
}

// ── scene audio commands resolve through the pump ───────────────────────────

/// Update the game one frame with `mask` held.
fn frame(game: &mut RunnerGame, mask: u8) {
    let mut input = InputState::new();
    input.set_from_bitmask(mask);
    game.update(&input);
}

fn idle(game: &mut RunnerGame, n: u32) {
    for _ in 0..n {
        frame(game, 0);
    }
}

fn press_a(game: &mut RunnerGame) {
    frame(game, GbButton::A.bit_mask());
    idle(game, 1);
}

fn dismiss_dialogue(game: &mut RunnerGame) {
    for _ in 0..20 {
        if game.dialogue_text().is_none() && game.choice_options().is_none() {
            return;
        }
        press_a(game);
    }
    panic!("dialogue did not close after 20 A presses");
}

#[test]
fn play_commands_resolve_and_the_scene_continues() {
    let (_tmp, root) = demo_project("pump");
    write_audio_dir(&root);

    // A storyline that plays music + sfx (one unknown id), then shows text —
    // the text proving the VM was not derailed by the audio commands.
    fs::write(
        root.join("data/maps/Cave/script.scene"),
        r#"game_scene Cave {
    @storyline("cave_enter") {
        @trigger(map = "Cave", on_enter = true)
        @command("playMusic", "theme")
        @command("playSound", "beep")
        @command("playMusic", "no-such-track")
        @command("fadeOutMusic")
        @command("stopMusic")
        @speaker("") {
            "A cold wind blows through the cave."
        }
    }
}
"#,
    )
    .unwrap();

    let project = LoadedProject::load(&root).expect("load demo project");
    let mut game = RunnerGame::new(
        project,
        RunnerOptions {
            map: Some("Cave".to_string()),
            headless: true, // never open a device
            ..RunnerOptions::default()
        },
    )
    .expect("boot game");
    assert_eq!(game.audio().track_count(), 2);

    // The on_enter storyline fires on boot: audio commands resolve silently…
    let page = game
        .dialogue_text()
        .expect("scene continues past audio commands");
    assert!(page.contains("cold wind"), "page: {page:?}");
    dismiss_dialogue(&mut game);
    idle(&mut game, 30);

    // …and no device was opened despite the library being non-empty.
    assert!(!game.audio().has_output());
}

// ── PCM pull-render mode (WASM shell) ───────────────────────────────────────

#[test]
fn pcm_mode_renders_audio_headless() {
    let (_tmp, root) = demo_project("pcm");
    write_audio_dir(&root);

    // Play the theme on map enter, like any scene would.
    fs::write(
        root.join("data/maps/Cave/script.scene"),
        r#"game_scene Cave {
    @storyline("cave_enter") {
        @trigger(map = "Cave", on_enter = true)
        @command("playMusic", "theme")
    }
}
"#,
    )
    .unwrap();

    let project = LoadedProject::load(&root).expect("load demo project");
    let mut game = RunnerGame::new(
        project,
        RunnerOptions {
            map: Some("Cave".to_string()),
            headless: true, // allow_device = false — pcm must work regardless
            pcm_audio: true,
            ..RunnerOptions::default()
        },
    )
    .expect("boot game");
    assert_eq!(game.audio().track_count(), 2);

    // The on_enter storyline fires on boot; give the sequencer a few video
    // frames to trigger the first note.
    idle(&mut game, 10);
    assert!(
        !game.audio().has_output(),
        "pcm mode must never open a device"
    );

    let pcm = game.render_audio(4410);
    assert_eq!(pcm.len(), 8820, "stereo frames: 2 * frames");
    assert!(
        pcm.iter().any(|s| *s != 0.0),
        "playing music must render non-silent PCM"
    );
}
