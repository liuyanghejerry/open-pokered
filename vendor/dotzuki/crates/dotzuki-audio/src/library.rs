//! In-memory audio library loaded from a directory of [`TrackDef`] JSON files.
//!
//! This is the game-agnostic, file-based counterpart to a game's hardcoded
//! audio tables: point it at a directory tree of `*.json` tracks and it gives a
//! name → track lookup that can drive the [`Sequencer`](crate::sequencer::Sequencer)
//! directly. Requires the `serde` feature.

use std::collections::BTreeMap;
use std::path::Path;

use crate::format::TrackDef;

/// A collection of tracks keyed by their [`TrackDef::id`].
#[derive(Debug, Clone, Default)]
pub struct AudioLibrary {
    tracks: BTreeMap<String, TrackDef>,
}

impl AudioLibrary {
    /// An empty library.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load every `*.json` track under `dir` (recursively).
    pub fn load_dir(dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let mut lib = Self::new();
        lib.add_dir(dir)?;
        Ok(lib)
    }

    /// Recursively add every `*.json` track under `dir` to this library.
    ///
    /// Files that fail to parse as a [`TrackDef`] are surfaced as an
    /// `InvalidData` error naming the offending path.
    pub fn add_dir(&mut self, dir: impl AsRef<Path>) -> std::io::Result<()> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                self.add_dir(&path)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let text = std::fs::read_to_string(&path)?;
                let track: TrackDef = serde_json::from_str(&text).map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("{}: {e}", path.display()),
                    )
                })?;
                self.tracks.insert(track.id.clone(), track);
            }
        }
        Ok(())
    }

    /// Insert (or replace) a track by its id.
    pub fn insert(&mut self, track: TrackDef) {
        self.tracks.insert(track.id.clone(), track);
    }

    /// Look up a track by id.
    pub fn get(&self, id: &str) -> Option<&TrackDef> {
        self.tracks.get(id)
    }

    /// Number of loaded tracks.
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    /// Whether the library holds no tracks.
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    /// Iterate over all track ids.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.tracks.keys().map(String::as_str)
    }

    /// Iterate over all tracks.
    pub fn tracks(&self) -> impl Iterator<Item = &TrackDef> {
        self.tracks.values()
    }

    /// Start the named track on `seq`. Returns `false` if no such track.
    pub fn play(&self, seq: &mut crate::sequencer::Sequencer, id: &str) -> bool {
        match self.get(id) {
            Some(track) => {
                track.play_on(seq);
                true
            }
            None => false,
        }
    }
}
