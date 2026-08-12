//! Native (Boa-free) script engine for pokered overworld scenes.
//!
//! Drives the DSL AST interpreter (`dotzuki_engine_dsl::interpreter`) with a
//! Pokémon-specific [`NativeHost`] that mirrors the `game` global of the Boa
//! path (`dotzuki-engine-script`'s core API + `pokered-data::script_api`):
//! async effects become [`ScriptCommand`]s dispatched by the overworld
//! driver; sync queries (`getFlag`, `hasItem`, `getMoney`, …) answer from
//! bridge state seeded by the app layer each frame.
//!
//! Protocol compatibility with the Boa engine (`dotzuki_engine_script::ScriptEngine`):
//! `load_*` → `tick` → (dispatch) → `signal_done` → … — the overworld glue in
//! `screen.rs` / `update.rs` calls the same surface, so swapping engines does
//! not change the driver. Scene ASTs come from `pokered-data`'s embedded
//! `SCENE_ASTS` table (or a disk `SceneAstProvider` with `--scripts-dir`).
//!
//! The one `@run` block in the game (VermilionGym's trash-can puzzle) is
//! ported as a native handler ([`VgymTrashState`]) registered under the
//! `storyline_trashCans` function.

use std::collections::{HashMap, VecDeque};

use dotzuki_engine_dsl::ast::{GameScene, StoryStmt};
use dotzuki_engine_dsl::interpreter::{HostCall, Interpreter, InterpState, ScriptHost, Value};
use dotzuki_engine_script::{CommandResult, ScriptCommand};

/// Non-zero default seed (a common splitmix64/golden-ratio constant) —
/// matches `dotzuki_engine_script::engine::DEFAULT_RNG_SEED` so `seed_rng` /
/// `mix_rng` semantics are identical to the Boa bridge.
const DEFAULT_RNG_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// Map a badge constant name (case-insensitive) to its bitfield index (0..7),
/// mirroring `pokered_data::script_api::badge_index`.
fn badge_index(name: &str) -> Option<u8> {
    match name.to_ascii_uppercase().as_str() {
        "BOULDERBADGE" => Some(0),
        "CASCADEBADGE" => Some(1),
        "THUNDERBADGE" => Some(2),
        "RAINBOWBADGE" => Some(3),
        "SOULBADGE" => Some(4),
        "MARSHBADGE" => Some(5),
        "VOLCANOBADGE" => Some(6),
        "EARTHBADGE" => Some(7),
        _ => None,
    }
}

/// Argument conversion helpers — all fail with a descriptive message,
/// mirroring the Boa registrar closures' type errors.
mod args {
    use super::Value;

    /// JS `String()` coercion: numbers/bools stringify (the Boa registrar
    /// converts every argument via `JsValue::to_string`, so `0` arrives as
    /// `"0"` — e.g. `showEmotionBubble(id, 0)`).
    pub fn text(v: &Value, what: &str) -> Result<String, String> {
        match v {
            Value::Text(s) => Ok(s.clone()),
            Value::Number(n) => Ok(format!("{}", n)),
            Value::Bool(b) => Ok(if *b { "true".to_string() } else { "false".to_string() }),
            other => Err(format!("{what}: expected string, got {}", other.type_name())),
        }
    }

    pub fn number(v: &Value, what: &str) -> Result<f64, String> {
        match v {
            Value::Number(n) => Ok(*n),
            other => Err(format!("{what}: expected number, got {}", other.type_name())),
        }
    }

    pub fn u8(v: &Value, what: &str) -> Result<u8, String> {
        number(v, what).map(|n| n as u8)
    }

    pub fn u16(v: &Value, what: &str) -> Result<u16, String> {
        number(v, what).map(|n| n as u16)
    }

    pub fn u32(v: &Value, what: &str) -> Result<u32, String> {
        number(v, what).map(|n| n as u32)
    }

    pub fn string_array(v: &Value, what: &str) -> Result<Vec<String>, String> {
        match v {
            Value::Array(items) => items
                .iter()
                .map(|i| text(i, &format!("{what} element")))
                .collect(),
            other => Err(format!("{what}: expected array, got {}", other.type_name())),
        }
    }

    pub fn path(v: &Value, what: &str) -> Result<Vec<(u8, u8)>, String> {
        match v {
            Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for (i, p) in items.iter().enumerate() {
                    match p {
                        Value::Array(xy) if xy.len() == 2 => {
                            out.push((u8(&xy[0], &format!("{what}[{i}].x"))?, u8(&xy[1], &format!("{what}[{i}].y"))?));
                        }
                        other => {
                            return Err(format!(
                                "{what}[{i}]: expected [x, y] pair, got {}",
                                other.type_name()
                            ))
                        }
                    }
                }
                Ok(out)
            }
            other => Err(format!("{what}: expected array, got {}", other.type_name())),
        }
    }

    /// `movePlayerRelative` steps: `[dx, dy]` pairs or direction strings.
    pub fn relative_steps(v: &Value, what: &str) -> Result<Vec<(i16, i16)>, String> {
        match v {
            Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for (i, p) in items.iter().enumerate() {
                    match p {
                        Value::Text(dir) => {
                            let (dx, dy) = match dir.to_ascii_lowercase().as_str() {
                                "up" | "north" => (0i16, -1i16),
                                "down" | "south" => (0, 1),
                                "left" | "west" => (-1, 0),
                                "right" | "east" => (1, 0),
                                other => {
                                    return Err(format!(
                                        "{what}[{i}]: unknown direction '{other}'"
                                    ))
                                }
                            };
                            out.push((dx, dy));
                        }
                        Value::Array(xy) if xy.len() == 2 => {
                            let dx = number(&xy[0], &format!("{what}[{i}].dx"))? as i16;
                            let dy = number(&xy[1], &format!("{what}[{i}].dy"))? as i16;
                            out.push((dx, dy));
                        }
                        other => {
                            return Err(format!(
                                "{what}[{i}]: expected direction string or [dx, dy], got {}",
                                other.type_name()
                            ))
                        }
                    }
                }
                Ok(out)
            }
            other => Err(format!("{what}: expected array, got {}", other.type_name())),
        }
    }
}

/// Bridge state + `game.*` dispatch for the native interpreter. Mirrors
/// `dotzuki_engine_script::SharedBridge` (seeded query state) and the registrars
/// of `dotzuki-engine-script/src/engine.rs` + `pokered-data/src/script_api.rs`.
pub struct NativeHost {
    flags: HashMap<String, bool>,
    numbers: HashMap<String, f64>,
    texts: HashMap<String, String>,
    sets: HashMap<String, Vec<String>>,
    player_x: u8,
    player_y: u8,
    lang: String,
    rng_state: u64,
}

impl NativeHost {
    fn new() -> Self {
        Self {
            flags: HashMap::new(),
            numbers: HashMap::new(),
            texts: HashMap::new(),
            sets: HashMap::new(),
            player_x: 0,
            player_y: 0,
            lang: "en".to_string(),
            rng_state: DEFAULT_RNG_SEED,
        }
    }

