//! Save/load for `dotzuki run`: [`GameSave`], a versioned JSON snapshot.
//!
//! The save lives at `<project>/.dotzuki-save.json` (a dotfile in the project
//! dir; override with `--save-file`). It captures only *stable* overworld
//! state — current map, player tile + facing, persistent story flags,
//! language — never a suspended scene engine, so it is written at stable
//! points (see [`crate::game::RunnerGame`]) and resumes into a fresh
//! overworld.
//!
//! A plain versioned serde JSON file was chosen over the engine's binary
//! slot/CRC16 framework (`dotzuki_engine::save`): one file, human-readable,
//! trivially debuggable, and corruption tolerance is handled by the
//! load-then-validate path below. Loading never fails hard: a missing,
//! unreadable, corrupt or version-mismatched file logs a warning and
//! returns `None`, and the game boots fresh.

use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Current save format version. Saves from a NEWER version are ignored
/// (fresh boot); older saves load with per-field defaults (v1 has no party
/// or inventory — both start fresh; v1/v2 have no money — the manifest's
/// `shop.startMoney` default applies; party members without `level`/`exp`
/// default to level 1 / 0 EXP — the levels fields are optional, so the
/// version stays 3).
pub const SAVE_VERSION: u32 = 3;

/// Default save file name in the project directory.
pub const DEFAULT_SAVE_FILE: &str = ".dotzuki-save.json";

/// Saved player state (tile coordinates + facing + elevation level).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSave {
    /// Tile X.
    pub x: i32,
    /// Tile Y.
    pub y: i32,
    /// Facing (`"down"` / `"up"` / `"left"` / `"right"`).
    #[serde(default = "default_facing")]
    pub facing: String,
    /// Elevation level on the map (multi-level maps); absent ⇒ 0 (ground).
    #[serde(default, skip_serializing_if = "is_ground_level")]
    pub level: u8,
}

fn default_facing() -> String {
    "down".to_string()
}

fn is_ground_level(v: &u8) -> bool {
    *v == 0
}

/// One party member's persistent battle state (v2): current HP/MP and the
/// non-volatile status (a `kind: Status` record id) carried between battles.
/// Base stats are NOT saved — they are rebuilt from the records each battle.
/// With a `battle.levels` block, `level`/`exp` ride along as OPTIONAL fields
/// (the version stays 3: a save without them simply defaults to level 1 /
/// 0 EXP, and older tooling ignores unknown keys).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyMemberSave {
    /// Party record id.
    pub id: String,
    /// Current HP (0 = fainted until healed).
    pub hp: u32,
    /// Current MP (resource pool).
    pub mp: u32,
    /// The persistent status record id, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Current level (`battle.levels`); absent ⇒ 1.
    #[serde(default = "default_level", skip_serializing_if = "is_default_level")]
    pub level: u8,
    /// EXP progress toward the next level; absent ⇒ 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub exp: u32,
}

fn default_level() -> u8 {
    1
}
fn is_default_level(v: &u8) -> bool {
    *v == 1
}
fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// A `dotzuki run` save file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSave {
    /// Format version; must be `<=` [`SAVE_VERSION`] to load (v1/v2 files
    /// parse fine — the newer fields simply default to absent).
    pub version: u32,
    /// Current map id; `None` for a dialogue-only (map-less) project.
    pub map: Option<String>,
    /// Player tile + facing.
    pub player: PlayerSave,
    /// Persistent story flags (cross-scene truth), restored verbatim.
    #[serde(default)]
    pub flags: HashMap<String, bool>,
    /// UI/script language at save time (informational; `--lang` wins).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Persistent party state (v2); absent ⇒ fresh party at the first battle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party: Option<Vec<PartyMemberSave>>,
    /// Persistent battle inventory (v2); absent ⇒ the manifest's
    /// `battle.items.starting` counts at the first battle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory: Option<HashMap<String, u32>>,
    /// The player's money (v3); absent ⇒ the manifest's `shop.startMoney`
    /// (or its default) applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub money: Option<u32>,
}

