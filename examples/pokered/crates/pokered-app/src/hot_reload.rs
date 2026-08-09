use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

const SUPPORTED_EXTENSIONS: &[&str] = &["tmx", "png", "js", "scene"];

fn is_supported_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map_or(false, |ext| SUPPORTED_EXTENSIONS.contains(&ext))
}

/// Describes a single detected asset change.
pub struct AssetChange {
    pub path: PathBuf,
}

impl std::fmt::Display for AssetChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

/// Watches directories for changes to supported asset files (.tmx, .png, .js).
///
/// Poll-based interface: call `poll_events()` each frame to collect
/// changes that have occurred since the last poll. Duplicate events
/// for the same file within a single poll cycle are deduplicated.
pub struct AssetWatcher {
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<Result<Event, notify::Error>>,
    seen: HashSet<PathBuf>,
}

impl AssetWatcher {
    /// Create a new asset watcher that watches the given directories recursively.
    ///
    /// Returns an error if the underlying file watcher cannot be initialized or
    /// if none of the provided directories exist.
    pub fn new(dirs: &[PathBuf]) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        )
        .map_err(|e| format!("Failed to create file watcher: {}", e))?;

        let mut watched_any = false;
        for dir in dirs {
            if dir.is_dir() {
                watcher
                    .watch(dir, RecursiveMode::Recursive)
                    .map_err(|e| format!("Failed to watch {}: {}", dir.display(), e))?;
                eprintln!("[hot-reload] Watching: {}", dir.display());
                watched_any = true;
            } else {
                eprintln!("[hot-reload] Skipping (not found): {}", dir.display());
            }
        }

        if !watched_any {
            return Err("No valid directories to watch".to_string());
        }

        Ok(Self {
            _watcher: watcher,
            rx,
            seen: HashSet::new(),
        })
    }

    /// Poll for file changes since the last call.
    ///
    /// Returns a deduplicated list of changed asset files matching
    /// supported extensions (.tmx, .png, .js).
    pub fn poll_events(&mut self) -> Vec<AssetChange> {
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

        self.seen
            .iter()
            .map(|p| AssetChange { path: p.clone() })
            .collect()
    }
}
