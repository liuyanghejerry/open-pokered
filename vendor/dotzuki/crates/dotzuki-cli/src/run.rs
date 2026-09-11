//! `dotzuki run` — boot a zero-Rust game project and play it.
//!
//! Window mode (default) opens a 320×240 (scaled) winit/pixels window via
//! `dotzuki_app`; `--headless` drives the game windowless for CI smoke tests,
//! optionally dumping the final frame to a PNG. `--watch` (windowed only)
//! hot-reloads scenes and the current map as files change on disk. A valid
//! save file (`<project>/.dotzuki-save.json`) resumes on boot — `--fresh`
//! ignores it, `--save-file` relocates it; windowed runs write saves at
//! stable points, headless runs only with `--save`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use dotzuki_runner::{run_headless, HeadlessOptions, LoadedProject, RunnerGame, RunnerOptions};

/// CLI arguments for `dotzuki run`.
pub struct RunArgs {
    /// Project root containing `.dotzuki-editor.json`.
    pub dir: PathBuf,
    /// Map to spawn on (overrides `game.entryMap`).
    pub map: Option<String>,
    /// UI/script language (`en` / `zh`).
    pub lang: String,
    /// Run without a window.
    pub headless: bool,
    /// Headless frame count.
    pub frames: u32,
    /// Headless PNG screenshot destination.
    pub screenshot: Option<PathBuf>,
    /// Window scale factor.
    pub scale: u32,
    /// Hot-reload scenes and the current map as files change on disk.
    pub watch: bool,
    /// Ignore an existing save and start fresh.
    pub fresh: bool,
    /// Save file location override.
    pub save_file: Option<PathBuf>,
    /// Headless: also write saves (windowed runs always save).
    pub save: bool,
}

/// Boot the project and run it (windowed unless `args.headless`).
pub fn run(args: RunArgs) -> Result<()> {
    let project = LoadedProject::load(&args.dir)
        .with_context(|| format!("failed to load project {}", args.dir.display()))?;
    let title = project.manifest().name.clone();
    let mut game = RunnerGame::new(
        project,
        RunnerOptions {
            map: args.map,
            lang: args.lang,
            watch: args.watch && !args.headless,
            headless: args.headless,
            fresh: args.fresh,
            save_file: args.save_file.clone(),
            // Windowed runs save at stable points; headless runs only on an
            // explicit `--save`, keeping CI side-effect-free.
            write_saves: !args.headless || args.save,
            rng_script: None,
            pcm_audio: false,
        },
    )?;

    if args.headless {
        if args.watch {
            eprintln!("--watch is ignored in headless mode");
        }
        run_headless(
            &mut game,
            &HeadlessOptions {
                frames: args.frames,
                screenshot: args.screenshot.clone(),
                ..HeadlessOptions::default()
            },
        )?;
        println!("headless run complete ({} frames)", args.frames);
        if args.save {
            // Saves only land at stable points (scene end / map transition);
            // a short run may never reach one, which otherwise looks like
            // --save silently failing.
            let save_path = args
                .save_file
                .clone()
                .unwrap_or_else(|| args.dir.join(dotzuki_runner::DEFAULT_SAVE_FILE));
            if !save_path.is_file() {
                eprintln!(
                    "note: no save file was written at {} — the run reached no stable point within {} frames",
                    save_path.display(),
                    args.frames
                );
            }
        }
        return Ok(());
    }

    dotzuki_app::run(
        dotzuki_app::GameWindowConfig {
            title,
            width: dotzuki_runner::SCREEN_W as u32,
            height: dotzuki_runner::SCREEN_H as u32,
            scale: args.scale,
            resizable: true,
        },
        game,
    )
    .map_err(|e| anyhow::anyhow!("window error: {e}"))
}
