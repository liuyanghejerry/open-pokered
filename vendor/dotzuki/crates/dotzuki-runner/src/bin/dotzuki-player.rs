//! `dotzuki-player` — the game-agnostic native player that
//! `dotzuki export --native` ships next to a `game.dzpk` pack.
//!
//! The binary knows nothing about any specific game: it opens the pack
//! (default `<exe dir>/game.dzpk`, overridable with the first positional
//! argument) as an in-memory project and boots it through the same
//! `RunnerGame` + `dotzuki-app` window `dotzuki run` uses. Packs written by
//! older exports (`game.bundle.json`, base64 JSON) still boot — the format is
//! sniffed from the magic bytes. The save file lives next to the pack as
//! `.dotzuki-save.json`.
//!
//! `--headless` (with `--frames` / `--screenshot`) smoke-drives an exported
//! pack without a window — CI proof that a shipped game boots.

#[cfg(not(target_arch = "wasm32"))]
mod player {
    use std::path::PathBuf;
    use std::sync::Arc;

    use anyhow::{bail, Context, Result};
    use dotzuki_runner::bundle::decode_bundle_files;
    use dotzuki_runner::pack::{PackFiles, MAGIC};
    use dotzuki_runner::vfs::ProjectFiles;
    use dotzuki_runner::{
        run_headless, HeadlessOptions, LoadedProject, MemoryFiles, RunnerGame, RunnerOptions,
        DEFAULT_SAVE_FILE, SCREEN_H, SCREEN_W,
    };

    const USAGE: &str = "\
dotzuki-player — boot an exported dotzuki game pack

USAGE:
    dotzuki-player [PACK] [OPTIONS]

ARGS:
    [PACK]    Path to game.dzpk (or a legacy game.bundle.json);
              default: game.dzpk next to this executable, falling back to
              game.bundle.json for exports from older dotzuki versions

OPTIONS:
    --lang <en|zh>        UI/script language [default: en]
    --scale <N>           Window scale factor [default: 3]
    --fresh               Ignore an existing save file and start fresh
    --headless            Run without a window (smoke tests)
    --frames <N>          Headless: frames to simulate [default: 120]
    --screenshot <PATH>   Headless: dump the final frame to this PNG
    -h, --help            Print this help";

    struct PlayerArgs {
        bundle: Option<PathBuf>,
        lang: String,
        scale: u32,
        fresh: bool,
        headless: bool,
        frames: u32,
        screenshot: Option<PathBuf>,
    }

    fn parse_args() -> Result<PlayerArgs> {
        let mut args = PlayerArgs {
            bundle: None,
            lang: "en".to_string(),
            scale: 3,
            fresh: false,
            headless: false,
            frames: 120,
            screenshot: None,
        };
        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--fresh" => args.fresh = true,
                "--headless" => args.headless = true,
                "--lang" => {
                    let lang = it.next().context("--lang needs a value")?;
                    if lang != "en" && lang != "zh" {
                        bail!("--lang must be 'en' or 'zh', got '{lang}'");
                    }
                    args.lang = lang;
                }
                "--scale" => {
                    let value = it.next().context("--scale needs a value")?;
                    args.scale = value.parse().context("--scale must be a number")?;
                }
                "--frames" => {
                    let value = it.next().context("--frames needs a value")?;
                    args.frames = value.parse().context("--frames must be a number")?;
                }
                "--screenshot" => {
                    let value = it.next().context("--screenshot needs a path")?;
                    args.screenshot = Some(PathBuf::from(value));
                }
                "-h" | "--help" => {
                    println!("{USAGE}");
                    std::process::exit(0);
                }
                _ if arg.starts_with('-') => bail!("unknown flag '{arg}'\n\n{USAGE}"),
                _ => {
                    if args.bundle.is_some() {
                        bail!("multiple pack paths given\n\n{USAGE}");
                    }
                    args.bundle = Some(PathBuf::from(arg));
                }
            }
        }
        Ok(args)
    }

    /// `<exe dir>/game.dzpk` — the layout `dotzuki export --native` writes —
    /// falling back to a legacy `<exe dir>/game.bundle.json` when no pack
    /// exists (exports from older dotzuki versions).
    fn default_bundle_path() -> Result<PathBuf> {
        let exe = std::env::current_exe().context("cannot locate the player executable")?;
        let dir = exe
            .parent()
            .context("player executable has no parent directory")?;
        let pack = dir.join("game.dzpk");
        if pack.is_file() {
            return Ok(pack);
        }
        Ok(dir.join("game.bundle.json"))
    }

    /// Open a pack file as a [`ProjectFiles`] view: `.dzpk` when the magic
    /// matches, legacy base64 JSON otherwise.
    fn open_bundle(path: &std::path::Path) -> Result<Arc<dyn ProjectFiles>> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read game pack {}", path.display()))?;
        if bytes.starts_with(&MAGIC) {
            let pack = PackFiles::from_bytes(bytes)
                .with_context(|| format!("failed to parse game pack {}", path.display()))?;
            return Ok(Arc::new(pack));
        }
        let json = String::from_utf8(bytes)
            .with_context(|| format!("{} is neither a .dzpk pack nor bundle JSON", path.display()))?;
        let files = decode_bundle_files(&json)
            .with_context(|| format!("failed to decode game bundle {}", path.display()))?;
        Ok(Arc::new(MemoryFiles::new(files)))
    }

    pub fn main() -> Result<()> {
        let args = parse_args()?;
        let bundle_path = match &args.bundle {
            Some(path) => path.clone(),
            None => default_bundle_path()?,
        };
        let files = open_bundle(&bundle_path)?;
        let project = LoadedProject::load_with_files(files)
            .context("failed to boot the bundled project")?;
        let title = project.manifest().name.clone();

        // A bundled project has no disk root, so the runner's default save
        // location cannot apply — the save lives next to the bundle instead.
        let save_dir = bundle_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let save_file = save_dir.join(DEFAULT_SAVE_FILE);

        let mut game = RunnerGame::new(
            project,
            RunnerOptions {
                map: None,
                lang: args.lang,
                watch: false,
                headless: args.headless,
                pcm_audio: false,
                fresh: args.fresh,
                save_file: Some(save_file),
                rng_script: None,
                write_saves: !args.headless,
            },
        )?;

        if args.headless {
            run_headless(
                &mut game,
                &HeadlessOptions {
                    frames: args.frames,
                    screenshot: args.screenshot.clone(),
                    ..HeadlessOptions::default()
                },
            )?;
            println!("headless run complete ({} frames)", args.frames);
            return Ok(());
        }

        dotzuki_app::run(
            dotzuki_app::GameWindowConfig {
                title,
                width: SCREEN_W as u32,
                height: SCREEN_H as u32,
                scale: args.scale,
                resizable: true,
            },
            game,
        )
        .map_err(|e| anyhow::anyhow!("window error: {e}"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    player::main()
}

// The player is a native shell; nothing in it can run on wasm32 (the web
// player is dotzuki-runner-web + the exported index.html instead).
#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("dotzuki-player is a native player; it is not available on wasm32");
}