    /// Advance the internal xorshift64 RNG and return the next 64-bit value
    /// (identical to `SharedBridge::next_rand`).
    fn next_rand(&mut self) -> u64 {
        let mut x = self.rng_state;
        if x == 0 {
            x = DEFAULT_RNG_SEED;
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        x
    }

    fn pick_random_text(&mut self, options: Vec<String>) -> String {
        if options.is_empty() {
            return String::new();
        }
        let idx = (self.next_rand() % options.len() as u64) as usize;
        options.into_iter().nth(idx).unwrap_or_default()
    }
}

impl ScriptHost for NativeHost {
    fn call(&mut self, name: &str, v: &[Value]) -> Result<HostCall, String> {
        match name {
            // ── sync flag queries/mutations ──────────────────────────────
            "getFlag" => {
                let flag = args::text(v.first().ok_or("getFlag: missing flag")?, "getFlag")?;
                Ok(HostCall::Value(Value::Bool(
                    self.flags.get(&flag).copied().unwrap_or(false),
                )))
            }
            "setFlag" => {
                let flag = args::text(v.first().ok_or("setFlag: missing flag")?, "setFlag")?;
                self.flags.insert(flag, true);
                Ok(HostCall::Value(Value::Undefined))
            }
            "resetFlag" => {
                let flag = args::text(v.first().ok_or("resetFlag: missing flag")?, "resetFlag")?;
                self.flags.insert(flag, false);
                Ok(HostCall::Value(Value::Undefined))
            }

            // ── seeded sync queries (pokered-data::script_api) ───────────
            "hasItem" => {
                let name = args::text(v.first().ok_or("hasItem: missing item")?, "hasItem")?;
                Ok(HostCall::Value(Value::Bool(
                    self.sets.get("bag").is_some_and(|s| s.iter().any(|i| *i == name)),
                )))
            }
            "getMoney" => Ok(HostCall::Value(Value::Number(
                self.numbers.get("money").copied().unwrap_or(0.0),
            ))),
            "hasMoney" => {
                let needed = args::number(v.first().ok_or("hasMoney: missing amount")?, "hasMoney")?;
                let money = self.numbers.get("money").copied().unwrap_or(0.0);
                Ok(HostCall::Value(Value::Bool(money >= needed)))
            }
            "getPokedexOwnedCount" => Ok(HostCall::Value(Value::Number(
                self.numbers.get("pokedexOwned").copied().unwrap_or(0.0),
            ))),
            "getPokedexSeenCount" => Ok(HostCall::Value(Value::Number(
                self.numbers.get("pokedexSeen").copied().unwrap_or(0.0),
            ))),
            "getPlayerFacing" => Ok(HostCall::Value(Value::Text(
                self.texts.get("playerFacing").cloned().unwrap_or_default(),
            ))),
            "getRivalStarter" => Ok(HostCall::Value(Value::Number(
                self.numbers.get("rivalStarter").copied().unwrap_or(0.0),
            ))),
            "getBadgeCount" => {
                let badges = self.numbers.get("obtainedBadges").copied().unwrap_or(0.0) as u8;
                Ok(HostCall::Value(Value::Number(badges.count_ones() as f64)))
            }
            "hasBadge" => {
                let badge = args::text(v.first().ok_or("hasBadge: missing badge")?, "hasBadge")?;
                let idx = badge_index(&badge)
                    .ok_or_else(|| format!("hasBadge: unknown badge '{badge}'"))?;
                let badges = self.numbers.get("obtainedBadges").copied().unwrap_or(0.0) as u8;
                Ok(HostCall::Value(Value::Bool(badges & (1 << idx) != 0)))
            }
            "getCoins" => Ok(HostCall::Value(Value::Number(
                self.numbers.get("coins").copied().unwrap_or(0.0),
            ))),
            "hasCoins" => {
                let needed = args::number(v.first().ok_or("hasCoins: missing amount")?, "hasCoins")?;
                let coins = self.numbers.get("coins").copied().unwrap_or(0.0);
                Ok(HostCall::Value(Value::Bool(coins >= needed)))
            }
            "isDaycareInUse" => Ok(HostCall::Value(Value::Bool(
                self.numbers.get("daycareInUse").copied().unwrap_or(0.0) != 0.0,
            ))),
            "getDaycareMonName" => Ok(HostCall::Value(Value::Text(
                self.texts.get("daycareMonName").cloned().unwrap_or_default(),
            ))),
            "getDaycareLevelsGrown" => Ok(HostCall::Value(Value::Number(
                self.numbers.get("daycareLevelsGrown").copied().unwrap_or(0.0),
            ))),
            "getDaycareCost" => Ok(HostCall::Value(Value::Number(
                self.numbers.get("daycareCost").copied().unwrap_or(0.0),
            ))),
            "getPartyCount" => Ok(HostCall::Value(Value::Number(
                self.numbers.get("partyCount").copied().unwrap_or(0.0),
            ))),
            "getPartyMonName" => {
                let idx = args::u32(v.first().ok_or("getPartyMonName: missing index")?, "getPartyMonName")?;
                Ok(HostCall::Value(Value::Text(
                    self.texts
                        .get(&format!("partyName{}", idx))
                        .cloned()
                        .unwrap_or_default(),
                )))
            }
            "partyMonKnowsHm" => {
                let idx = args::u32(v.first().ok_or("partyMonKnowsHm: missing index")?, "partyMonKnowsHm")?;
                Ok(HostCall::Value(Value::Bool(
                    self.numbers.get(&format!("partyKnowsHm{}", idx)).copied().unwrap_or(0.0) != 0.0,
                )))
            }
            "getPlayerX" => Ok(HostCall::Value(Value::Number(self.player_x as f64))),
            "getPlayerY" => Ok(HostCall::Value(Value::Number(self.player_y as f64))),
            "getPlayerPosition" => Err(
                "getPlayerPosition returns an object, which the native interpreter does not \
                 support (only the VermilionGym @run block used it; it is ported natively)"
                    .to_string(),
            ),
            "lang" => Ok(HostCall::Value(Value::Text(self.lang.clone()))),
            "t" => {
                let en = args::text(v.first().ok_or("t: missing en")?, "t")?;
                let zh = args::text(v.get(1).ok_or("t: missing zh")?, "t")?;
                Ok(HostCall::Value(Value::Text(if self.lang == "zh" { zh } else { en })))
            }

            // ── showRandomText: picks from a pool via the bridge RNG ─────
            "showRandomText" => {
                let options: Vec<String> = if v.len() == 1 && matches!(v[0], Value::Array(_)) {
                    args::string_array(&v[0], "showRandomText")?
                } else {
                    v.iter()
                        .map(|x| args::text(x, "showRandomText option"))
                        .collect::<Result<_, _>>()?
                };
                let text = self.pick_random_text(options);
                Ok(HostCall::Command(ScriptCommand::ShowText { text }))
            }

            // ── async commands: build a ScriptCommand for the driver ──────
            "showText" => {
                let text = args::text(v.first().ok_or("showText: missing text")?, "showText")?;
                Ok(HostCall::Command(ScriptCommand::ShowText { text }))
            }
            "showChoice" => {
                let options = args::string_array(v.first().ok_or("showChoice: missing options")?, "showChoice")?;
                Ok(HostCall::Command(ScriptCommand::ShowChoice { options }))
            }
            "giveItem" => {
                let item_id = args::text(v.first().ok_or("giveItem: missing item")?, "giveItem")?;
                let quantity = args::u8(v.get(1).ok_or("giveItem: missing quantity")?, "giveItem")?;
                Ok(HostCall::Command(ScriptCommand::GiveItem { item_id, quantity }))
            }
            "takeItem" => {
                let item_id = args::text(v.first().ok_or("takeItem: missing item")?, "takeItem")?;
                let quantity = args::u8(v.get(1).ok_or("takeItem: missing quantity")?, "takeItem")?;
                Ok(HostCall::Command(ScriptCommand::TakeItem { item_id, quantity }))
            }
            "givePokemon" => {
                let species = args::text(v.first().ok_or("givePokemon: missing species")?, "givePokemon")?;
                let level = args::u8(v.get(1).ok_or("givePokemon: missing level")?, "givePokemon")?;
                Ok(HostCall::Command(ScriptCommand::GivePokemon { species, level }))
            }
            "startBattle" => {
                let trainer_id = args::text(v.first().ok_or("startBattle: missing trainer")?, "startBattle")?;
                Ok(HostCall::Command(ScriptCommand::StartBattle { trainer_id }))
            }
            "startWildBattle" => {
                let species = args::text(v.first().ok_or("startWildBattle: missing species")?, "startWildBattle")?;
                let level = args::u8(v.get(1).ok_or("startWildBattle: missing level")?, "startWildBattle")?;
                Ok(HostCall::Command(ScriptCommand::StartWildBattle { species, level }))
            }
            "oldManTutorial" => Ok(HostCall::Command(ScriptCommand::OldManTutorial)),
            "tradePokemon" => {
                let offered = args::text(v.first().ok_or("tradePokemon: missing offered")?, "tradePokemon")?;
                let received = args::text(v.get(1).ok_or("tradePokemon: missing received")?, "tradePokemon")?;
                let nickname = args::text(v.get(2).ok_or("tradePokemon: missing nickname")?, "tradePokemon")?;
                Ok(HostCall::Command(ScriptCommand::TradePokemon {
                    offered,
                    received,
                    nickname,
                }))
            }
            "showPokedexEntry" => {
                let species = args::text(v.first().ok_or("showPokedexEntry: missing species")?, "showPokedexEntry")?;
                Ok(HostCall::Command(ScriptCommand::ShowPokedexEntry { species }))
            }
            "giveMoney" => {
                let amount = args::u32(v.first().ok_or("giveMoney: missing amount")?, "giveMoney")?;
                Ok(HostCall::Command(ScriptCommand::GiveMoney { amount }))
            }
            "takeMoney" => {
                let amount = args::u32(v.first().ok_or("takeMoney: missing amount")?, "takeMoney")?;
                Ok(HostCall::Command(ScriptCommand::TakeMoney { amount }))
            }
            "replaceTileBlock" => {
                let x = args::u8(v.first().ok_or("replaceTileBlock: missing x")?, "replaceTileBlock")?;
                let y = args::u8(v.get(1).ok_or("replaceTileBlock: missing y")?, "replaceTileBlock")?;
                let block_id = args::u8(v.get(2).ok_or("replaceTileBlock: missing block")?, "replaceTileBlock")?;
                Ok(HostCall::Command(ScriptCommand::ReplaceTileBlock { x, y, block_id }))
            }
            "playCry" => {
                let species = args::text(v.first().ok_or("playCry: missing species")?, "playCry")?;
                Ok(HostCall::Command(ScriptCommand::PlayCry { species }))
            }
            "giveBadge" => {
                let badge = match v.first().ok_or("giveBadge: missing badge")? {
                    Value::Number(n) => *n as u8,
                    other => {
                        let name = args::text(other, "giveBadge")?;
                        badge_index(&name)
                            .ok_or_else(|| format!("giveBadge: unknown badge '{name}'"))?
                    }
                };
                Ok(HostCall::Command(ScriptCommand::GiveBadge { badge }))
            }
            "openSlots" => {
                let lucky = match v.first() {
                    Some(Value::Bool(b)) => *b,
                    _ => false,
                };
                Ok(HostCall::Command(ScriptCommand::OpenSlots { lucky }))
            }
            "elevatorMenu" => {
                let floors = args::string_array(v.first().ok_or("elevatorMenu: missing floors")?, "elevatorMenu")?;
                Ok(HostCall::Command(ScriptCommand::ElevatorMenu { floors }))
            }
            "filterBag" => {
                let item_ids = args::string_array(v.first().ok_or("filterBag: missing items")?, "filterBag")?;
                Ok(HostCall::Command(ScriptCommand::FilterBag { item_ids }))
            }
            "showDiploma" => Ok(HostCall::Command(ScriptCommand::ShowDiploma)),
            "openPC" => Ok(HostCall::Command(ScriptCommand::OpenPc {
                kind: "center".to_string(),
            })),
            "openItemPC" => Ok(HostCall::Command(ScriptCommand::OpenPc {
                kind: "items".to_string(),
            })),
            "openBillsPC" => Ok(HostCall::Command(ScriptCommand::OpenPc {
                kind: "bills".to_string(),
            })),
            "linkStart" => Ok(HostCall::Command(ScriptCommand::LinkStart)),
            "enterHallOfFame" => Ok(HostCall::Command(ScriptCommand::EnterHallOfFame)),
            "giveCoins" => {
                let amount = args::u32(v.first().ok_or("giveCoins: missing amount")?, "giveCoins")?.min(u16::MAX as u32) as u16;
                Ok(HostCall::Command(ScriptCommand::GiveCoins { amount }))
            }
            "takeCoins" => {
                let amount = args::u32(v.first().ok_or("takeCoins: missing amount")?, "takeCoins")?.min(u16::MAX as u32) as u16;
                Ok(HostCall::Command(ScriptCommand::TakeCoins { amount }))
            }
            "depositDaycare" => {
                let index = args::u8(v.first().ok_or("depositDaycare: missing index")?, "depositDaycare")?;
                Ok(HostCall::Command(ScriptCommand::DepositDaycare { index }))
            }
            "withdrawDaycare" => Ok(HostCall::Command(ScriptCommand::WithdrawDaycare)),

            // ── core engine commands (dotzuki-engine-script/src/engine.rs) ──
            "moveNpc" => {
                let npc_id = args::text(v.first().ok_or("moveNpc: missing npc")?, "moveNpc")?;
                let path = args::path(v.get(1).ok_or("moveNpc: missing path")?, "moveNpc")?;
                Ok(HostCall::Command(ScriptCommand::MoveNpc { npc_id, path }))
            }
            "startNpcMove" => {
                let npc_id = args::text(v.first().ok_or("startNpcMove: missing npc")?, "startNpcMove")?;
                let path = args::path(v.get(1).ok_or("startNpcMove: missing path")?, "startNpcMove")?;
                Ok(HostCall::Command(ScriptCommand::StartNpcMove { npc_id, path }))
            }
            "awaitNpcMove" => {
                let npc_id = args::text(v.first().ok_or("awaitNpcMove: missing npc")?, "awaitNpcMove")?;
                Ok(HostCall::Command(ScriptCommand::AwaitNpcMove { npc_id }))
            }
            "movePlayer" => {
                let path = args::path(v.first().ok_or("movePlayer: missing path")?, "movePlayer")?;
                Ok(HostCall::Command(ScriptCommand::MovePlayer { path }))
            }
            "movePlayerRelative" => {
                let steps = args::relative_steps(v.first().ok_or("movePlayerRelative: missing steps")?, "movePlayerRelative")?;
                Ok(HostCall::Command(ScriptCommand::MovePlayerRelative { steps }))
            }
            "moveNpcTo" => {
                let npc_id = args::text(v.first().ok_or("moveNpcTo: missing npc")?, "moveNpcTo")?;
                let x = args::u8(v.get(1).ok_or("moveNpcTo: missing x")?, "moveNpcTo")?;
                let y = args::u8(v.get(2).ok_or("moveNpcTo: missing y")?, "moveNpcTo")?;
                Ok(HostCall::Command(ScriptCommand::MoveNpcTo { npc_id, x, y }))
            }
            "startNpcMoveTo" => {
                let npc_id = args::text(v.first().ok_or("startNpcMoveTo: missing npc")?, "startNpcMoveTo")?;
                let x = args::u8(v.get(1).ok_or("startNpcMoveTo: missing x")?, "startNpcMoveTo")?;
                let y = args::u8(v.get(2).ok_or("startNpcMoveTo: missing y")?, "startNpcMoveTo")?;
                Ok(HostCall::Command(ScriptCommand::StartNpcMoveTo { npc_id, x, y }))
            }
            "movePlayerTo" => {
                let x = args::u8(v.first().ok_or("movePlayerTo: missing x")?, "movePlayerTo")?;
                let y = args::u8(v.get(1).ok_or("movePlayerTo: missing y")?, "movePlayerTo")?;
                Ok(HostCall::Command(ScriptCommand::MovePlayerTo { x, y }))
            }
            "faceNpc" => {
                let npc_id = args::text(v.first().ok_or("faceNpc: missing npc")?, "faceNpc")?;
                let direction = args::text(v.get(1).ok_or("faceNpc: missing direction")?, "faceNpc")?;
                Ok(HostCall::Command(ScriptCommand::FaceNpc { npc_id, direction }))
            }
            "facePlayer" => {
                let direction = args::text(v.first().ok_or("facePlayer: missing direction")?, "facePlayer")?;
                Ok(HostCall::Command(ScriptCommand::FacePlayer { direction }))
            }
            "setNpcFrame" => {
                let npc_id = args::text(v.first().ok_or("setNpcFrame: missing npc")?, "setNpcFrame")?;
                let frame = args::u8(v.get(1).ok_or("setNpcFrame: missing frame")?, "setNpcFrame")?;
                Ok(HostCall::Command(ScriptCommand::SetNpcFrame { npc_id, frame }))
            }
            "playMusic" => {
                let music_id = args::text(v.first().ok_or("playMusic: missing music")?, "playMusic")?;
                Ok(HostCall::Command(ScriptCommand::PlayMusic { music_id }))
            }
            "playSound" => {
                let sound_id = args::text(v.first().ok_or("playSound: missing sound")?, "playSound")?;
                Ok(HostCall::Command(ScriptCommand::PlaySound { sound_id }))
            }
            "playShipDeparture" => Ok(HostCall::Command(ScriptCommand::PlayShipDeparture)),
            "stopMusic" => Ok(HostCall::Command(ScriptCommand::StopMusic)),
            "fadeOutMusic" => Ok(HostCall::Command(ScriptCommand::FadeOutMusic)),
            "delay" => {
                let frames = args::u16(v.first().ok_or("delay: missing frames")?, "delay")?;
                Ok(HostCall::Command(ScriptCommand::Delay { frames }))
            }
            "warpTo" => {
                let map = args::text(v.first().ok_or("warpTo: missing map")?, "warpTo")?;
                let x = args::u8(v.get(1).ok_or("warpTo: missing x")?, "warpTo")?;
                let y = args::u8(v.get(2).ok_or("warpTo: missing y")?, "warpTo")?;
                Ok(HostCall::Command(ScriptCommand::WarpTo { map, x, y }))
            }
            "heal" => Ok(HostCall::Command(ScriptCommand::Heal)),
            "animateHealingMachine" => Ok(HostCall::Command(ScriptCommand::AnimateHealingMachine)),
            "fadeScreen" => {
                let fade_type = args::text(v.first().ok_or("fadeScreen: missing type")?, "fadeScreen")?;
                Ok(HostCall::Command(ScriptCommand::FadeScreen { fade_type }))
            }
            "showObject" => {
                let arg = v.first().ok_or("showObject: missing argument")?;
                match arg {
                    Value::Text(toggle_id) => Ok(HostCall::Command(ScriptCommand::ShowObjectByName {
                        toggle_id: toggle_id.clone(),
                    })),
                    Value::Number(_) => Ok(HostCall::Command(ScriptCommand::ShowObject {
                        object_index: args::u8(arg, "showObject")?,
                    })),
                    other => Err(format!("showObject: expected number or string, got {}", other.type_name())),
                }
            }
            "hideObject" => {
                let arg = v.first().ok_or("hideObject: missing argument")?;
                match arg {
                    Value::Text(toggle_id) => Ok(HostCall::Command(ScriptCommand::HideObjectByName {
                        toggle_id: toggle_id.clone(),
                    })),
                    Value::Number(_) => Ok(HostCall::Command(ScriptCommand::HideObject {
                        object_index: args::u8(arg, "hideObject")?,
                    })),
                    other => Err(format!("hideObject: expected number or string, got {}", other.type_name())),
                }
            }
            "showObjectByName" => {
                let toggle_id = args::text(v.first().ok_or("showObjectByName: missing id")?, "showObjectByName")?;
                Ok(HostCall::Command(ScriptCommand::ShowObjectByName { toggle_id }))
            }
            "hideObjectByName" => {
                let toggle_id = args::text(v.first().ok_or("hideObjectByName: missing id")?, "hideObjectByName")?;
                Ok(HostCall::Command(ScriptCommand::HideObjectByName { toggle_id }))
            }
            "setJoyIgnore" => {
                let mask = args::u8(v.first().ok_or("setJoyIgnore: missing mask")?, "setJoyIgnore")?;
                Ok(HostCall::Command(ScriptCommand::SetJoyIgnore { mask }))
            }
            "clearJoyIgnore" => Ok(HostCall::Command(ScriptCommand::ClearJoyIgnore)),
            "followNpc" => {
                let npc_id = args::text(v.first().ok_or("followNpc: missing npc")?, "followNpc")?;
                let target_x = args::u8(v.get(1).ok_or("followNpc: missing x")?, "followNpc")?;
                let target_y = args::u8(v.get(2).ok_or("followNpc: missing y")?, "followNpc")?;
                Ok(HostCall::Command(ScriptCommand::FollowNpc { npc_id, target_x, target_y }))
            }
            "openNamingScreen" => {
                let species = args::text(v.first().ok_or("openNamingScreen: missing species")?, "openNamingScreen")?;
                Ok(HostCall::Command(ScriptCommand::OpenNamingScreen { species }))
            }
            "choosePartyPokemon" => Ok(HostCall::Command(ScriptCommand::ChoosePartyPokemon)),
            "setPartyNickname" => {
                let index = args::u8(v.first().ok_or("setPartyNickname: missing index")?, "setPartyNickname")?;
                let nickname = args::text(v.get(1).ok_or("setPartyNickname: missing nickname")?, "setPartyNickname")?;
                Ok(HostCall::Command(ScriptCommand::SetPartyNickname { index, nickname }))
            }
            "openShop" => {
                let items = args::string_array(v.first().ok_or("openShop: missing items")?, "openShop")?;
                Ok(HostCall::Command(ScriptCommand::OpenShop { items }))
            }
            "showEmotionBubble" => {
                let npc_id = args::text(v.first().ok_or("showEmotionBubble: missing npc")?, "showEmotionBubble")?;
                let emotion = args::text(v.get(1).ok_or("showEmotionBubble: missing emotion")?, "showEmotionBubble")?;
                Ok(HostCall::Command(ScriptCommand::ShowEmotionBubble { npc_id, emotion }))
            }
            "setNpcPosition" => {
                let npc_id = args::text(v.first().ok_or("setNpcPosition: missing npc")?, "setNpcPosition")?;
                let x = args::u8(v.get(1).ok_or("setNpcPosition: missing x")?, "setNpcPosition")?;
                let y = args::u8(v.get(2).ok_or("setNpcPosition: missing y")?, "setNpcPosition")?;
                Ok(HostCall::Command(ScriptCommand::SetNpcPosition { npc_id, x, y }))
            }
            // dotzuki-engine UI/scene commands — never produced by pokered scenes;
            // accepted for parity with the Boa registrar.
            "showScene" => {
                let scene_name = args::text(v.first().ok_or("showScene: missing name")?, "showScene")?;
                Ok(HostCall::Command(ScriptCommand::ShowScene {
                    scene_name,
                    layout_json: None,
                }))
            }
            "hideScene" => {
                let scene_name = args::text(v.first().ok_or("hideScene: missing name")?, "hideScene")?;
                Ok(HostCall::Command(ScriptCommand::HideScene { scene_name }))
            }
            _ => Err(format!(
                "unknown game function '{name}' (native interpreter host)"
            )),
        }
    }

