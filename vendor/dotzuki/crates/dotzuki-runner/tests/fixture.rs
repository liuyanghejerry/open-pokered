//! Integration tests for `dotzuki-runner` against the committed fixture project
//! under `tests/fixtures/demo/`. The fixture's `tileset.png` is generated in
//! code (4 flat-colour 16×16 tiles) so no binary assets are committed.

use crate::alloc_prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use dotzuki_engine::overworld::actor::OverworldCollision;
use dotzuki_engine::render::Rgba;
use dotzuki_runner::map::{MapObjects, RuntimeMap};
use dotzuki_runner::tileset::PngTileset;
use dotzuki_runner::LoadedProject;

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

/// Unique temp directory, removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(test: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("dotzuki-runner-{test}-{}-{id}", std::process::id()));
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

/// Flat colours of the four generated tiles (GIDs 1..=4).
const TILE_COLORS: [[u8; 4]; 4] = [
    [0xFF, 0x00, 0x00, 0xFF],
    [0x00, 0xFF, 0x00, 0xFF],
    [0x00, 0x00, 0xFF, 0xFF],
    [0xFF, 0xFF, 0x00, 0xFF],
];

/// Write a 64×16 `tileset.png` (four 16×16 flat-colour tiles, row-major).
fn write_tileset(map_dir: &Path) {
    let tile = 16u32;
    let mut img = image::RgbaImage::new(tile * 4, tile);
    for (i, &[r, g, b, a]) in TILE_COLORS.iter().enumerate() {
        for y in 0..tile {
            for x in 0..tile {
                img.put_pixel(i as u32 * tile + x, y, image::Rgba([r, g, b, a]));
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

// ── LoadedProject ────────────────────────────────────────────────────────────

#[test]
fn loads_fixture_project() {
    let (_tmp, root) = demo_project("load");
    let project = LoadedProject::load(&root).expect("load demo project");

    assert_eq!(project.manifest().name, "demo");
    assert_eq!(project.data_root(), root.join("data"));
    assert_eq!(project.gfx_root(), Some(root.join("gfx").as_path()));
    assert_eq!(project.maps_dir(), root.join("data/maps"));

    // Scenes compiled and registered: the story scene and the per-map
    // scripts (the script activity's scriptsDir points at data/maps).
    assert!(project.scripts().has_script("Main"));
    assert!(project.scripts().has_script("Town"));
    assert!(project.scripts().has_script("Cave"));

    // Scene-name ↔ stem index. Both per-map scripts share the stem
    // `script` — the stem index maps it to one of them (order-dependent).
    assert_eq!(project.scene_name_for_stem("main"), Some("Main"));
    assert!(matches!(
        project.scene_name_for_stem("script"),
        Some("Town" | "Cave")
    ));
    assert_eq!(project.stem_for_scene_name("Main"), Some("main"));
    assert_eq!(project.stem_for_scene_name("Town"), Some("script"));
    assert_eq!(project.scene_name_for_stem("nope"), None);

    // Entry resolution from the game section.
    assert_eq!(project.entry_map().unwrap(), "Town");
    assert_eq!(project.entry_scene_name().unwrap(), "Main");

    // The Cave scene declares two @trigger routes (on_enter + npc).
    let routes = project.routes();
    assert_eq!(routes.len(), 2, "{routes:?}");
    assert!(routes
        .iter()
        .any(|r| r.map == "Cave" && r.on_enter && r.storyline == "cave_enter"));
    assert!(routes.iter().any(|r| r.map == "Cave"
        && r.npc.as_deref() == Some("Hermit")
        && r.storyline == "hermit_choice"));
    assert!(project.report().diagnostics.is_empty());
}

#[test]
fn entry_falls_back_when_game_section_absent() {
    let (_tmp, root) = demo_project("fallback");
    // Strip the game section: entry resolution must fall back to the first
    // map dir (sorted) and the first scene by source path.
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join(".dotzuki-editor.json")).unwrap())
            .unwrap();
    let mut manifest = manifest;
    manifest.as_object_mut().unwrap().remove("game");
    fs::write(
        root.join(".dotzuki-editor.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let project = LoadedProject::load(&root).expect("load");
    assert_eq!(
        project.map_ids(),
        vec!["Cave".to_string(), "Town".to_string()]
    );
    // First map dir (sorted) wins without a game section.
    assert_eq!(project.entry_map().unwrap(), "Cave");
    // assets/scenes/main.scene sorts before data/maps/Town/script.scene.
    assert_eq!(project.entry_scene_name().unwrap(), "Main");
}

#[test]
fn load_fails_on_dsl_diagnostics() {
    let (_tmp, root) = demo_project("diag");
    fs::write(
        root.join("assets/scenes/broken.scene"),
        "game_scene Broken {\n    @storylines {\n        @@@ not valid dsl @@@\n    }\n}\n",
    )
    .unwrap();

    let err = LoadedProject::load(&root).err().expect("load should fail");
    let msg = format!("{err:#}");
    assert!(msg.contains("diagnostic"), "{msg}");
    assert!(msg.contains("broken.scene"), "{msg}");
}

#[test]
fn load_fails_without_manifest() {
    let tmp = TestDir::new("nomanifest");
    let err = LoadedProject::load(tmp.path())
        .err()
        .expect("load should fail");
    assert!(format!("{err:#}").contains(".dotzuki-editor.json"));
}

// ── RuntimeMap ───────────────────────────────────────────────────────────────

#[test]
fn runtime_map_loads_tmx_collision_and_tileset() {
    let (_tmp, root) = demo_project("map");
    let map = RuntimeMap::load(&root.join("data/maps"), "Town").expect("load Town");

    assert_eq!(map.id(), "Town");
    assert_eq!(map.width(), 4);
    assert_eq!(map.height(), 3);
    assert_eq!(map.tile_size(), (16, 16));
    assert_eq!(map.pixel_width(), 64);
    assert_eq!(map.pixel_height(), 48);

    // Collision: the fixture walls off the border; interior is walkable —
    // except (0, 1), the Town→Cave warp tile, which must be enterable.
    assert!(map.is_blocked(0, 0), "border corner solid");
    assert!(map.is_blocked(3, 2), "far corner solid");
    assert!(!map.is_blocked(0, 1), "warp tile walkable");
    assert!(!map.is_blocked(1, 1), "interior walkable");
    assert!(!map.is_blocked(2, 1), "interior walkable");
    // Out-of-bounds is solid and never panics.
    assert!(map.is_blocked(-1, 0));
    assert!(map.is_blocked(0, -1));
    assert!(map.is_blocked(4, 0));
    assert!(map.is_blocked(0, 3));
    assert!(map.is_blocked(70000, -70000));

    // The collision layer is excluded from the visual render state.
    assert_eq!(map.render_state().layers.len(), 1);
    assert_eq!(
        map.render_state().background_color,
        (0x30, 0x68, 0x50, 0xFF)
    );

    // Tileset sliced: four 16×16 tiles, 1-based GIDs.
    let ts = map.tileset();
    assert_eq!(ts.tile_count(), 4);
    assert_eq!(ts.tile_size(), (16, 16));
    for (i, &[r, g, b, a]) in TILE_COLORS.iter().enumerate() {
        let gid = (i + 1) as u16;
        assert_eq!(map.gid_pixel(gid, 0, 0), Rgba::new(r, g, b, a), "GID {gid}");
        assert_eq!(
            map.gid_pixel(gid, 15, 15),
            Rgba::new(r, g, b, a),
            "GID {gid}"
        );
    }
    // GID 0 (empty) and out-of-range GIDs are transparent.
    assert_eq!(map.gid_pixel(0, 0, 0), Rgba::TRANSPARENT);
    assert_eq!(map.gid_pixel(5, 0, 0), Rgba::TRANSPARENT);

    // OverworldCollision trait impl agrees with the inherent method.
    let collision: &dyn OverworldCollision = &map;
    assert!(collision.is_blocked(0, 0));
    assert!(!collision.is_blocked(1, 1));
}

#[test]
fn runtime_map_parses_objects_sidecar() {
    let (_tmp, root) = demo_project("objects");
    let map = RuntimeMap::load(&root.join("data/maps"), "Town").expect("load Town");
    let objects = map.objects();

    assert_eq!(objects.npcs.len(), 1);
    let npc = &objects.npcs[0];
    assert_eq!(npc.id, 1);
    assert_eq!(npc.name, "Guide");
    assert_eq!((npc.x, npc.y), (2, 1));
    assert_eq!(npc.facing, "down");
    assert_eq!(npc.sprite, "guide.png");
    assert_eq!(npc.talk, "Welcome to Town.");

    assert_eq!(objects.warps.len(), 1);
    let warp = &objects.warps[0];
    assert_eq!((warp.x, warp.y), (0, 1));
    // A dangling dest_map ("Cave" does not exist) must NOT fail map loading.
    assert_eq!(warp.dest_map, "Cave");
    assert_eq!((warp.dest_x, warp.dest_y), (3, 4));

    assert_eq!(objects.signs.len(), 1);
    assert_eq!(objects.signs[0].text, "You are standing in Town.");
}

#[test]
fn legacy_map_json_sidecar_is_read_as_fallback() {
    let (_tmp, root) = demo_project("legacy");
    // A second map whose sidecar is the legacy `map.json` (e2e-fixture shape).
    let dir = root.join("data/maps/Legacy");
    fs::create_dir_all(&dir).unwrap();
    fs::copy(
        root.join("data/maps/Town/map.tmx.json"),
        dir.join("map.tmx.json"),
    )
    .unwrap();
    write_tileset(&dir);
    fs::write(
        dir.join("map.json"),
        r#"{
  "name": "Legacy",
  "width": 4,
  "height": 3,
  "tileset": "demo",
  "music": "legacy-theme",
  "npcs": [{ "id": 7, "x": 1, "y": 1 }],
  "warps": [{ "x": 3, "y": 1, "dest_map": "Town", "dest_x": 1, "dest_y": 1 }],
  "signs": []
}"#,
    )
    .unwrap();

    let map = RuntimeMap::load(&root.join("data/maps"), "Legacy").expect("load Legacy");
    assert_eq!(map.objects().npcs.len(), 1);
    assert_eq!(map.objects().npcs[0].id, 7);
    // Missing optional fields take defaults.
    assert_eq!(map.objects().npcs[0].facing, "down");
    assert_eq!(map.objects().warps.len(), 1);
    assert_eq!(map.objects().warps[0].dest_map, "Town");

    // objects.json wins when both exist (Town has objects.json only, so check
    // precedence directly via MapObjects::load on a dir holding both files).
    fs::write(dir.join("objects.json"), r#"{ "npcs": [], "warps": [] }"#).unwrap();
    let objects = MapObjects::load(&dir).unwrap();
    assert!(objects.npcs.is_empty(), "objects.json takes precedence");
}

#[test]
fn missing_tileset_is_a_clear_error() {
    let (_tmp, root) = demo_project("notileset");
    fs::remove_file(root.join("data/maps/Town/tileset.png")).unwrap();
    let err = RuntimeMap::load(&root.join("data/maps"), "Town")
        .err()
        .expect("load should fail");
    let msg = format!("{err:#}");
    assert!(msg.contains("tileset.png"), "{msg}");
}

#[test]
fn missing_sidecar_is_not_an_error() {
    let (_tmp, root) = demo_project("nosidecar");
    fs::remove_file(root.join("data/maps/Town/objects.json")).unwrap();
    let map = RuntimeMap::load(&root.join("data/maps"), "Town").expect("load Town");
    assert!(map.objects().npcs.is_empty());
    assert!(map.objects().warps.is_empty());
}

#[test]
fn renders_into_framebuffer() {
    use dotzuki_engine::render::FrameBuffer;
    use dotzuki_engine::render_config::RenderConfig;

    let (_tmp, root) = demo_project("render");
    let map = RuntimeMap::load(&root.join("data/maps"), "Town").expect("load Town");

    let mut fb = FrameBuffer::new(RenderConfig::new(64, 48), Rgba::TRANSPARENT);
    map.render(&mut fb, 0, 0, 64, 48).expect("render");

    // Ground layer row 0 is GIDs [1, 1, 2, 2] → red, red, green, green.
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::from(TILE_COLORS[0])));
    assert_eq!(fb.get_pixel(32, 0), Some(Rgba::from(TILE_COLORS[1])));
    // Row 1 is GIDs [3, 3, 4, 4] → blue at (0,16), yellow at (32,16).
    assert_eq!(fb.get_pixel(0, 16), Some(Rgba::from(TILE_COLORS[2])));
    assert_eq!(fb.get_pixel(32, 16), Some(Rgba::from(TILE_COLORS[3])));
}

// ── PngTileset ───────────────────────────────────────────────────────────────

#[test]
fn tileset_rejects_bad_geometry() {
    let tmp = TestDir::new("badgeo");
    let map_dir = tmp.path().join("m");
    fs::create_dir_all(&map_dir).unwrap();
    write_tileset(&map_dir); // 64×16
    let bytes = fs::read(map_dir.join("tileset.png")).unwrap();

    // 16×16 tiles: OK.
    assert!(PngTileset::from_png_bytes(&bytes, 16, 16).is_ok());
    // Zero tile size: error.
    assert!(PngTileset::from_png_bytes(&bytes, 0, 16).is_err());
    // Image not a multiple of the tile size: error.
    assert!(PngTileset::from_png_bytes(&bytes, 24, 16).is_err());
    // Garbage bytes: decode error.
    assert!(PngTileset::from_png_bytes(b"not a png", 16, 16).is_err());
}