impl GameSave {
    /// Serialize as pretty JSON — the platform-neutral form a WASM shell
    /// persists to localStorage (see [`RunnerGame::export_save`]).
    ///
    /// [`RunnerGame::export_save`]: crate::game::RunnerGame::export_save
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("save serialization is infallible")
    }

    /// Parse a save from JSON (no version check — callers compare
    /// `version` against [`SAVE_VERSION`]).
    ///
    /// # Errors
    ///
    /// Fails when the text is not a valid [`GameSave`] JSON document.
    pub fn from_json(text: &str) -> Result<Self> {
        serde_json::from_str(text).context("failed to parse save JSON")
    }

    /// Read and validate a save file.
    ///
    /// Returns `None` — with a warning for anything but a simply-absent
    /// file — when the file is missing, unreadable, unparseable, or from a
    /// NEWER version: a bad save never crashes the boot. Older versions load
    /// with per-field defaults (a v1 save has no party/inventory, so both
    /// start fresh).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(path: &Path) -> Option<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => {
                log::warn!("save: cannot read {}: {e} — starting fresh", path.display());
                return None;
            }
        };
        match Self::from_json(&text) {
            Ok(save) if save.version <= SAVE_VERSION => Some(save),
            Ok(save) => {
                log::warn!(
                    "save: {} is version {} (newer than {SAVE_VERSION}) — starting fresh",
                    path.display(),
                    save.version
                );
                None
            }
            Err(e) => {
                log::warn!(
                    "save: {} is corrupt ({e:#}) — starting fresh",
                    path.display()
                );
                None
            }
        }
    }

    /// Write the save as pretty JSON, via a temp file + rename so a crash
    /// mid-write can't leave a truncated save behind.
    ///
    /// # Errors
    ///
    /// Fails when the temp file can't be written or renamed into place.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn write(&self, path: &Path) -> Result<()> {
        let text = self.to_json();
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = Path::new(&tmp);
        std::fs::write(tmp, text).with_context(|| format!("failed to write {}", tmp.display()))?;
        std::fs::rename(tmp, path)
            .with_context(|| format!("failed to rename save into {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    /// A unique temp save path (no file written unless the test writes one).
    fn save_path(test: &str) -> std::path::PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "dotzuki-runner-save-{test}-{}-{id}.json",
            std::process::id()
        ))
    }

    fn base_save() -> GameSave {
        GameSave {
            version: SAVE_VERSION,
            map: Some("Town".to_string()),
            player: PlayerSave {
                x: 3,
                y: 4,
                facing: "down".to_string(),
                level: 0,
            },
            flags: HashMap::new(),
            lang: None,
            party: None,
            inventory: None,
            money: None,
        }
    }

    #[test]
    fn v2_round_trip_with_party_and_inventory() {
        let path = save_path("roundtrip");
        let mut save = base_save();
        save.party = Some(vec![
            PartyMemberSave {
                id: "aria".to_string(),
                hp: 32,
                mp: 20,
                status: Some("poison".to_string()),
                level: 1,
                exp: 0,
            },
            PartyMemberSave {
                id: "bryn".to_string(),
                hp: 0,
                mp: 55,
                status: None,
                level: 1,
                exp: 0,
            },
        ]);
        save.inventory = Some(HashMap::from([("potion".to_string(), 2)]));
        save.write(&path).expect("write");
        let loaded = GameSave::load(&path).expect("v2 save loads");
        let party = loaded.party.expect("party");
        assert_eq!(party.len(), 2);
        assert_eq!(
            (party[0].id.as_str(), party[0].hp, party[0].mp),
            ("aria", 32, 20)
        );
        assert_eq!(party[0].status.as_deref(), Some("poison"));
        assert_eq!((party[1].id.as_str(), party[1].hp), ("bryn", 0));
        assert_eq!(loaded.inventory.as_ref().unwrap().get("potion"), Some(&2));
        assert_eq!(loaded.money, None, "v2-shaped save ⇒ money defaults");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn v3_round_trip_with_money() {
        let path = save_path("v3money");
        let mut save = base_save();
        save.money = Some(750);
        save.write(&path).expect("write");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("\"version\": 3"),
            "written with the current version: {text}"
        );
        let loaded = GameSave::load(&path).expect("v3 save loads");
        assert_eq!(loaded.version, 3);
        assert_eq!(loaded.money, Some(750));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn v2_json_loads_with_money_defaulted() {
        // A hand-written v2 save (the previous shape: no money field).
        let path = save_path("v2");
        std::fs::write(
            &path,
            r#"{
  "version": 2,
  "map": "Town",
  "player": { "x": 1, "y": 2, "facing": "up" },
  "flags": { "MET_GUIDE": true },
  "party": [{ "id": "aria", "hp": 32, "mp": 20 }],
  "inventory": { "potion": 2 }
}"#,
        )
        .unwrap();
        let loaded = GameSave::load(&path).expect("v2 save must still load");
        assert_eq!(loaded.version, 2);
        assert_eq!(loaded.party.as_ref().unwrap()[0].hp, 32);
        assert_eq!(loaded.inventory.as_ref().unwrap().get("potion"), Some(&2));
        assert_eq!(loaded.money, None, "v2 ⇒ money defaults at boot");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn v1_json_loads_with_defaults() {
        // A hand-written v1 save (the old shape: no party/inventory fields).
        let path = save_path("v1");
        std::fs::write(
            &path,
            r#"{
  "version": 1,
  "map": "Town",
  "player": { "x": 1, "y": 2, "facing": "up" },
  "flags": { "MET_GUIDE": true },
  "lang": "en"
}"#,
        )
        .unwrap();
        let loaded = GameSave::load(&path).expect("v1 save must still load");
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.map.as_deref(), Some("Town"));
        assert!(loaded.flags.get("MET_GUIDE").copied().unwrap_or(false));
        assert!(loaded.party.is_none(), "v1 ⇒ fresh party");
        assert!(loaded.inventory.is_none(), "v1 ⇒ fresh inventory");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn party_member_level_exp_round_trip_and_defaults() {
        // level/exp ride the save when a levels block produced them…
        let path = save_path("levels");
        let mut save = base_save();
        save.party = Some(vec![PartyMemberSave {
            id: "aria".to_string(),
            hp: 63,
            mp: 21,
            status: None,
            level: 2,
            exp: 3,
        }]);
        save.write(&path).expect("write");
        let loaded = GameSave::load(&path).expect("save loads");
        let aria = &loaded.party.expect("party")[0];
        assert_eq!((aria.level, aria.exp), (2, 3));
        let _ = std::fs::remove_file(&path);

        // …and a member WITHOUT the fields (an older save shape) reads as
        // level 1 / 0 EXP — no version bump needed.
        let path = save_path("levelsdefault");
        std::fs::write(
            &path,
            r#"{
  "version": 3,
  "map": "Town",
  "player": { "x": 1, "y": 2 },
  "party": [{ "id": "aria", "hp": 32, "mp": 20 }]
}"#,
        )
        .unwrap();
        let loaded = GameSave::load(&path).expect("save without level/exp loads");
        let aria = &loaded.party.expect("party")[0];
        assert_eq!((aria.level, aria.exp), (1, 0));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn player_level_round_trip_and_default() {
        // A non-zero elevation level rides the save…
        let path = save_path("playerlevel");
        let mut save = base_save();
        save.player.level = 2;
        save.write(&path).expect("write");
        let loaded = GameSave::load(&path).expect("save loads");
        assert_eq!(loaded.player.level, 2);
        let _ = std::fs::remove_file(&path);

        // …and a save without the field (the older shape) reads as ground.
        let path = save_path("playerleveldefault");
        std::fs::write(
            &path,
            r#"{ "version": 3, "map": "Town", "player": { "x": 1, "y": 2 } }"#,
        )
        .unwrap();
        let loaded = GameSave::load(&path).expect("save without level loads");
        assert_eq!(loaded.player.level, 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn future_version_is_a_fresh_boot() {
        let path = save_path("v99");
        std::fs::write(
            &path,
            r#"{ "version": 99, "map": "Town", "player": { "x": 0, "y": 0 } }"#,
        )
        .unwrap();
        assert!(GameSave::load(&path).is_none(), "version 99 ⇒ fresh boot");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corrupt_save_is_a_fresh_boot() {
        let path = save_path("corrupt");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(GameSave::load(&path).is_none());
        let _ = std::fs::remove_file(&path);
    }
}
