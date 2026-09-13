//! M5 determinism: seedable RNG plumbing + frame-level fork/restore.
//!
//! `set_seed` replaces both RNG streams (overworld + battle) with
//! seeded ChaCha12 streams, so every later random event is reproducible.
//! `agent_save_state` / `agent_restore_state` capture and restore the
//! full runtime (save data + screen + overworld/battle internals +
//! frame counters + RNG state) as a serde [`GameSnapshot`]; restoring
//! is bit-for-bit — identical inputs afterwards produce identical
//! frames. Scope boundary: only overworld and battle screens can be
//! captured; menus/shops and mid-movie takeovers error cleanly.

use pokered_core::game_state::GameScreen;
use pokered_core::save::SaveData;
use pokered_core::snapshot::{BattleSnapshot, OverworldSnapshot};
use serde::{Deserialize, Serialize};

use crate::game::PokemonGame;

/// Which screen a [`GameSnapshot`] was taken on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameScreenTag {
    Overworld,
    Battle,
}

/// The full runtime snapshot for frame-level fork/restore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSnapshot {
    pub version: u32,
    pub frame_count: u64,
    pub screen: GameScreenTag,
    pub save_data: SaveData,
    pub overworld: OverworldSnapshot,
    pub battle: BattleSnapshot,
    /// The active seed, if determinism was pinned via `set_seed`.
    pub seed: Option<u64>,
    /// Battles started so far (feeds per-battle RNG seeding).
    pub battle_count: u64,
}

/// FNV-1a 64-bit hash of the snapshot's canonical JSON, hex-formatted.
/// Cheap, stable, and good enough for identity assertions in tests and
/// over the wire (not a security hash).
///
/// The JSON is canonicalized through `serde_json::Value` first (its
/// maps are BTreeMaps): plain serialization of the snapshot's HashMaps
/// (flags, script sets) iterates in arbitrary order and would produce a
/// different string — and a different hash — for identical state.
pub fn snapshot_hash(snapshot: &GameSnapshot) -> String {
    let value = serde_json::to_value(snapshot).expect("snapshot serializes");
    let json = serde_json::to_string(&value).expect("canonical json");
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in json.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

/// Why a save/restore was rejected (wire string in `error`).
#[derive(Debug)]
pub enum StateError {
    /// The current screen is not overworld/battle.
    UnsupportedScreen(String),
    /// A movie takeover (trade/evolution/HoF/credits) is active.
    MidMovie(String),
    /// No snapshot in the requested slot.
    EmptySlot(u8),
}

impl StateError {
    pub fn message(&self) -> String {
        match self {
            StateError::UnsupportedScreen(screen) => {
                format!("save_state is only supported in overworld/battle (current: {screen})")
            }
            StateError::MidMovie(what) => {
                format!("save_state is unsupported mid-movie ({what} active)")
            }
            StateError::EmptySlot(slot) => format!("no snapshot in slot {slot}"),
        }
    }
}

impl PokemonGame {
    /// Pin determinism: replace the overworld and battle RNG streams
    /// with seeded ChaCha12 streams (the battle stream is
    /// domain-separated by a golden-ratio offset). Default (no seed)
    /// stays entropy-based.
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = Some(seed);
        self.overworld.set_rng_seed(seed);
        self.battle.rng =
            pokered_core::battle::pokered_rules::runtime::StdBattleRng::from_seed(
                seed.wrapping_add(0x9E3779B97F4A7C15),
            );
    }

    fn screen_tag(&self) -> Result<GameScreenTag, StateError> {
        match self.state.screen {
            GameScreen::Overworld => Ok(GameScreenTag::Overworld),
            GameScreen::Battle => Ok(GameScreenTag::Battle),
            _ => Err(StateError::UnsupportedScreen(format!(
                "{:?}",
                self.state.screen
            ))),
        }
    }

    fn check_not_mid_movie(&self) -> Result<(), StateError> {
        if self.trade_anim.is_some() {
            return Err(StateError::MidMovie("trade".to_string()));
        }
        if self.evolution_anim.is_some() {
            return Err(StateError::MidMovie("evolution".to_string()));
        }
        if self.hof_ceremony.is_some() {
            return Err(StateError::MidMovie("hall_of_fame".to_string()));
        }
        if self.credits.is_some() {
            return Err(StateError::MidMovie("credits".to_string()));
        }
        Ok(())
    }

    /// Capture the full runtime into a serde snapshot. Script-requested
    /// data mutations queued on the overworld are flushed first, so no
    /// committed effect is lost sitting in the queue.
    pub fn agent_save_state(&mut self) -> Result<GameSnapshot, StateError> {
        let screen = self.screen_tag()?;
        self.check_not_mid_movie()?;
        self.apply_overworld_game_data_requests();
        Ok(GameSnapshot {
            version: 1,
            frame_count: self.frame_count,
            screen,
            save_data: self.save_data.clone(),
            overworld: OverworldSnapshot::capture(&self.overworld),
            battle: BattleSnapshot::capture(&self.battle),
            seed: self.seed,
            battle_count: self.battle_count,
        })
    }

    /// Capture and store into an in-memory slot (JSON string, so the
    /// restore path exercises the same serialization the wire uses).
    pub fn agent_save_state_slot(&mut self, slot: u8) -> Result<String, StateError> {
        let snapshot = self.agent_save_state()?;
        let hash = snapshot_hash(&snapshot);
        let json = serde_json::to_string(&snapshot).expect("snapshot serializes");
        self.agent_state_slots.insert(slot, json);
        Ok(hash)
    }

    /// Restore from an in-memory slot. Bit-for-bit: afterwards the game
    /// is exactly where the snapshot left it.
    pub fn agent_restore_state_slot(&mut self, slot: u8) -> Result<String, StateError> {
        let Some(json) = self.agent_state_slots.get(&slot).cloned() else {
            return Err(StateError::EmptySlot(slot));
        };
        let snapshot: GameSnapshot = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("snapshot deserialize failed: {e}"));
        let hash = snapshot_hash(&snapshot);
        self.agent_restore_state(&snapshot)?;
        Ok(hash)
    }

    /// Restore a snapshot into this game. Mid-movie takeovers and queued
    /// debug inputs are cleared; battle VFX restarts (presentation).
    pub fn agent_restore_state(&mut self, snapshot: &GameSnapshot) -> Result<(), StateError> {
        self.check_not_mid_movie()?;
        self.state.screen = match snapshot.screen {
            GameScreenTag::Overworld => GameScreen::Overworld,
            GameScreenTag::Battle => GameScreen::Battle,
        };
        self.frame_count = snapshot.frame_count;
        self.save_data = snapshot.save_data.clone();
        snapshot.overworld.restore_into(&mut self.overworld);
        snapshot.battle.restore_into(&mut self.battle);
        self.seed = snapshot.seed;
        self.battle_count = snapshot.battle_count;
        self.player_name = snapshot.overworld.player_name.clone();
        self.rival_name = snapshot.overworld.rival_name.clone();
        // Queued inputs belong to the pre-fork timeline; drop them.
        self.pending_debug_inputs.clear();
        self.pending_debug_frames = 0;
        // Presentation state restarts (not part of the logic snapshot).
        self.battle_vfx = Default::default();
        self.trade_anim = None;
        self.evolution_anim = None;
        self.hof_ceremony = None;
        self.credits = None;
        Ok(())
    }
}
