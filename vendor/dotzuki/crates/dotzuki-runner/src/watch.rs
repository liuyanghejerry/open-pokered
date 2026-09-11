//! File watching for `dotzuki run --watch`: [`ProjectWatcher`].
//!
//! Watches the project's data/gfx/scene directories recursively and reports
//! changed `.scene`/`.json`/`.png`/`.tmx` files. Same notify version and
//! poll-per-frame shape as `dotzuki_app::hot_reload::AssetWatcher` — it is not
//! reused because its extension filter (`tmx/png/js`) lacks `.scene` and is
//! not configurable.
//!
//! The watcher only *reports* paths; reload policy (which paths trigger a
//! scene recompile or a map reload) lives in [`crate::game::RunnerGame`].

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Extensions a reload can act on: DSL scenes, TMX maps, tileset PNGs and
/// JSON data (map sidecars, `map.tmx.json`).
const SUPPORTED_EXTENSIONS: &[&str] = &["scene", "json", "png", "tmx"];

fn is_supported_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| SUPPORTED_EXTENSIONS.contains(&ext))
}

/// Watches project directories for changes to reloadable content files.
///
/// Poll-based interface: call [`poll_events`](Self::poll_events) each frame
/// to collect the changes since the last poll. Duplicate events for the same
/// file within one poll cycle are deduplicated.
pub struct ProjectWatcher {
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<Result<Event, notify::Error>>,
    seen: HashSet<PathBuf>,
}

impl ProjectWatcher {
    /// Watch every directory in `dirs` recursively. Missing directories are
    /// skipped (with a log line); an error is returned only when the watcher
    /// itself cannot be created or no directory exists to watch.
    pub fn new(dirs: &[PathBuf]) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        )
        .map_err(|e| format!("failed to create file watcher: {e}"))?;

        let mut watched_any = false;
        for dir in dirs {
            if dir.is_dir() {
                watcher
                    .watch(dir, RecursiveMode::Recursive)
                    .map_err(|e| format!("failed to watch {}: {e}", dir.display()))?;
                log::info!("hot-reload: watching {}", dir.display());
                watched_any = true;
            } else {
                log::info!("hot-reload: skipping {} (not found)", dir.display());
            }
        }

        if !watched_any {
            return Err("no valid directories to watch".to_string());
        }

        Ok(Self {
            _watcher: watcher,
            rx,
            seen: HashSet::new(),
        })
    }

    /// Changed content files since the last call, deduplicated by path.
    pub fn poll_events(&mut self) -> Vec<PathBuf> {
        self.seen.clear();

        while let Ok(Ok(event)) = self.rx.try_recv() {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    for path in event.paths {
                        if is_supported_file(&path) {
                            self.seen.insert(path);
                        }
                    }
                }
                _ => {}
            }
        }

        self.seen.iter().cloned().collect()
    }
}
