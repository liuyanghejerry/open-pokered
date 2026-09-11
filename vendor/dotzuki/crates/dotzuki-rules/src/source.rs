//! Dual-mode rule sourcing (doc 11 §4.2, §6): ONE `rules.ron`, two access modes
//! that produce the **same** runtime [`Ruleset`].
//!
//! * **RELEASE / baked** ([`RuleSource::baked`]) — the caller `include_str!`s the
//!   canonical `rules.ron` so it is compiled into the binary; **zero file IO**,
//!   parsed once. This is the default build (the `hot-reload` feature is OFF).
//! * **DEV / hot-reload** ([`RuleSource::from_path`]) — read `rules.ron` from
//!   disk at startup and (behind the `hot-reload` feature) watch it; [`poll_changed`]
//!   signals a change so the game rebuilds the registry **between turns**.
//!
//! Per doc 11 §4.2, swapping the compiled registry (the [`EffectId`]→op-list map)
//! is **safe mid-battle**: effects are addressed by `EffectId` and live state
//! lives in the engine's `EffectState` arena, **not** in the data — so a reload
//! replaces the *vocabulary*, never the *in-flight state*.
//!
//! This module is **game-agnostic**: it yields the engine-agnostic [`Ruleset`]
//! (game binding ↔ `P::Stat`/`P::Type` happens later in
//! [`CompiledRuleset::compile`](crate::CompiledRuleset::compile)). The decisive
//! invariant — proved by tests — is **baked text and disk text parse to the same
//! `Ruleset`**, so both modes drive an identical runtime ruleset.
//!
//! [`poll_changed`]: RuleSource::poll_changed
//! [`EffectId`]: dotzuki_engine::battle::stack::EffectId

#[cfg(not(target_os = "none"))]
use std::path::{Path, PathBuf};

use crate::model::{LoadError, Ruleset};

// Hosted-only pieces below (std::path / std::fs / notify): bare-metal builds
// use the baked source exclusively (RuleSource::baked), which is the RELEASE
// mode by design — zero file IO. The `Disk` variant, `from_path`, and the
// watcher are compiled out on `target_os = "none"`.

/// A source of rule text yielding a runtime [`Ruleset`], in one of two modes
/// (doc 11 §4.2). Both modes call the **same** [`Ruleset::from_ron`], so a baked
/// build and a disk build of the *same* `rules.ron` produce byte-identical
/// rulesets — the dual-mode guarantee.
pub enum RuleSource {
    /// **Baked** (RELEASE): the canonical `rules.ron` text compiled into the
    /// binary via the caller's `include_str!`. Zero file IO. The default build.
    Baked {
        /// The `include_str!`'d source text.
        text: &'static str,
    },
    /// **Disk** (DEV): `rules.ron` read from a file path. With the `hot-reload`
    /// feature a [`Watcher`](self::watch::Watcher) observes the file and
    /// [`poll_changed`](RuleSource::poll_changed) signals edits so the registry
    /// is rebuilt between turns.
    #[cfg(not(target_os = "none"))]
    Disk {
        /// The on-disk `rules.ron` path.
        path: PathBuf,
        /// The optional file watcher (present only with the `hot-reload` feature
        /// and a successful watch init).
        #[cfg(feature = "hot-reload")]
        watcher: Option<watch::Watcher>,
    },
}

impl RuleSource {
    /// A **baked** source over `include_str!`'d text (RELEASE; the default build).
    /// The text is compiled into the binary; loading never touches the filesystem.
    pub fn baked(text: &'static str) -> Self {
        RuleSource::Baked { text }
    }