    fn lang(&self) -> &str {
        &self.lang
    }
}

/// One step of the native VermilionGym trash-can puzzle.
#[derive(Debug, Clone, PartialEq)]
enum TrashStep {
    ShowText(String),
    PlaySound(String),
    ReplaceTileBlock(u8, u8, u8),
    SetFlag(&'static str),
    ResetFlag(&'static str),
}

/// Native port of the VermilionGym `@run` trash-can puzzle
/// (`maps/VermilionGym/script.scene:66-125`; original
/// `engine/events/hidden_objects/gym_trash.asm`). The `@run` block used
/// `globalThis` persistent state + `Math.random`; here the state lives in
/// the engine (recreated per map, same as the Boa engine) and randomness
/// comes from the bridge RNG (`seed_rng`/`mix_rng`-driven), matching the
/// original's hardware-RNG re-roll on every reset.
#[derive(Debug, Clone)]
pub struct VgymTrashState {
    active: bool,
    first: i32,
    second: i32,
    /// 0 = hunting the 1st switch, 1 = hunting the 2nd.
    phase: u8,
    steps: VecDeque<TrashStep>,
    /// The command currently dispatched to the driver (re-returned by
    /// `tick` while waiting, like the interpreter's `pending_command`).
    pending: Option<ScriptCommand>,
}

impl VgymTrashState {
    fn new() -> Self {
        Self {
            active: false,
            first: -1,
            second: -1,
            phase: 0,
            steps: VecDeque::new(),
            pending: None,
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    /// Re-return the dispatched command while the puzzle waits on it.
    fn tick(&self) -> Option<ScriptCommand> {
        self.pending.clone()
    }

    /// Compute which can (0..14) the player is inspecting, from the bridge
    /// player position + facing (mirrors the `@run` block's arithmetic:
    /// cans on a 5-col × 3-row grid at x ∈ {1,3,5,7,9}, y ∈ {7,9,11}).
    fn can_index(host: &NativeHost) -> i32 {
        let facing = host.texts.get("playerFacing").map(|s| s.as_str()).unwrap_or("");
        let mut fx = host.player_x as i32;
        let mut fy = host.player_y as i32;
        match facing {
            "up" => fy -= 1,
            "down" => fy += 1,
            "left" => fx -= 1,
            "right" => fx += 1,
            _ => {}
        }
        (((fx - 1) / 2) * 3) + ((fy - 7) / 2)
    }

    /// Begin a trash-can interaction: decide the outcome from the current
    /// puzzle state + which can was inspected, and queue the effect steps.
    fn start(&mut self, host: &mut NativeHost) {
        self.steps.clear();
        self.pending = None;
        let solved = host.flags.get("EVENT_2ND_LOCK_OPENED").copied().unwrap_or(false);
        if solved {
            self.steps.push_back(TrashStep::ShowText(zh_or_en(
                &host.lang,
                "Nope, there's\nonly trash here.",
                "不，这里\n只有垃圾。",
            )));
        } else if self.phase == 0 {
            if self.first < 0 {
                self.first = (host.next_rand() % 15) as i32;
            }
            let can = Self::can_index(host);
            if can == self.first {
                // Found the 1st switch: seed the 2nd into an adjacent can.
                let col = self.first / 3;
                let row = self.first % 3;
                let mut adj: Vec<i32> = Vec::new();
                if row > 0 {
                    adj.push(self.first - 1);
                }
                if row < 2 {
                    adj.push(self.first + 1);
                }
                if col > 0 {
                    adj.push(self.first - 3);
                }
                if col < 4 {
                    adj.push(self.first + 3);
                }
                self.second = adj[(host.next_rand() % adj.len() as u64) as usize];
                self.phase = 1;
                self.steps.push_back(TrashStep::SetFlag("EVENT_1ST_LOCK_OPENED"));
                self.steps.push_back(TrashStep::PlaySound("SFX_SWITCH".to_string()));
                self.steps.push_back(TrashStep::ShowText(zh_or_en(
                    &host.lang,
                    "Hey! There's a\nswitch under the\ntrash!\nTurn it on!\n\nThe 1st electric\nlock opened!",
                    "嘿！垃圾桶\n下面有开关！\n打开它！\n\n第1道电子锁\n打开了！",
                )));
            } else {
                self.steps.push_back(TrashStep::ShowText(zh_or_en(
                    &host.lang,
                    "Nope, there's\nonly trash here.",
                    "不，这里\n只有垃圾。",
                )));
            }
        } else {
            let can = Self::can_index(host);
            if can == self.second {
                // Found the 2nd switch: open both locks and the door.
                self.steps.push_back(TrashStep::SetFlag("EVENT_2ND_LOCK_OPENED"));
                self.steps.push_back(TrashStep::ReplaceTileBlock(2, 2, 5));
                self.steps.push_back(TrashStep::PlaySound("SFX_GO_INSIDE".to_string()));
                self.steps.push_back(TrashStep::ShowText(zh_or_en(
                    &host.lang,
                    "The 2nd electric\nlock opened!\n\nThe motorized door\nopened!",
                    "第2道电子锁\n打开了！\n\n电动门\n打开了！",
                )));
            } else {
                // Wrong can: both locks re-lock, the 1st switch relocates.
                self.phase = 0;
                self.first = (host.next_rand() % 15) as i32;
                self.second = -1;
                self.steps.push_back(TrashStep::ResetFlag("EVENT_1ST_LOCK_OPENED"));
                self.steps.push_back(TrashStep::PlaySound("SFX_DENIED".to_string()));
                self.steps.push_back(TrashStep::ShowText(zh_or_en(
                    &host.lang,
                    "Nope, there's\nonly trash here!\n\nHold on!\n\nThe electric locks\nare re-locked!",
                    "不，这里\n只有垃圾！\n\n等等！\n\n电子锁\n又锁上了！",
                )));
            }
        }
        self.active = true;
    }

    /// Emit the next step as a `ScriptCommand` (applying sync flag steps
    /// immediately). Returns `None` when the puzzle script finished.
    fn next_command(&mut self, host: &mut NativeHost) -> Option<ScriptCommand> {
        while let Some(step) = self.steps.pop_front() {
            match step {
                TrashStep::SetFlag(flag) => {
                    host.flags.insert(flag.to_string(), true);
                }
                TrashStep::ResetFlag(flag) => {
                    host.flags.insert(flag.to_string(), false);
                }
                TrashStep::ShowText(text) => {
                    let cmd = ScriptCommand::ShowText { text };
                    self.pending = Some(cmd.clone());
                    self.active = true;
                    return Some(cmd);
                }
                TrashStep::PlaySound(sound_id) => {
                    let cmd = ScriptCommand::PlaySound { sound_id };
                    self.pending = Some(cmd.clone());
                    self.active = true;
                    return Some(cmd);
                }
                TrashStep::ReplaceTileBlock(x, y, block_id) => {
                    let cmd = ScriptCommand::ReplaceTileBlock { x, y, block_id };
                    self.pending = Some(cmd.clone());
                    self.active = true;
                    return Some(cmd);
                }
            }
        }
        self.active = false;
        self.pending = None;
        None
    }
}

fn zh_or_en(lang: &str, en: &str, zh: &str) -> String {
    if lang == "zh" {
        zh.to_string()
    } else {
        en.to_string()
    }
}

/// A registered storyline: a list of DSL statements, or the special-cased
/// native trash-can puzzle.
#[derive(Clone)]
enum FunctionDef {
    Story(Vec<StoryStmt>),
    VgymTrash,
}

/// Native replacement for `dotzuki_engine_script::ScriptEngine` (Boa): same
/// protocol surface (`load` → `tick` → `signal_done`), driven by the AST
/// interpreter. Recreated per map (like the Boa engine), so script flags
/// must be re-seeded via `seed_flags` after every map load.
pub struct NativeScriptEngine {
    interp: Interpreter<NativeHost>,
    functions: HashMap<String, FunctionDef>,
    /// Baseline of shared-module functions (registered via
    /// [`register_shared_scene`](Self::register_shared_scene)).
    /// [`load_map`](Self::load_map) rebuilds `functions` from this so a map's
    /// own same-named storyline shadows the shared fallback for that map only
    /// — the next map load re-derives the shared bindings instead of keeping
    /// the previous map's stale definitions.
    shared_functions: HashMap<String, FunctionDef>,
    vgym: VgymTrashState,
    state: InterpState,
}

impl NativeScriptEngine {
    pub fn new() -> Self {
        Self {
            interp: Interpreter::new(NativeHost::new()),
            functions: HashMap::new(),
            shared_functions: HashMap::new(),
            vgym: VgymTrashState::new(),
            state: InterpState::Idle,
        }
    }

    /// Register a shared module scene (e.g. the embedded `shared/pokecenter`
    /// AST). Storylines are registered under their bare name AND the
    /// `storyline_`-prefixed name (the codegen names compiled functions
    /// `storyline_<name>`, while configs bind the bare name). Shared
    /// functions form the baseline every [`load_map`](Self::load_map) starts
    /// from — a map load only replaces the previous map's own functions.
    pub fn register_shared_scene(&mut self, scene: &GameScene) {
        for storyline in &scene.storylines {
            for name in [storyline.name.clone(), format!("storyline_{}", storyline.name)] {
                let def = FunctionDef::Story(storyline.statements.clone());
                self.functions.insert(name.clone(), def.clone());
                self.shared_functions.insert(name, def);
            }
        }
    }

    /// Load a map's scene: register every `@storyline` (compiled function
    /// name `storyline_<name>`, plus the bare name configs bind) and the
    /// `@load` block under `<SceneName>OnLoad`. The VermilionGym `trashCans`
    /// storyline is special-cased to the native puzzle handler (its `@run`
    /// block cannot run in the interpreter). The function table is rebuilt
    /// from the shared baseline each load, so a map's own same-named
    /// storyline shadows the shared fallback (exact-name dispatch finds the
    /// bare key first) without leaking into the next map.
    pub fn load_map(&mut self, map_name: &str, scene: &GameScene) {
        self.functions = self.shared_functions.clone();
        for storyline in &scene.storylines {
            if map_name == "VermilionGym" && storyline.name == "trashCans" {
                self.functions
                    .insert("storyline_trashCans".to_string(), FunctionDef::VgymTrash);
            } else {
                let def = FunctionDef::Story(storyline.statements.clone());
                self.functions
                    .insert(format!("storyline_{}", storyline.name), def.clone());
                self.functions.insert(storyline.name.clone(), def);
            }
        }
        if let Some(on_load) = &scene.on_load {
            self.functions.insert(
                format!("{}OnLoad", scene.name),
                FunctionDef::Story(on_load.statements.clone()),
            );
        }
    }

    pub fn state(&self) -> InterpState {
        self.state
    }

    pub fn is_idle(&self) -> bool {
        self.state == InterpState::Idle
    }

    pub fn is_waiting(&self) -> bool {
        self.state == InterpState::WaitingForCommand
    }

    pub fn set_flag(&mut self, flag: &str, value: bool) {
        self.interp.host_mut().flags.insert(flag.to_string(), value);
    }

    pub fn get_flag(&self, flag: &str) -> bool {
        self.interp.host().flags.get(flag).copied().unwrap_or(false)
    }

    pub fn get_all_flags(&self) -> HashMap<String, bool> {
        self.interp.host().flags.clone()
    }

    pub fn seed_flags(&mut self, flags: &HashMap<String, bool>) {
        for (k, v) in flags {
            self.interp.host_mut().flags.insert(k.clone(), *v);
        }
    }

    pub fn seed_rng(&mut self, seed: u64) {
        self.interp.host_mut().rng_state = if seed == 0 { DEFAULT_RNG_SEED } else { seed };
    }

    pub fn mix_rng(&mut self, entropy: u64) {
        let host = self.interp.host_mut();
        host.rng_state ^= entropy.wrapping_mul(0x2545_F491_4F6C_DD1D);
        if host.rng_state == 0 {
            host.rng_state = DEFAULT_RNG_SEED;
        }
    }

    pub fn seed_number(&mut self, k: &str, v: f64) {
        self.interp.host_mut().numbers.insert(k.into(), v);
    }

    pub fn seed_text(&mut self, k: &str, v: &str) {
        self.interp.host_mut().texts.insert(k.into(), v.into());
    }

    pub fn seed_set(&mut self, k: &str, vals: &[String]) {
        self.interp.host_mut().sets.insert(k.into(), vals.to_vec());
    }

    pub fn set_player_position(&mut self, x: u8, y: u8) {
        let host = self.interp.host_mut();
        host.player_x = x;
        host.player_y = y;
    }

    pub fn set_lang(&mut self, lang: &str) {
        self.interp.host_mut().lang = lang.to_string();
    }

    /// Whether `name` resolves to a registered function — exact name first,
    /// then the `storyline_` prefix (mirrors the Boa `resolved_fn_name`).
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
            || self.functions.contains_key(&format!("storyline_{}", name))
    }

    /// Start a script function (no arguments — the only call form the
    /// overworld uses). Returns the first pending command, or `None` when
    /// the function completes without awaiting anything.
    pub fn call_function_no_args(
        &mut self,
        fn_name: &str,
    ) -> Result<Option<ScriptCommand>, String> {
        let resolved = if self.functions.contains_key(fn_name) {
            fn_name.to_string()
        } else {
            format!("storyline_{}", fn_name)
        };
        let def = self
            .functions
            .get(&resolved)
            .cloned()
            .ok_or_else(|| format!("function not found: {}", fn_name))?;
        match def {
            FunctionDef::VgymTrash => {
                log::info!(target: "pokered::overworld", "[NativeScript] VermilionGym trash-can puzzle start");
                self.vgym.start(self.interp.host_mut());
                let cmd = self.vgym.next_command(self.interp.host_mut());
                if cmd.is_some() {
                    self.state = InterpState::WaitingForCommand;
                } else {
                    self.state = InterpState::Idle;
                }
                Ok(cmd)
            }
            FunctionDef::Story(stmts) => {
                self.interp.load_function(&stmts);
                self.state = InterpState::Running;
                let cmd = self.interp.tick();
                match cmd {
                    Ok(Some(c)) => {
                        self.state = InterpState::WaitingForCommand;
                        Ok(Some(c))
                    }
                    Ok(None) => {
                        self.state = InterpState::Idle;
                        Ok(None)
                    }
                    Err(e) => {
                        log::warn!(target: "pokered::overworld", "[NativeScript] script error in {}: {}", fn_name, e);
                        self.state = InterpState::Finished;
                        Ok(None)
                    }
                }
            }
        }
    }

    /// Called each frame: returns the pending command while waiting.
    pub fn tick(&mut self) -> Option<ScriptCommand> {
        if self.vgym.is_active() {
            return self.vgym.tick();
        }
        match self.state {
            InterpState::WaitingForCommand => {
                self.interp.tick().unwrap_or_else(|e| {
                    log::warn!(target: "pokered::overworld", "[NativeScript] tick error: {}", e);
                    self.state = InterpState::Finished;
                    None
                })
            }
            InterpState::Running => match self.interp.tick() {
                Ok(Some(cmd)) => {
                    self.state = InterpState::WaitingForCommand;
                    Some(cmd)
                }
                Ok(None) => {
                    self.state = InterpState::Idle;
                    None
                }
                Err(e) => {
                    log::warn!(target: "pokered::overworld", "[NativeScript] tick error: {}", e);
                    self.state = InterpState::Finished;
                    None
                }
            },
            InterpState::Idle | InterpState::Finished => None,
        }
    }

    /// Deliver the result of the dispatched command and resume the script,
    /// returning the next pending command if the script immediately awaits.
    pub fn signal_done(
        &mut self,
        result: CommandResult,
    ) -> Result<Option<ScriptCommand>, String> {
        if self.vgym.is_active() {
            let cmd = self.vgym.next_command(self.interp.host_mut());
            if cmd.is_some() {
                self.state = InterpState::WaitingForCommand;
                return Ok(cmd);
            }
            self.state = InterpState::Idle;
            return Ok(None);
        }
        match self.interp.signal_done(result) {
            Ok(Some(cmd)) => {
                self.state = InterpState::WaitingForCommand;
                Ok(Some(cmd))
            }
            Ok(None) => {
                self.state = InterpState::Idle;
                Ok(None)
            }
            Err(e) => {
                log::warn!(target: "pokered::overworld", "[NativeScript] script error: {}", e);
                self.state = InterpState::Finished;
                Ok(None)
            }
        }
    }
}

impl Default for NativeScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Engine-agnostic handle the overworld stores in `OverworldScreen`: either
/// the legacy Boa engine (feature `script-boa`) or the native AST engine
/// (default). All methods delegate to the active variant, so the glue in
/// `screen.rs` / `update.rs` is engine-agnostic.
pub enum OverworldScriptEngine {
    #[cfg(feature = "script-boa")]
    Boa(dotzuki_engine_script::ScriptEngine),
    Native(NativeScriptEngine),
}

impl OverworldScriptEngine {
    pub fn new() -> Self {
        #[cfg(feature = "script-boa")]
        {
            OverworldScriptEngine::Boa(dotzuki_engine_script::ScriptEngine::with_api(
                &pokered_data::script_api::PokemonScriptApi,
            ))
        }
        #[cfg(not(feature = "script-boa"))]
        {
            OverworldScriptEngine::Native(NativeScriptEngine::new())
        }
    }

    pub fn is_idle(&self) -> bool {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.is_idle(),
            OverworldScriptEngine::Native(e) => e.is_idle(),
        }
    }

    pub fn is_waiting(&self) -> bool {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.is_waiting(),
            OverworldScriptEngine::Native(e) => e.is_waiting(),
        }
    }

    pub fn set_flag(&mut self, flag: &str, value: bool) {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.set_flag(flag, value),
            OverworldScriptEngine::Native(e) => e.set_flag(flag, value),
        }
    }

