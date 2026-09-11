//! Native integration test: boot the committed `dotzuki-runner` fixture project
//! through the real `files_json → base64 → MemoryFiles → RunnerGame` path and
//! drive a few frames. `pkg`/wasm-pack still consumes the cdylib; this test
//! exercises the rlib.

use crate::alloc_prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use dotzuki_runner_web::WasmRunner;

/// Flat colours of the four generated tiles — the same recipe as
/// `dotzuki-runner/tests/fixture.rs` (no binary assets committed).
const TILE_COLORS: [[u8; 4]; 4] = [
    [0xFF, 0x00, 0x00, 0xFF],
    [0x00, 0xFF, 0x00, 0xFF],
    [0x00, 0x00, 0xFF, 0xFF],
    [0xFF, 0xFF, 0x00, 0xFF],
];

/// A 64×16 `tileset.png` (four 16×16 flat-colour tiles, row-major), as bytes.
fn tileset_png() -> Vec<u8> {
    let tile = 16u32;
    let mut img = image::RgbaImage::new(tile * 4, tile);
    for (i, &[r, g, b, a]) in TILE_COLORS.iter().enumerate() {
        for y in 0..tile {
            for x in 0..tile {
                img.put_pixel(i as u32 * tile + x, y, image::Rgba([r, g, b, a]));
            }
        }
    }
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

fn walk(dir: &Path, root: &Path, out: &mut HashMap<String, String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            walk(&path, root, out);
        } else {
            let rel: PathBuf = path.strip_prefix(root).unwrap().to_path_buf();
            let posix = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.insert(posix, BASE64.encode(fs::read(&path).unwrap()));
        }
    }
}

/// The fixture project as a `files_json` string, with the tilesets that the
/// dotzuki-runner tests generate in code added for both maps.
fn fixture_files_json() -> String {
    serde_json::to_string(&fixture_files()).unwrap()
}

/// The fixture project as a path→base64 map (before JSON serialisation), so
/// tests can inject extra files.
fn fixture_files() -> HashMap<String, String> {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../dotzuki-runner/tests/fixtures/demo");
    let mut files = HashMap::new();
    walk(&fixture, &fixture, &mut files);
    for map in ["Town", "Cave"] {
        files.insert(
            format!("data/maps/{map}/tileset.png"),
            BASE64.encode(tileset_png()),
        );
    }
    files
}

#[test]
fn boots_fixture_ticks_and_round_trips_save() {
    let files_json = fixture_files_json();
    let mut runner = WasmRunner::new(&files_json, None).expect("boot fixture project");

    assert_eq!(runner.width(), 320);
    assert_eq!(runner.height(), 240);

    // Idle frames: the framebuffer is always the full 320×240×4 RGBA surface.
    for _ in 0..10 {
        let fb = runner.tick(0);
        assert_eq!(fb.len(), 320 * 240 * 4);
    }

    // The fixture boots into its entry scene; alternate A presses to play it
    // through until the game reaches a stable (exportable) state.
    let mut save = None;
    for i in 0..600 {
        runner.tick(if i % 2 == 0 { 0b0000_0001 } else { 0 });
        if let Some(json) = runner.export_save() {
            save = Some(json);
            break;
        }
    }
    let save = save.expect("game should reach a stable, exportable state");

    // The save imports back, and garbage is rejected without losing state.
    assert!(runner.import_save(&save));
    assert!(!runner.import_save("this is not save json"));
    // A corrupt save passed to the constructor is ignored (boot still works).
    let _fresh = WasmRunner::new(&files_json, Some("garbage".to_string()))
        .expect("boot with corrupt save should fall through");
}

/// A minimal music track (dotzuki-audio `TrackDef` JSON), same as
/// `dotzuki-runner/tests/audio.rs`.
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

/// Town scene that starts the theme on map entry.
const TOWN_SCENE: &str = r#"game_scene Town {
    @storyline("town_enter") {
        @trigger(map = "Town", on_enter = true)
        @command("playMusic", "theme")
    }
}
"#;

#[test]
fn tick_accumulates_pcm_and_take_audio_drains() {
    let mut files = fixture_files();
    files.insert(
        "data/audio/music/theme.json".to_string(),
        BASE64.encode(THEME_JSON),
    );
    files.insert(
        "data/maps/Town/script.scene".to_string(),
        BASE64.encode(TOWN_SCENE),
    );
    let files_json = serde_json::to_string(&files).unwrap();

    let mut runner = WasmRunner::new(&files_json, None).expect("boot fixture with audio");

    // The on_enter storyline starts the theme; give the sequencer a few video
    // frames to trigger the first note.
    for _ in 0..10 {
        runner.tick(0);
    }
    let pcm = runner.take_audio();
    assert!(!pcm.is_empty(), "music must render PCM");
    assert!(
        pcm.iter().any(|s| *s != 0.0),
        "playing music must render non-silent PCM"
    );

    // Each tick adds ~738.4 stereo frames (≈1476.9 f32 samples).
    runner.tick(0);
    let one = runner.take_audio();
    assert!(
        (736 * 2..=741 * 2).contains(&one.len()),
        "one tick should accumulate ~1477 samples, got {}",
        one.len()
    );

    // take_audio drains.
    assert!(runner.take_audio().is_empty());
}
