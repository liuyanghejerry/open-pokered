//! Generic runtime for **zero-Rust game projects**.
//!
//! A game project is a directory with a `.dotzuki-editor.json` manifest, a
//! `dataRoot` holding `maps/<MapId>/` directories (Tiled `map.tmx.json` +
//! `tileset.png` + `objects.json` sidecar + per-map `script.scene`), and a
//! scenes directory with story `.scene` files. See
//! `docs/reference/project-manifest.md` for the full contract.
//!
//! This crate boots such a project without any game-specific Rust:
//!
//! - [`manifest`] — serde model of `.dotzuki-editor.json`;
//! - [`project`] — [`project::LoadedProject`]: manifest → DSL compile →
//!   script registry + storyline routes + entry scene/map resolution;
//! - [`map`] — [`map::RuntimeMap`]: TMX layers, collision grid, objects
//!   sidecar (NPCs/warps/signs);
//! - [`tileset`] — [`tileset::PngTileset`]: a `tileset.png` atlas sliced into
//!   per-tile RGBA pixels, addressable by 1-based Tiled GID;
//! - [`game`] — [`game::RunnerGame`]: the playable runtime (overworld +
//!   scene VM + dialogue/choice UI), implementing `dotzuki_app::GameLoop` for
//!   window mode and exposing windowless `update`/`draw` for headless use;
//! - [`headless`] — [`headless::run_headless`]: a windowless frame driver
//!   with auto-A input and PNG screenshots (native only);
//! - [`watch`] — [`watch::ProjectWatcher`]: notify-based file watching for
//!   `dotzuki run --watch` hot reload (feature `watch`);
//! - [`vfs`] — [`vfs::ProjectFiles`]: the virtual file system every project
//!   read goes through ([`vfs::DiskFiles`] native, [`vfs::MemoryFiles`] for
//!   the WASM shell);
//! - [`audio`] — [`audio::RunnerAudio`]: optional music/SFX playback for
//!   scene audio commands (dotzuki-audio APU + cpal; silent when a project has
//!   no `data/audio/` tracks or no output device is available);
//! - [`battle`] — the generic, data-driven battle system (manifest `battle`
//!   section → record-driven combatants, party switching, battle-usable
//!   items, the standard damage formula, a menu/narration turn loop,
//!   `startBattle` scene integration);
//! - [`save`] — [`save::GameSave`]: versioned JSON save/load at
//!   `<project>/.dotzuki-save.json`, written at stable overworld points;
//! - [`bundle`] — [`bundle::decode_bundle_files`]: decode a legacy exported
//!   `game.bundle.json` into the `path → content` map a [`vfs::MemoryFiles`]
//!   boots from (superseded by `.dzpk` packs; kept so old exports still boot);
//! - [`pack`] — [`pack::PackFiles`]: read a `.dzpk` binary game pack (the
//!   format `dotzuki export` ships) through [`vfs::ProjectFiles`], plus
//!   [`pack::encode_pack`] for writing one.

pub mod audio;
pub mod battle;
pub mod bundle;
pub mod game;
#[cfg(not(target_arch = "wasm32"))]
pub mod headless;
pub mod manifest;
pub mod map;
pub mod pack;
pub mod project;
pub mod save;
pub mod tileset;
pub mod vfs;
#[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
pub mod watch;

pub use audio::RunnerAudio;
pub use battle::hooks::validate_ruleset;
pub use battle::{Battle, BattleOutcome};
pub use game::{RunnerGame, RunnerOptions, SCREEN_H, SCREEN_W};
#[cfg(not(target_arch = "wasm32"))]
pub use headless::{run_headless, HeadlessOptions};
pub use map::{
    EncounterConfig, EncounterTableEntry, EncounterZone, MapObjects, NpcDef, RuntimeMap, SignDef,
    WarpDef,
};
pub use project::LoadedProject;
pub use save::{GameSave, PlayerSave, DEFAULT_SAVE_FILE, SAVE_VERSION};
pub use tileset::PngTileset;
pub use pack::PackFiles;
pub use vfs::{DiskFiles, MemoryFiles, ProjectFiles};
#[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
pub use watch::ProjectWatcher;