    pub fn get_flag(&self, flag: &str) -> bool {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.get_flag(flag),
            OverworldScriptEngine::Native(e) => e.get_flag(flag),
        }
    }

    pub fn get_all_flags(&self) -> HashMap<String, bool> {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.get_all_flags(),
            OverworldScriptEngine::Native(e) => e.get_all_flags(),
        }
    }

    pub fn seed_flags(&mut self, flags: &HashMap<String, bool>) {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.seed_flags(flags),
            OverworldScriptEngine::Native(e) => e.seed_flags(flags),
        }
    }

    pub fn seed_rng(&mut self, seed: u64) {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.seed_rng(seed),
            OverworldScriptEngine::Native(e) => e.seed_rng(seed),
        }
    }

    pub fn mix_rng(&mut self, entropy: u64) {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.mix_rng(entropy),
            OverworldScriptEngine::Native(e) => e.mix_rng(entropy),
        }
    }

    pub fn seed_number(&mut self, k: &str, v: f64) {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.seed_number(k, v),
            OverworldScriptEngine::Native(e) => e.seed_number(k, v),
        }
    }

    pub fn seed_text(&mut self, k: &str, v: &str) {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.seed_text(k, v),
            OverworldScriptEngine::Native(e) => e.seed_text(k, v),
        }
    }

    pub fn seed_set(&mut self, k: &str, vals: &[String]) {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.seed_set(k, vals),
            OverworldScriptEngine::Native(e) => e.seed_set(k, vals),
        }
    }

    pub fn set_player_position(&mut self, x: u8, y: u8) {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.set_player_position(x, y),
            OverworldScriptEngine::Native(e) => e.set_player_position(x, y),
        }
    }

    pub fn set_lang(&mut self, lang: &str) {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.set_lang(lang),
            OverworldScriptEngine::Native(e) => e.set_lang(lang),
        }
    }

    /// Load raw JS into the Boa engine. The native engine has no JS path —
    /// load scene ASTs via the `load_map`-style methods on the `Native`
    /// variant instead.
    pub fn load_script(&mut self, _source: &str) -> Result<(), String> {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e
                .load_script(_source)
                .map_err(|err| format!("JS load failed: {}", err)),
            OverworldScriptEngine::Native(_) => Ok(()),
        }
    }

    pub fn load_shared_module(&mut self, _name: &str, _source: &str) -> Result<(), String> {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e
                .load_shared_module(_name, _source)
                .map_err(|err| format!("JS shared module load failed: {}", err)),
            OverworldScriptEngine::Native(_) => Ok(()),
        }
    }

    pub fn has_function(&mut self, fn_name: &str) -> bool {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.has_function(fn_name),
            OverworldScriptEngine::Native(e) => e.has_function(fn_name),
        }
    }

    /// Native-only: register a shared module scene (e.g. `shared/pokecenter`).
    /// No-op on the Boa variant (which loads shared modules as raw JS).
    #[cfg_attr(not(feature = "script-boa"), allow(irrefutable_let_patterns))]
    pub fn register_shared_scene_native(&mut self, scene: &dotzuki_engine_dsl::ast::GameScene) {
        if let OverworldScriptEngine::Native(e) = self {
            e.register_shared_scene(scene);
        }
    }

    /// Native-only: load a map's scene AST. No-op on the Boa variant (which
    /// loads the compiled JS instead).
    #[cfg_attr(not(feature = "script-boa"), allow(irrefutable_let_patterns))]
    pub fn load_map_native(&mut self, map_name: &str, scene: &dotzuki_engine_dsl::ast::GameScene) {
        if let OverworldScriptEngine::Native(e) = self {
            e.load_map(map_name, scene);
        }
    }

    pub fn call_function_no_args(&mut self, fn_name: &str) -> Result<Option<ScriptCommand>, String> {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e
                .call_function_no_args(fn_name)
                .map_err(|err| err.to_string()),
            OverworldScriptEngine::Native(e) => e.call_function_no_args(fn_name),
        }
    }

    pub fn tick(&mut self) -> Option<ScriptCommand> {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.tick(),
            OverworldScriptEngine::Native(e) => e.tick(),
        }
    }

    pub fn signal_done(&mut self, result: CommandResult) -> Result<Option<ScriptCommand>, String> {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.signal_done(result).map_err(|err| err.to_string()),
            OverworldScriptEngine::Native(e) => e.signal_done(result),
        }
    }
}