    /// A **disk** source over a `rules.ron` path (DEV). Without the `hot-reload`
    /// feature this still reads the file at [`load`](RuleSource::load) time (it
    /// just never watches it); with the feature it also starts a watcher so
    /// [`poll_changed`](RuleSource::poll_changed) can signal edits.
    #[cfg(not(target_os = "none"))]
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        #[cfg(feature = "hot-reload")]
        {
            let watcher = watch::Watcher::new(&path).ok();
            RuleSource::Disk { path, watcher }
        }
        #[cfg(not(feature = "hot-reload"))]
        {
            RuleSource::Disk { path }
        }
    }

    /// Read the current rule text + parse it into a [`Ruleset`]. The decisive
    /// dual-mode invariant: baked text and disk text run through the **same**
    /// [`Ruleset::from_ron`], so identical bytes ⇒ identical ruleset.
    pub fn load(&self) -> Result<Ruleset, LoadError> {
        match self {
            RuleSource::Baked { text } => Ruleset::from_ron(text),
            #[cfg(not(target_os = "none"))]
            RuleSource::Disk { path, .. } => {
                let text = read_to_string(path)?;
                Ruleset::from_ron(&text)
            }
        }
    }

    /// Poll whether the on-disk `rules.ron` has changed since the last poll
    /// (DEV / hot-reload). A `true` return is the game's cue to re-[`load`] and
    /// rebuild the compiled registry **between turns** (safe mid-battle —
    /// doc 11 §4.2). A **baked** source never changes ⇒ always `false`; a disk
    /// source without the `hot-reload` feature also returns `false` (no watcher).
    ///
    /// [`load`]: RuleSource::load
    pub fn poll_changed(&mut self) -> bool {
        match self {
            RuleSource::Baked { .. } => false,
            #[cfg(all(feature = "hot-reload", not(target_os = "none")))]
            RuleSource::Disk { watcher, .. } => {
                watcher.as_mut().map(|w| w.poll_changed()).unwrap_or(false)
            }
            #[cfg(all(not(feature = "hot-reload"), not(target_os = "none")))]
            RuleSource::Disk { .. } => false,
        }
    }

    /// Whether this source can hot-reload (a disk source with a live watcher).
    /// `false` for a baked source or a feature-off build. Useful for diagnostics.
    pub fn is_hot_reloadable(&self) -> bool {
        match self {
            RuleSource::Baked { .. } => false,
            #[cfg(all(feature = "hot-reload", not(target_os = "none")))]
            RuleSource::Disk { watcher, .. } => watcher.is_some(),
            #[cfg(all(not(feature = "hot-reload"), not(target_os = "none")))]
            RuleSource::Disk { .. } => false,
        }
    }
}

/// Read a file to a string, mapping IO errors into the loader's [`LoadError::Ron`]
/// channel (a missing/unreadable `rules.ron` is a load error, never a battle-time
/// surprise — doc 11 §4.2).
#[cfg(not(target_os = "none"))]
fn read_to_string(path: &Path) -> Result<String, LoadError> {
    std::fs::read_to_string(path)
        .map_err(|e| LoadError::Ron(format!("reading {}: {e}", path.display())))
}

/// The `notify`-based file watcher (DEV only; behind the `hot-reload` feature).
/// Mirrors the existing `dotzuki-app` `AssetWatcher` pattern (notify v6, poll-based,
/// `mpsc` drain) but scoped to a single `rules.ron`. It draws NO randomness,
/// reads NO clock that affects draw order, and never touches the interpreter —
/// it is a pure file-change signal feeding a between-turns rebuild.
#[cfg(all(feature = "hot-reload", not(target_os = "none")))]
pub mod watch {
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;

    use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

    /// A poll-based single-file watcher for `rules.ron` (doc 11 §4.2). Call
    /// [`poll_changed`](Watcher::poll_changed) each frame; it drains the notify
    /// channel and returns `true` if the watched file was modified/created.
    pub struct Watcher {
        _watcher: RecommendedWatcher,
        rx: mpsc::Receiver<Result<notify::Event, notify::Error>>,
        target: PathBuf,
    }

    impl Watcher {
        /// Start watching `rules.ron`. Watches the file's **parent directory**
        /// (the robust pattern: editors often replace-on-save, which removes the
        /// inode a direct file-watch holds), filtering events down to the target
        /// path. Returns an error if the watcher cannot be initialized.
        pub fn new(path: &Path) -> Result<Self, String> {
            let target = path.to_path_buf();
            let dir = target
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            let (tx, rx) = mpsc::channel();
            let mut watcher = RecommendedWatcher::new(
                move |res| {
                    let _ = tx.send(res);
                },
                Config::default(),
            )
            .map_err(|e| format!("failed to create rules.ron watcher: {e}"))?;
            watcher
                .watch(&dir, RecursiveMode::NonRecursive)
                .map_err(|e| format!("failed to watch {}: {e}", dir.display()))?;
            Ok(Self {
                _watcher: watcher,
                rx,
                target,
            })
        }

        /// Drain pending notify events; return `true` if the watched `rules.ron`
        /// was modified or created since the last poll. Deduplicated implicitly
        /// (any matching event ⇒ one `true`). Pure: no RNG, no draw-order effect.
        pub fn poll_changed(&mut self) -> bool {
            let mut changed = false;
            while let Ok(Ok(event)) = self.rx.try_recv() {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_))
                    && event.paths.iter().any(|p| paths_match(p, &self.target))
                {
                    changed = true;
                }
            }
            changed
        }
    }

    /// Compare an event path against the target, tolerating symlink/`.`/`..`
    /// differences by falling back to file-name equality when canonicalization
    /// is unavailable.
    fn paths_match(event_path: &Path, target: &Path) -> bool {
        if event_path == target {
            return true;
        }
        match (event_path.canonicalize(), target.canonicalize()) {
            (Ok(a), Ok(b)) if a == b => true,
            _ => event_path.file_name() == target.file_name(),
        }
    }
}