impl Default for OverworldScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_sync_queries_read_seeded_state() {
        let mut host = NativeHost::new();
        host.numbers.insert("money".into(), 5000.0);
        host.sets.insert("bag".into(), vec!["SILPH_SCOPE".to_string()]);
        let money = host.call("getMoney", &[]).unwrap();
        assert_eq!(money, HostCall::Value(Value::Number(5000.0)));
        let has = host.call("hasItem", &[Value::Text("SILPH_SCOPE".into())]).unwrap();
        assert_eq!(has, HostCall::Value(Value::Bool(true)));
        let missing = host.call("hasItem", &[Value::Text("MASTER_BALL".into())]).unwrap();
        assert_eq!(missing, HostCall::Value(Value::Bool(false)));
        let has_money = host.call("hasMoney", &[Value::Number(1000.0)]).unwrap();
        assert_eq!(has_money, HostCall::Value(Value::Bool(true)));
    }

    #[test]
    fn host_flag_mutations_are_sync() {
        let mut host = NativeHost::new();
        assert_eq!(
            host.call("getFlag", &[Value::Text("X".into())]).unwrap(),
            HostCall::Value(Value::Bool(false))
        );
        host.call("setFlag", &[Value::Text("X".into())]).unwrap();
        assert_eq!(
            host.call("getFlag", &[Value::Text("X".into())]).unwrap(),
            HostCall::Value(Value::Bool(true))
        );
        host.call("resetFlag", &[Value::Text("X".into())]).unwrap();
        assert_eq!(
            host.call("getFlag", &[Value::Text("X".into())]).unwrap(),
            HostCall::Value(Value::Bool(false))
        );
    }

    #[test]
    fn host_builds_commands_from_values() {
        let mut host = NativeHost::new();
        let cmd = host
            .call(
                "movePlayerRelative",
                &[Value::Array(vec![
                    Value::Text("up".into()),
                    Value::Array(vec![Value::Number(1.0), Value::Number(-2.0)]),
                ])],
            )
            .unwrap();
        assert_eq!(
            cmd,
            HostCall::Command(ScriptCommand::MovePlayerRelative {
                steps: vec![(0, -1), (1, -2)]
            })
        );
        let cmd = host
            .call(
                "showObject",
                &[Value::Text("PALLET_TOWN_OAK".into())],
            )
            .unwrap();
        assert_eq!(
            cmd,
            HostCall::Command(ScriptCommand::ShowObjectByName {
                toggle_id: "PALLET_TOWN_OAK".into()
            })
        );
        let cmd = host.call("showObject", &[Value::Number(3.0)]).unwrap();
        assert_eq!(cmd, HostCall::Command(ScriptCommand::ShowObject { object_index: 3 }));
    }

    #[test]
    fn host_show_random_text_picks_one_of_the_pool() {
        let mut host = NativeHost::new();
        host.rng_state = 0x1234_5678;
        let cmd = host
            .call(
                "showRandomText",
                &[Value::Array(vec![
                    Value::Text("a".into()),
                    Value::Text("b".into()),
                    Value::Text("c".into()),
                ])],
            )
            .unwrap();
        match cmd {
            HostCall::Command(ScriptCommand::ShowText { text }) => {
                assert!(matches!(text.as_str(), "a" | "b" | "c"))
            }
            other => panic!("expected ShowText command, got {:?}", other),
        }
    }

    #[test]
    fn unknown_function_errors() {
        let mut host = NativeHost::new();
        let err = host.call("noSuchFunction", &[]).unwrap_err();
        assert!(err.contains("unknown game function"));
    }

    #[test]
    fn vgym_can_index_matches_js_arithmetic() {
        let mut host = NativeHost::new();
        host.player_x = 3;
        host.player_y = 7;
        host.texts.insert("playerFacing".into(), "up".into());
        // Facing up from (3,7) → inspecting (3,6): ((3-1)/2)*3 + ((6-7)/2) = 3 + 0 = 3.
        assert_eq!(VgymTrashState::can_index(&host), 3);
        host.player_x = 9;
        host.player_y = 11;
        host.texts.insert("playerFacing".into(), "down".into());
        // Facing down from (9,11) → (9,12): ((9-1)/2)*3 + ((12-7)/2) = 12 + 2 = 14.
        assert_eq!(VgymTrashState::can_index(&host), 14);
    }

    #[test]
    fn vgym_first_switch_locks_and_opens() {
        let mut host = NativeHost::new();
        host.rng_state = 42;
        let mut vgym = VgymTrashState::new();
        vgym.first = 7; // pin the 1st switch can
        host.player_x = 5;
        host.player_y = 9;
        host.texts.insert("playerFacing".into(), "up".into());
        // Facing up from (5,9) → (5,8): ((5-1)/2)*3 + ((8-7)/2) = 6 + 0 = 6. Not 7 → trash text.
        vgym.start(&mut host);
        let cmd = vgym.next_command(&mut host);
        assert!(matches!(cmd, Some(ScriptCommand::ShowText { .. })));
        assert_eq!(vgym.phase, 0);
        // Now stand on can 7: facing up from (5,11) → (5,10): 6 + 1 = 7.
        host.player_y = 11;
        vgym.start(&mut host);
        let cmd = vgym.next_command(&mut host);
        assert!(matches!(cmd, Some(ScriptCommand::PlaySound { .. })));
        assert_eq!(vgym.phase, 1);
        assert!(host.flags.get("EVENT_1ST_LOCK_OPENED").copied().unwrap_or(false));
        // The 2nd switch must be orthogonally adjacent to can 7
        // (cols x∈{1,3,5,7,9}, rows y∈{7,9,11}): 4, 6, 8 or 10.
        assert!(
            vgym.second == 4 || vgym.second == 6 || vgym.second == 8 || vgym.second == 10,
            "second={}",
            vgym.second
        );
    }

    #[test]
    fn vgym_trash_cans_route_through_engine_special_case() {
        // The embedded VermilionGym scene's trashCans storyline is replaced by
        // the native puzzle handler; calling it must emit puzzle commands and
        // the interpreter must never see the @run block.
        let scene = pokered_data::embedded_scenes::get_scene_ast("VermilionGym")
            .expect("VermilionGym AST embedded");
        let mut engine = NativeScriptEngine::new();
        engine.load_map("VermilionGym", &scene);
        assert!(
            engine.has_function("trashCans"),
            "trashCans must resolve via the storyline_ fallback"
        );
        engine.seed_rng(7);
        engine.set_player_position(1, 7);
        engine.seed_text("playerFacing", "up");
        let cmd = engine
            .call_function_no_args("trashCans")
            .expect("trashCans call");
        // Facing up from (1,7) → (1,6): ((1-1)/2)*3 + ((6-7)/2) = 0 — can 0.
        // With no pinned switch, the first rng draw picks it; can 0 may or
        // may not match — accept either outcome, then pin the switch for the
        // deterministic part.
        match cmd {
            Some(ScriptCommand::ShowText { text }) => {
                assert!(text.contains("only trash") || text.contains("switch"), "got: {}", text)
            }
            other => panic!("expected ShowText, got {:?}", other),
        }

        // Deterministic lock-open: pin the 1st switch at can 7, stand on it.
        let mut e = NativeScriptEngine::new();
        e.load_map("VermilionGym", &scene);
        e.vgym.first = 7;
        e.set_player_position(5, 11);
        e.seed_text("playerFacing", "up");
        // Facing up from (5,11) → (5,10): ((5-1)/2)*3 + ((10-7)/2) = 6 + 1 = 7.
        // Driver pattern: one call_function_no_args starts the interaction,
        // signal_done drains its queued steps.
        let mut seen_switch = false;
        match e.call_function_no_args("trashCans") {
            Ok(Some(ScriptCommand::PlaySound { sound_id })) if sound_id == "SFX_SWITCH" => {
                seen_switch = true;
            }
            Ok(Some(_)) => {}
            Ok(None) => panic!("puzzle produced no command"),
            Err(err) => panic!("puzzle error: {}", err),
        }
        let mut guard = 0;
        while e.is_waiting() || seen_switch {
            guard += 1;
            if guard > 200 {
                panic!("puzzle did not complete");
            }
            match e.signal_done(CommandResult::Void) {
                Ok(Some(ScriptCommand::PlaySound { sound_id })) if sound_id == "SFX_SWITCH" => {
                    seen_switch = true;
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(err) => panic!("puzzle signal error: {}", err),
            }
        }
        assert!(seen_switch, "first switch must be findable");
        assert!(
            e.get_flag("EVENT_1ST_LOCK_OPENED"),
            "1st lock flag must be set after finding the switch"
        );
        // Wrong can while hunting the 2nd switch: both locks re-lock and the
        // 1st switch relocates (phase → 0).
        e.set_player_position(1, 7);
        e.seed_text("playerFacing", "up"); // can 0 ≠ the seeded 2nd switch
        let _ = e.call_function_no_args("trashCans");
        assert!(
            !e.get_flag("EVENT_1ST_LOCK_OPENED"),
            "wrong can must re-lock the 1st lock"
        );
        assert_eq!(e.vgym.phase, 0, "wrong can must restart the 1st-switch hunt");
        // Re-open the 1st lock (the relock re-rolled the switch).
        e.vgym.first = 7;
        e.set_player_position(5, 11);
        e.seed_text("playerFacing", "up"); // can 7
        let _ = e.call_function_no_args("trashCans");
        assert!(e.get_flag("EVENT_1ST_LOCK_OPENED"));
        // The 2nd switch sits in a can adjacent to 7 (the adjacency itself is
        // covered by vgym_first_switch_locks_and_opens); pin it at can 4 and
        // confirm the door-opening flow. Can 4 = col 1, row 1: facing up from
        // (3,10) → (3,9) → ((3-1)/2)*3 + ((9-7)/2) = 3 + 1 = 4.
        e.vgym.second = 4;
        e.set_player_position(3, 10);
        e.seed_text("playerFacing", "up");
        let mut door_opened = false;
        match e.call_function_no_args("trashCans") {
            Ok(Some(ScriptCommand::PlaySound { sound_id })) if sound_id == "SFX_GO_INSIDE" => {
                door_opened = true;
            }
            Ok(Some(_)) => {}
            Ok(None) => panic!("puzzle produced no command"),
            Err(err) => panic!("puzzle error: {}", err),
        }
        let mut guard = 0;
        while e.is_waiting() || door_opened {
            guard += 1;
            if guard > 200 {
                panic!("door-open flow did not complete");
            }
            match e.signal_done(CommandResult::Void) {
                Ok(Some(ScriptCommand::PlaySound { sound_id })) if sound_id == "SFX_GO_INSIDE" => {
                    door_opened = true;
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(err) => panic!("puzzle signal error: {}", err),
            }
        }
        assert!(door_opened, "2nd switch must be reachable in an adjacent can");
        assert!(
            e.get_flag("EVENT_2ND_LOCK_OPENED"),
            "2nd lock flag must be set after the door opens"
        );
    }

    /// Regression (review F1): shared-module functions must survive
    /// [`load_map`](Self::load_map) — a map load only replaces the previous
    /// map's own functions — and a map's own same-named storyline must win
    /// at dispatch (configs bind the bare name, which used to stay the
    /// shared English-only definition, losing `@t` localization).
    #[test]
    fn shared_scene_survives_map_load() {
        let shared = dotzuki_engine_dsl::compiler::compile_scene_to_ast(
            "game_scene shared { @storyline(\"talkNurse\") { setFlag(\"SHARED_NURSE\") } }",
            "shared/pokecenter",
        )
        .expect("shared scene compiles");
        let map_scene = dotzuki_engine_dsl::compiler::compile_scene_to_ast(
            "game_scene PalletTown { @storyline(\"greet\") { setFlag(\"TEST_FLAG\") } }",
            "PalletTown",
        )
        .expect("map scene compiles");
        let mut engine = NativeScriptEngine::new();
        engine.register_shared_scene(&shared);
        engine.load_map("PalletTown", &map_scene);
        assert!(engine.has_function("talkNurse"), "shared bare name survives");
        assert!(
            engine.has_function("storyline_talkNurse"),
            "shared prefixed name survives"
        );
        assert!(engine.has_function("storyline_greet"));
        // A second map load keeps the shared functions too.
        engine.load_map("ViridianCity", &map_scene);
        assert!(engine.has_function("talkNurse"));
        // A map defining its own same-named storyline wins over the shared
        // one — verify by dispatching `talkNurse`, not just key presence
        // (the shared registration alone used to satisfy the old assertion).
        let own = dotzuki_engine_dsl::compiler::compile_scene_to_ast(
            "game_scene CeruleanPokecenter { @storyline(\"talkNurse\") { setFlag(\"OWN_NURSE\") } }",
            "CeruleanPokecenter",
        )
        .expect("own scene compiles");
        engine.load_map("CeruleanPokecenter", &own);
        engine
            .call_function_no_args("talkNurse")
            .expect("talkNurse call");
        assert!(
            engine.get_flag("OWN_NURSE"),
            "the map's own talkNurse must execute"
        );
        assert!(
            !engine.get_flag("SHARED_NURSE"),
            "the shared fallback must not shadow the map's own storyline"
        );
    }

    /// A map without its own same-named storyline still falls back to the
    /// shared definition.
    #[test]
    fn map_without_own_storyline_uses_shared_definition() {
        let shared = dotzuki_engine_dsl::compiler::compile_scene_to_ast(
            "game_scene shared { @storyline(\"talkNurse\") { setFlag(\"SHARED_NURSE\") } }",
            "shared/pokecenter",
        )
        .expect("shared scene compiles");
        let map_scene = dotzuki_engine_dsl::compiler::compile_scene_to_ast(
            "game_scene PalletTown { @storyline(\"greet\") { setFlag(\"TEST_FLAG\") } }",
            "PalletTown",
        )
        .expect("map scene compiles");
        let mut engine = NativeScriptEngine::new();
        engine.register_shared_scene(&shared);
        engine.load_map("PalletTown", &map_scene);
        engine
            .call_function_no_args("talkNurse")
            .expect("shared talkNurse call");
        assert!(
            engine.get_flag("SHARED_NURSE"),
            "map without its own talkNurse must run the shared definition"
        );
    }

    /// Loading map A (own talkNurse) then map B (no own talkNurse): B must
    /// get the shared definition, not A's stale one — the bare shared key is
    /// re-derived from the shared baseline on every load.
    #[test]
    fn own_storyline_does_not_leak_into_next_map() {
        let shared = dotzuki_engine_dsl::compiler::compile_scene_to_ast(
            "game_scene shared { @storyline(\"talkNurse\") { setFlag(\"SHARED_NURSE\") } }",
            "shared/pokecenter",
        )
        .expect("shared scene compiles");
        let own = dotzuki_engine_dsl::compiler::compile_scene_to_ast(
            "game_scene CeruleanPokecenter { @storyline(\"talkNurse\") { setFlag(\"OWN_NURSE\") } }",
            "CeruleanPokecenter",
        )
        .expect("own scene compiles");
        let plain = dotzuki_engine_dsl::compiler::compile_scene_to_ast(
            "game_scene PalletTown { @storyline(\"greet\") { setFlag(\"TEST_FLAG\") } }",
            "PalletTown",
        )
        .expect("plain scene compiles");
        let mut engine = NativeScriptEngine::new();
        engine.register_shared_scene(&shared);
        engine.load_map("CeruleanPokecenter", &own);
        engine
            .call_function_no_args("talkNurse")
            .expect("own talkNurse call");
        assert!(engine.get_flag("OWN_NURSE"));
        engine.load_map("PalletTown", &plain);
        engine
            .call_function_no_args("talkNurse")
            .expect("shared talkNurse call");
        assert!(
            engine.get_flag("SHARED_NURSE"),
            "map B must fall back to the shared definition"
        );
        assert!(
            engine.functions.get("talkNurse").is_some(),
            "shared bare binding must be restored"
        );
    }
}
