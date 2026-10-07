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

use crate::alloc_prelude::*;
#[cfg(not(target_os = "none"))]
use crate::hash_compat::HashMap;
#[cfg(target_os = "none")]
use crate::hash_compat::HashMap;
use alloc::{borrow::Cow, rc::Rc};

use dotzuki_engine_dsl::ast::{GameScene, StoryStmt};
use dotzuki_engine_dsl::core_host::dispatch_core_async;
use dotzuki_engine_dsl::interpreter::{HostCall, Interpreter, InterpState, ScriptHost, Value};
use dotzuki_engine_script::{CommandResult, ScriptCommand};
use pokered_data::script_command::PokemonScriptCommand;
#[cfg(test)]
use serde_json::json;

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

/// Build a game-defined command from its JS verb name and JSON args
/// (`ScriptCommand::Custom` — the engine's generic escape hatch for
/// game-specific verbs; the engine dropped their dedicated variants).
#[cfg(test)]
fn custom(name: &str, args: Vec<serde_json::Value>) -> ScriptCommand {
    ScriptCommand::Custom {
        name: name.to_string(),
        args,
    }
}

fn pokemon(command: PokemonScriptCommand) -> HostCall {
    HostCall::Command(command.into_script_command())
}

/// Argument conversion helpers — all fail with a descriptive message,
/// mirroring the Boa registrar closures' type errors.
mod args {
    use crate::alloc_prelude::*;
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
}

/// Bridge state + `game.*` dispatch for the native interpreter. Mirrors
/// `dotzuki_engine_script::SharedBridge` (seeded query state) and the registrars
/// of `dotzuki-engine-script/src/engine.rs` + `pokered-data/src/script_api.rs`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
            flags: HashMap::default(),
            numbers: HashMap::default(),
            texts: HashMap::default(),
            sets: HashMap::default(),
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
        if let Some(command) = dispatch_core_async(name, v)? {
            return Ok(HostCall::Command(command));
        }

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
            "getGameVersion" => Ok(HostCall::Value(Value::Number(
                self.numbers.get("gameVersion").copied().unwrap_or(0.0),
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
            "partyMonCanRename" => {
                let idx = args::u32(v.first().ok_or("partyMonCanRename: missing index")?, "partyMonCanRename")?;
                Ok(HostCall::Value(Value::Bool(
                    self.numbers.get(&format!("partyCanRename{idx}")).copied().unwrap_or(0.0) != 0.0,
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
            "showItemDialogue" => {
                let text = args::text(v.first().ok_or("showItemDialogue: missing text")?, "showItemDialogue")?;
                Ok(pokemon(PokemonScriptCommand::ShowItemDialogue { text }))
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
                Ok(HostCall::Command(ScriptCommand::GiveMonster { species, level }))
            }
            "startBattle" => {
                let trainer_id = args::text(v.first().ok_or("startBattle: missing trainer")?, "startBattle")?;
                Ok(HostCall::Command(ScriptCommand::StartBattle { trainer_id }))
            }
            "startBattleSet" => {
                let trainer_id = args::text(v.first().ok_or("startBattleSet: missing trainer")?, "startBattleSet")?;
                let base = args::u8(v.get(1).ok_or("startBattleSet: missing base")?, "startBattleSet")?;
                Ok(pokemon(PokemonScriptCommand::StartBattleSet {
                    trainer_id,
                    rival_triplet_base: base,
                }))
            }
            "startWildBattle" => {
                let species = args::text(v.first().ok_or("startWildBattle: missing species")?, "startWildBattle")?;
                let level = args::u8(v.get(1).ok_or("startWildBattle: missing level")?, "startWildBattle")?;
                Ok(HostCall::Command(ScriptCommand::StartWildBattle { species, level }))
            }
            "oldManTutorial" => Ok(pokemon(PokemonScriptCommand::OldManTutorial)),
            "tradePokemon" => {
                let offered = args::text(v.first().ok_or("tradePokemon: missing offered")?, "tradePokemon")?;
                let received = args::text(v.get(1).ok_or("tradePokemon: missing received")?, "tradePokemon")?;
                let nickname = args::text(v.get(2).ok_or("tradePokemon: missing nickname")?, "tradePokemon")?;
                Ok(pokemon(PokemonScriptCommand::TradePokemon {
                    offered,
                    received,
                    nickname,
                }))
            }
            "showPokedexEntry" => {
                let species = args::text(v.first().ok_or("showPokedexEntry: missing species")?, "showPokedexEntry")?;
                Ok(pokemon(PokemonScriptCommand::ShowPokedexEntry { species }))
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
                Ok(pokemon(PokemonScriptCommand::ReplaceTileBlock { x, y, block_id }))
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
                Ok(pokemon(PokemonScriptCommand::OpenSlots { lucky: Some(lucky) }))
            }
            "elevatorMenu" => {
                let floors = args::string_array(v.first().ok_or("elevatorMenu: missing floors")?, "elevatorMenu")?;
                Ok(pokemon(PokemonScriptCommand::ElevatorMenu { floors }))
            }
            "filterBag" => {
                let item_ids = args::string_array(v.first().ok_or("filterBag: missing items")?, "filterBag")?;
                Ok(pokemon(PokemonScriptCommand::FilterBag { item_ids }))
            }
            "showDiploma" => Ok(pokemon(PokemonScriptCommand::ShowDiploma)),
            "openPC" => Ok(pokemon(PokemonScriptCommand::OpenPc)),
            "openItemPC" => Ok(pokemon(PokemonScriptCommand::OpenItemPc)),
            "openBillsPC" => Ok(pokemon(PokemonScriptCommand::OpenBillsPc)),
            "linkStart" => Ok(pokemon(PokemonScriptCommand::LinkStart)),
            "enterHallOfFame" => Ok(pokemon(PokemonScriptCommand::EnterHallOfFame)),
            "giveCoins" => {
                let amount = args::u32(v.first().ok_or("giveCoins: missing amount")?, "giveCoins")?.min(u16::MAX as u32) as u16;
                Ok(pokemon(PokemonScriptCommand::GiveCoins { amount }))
            }
            "takeCoins" => {
                let amount = args::u32(v.first().ok_or("takeCoins: missing amount")?, "takeCoins")?.min(u16::MAX as u32) as u16;
                Ok(pokemon(PokemonScriptCommand::TakeCoins { amount }))
            }
            "depositDaycare" => {
                let index = args::u8(v.first().ok_or("depositDaycare: missing index")?, "depositDaycare")?;
                Ok(pokemon(PokemonScriptCommand::DepositDaycare { index }))
            }
            "withdrawDaycare" => Ok(pokemon(PokemonScriptCommand::WithdrawDaycare)),

            "waitMusic" => Ok(pokemon(PokemonScriptCommand::WaitMusic)),
            "vendingDelivery" => Ok(pokemon(PokemonScriptCommand::VendingDelivery)),
            "showMoneyBox" => {
                let amount = args::number(v.first().ok_or("showMoneyBox: missing amount")?, "showMoneyBox")? as i64;
                Ok(pokemon(PokemonScriptCommand::ShowMoneyBox { amount }))
            }
            "readingMenu" => {
                let options = args::string_array(v.first().ok_or("readingMenu: missing options")?, "readingMenu")?;
                let texts = args::string_array(v.get(1).ok_or("readingMenu: missing texts")?, "readingMenu")?;
                if options.len() != texts.len() + 1 || texts.is_empty() {
                    return Err("readingMenu: one text per heading plus a final exit option required".to_string());
                }
                Ok(pokemon(PokemonScriptCommand::ReadingMenu { options, texts }))
            }
            "playShipDeparture" => Ok(pokemon(PokemonScriptCommand::PlayShipDeparture)),
            "animateHealingMachine" => Ok(pokemon(PokemonScriptCommand::AnimateHealingMachine)),
            "openNamingScreen" => {
                let species = args::text(v.first().ok_or("openNamingScreen: missing species")?, "openNamingScreen")?;
                Ok(pokemon(PokemonScriptCommand::OpenNamingScreen { species }))
            }
            "choosePartyPokemon" => Ok(pokemon(PokemonScriptCommand::ChoosePartyPokemon)),
            "setPartyNickname" => {
                let index = args::u8(v.first().ok_or("setPartyNickname: missing index")?, "setPartyNickname")?;
                let nickname = args::text(v.get(1).ok_or("setPartyNickname: missing nickname")?, "setPartyNickname")?;
                Ok(pokemon(PokemonScriptCommand::SetPartyNickname { index, nickname }))
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
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
enum TrashStep {
    ShowText(String),
    PlaySound(String),
    ReplaceTileBlock(u8, u8, u8),
    SetFlag(&'static str),
    ResetFlag(&'static str),
}

/// The static table's flag names are string literals; deserialization
/// leaks the owned form to keep the 'static field type.
impl<'de> serde::Deserialize<'de> for TrashStep {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        enum Owned {
            ShowText(String),
            PlaySound(String),
            ReplaceTileBlock(u8, u8, u8),
            SetFlag(String),
            ResetFlag(String),
        }
        Ok(match Owned::deserialize(deserializer)? {
            Owned::ShowText(t) => TrashStep::ShowText(t),
            Owned::PlaySound(t) => TrashStep::PlaySound(t),
            Owned::ReplaceTileBlock(x, y, b) => TrashStep::ReplaceTileBlock(x, y, b),
            Owned::SetFlag(f) => TrashStep::SetFlag(Box::leak(f.into_boxed_str())),
            Owned::ResetFlag(f) => TrashStep::ResetFlag(Box::leak(f.into_boxed_str())),
        })
    }
}

/// Native port of the VermilionGym `@run` trash-can puzzle
/// (`maps/VermilionGym/script.scene:66-125`; original
/// `engine/events/hidden_objects/gym_trash.asm`). The `@run` block used
/// `globalThis` persistent state + `Math.random`; here the state lives in
/// the engine (recreated per map, same as the Boa engine) and randomness
/// comes from the bridge RNG (`seed_rng`/`mix_rng`-driven), matching the
/// original's hardware-RNG re-roll on every reset.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
            first: 0,
            second: 0,
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

    fn second_index(first: u8, random: u8) -> u8 {
        const CANS: [[u8; 5]; 15] = [
            [2, 1, 3, 0, 0], [3, 0, 2, 4, 0], [2, 1, 5, 0, 0],
            [3, 0, 4, 6, 0], [4, 1, 3, 5, 7], [3, 2, 4, 8, 0],
            [3, 3, 7, 9, 0], [4, 4, 6, 8, 10], [3, 5, 7, 11, 0],
            [3, 6, 10, 12, 0], [4, 7, 9, 11, 13], [3, 8, 10, 14, 0],
            [2, 9, 13, 0, 0], [3, 10, 12, 14, 0], [2, 11, 13, 0, 0],
        ];
        let row = &CANS[first.min(14) as usize];
        let selected = row[0] & random.rotate_left(4);
        // DEC $00 -> $ff: the original lands in zero bank padding.
        if selected == 0 { 0 } else { row[selected as usize] & 0x0f }
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
        } else if !host.flags.get("EVENT_1ST_LOCK_OPENED").copied().unwrap_or(false) {
            self.phase = 0;
            let can = Self::can_index(host);
            if can == self.first {
                // GymTrashCans: preserve the original mask/swap/DEC bug.
                // A zero masked result underflows and reads bank padding (can 0).
                self.second = Self::second_index(self.first as u8, host.next_rand() as u8) as i32;
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
                self.first = (host.next_rand() as u8 & 0x0e) as i32;
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
                    let cmd = PokemonScriptCommand::ReplaceTileBlock {
                        x,
                        y,
                        block_id,
                    }
                    .into_script_command();
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
    Story(Rc<[StoryStmt]>),
    /// JSON-serialized `Vec<StoryStmt>` retained in ROM and decoded only when
    /// the function runs. This keeps map transitions from materializing every
    /// storyline at once on memory-constrained targets such as GBA.
    Embedded(&'static [u8]),
    VgymTrash,
}

/// Native replacement for `dotzuki_engine_script::ScriptEngine` (Boa): same
/// protocol surface (`load` → `tick` → `signal_done`), driven by the AST
/// interpreter. Recreated per map (like the Boa engine), so script flags
/// must be re-seeded via `seed_flags` after every map load.
pub struct NativeScriptEngine {
    interp: Interpreter<NativeHost>,
    functions: HashMap<Cow<'static, str>, FunctionDef>,
    /// Baseline of shared-module functions (registered via
    /// [`register_shared_scene`](Self::register_shared_scene)).
    /// [`load_map`](Self::load_map) rebuilds `functions` from this so a map's
    /// own same-named storyline shadows the shared fallback for that map only
    /// — the next map load re-derives the shared bindings instead of keeping
    /// the previous map's stale definitions.
    shared_functions: HashMap<Cow<'static, str>, FunctionDef>,
    vgym: VgymTrashState,
    state: InterpState,
    split_battle_active: bool,
    split_battle_waiting: bool,
    split_fossil_active: bool,
    split_fossil_waiting: bool,
}

fn embedded_function_alias(name: &'static str) -> Cow<'static, str> {
    pokered_data::embedded_scenes::scene_function_alias(name)
        .map(Cow::Borrowed)
        .unwrap_or_else(|| Cow::Owned(format!("storyline_{}", name)))
}

impl NativeScriptEngine {
    pub fn new() -> Self {
        Self {
            interp: Interpreter::new(NativeHost::new()),
            functions: HashMap::default(),
            shared_functions: HashMap::default(),
            vgym: VgymTrashState::new(),
            state: InterpState::Idle,
            split_battle_active: false,
            split_battle_waiting: false,
            split_fossil_active: false,
            split_fossil_waiting: false,
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
                let def = FunctionDef::Story(Rc::from(
                    storyline.statements.clone().into_boxed_slice(),
                ));
                // A map-local storyline must shadow the shared fallback even
                // when the shared module is registered after the map. GBA
                // loads in that order to keep peak AST memory bounded.
                if !self.functions.contains_key(name.as_str()) {
                    self.functions.insert(name.clone().into(), def.clone());
                }
                self.shared_functions.insert(name.into(), def);
            }
        }
    }

    /// Register a shared scene from independently serialized functions.
    pub fn register_embedded_shared(
        &mut self,
        map_name: &str,
        entries: &'static [(&'static str, &'static str, &'static [u8])],
    ) {
        for (_, function_name, bytes) in entries.iter().filter(|(map, _, _)| *map == map_name) {
            let def = FunctionDef::Embedded(bytes);
            for name in [
                Cow::Borrowed(*function_name),
                embedded_function_alias(function_name),
            ] {
                if !self.functions.contains_key(name.as_ref()) {
                    self.functions.insert(name.clone().into(), def.clone());
                }
                self.shared_functions.insert(name.into(), def.clone());
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
                    .insert(Cow::Borrowed("storyline_trashCans"), FunctionDef::VgymTrash);
            } else {
                let def = FunctionDef::Story(Rc::from(
                    storyline.statements.clone().into_boxed_slice(),
                ));
                self.functions
                    .insert(format!("storyline_{}", storyline.name).into(), def.clone());
                self.functions.insert(storyline.name.clone().into(), def);
            }
        }
        if let Some(on_load) = &scene.on_load {
            self.functions.insert(
                format!("{}OnLoad", scene.name).into(),
                FunctionDef::Story(Rc::from(on_load.statements.clone().into_boxed_slice())),
            );
        }
    }

    /// Load a scene by value, transferring its statement buffers into the
    /// function table. This avoids cloning an entire map AST after
    /// deserialization and lets the bare/prefixed function aliases share one
    /// allocation, which is essential for large GBA scenes such as Oak's Lab.
    pub fn load_map_owned(&mut self, map_name: &str, scene: GameScene) {
        self.functions = self.shared_functions.clone();
        for storyline in scene.storylines {
            if map_name == "VermilionGym" && storyline.name == "trashCans" {
                self.functions
                    .insert(Cow::Borrowed("storyline_trashCans"), FunctionDef::VgymTrash);
            } else {
                let name = storyline.name;
                let def = FunctionDef::Story(Rc::from(storyline.statements.into_boxed_slice()));
                self.functions
                    .insert(format!("storyline_{}", name).into(), def.clone());
                self.functions.insert(name.into(), def);
            }
        }
        if let Some(on_load) = scene.on_load {
            self.functions.insert(
                format!("{}OnLoad", scene.name).into(),
                FunctionDef::Story(Rc::from(on_load.statements.into_boxed_slice())),
            );
        }
    }

    /// Load a map by registering its ROM-backed functions without decoding
    /// their statements. Only the function invoked by a trigger is expanded.
    pub fn load_embedded_map(
        &mut self,
        map_name: &str,
        entries: &'static [(&'static str, &'static str, &'static [u8])],
    ) -> usize {
        self.functions = self.shared_functions.clone();
        let count = entries
            .iter()
            .filter(|(map, _, _)| *map == map_name)
            .count();
        self.functions.reserve(count * 2);
        let mut count = 0;
        for (_, function_name, bytes) in entries.iter().filter(|(map, _, _)| *map == map_name) {
            count += 1;
            if map_name == "VermilionGym" && *function_name == "trashCans" {
                self.functions
                    .insert(Cow::Borrowed("storyline_trashCans"), FunctionDef::VgymTrash);
                self.functions
                    .insert(Cow::Borrowed("trashCans"), FunctionDef::VgymTrash);
                continue;
            }
            let def = FunctionDef::Embedded(bytes);
            self.functions
                .insert(embedded_function_alias(function_name), def.clone());
            self.functions.insert(Cow::Borrowed(*function_name), def);
        }
        count
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

    pub fn gym_trash_indices(&self) -> (u8, u8) {
        (self.vgym.first as u8, self.vgym.second as u8)
    }

    pub fn set_gym_trash_indices(&mut self, first: u8, second: u8) {
        self.vgym.first = first as i32;
        self.vgym.second = second as i32;
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

    /// The current script language ("en" / "zh") driving `t()` dialogue selection.
    pub fn script_lang(&self) -> &str {
        self.interp.host().lang()
    }

    /// Whether `name` resolves to a registered function — exact name first,
    /// then the `storyline_` prefix (mirrors the Boa `resolved_fn_name`).
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
            || self
                .functions
                .contains_key(format!("storyline_{}", name).as_str())
    }

    /// Start a script function (no arguments — the only call form the
    /// overworld uses). Returns the first pending command, or `None` when
    /// the function completes without awaiting anything.
    pub fn call_function_no_args(
        &mut self,
        fn_name: &str,
    ) -> Result<Option<ScriptCommand>, String> {
        let variant = if fn_name == "talkOak1"
            && self.functions.contains_key("__native_talkOak1_choose")
        {
            Some(self.oaks_lab_oak1_variant())
        } else if matches!(fn_name, "talkScientist1" | "storyline_talkScientist1")
            && self.functions.contains_key("__native_talkScientist1_entry")
        {
            self.fossil_lab_variant()
        } else if fn_name == "coordDontGoAway"
            && self
                .functions
                .contains_key("__native_coordDontGoAway_battle_before")
        {
            Some(self.oaks_lab_exit_variant())
        } else if fn_name == "coordRivalBattle"
            && self
                .functions
                .contains_key("__native_coordRivalBattle_noop")
        {
            let flag = |name: &str| self.interp.host().flags.get(name).copied().unwrap_or(false);
            let wants = flag("EVENT_ROUTE22_RIVAL_WANTS_BATTLE");
            let early = wants
                && flag("EVENT_1ST_ROUTE22_RIVAL_BATTLE")
                && !flag("EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE");
            let late = wants
                && flag("EVENT_2ND_ROUTE22_RIVAL_BATTLE")
                && !flag("EVENT_BEAT_ROUTE22_RIVAL_2ND_BATTLE");
            match (early, late) {
                (true, false) => Some("__native_coordRivalBattle_early".to_string()),
                (false, true) => Some("__native_coordRivalBattle_late".to_string()),
                (false, false) => Some("__native_coordRivalBattle_noop".to_string()),
                // Corrupt/debug saves can enable both. Preserve the original
                // sequence and re-evaluation after the first battle's result.
                (true, true) => None,
            }
        } else {
            None
        };
        let fn_name = variant.as_deref().unwrap_or(fn_name);
        self.split_battle_active = fn_name == "__native_coordDontGoAway_battle_before";
        self.split_battle_waiting = false;
        self.split_fossil_active = fn_name == "__native_talkScientist1_entry";
        self.split_fossil_waiting = false;
        let resolved = if self.functions.contains_key(fn_name) {
            fn_name.to_string()
        } else {
            format!("storyline_{}", fn_name)
        };
        let def = self
            .functions
            .get(resolved.as_str())
            .cloned()
            .ok_or_else(|| format!("function not found: {}", fn_name))?;
        let outcome = match def {
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
            FunctionDef::Story(stmts) => self.start_story(fn_name, stmts.as_ref()),
            FunctionDef::Embedded(bytes) => {
                let stmts: Vec<StoryStmt> = serde_json::from_slice(bytes)
                    .map_err(|e| format!("decode embedded function {}: {}", fn_name, e))?;
                self.start_story(fn_name, &stmts)
            }
        };
        self.track_split_battle_command(&outcome);
        if let Ok(command) = &outcome {
            self.track_split_fossil_command(command);
        }
        outcome
    }

    fn fossil_lab_variant(&self) -> Option<String> {
        if !self.get_flag("EVENT_GAVE_FOSSIL_TO_LAB") {
            return Some("__native_talkScientist1_entry".into());
        }
        if self.get_flag("EVENT_LAB_STILL_REVIVING_FOSSIL") {
            return Some("__native_talkScientist1_still".into());
        }
        let mut selected = None;
        for (flag, name) in [
            ("EVENT_REVIVING_KABUTO", "__native_talkScientist1_ready_kabuto"),
            ("EVENT_REVIVING_OMANYTE", "__native_talkScientist1_ready_omanyte"),
            ("EVENT_REVIVING_AERODACTYL", "__native_talkScientist1_ready_aerodactyl"),
        ] {
            if self.get_flag(flag) {
                // Multiple ready flags only occur in corrupt/debug state.
                // Keep the full source's three independent If statements,
                // including later gifts when an earlier delivery fails.
                if selected.is_some() {
                    return None;
                }
                selected = Some(name);
            }
        }
        Some(selected.unwrap_or("__native_talkScientist1_ready_none").into())
    }

    fn track_split_fossil_command(&mut self, command: &Option<ScriptCommand>) {
        if self.split_fossil_active
            && matches!(command, Some(ScriptCommand::Custom { name, .. }) if name == "filterBag")
        {
            self.split_fossil_waiting = true;
            // The real filtered menu remains pending in the overworld. Its
            // returned item selects the continuation; discard the entry AST
            // before constructing the menu, as with Oak's battle handoff.
            self.interp.load_function(&[]);
            let _ = self.interp.tick();
        } else if self.state == InterpState::Idle {
            self.split_fossil_active = false;
        }
    }

    fn oaks_lab_oak1_variant(&self) -> String {
        let host = self.interp.host();
        let flag = |name: &str| host.flags.get(name).copied().unwrap_or(false);
        if flag("EVENT_GOT_POKEDEX") {
            let owned = host.numbers.get("pokedexOwned").copied().unwrap_or(0.0);
            if flag("EVENT_PALLET_AFTER_GETTING_POKEBALLS") || owned >= 2.0 {
                return format!(
                    "__native_talkOak1_rating_{}",
                    ((owned as usize) / 10).min(15)
                );
            }
            return "__native_talkOak1_dex_other".to_string();
        }
        if flag("EVENT_BATTLED_RIVAL_IN_OAKS_LAB")
            && host
                .sets
                .get("bag")
                .is_some_and(|items| items.iter().any(|item| item == "OAKS_PARCEL"))
        {
            return match host.player_y {
                1 => "__native_talkOak1_parcel_y1",
                3 => "__native_talkOak1_parcel_y3",
                _ => "__native_talkOak1_parcel",
            }.to_string();
        }
        if flag("EVENT_BATTLED_RIVAL_IN_OAKS_LAB") {
            return "__native_talkOak1_battled".to_string();
        }
        if flag("EVENT_GOT_STARTER") {
            return "__native_talkOak1_starter".to_string();
        }
        "__native_talkOak1_choose".to_string()
    }

    fn oaks_lab_exit_variant(&self) -> String {
        let host = self.interp.host();
        let flag = |name: &str| host.flags.get(name).copied().unwrap_or(false);
        if flag("EVENT_GOT_STARTER") && !flag("EVENT_BATTLED_RIVAL_IN_OAKS_LAB") {
            "__native_coordDontGoAway_battle_before".to_string()
        } else if flag("EVENT_OAK_ASKED_TO_CHOOSE_MON") && !flag("EVENT_GOT_STARTER") {
            "__native_coordDontGoAway_dont_go".to_string()
        } else {
            "__native_coordDontGoAway_noop".to_string()
        }
    }

    fn track_split_battle_command(
        &mut self,
        outcome: &Result<Option<ScriptCommand>, String>,
    ) {
        if self.split_battle_active
            && matches!(outcome, Ok(Some(ScriptCommand::StartBattle { .. })))
        {
            self.split_battle_waiting = true;
            // The continuation is stored as a separate ROM blob, so the
            // interpreter no longer needs the pre-battle frame. Release it
            // before the app constructs BattleScreen and its render caches;
            // waiting until the battle result arrives is too late for GBA's
            // peak heap usage.
            self.interp.load_function(&[]);
            let _ = self.interp.tick();
        }
    }

    fn start_story(
        &mut self,
        fn_name: &str,
        stmts: &[StoryStmt],
    ) -> Result<Option<ScriptCommand>, String> {
        self.interp.load_function(stmts);
        self.state = InterpState::Running;
        match self.interp.tick() {
            Ok(Some(command)) => {
                self.state = InterpState::WaitingForCommand;
                Ok(Some(command))
            }
            Ok(None) => {
                self.state = InterpState::Idle;
                Ok(None)
            }
            Err(error) => {
                log::warn!(target: "pokered::overworld", "[NativeScript] script error in {}: {}", fn_name, error);
                self.state = InterpState::Finished;
                Ok(None)
            }
        }
    }

    /// Called each frame: returns the pending command while waiting.
    pub fn tick(&mut self) -> Option<ScriptCommand> {
        if self.vgym.is_active() {
            return self.vgym.tick();
        }
        let command = match self.state {
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
        };
        if self.split_battle_active
            && matches!(command, Some(ScriptCommand::StartBattle { .. }))
        {
            self.split_battle_waiting = true;
            self.interp.load_function(&[]);
            let _ = self.interp.tick();
        }
        self.track_split_fossil_command(&command);
        command
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
        if self.split_fossil_waiting {
            let continuation = match &result {
                CommandResult::Text(item) if item == "DOME_FOSSIL" => "__native_talkScientist1_fossil_dome",
                CommandResult::Text(item) if item == "HELIX_FOSSIL" => "__native_talkScientist1_fossil_helix",
                CommandResult::Text(item) if item == "OLD_AMBER" => "__native_talkScientist1_fossil_amber",
                CommandResult::Text(item) if item.is_empty() => "__native_talkScientist1_cancel",
                _ => "__native_talkScientist1_noop",
            };
            self.split_fossil_active = false;
            self.split_fossil_waiting = false;
            return self.call_function_no_args(continuation);
        }
        if self.split_battle_waiting {
            let won = matches!(&result, CommandResult::Text(value) if value == "win");
            let continuation = if won {
                "__native_coordDontGoAway_battle_win"
            } else {
                "__native_coordDontGoAway_battle_loss"
            };
            self.split_battle_active = false;
            self.split_battle_waiting = false;

            let bytes = match self.functions.get(continuation) {
                Some(FunctionDef::Embedded(bytes)) => *bytes,
                _ => return Err(format!("missing split-battle continuation: {}", continuation)),
            };
            let statements: Vec<StoryStmt> = serde_json::from_slice(bytes)
                .map_err(|e| format!("decode {}: {}", continuation, e))?;
            return self.start_story(continuation, &statements);
        }
        let outcome = match self.interp.signal_done(result) {
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
        };
        self.track_split_battle_command(&outcome);
        if let Ok(command) = &outcome {
            self.track_split_fossil_command(command);
        }
        outcome
    }
}

impl Default for NativeScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Frame-level snapshot of the native script engine (agent M5): the
/// interpreter (execution state, stack, suspended await, host incl.
/// flags + bridge RNG) plus the VermilionGym puzzle state and the
/// engine-level InterpState. Function tables are NOT captured — they are
/// deterministic per map and rebuilt by `load_map_script_ex` on restore.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NativeScriptEngineSnapshot {
    interp: Interpreter<NativeHost>,
    vgym: VgymTrashState,
    state: InterpState,
    split_battle_active: bool,
    split_battle_waiting: bool,
    #[serde(default)]
    split_fossil_active: bool,
    #[serde(default)]
    split_fossil_waiting: bool,
}

impl NativeScriptEngine {
    /// Capture the engine's runtime state (see [`NativeScriptEngineSnapshot`]).
    pub fn snapshot(&self) -> NativeScriptEngineSnapshot {
        NativeScriptEngineSnapshot {
            interp: self.interp.clone(),
            vgym: self.vgym.clone(),
            state: self.state,
            split_battle_active: self.split_battle_active,
            split_battle_waiting: self.split_battle_waiting,
            split_fossil_active: self.split_fossil_active,
            split_fossil_waiting: self.split_fossil_waiting,
        }
    }

    /// Restore a previously captured state. The caller must have rebuilt
    /// the function tables for the current map first
    /// (`load_map_script_ex`); this only overwrites runtime state.
    pub fn restore_snapshot(&mut self, snapshot: &NativeScriptEngineSnapshot) {
        self.interp = snapshot.interp.clone();
        self.vgym = snapshot.vgym.clone();
        self.state = snapshot.state;
        self.split_battle_active = snapshot.split_battle_active;
        self.split_battle_waiting = snapshot.split_battle_waiting;
        self.split_fossil_active = snapshot.split_fossil_active;
        self.split_fossil_waiting = snapshot.split_fossil_waiting;
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

    pub fn gym_trash_indices(&self) -> (u8, u8) {
        match self {
            Self::Native(engine) => engine.gym_trash_indices(),
            #[cfg(feature = "script-boa")]
            Self::Boa(_) => (0, 0),
        }
    }

    pub fn set_gym_trash_indices(&mut self, first: u8, second: u8) {
        if let Self::Native(engine) = self {
            engine.set_gym_trash_indices(first, second);
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
            OverworldScriptEngine::Boa(e) => e.get_all_flags().into_iter().collect(),
            OverworldScriptEngine::Native(e) => e.get_all_flags(),
        }
    }

    pub fn seed_flags(&mut self, flags: &HashMap<String, bool>) {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => {
                let hosted_flags = flags
                    .iter()
                    .map(|(key, value)| (key.clone(), *value))
                    .collect::<std::collections::HashMap<_, _>>();
                e.seed_flags(&hosted_flags)
            }
            OverworldScriptEngine::Native(e) => e.seed_flags(flags),
        }
    }

    /// Frame-level snapshot of the script engine (agent M5). `None`
    /// under the `script-boa` feature — the Boa JS heap is not
    /// serializable; the default native engine is fully covered.
    pub fn snapshot(&self) -> Option<NativeScriptEngineSnapshot> {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(_) => None,
            OverworldScriptEngine::Native(e) => Some(e.snapshot()),
        }
    }

    /// Restore a script-engine snapshot (function tables must already
    /// be rebuilt for the current map). No-op under `script-boa`.
    pub fn restore_snapshot(&mut self, snapshot: &NativeScriptEngineSnapshot) {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(_) => {}
            OverworldScriptEngine::Native(e) => e.restore_snapshot(snapshot),
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

    /// The current script language ("en" / "zh"), when the active engine
    /// exposes it. The dormant Boa fallback keeps its language inside a
    /// private bridge with no getter, hence `None` there.
    pub fn script_lang(&self) -> Option<&str> {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(_) => None,
            OverworldScriptEngine::Native(e) => Some(e.script_lang()),
        }
    }

    /// Load raw JS into the Boa engine.
    ///
    /// This API does not exist in native-AST builds, so selecting the wrong
    /// representation fails at compile time instead of silently succeeding.
    #[cfg(feature = "script-boa")]
    pub fn load_script(&mut self, _source: &str) -> Result<(), String> {
        match self {
            OverworldScriptEngine::Boa(e) => e
                .load_script(_source)
                .map_err(|err| format!("JS load failed: {}", err)),
            OverworldScriptEngine::Native(_) => {
                Err("raw JavaScript is not supported by the native AST engine".to_string())
            }
        }
    }

    /// Load a raw JS shared module into the Boa engine.
    #[cfg(feature = "script-boa")]
    pub fn load_shared_module(&mut self, _name: &str, _source: &str) -> Result<(), String> {
        match self {
            OverworldScriptEngine::Boa(e) => e
                .load_shared_module(_name, _source)
                .map_err(|err| format!("JS shared module load failed: {}", err)),
            OverworldScriptEngine::Native(_) => {
                Err("raw JavaScript modules are not supported by the native AST engine".to_string())
            }
        }
    }

    pub fn has_function(&mut self, fn_name: &str) -> bool {
        match self {
            #[cfg(feature = "script-boa")]
            OverworldScriptEngine::Boa(e) => e.has_function(fn_name),
            OverworldScriptEngine::Native(e) => e.has_function(fn_name),
        }
    }

    /// Register a shared native scene (e.g. `shared/pokecenter`).
    #[cfg(not(feature = "script-boa"))]
    pub fn register_shared_scene_native(&mut self, scene: &dotzuki_engine_dsl::ast::GameScene) {
        match self {
            OverworldScriptEngine::Native(e) => e.register_shared_scene(scene),
        }
    }

    /// Register shared native functions while leaving their statements in ROM.
    #[cfg(not(feature = "script-boa"))]
    pub fn register_embedded_shared_native(
        &mut self,
        map_name: &str,
        entries: &'static [(&'static str, &'static str, &'static [u8])],
    ) {
        match self {
            OverworldScriptEngine::Native(engine) => {
                engine.register_embedded_shared(map_name, entries)
            }
        }
    }

    /// Load a map's native scene AST.
    #[cfg(not(feature = "script-boa"))]
    pub fn load_map_native(&mut self, map_name: &str, scene: &dotzuki_engine_dsl::ast::GameScene) {
        match self {
            OverworldScriptEngine::Native(e) => e.load_map(map_name, scene),
        }
    }

    /// Register an embedded map's native functions without decoding them.
    #[cfg(not(feature = "script-boa"))]
    pub fn load_embedded_map_native(
        &mut self,
        map_name: &str,
        entries: &'static [(&'static str, &'static str, &'static [u8])],
    ) -> usize {
        match self {
            OverworldScriptEngine::Native(engine) => engine.load_embedded_map(map_name, entries),
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
    fn oaks_lab_rival_battle_releases_pre_battle_ast_before_continuation() {
        let mut engine = NativeScriptEngine::new();
        engine.load_embedded_map(
            "OaksLab",
            pokered_data::embedded_scenes::scene_functions(),
        );
        engine.set_flag("EVENT_GOT_STARTER", true);

        let mut next = engine.call_function_no_args("coordDontGoAway").unwrap();
        let mut saw_battle = false;
        for _ in 0..32 {
            match next {
                Some(ScriptCommand::StartBattle { ref trainer_id }) => {
                    assert_eq!(trainer_id, "OPP_RIVAL1");
                    saw_battle = true;
                    next = engine
                        .signal_done(CommandResult::Text("win".to_string()))
                        .unwrap();
                    break;
                }
                Some(_) => next = engine.signal_done(CommandResult::Void).unwrap(),
                None => break,
            }
        }
        assert!(saw_battle, "split pre-battle segment must reach Blue's battle");
        assert!(next.is_some(), "winning must start the post-battle continuation");
        assert!(!engine.split_battle_active);
        assert!(!engine.split_battle_waiting);
    }

    #[test]
    fn oaks_lab_lazy_dialogue_matches_full_script_at_every_dex_count_and_story_branch() {
        fn run(
            owned: usize,
            bits: u8,
            bag: &[String],
            lang: &str,
            player_y: u8,
            lazy: bool,
        ) -> (Vec<ScriptCommand>, HashMap<String, bool>) {
            let mut engine = NativeScriptEngine::new();
            engine.load_embedded_map("OaksLab", pokered_data::embedded_scenes::scene_functions());
            if !lazy {
                // Disable the native selector, retaining the exact original
                // serialized storyline as the semantic oracle.
                engine.functions.remove("__native_talkOak1_choose");
            }
            for (bit, flag) in [
                "EVENT_GOT_POKEDEX",
                "EVENT_BATTLED_RIVAL_IN_OAKS_LAB",
                "EVENT_GOT_STARTER",
                "EVENT_PALLET_AFTER_GETTING_POKEBALLS",
                "EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE",
                "EVENT_GOT_POKEBALLS_FROM_OAK",
            ]
            .iter()
            .enumerate()
            {
                engine.set_flag(flag, bits & (1 << bit) != 0);
            }
            engine.seed_number("pokedexOwned", owned as f64);
            engine.seed_number("pokedexSeen", 151.0);
            engine.seed_set("bag", bag);
            engine.set_lang(lang);
            engine.set_player_position(0, player_y);
            let mut commands = Vec::new();
            let mut command = engine.call_function_no_args("talkOak1").unwrap();
            for _ in 0..128 {
                let Some(next) = command else {
                    return (commands, engine.get_all_flags());
                };
                commands.push(next);
                command = engine.signal_done(CommandResult::Void).unwrap();
            }
            panic!("Oak dialogue did not complete");
        }
        for lang in ["en", "zh"] {
            for owned in 0..=151 {
                assert_eq!(
                    run(owned, 15, &[], lang, 0, true),
                    run(owned, 15, &[], lang, 0, false),
                    "rating changed at {owned} owned, {lang}"
                );
            }
            for bits in 0..64 {
                for bag in [vec![], vec!["POKE_BALL".into()], vec!["OAKS_PARCEL".into()]] {
                    for player_y in [0, 1, 2, 3, 4] {
                        assert_eq!(
                            run(1, bits, &bag, lang, player_y, true),
                            run(1, bits, &bag, lang, player_y, false),
                            "story branch changed for flags={bits}, bag={bag:?}, row={player_y}, {lang}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn native_host_implements_every_cataloged_capability() {
        let mut host = NativeHost::new();
        for &name in pokered_data::script_function_catalog::POKERED_SCRIPT_FUNCTIONS {
            if let Err(error) = host.call(name, &[]) {
                assert!(
                    !error.contains("unknown game function"),
                    "cataloged capability {name:?} is missing from NativeHost"
                );
            }
        }
    }

    #[test]
    fn rival_battle_set_resumes_scene_with_battle_result() {
        let scene = dotzuki_engine_dsl::compiler::compile_scene_to_ast(
            r#"game_scene Test {
  @storyline("rival") {
    result = startBattleSet("OPP_RIVAL1", 6)
    @if (result == "win") { setFlag("RIVAL_WON") }
  }
}"#,
            "Test",
        ).expect("scene compiles");
        for won in [false, true] {
            let mut engine = NativeScriptEngine::new();
            engine.load_map("Test", &scene);
            assert_eq!(engine.call_function_no_args("rival").unwrap(),
                Some(custom("startBattleSet", vec![json!("OPP_RIVAL1"), json!(6)])));
            assert!(!engine.get_flag("RIVAL_WON"));
            engine.signal_done(CommandResult::Text(if won { "win" } else { "loss" }.into())).unwrap();
            assert_eq!(engine.get_flag("RIVAL_WON"), won);
        }
    }

    #[test]
    fn cerulean_rival_scene_completes_after_victory() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("CeruleanCity").unwrap();
        let mut engine = NativeScriptEngine::new();
        engine.load_map("CeruleanCity", &scene);
        engine.set_player_position(20, 6);
        let mut next = engine.call_function_no_args("coordRivalBattle").unwrap();
        let mut battles = 0;
        for _ in 0..100 {
            let Some(cmd) = next else { break };
            let result = if cmd == custom("startBattleSet", vec![json!("OPP_RIVAL1"), json!(6)]) {
                battles += 1;
                CommandResult::Text("win".into())
            } else { CommandResult::Void };
            next = engine.signal_done(result).unwrap();
        }
        assert_eq!(battles, 1);
        assert!(engine.get_flag("EVENT_BEAT_CERULEAN_RIVAL"));
    }

    #[test]
    fn ship_gate_recognizes_the_inventory_ticket_name() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("VermilionCity").unwrap();
        for has_ticket in [false, true] {
            let mut engine = NativeScriptEngine::new();
            engine.load_map("VermilionCity", &scene);
            let bag = if has_ticket { vec![pokered_data::items::ItemId::SsTicket.const_name()] } else { vec![] };
            engine.seed_set("bag", &bag);
            let mut next = engine.call_function_no_args("coordSailorGate").unwrap();
            let mut welcomed = false;
            let mut pushed_back = false;
            for _ in 0..20 {
                let Some(cmd) = next else { break };
                if let ScriptCommand::ShowText { text } = &cmd {
                    welcomed |= text.contains("flashed");
                }
                pushed_back |= matches!(cmd, ScriptCommand::MovePlayerRelative { .. });
                next = engine.signal_done(CommandResult::Void).unwrap();
            }
            assert_eq!(welcomed, has_ticket);
            assert_eq!(pushed_back, !has_ticket);
        }
    }

    #[test]
    fn ss_anne_rival_scene_completes_after_victory() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("SSAnne2F").unwrap();
        let mut engine = NativeScriptEngine::new();
        engine.load_map("SSAnne2F", &scene);
        engine.set_player_position(36, 8);
        let mut next = engine.call_function_no_args("coordRivalBattle").unwrap();
        let mut battles = 0;
        for _ in 0..100 {
            let Some(cmd) = next else { break };
            let result = if matches!(cmd, ScriptCommand::StartBattle { .. }) {
                battles += 1;
                CommandResult::Text("win".into())
            } else { CommandResult::Void };
            next = engine.signal_done(result).unwrap();
        }
        assert_eq!(battles, 1);
        assert!(engine.get_flag("EVENT_BEAT_SS_ANNE_RIVAL"));
    }

    #[test]
    fn tower_rival_scene_completes_after_victory() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("PokemonTower2F").unwrap();
        let mut engine = NativeScriptEngine::new();
        engine.load_map("PokemonTower2F", &scene);
        engine.set_player_position(15, 5);
        let mut next = engine.call_function_no_args("coordRivalBattle").unwrap();
        let mut battles = 0;
        for _ in 0..100 {
            let Some(cmd) = next else { break };
            let result = if cmd == custom("startBattleSet", vec![json!("OPP_RIVAL2"), json!(3)]) {
                battles += 1;
                CommandResult::Text("win".into())
            } else { CommandResult::Void };
            next = engine.signal_done(result).unwrap();
        }
        assert_eq!(battles, 1);
        assert!(engine.get_flag("EVENT_BEAT_POKEMON_TOWER_RIVAL"));
    }

    #[test]
    fn silph_rival_scene_uses_silph_party_and_only_completes_on_victory() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("SilphCo7F").unwrap();
        for outcome in ["win", "lose"] {
            let mut engine = NativeScriptEngine::new();
            engine.load_map("SilphCo7F", &scene);
            engine.set_player_position(3, 3);
            let mut next = engine.call_function_no_args("coordRivalBattle").unwrap();
            let mut battles = Vec::new();
            for _ in 0..100 {
                let Some(cmd) = next else { break };
                let is_battle = matches!(&cmd, ScriptCommand::StartBattle { .. })
                    || matches!(&cmd, ScriptCommand::Custom { name, .. } if name == "startBattleSet");
                let result = if is_battle {
                    battles.push(cmd);
                    CommandResult::Text(outcome.into())
                } else { CommandResult::Void };
                next = engine.signal_done(result).unwrap();
            }
            assert_eq!(engine.get_flag("EVENT_BEAT_SILPH_CO_RIVAL"), outcome == "win");
            assert_eq!(battles, vec![custom("startBattleSet", vec![json!("OPP_RIVAL2"), json!(6)])]);
        }
    }

    #[test]
    fn silph_giovanni_scene_uses_second_party_for_both_triggers() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("SilphCo11F").unwrap();
        for trigger in ["giovanniStep", "talkGiovanni"] {
            let mut engine = NativeScriptEngine::new();
            engine.load_map("SilphCo11F", &scene);
            engine.set_player_position(6, 13);
            let mut next = engine.call_function_no_args(trigger).unwrap();
            let mut battles = Vec::new();
            for _ in 0..100 {
                let Some(cmd) = next else { break };
                let result = if let ScriptCommand::StartBattle { trainer_id } = cmd {
                    battles.push(trainer_id);
                    CommandResult::Text("win".into())
                } else { CommandResult::Void };
                next = engine.signal_done(result).unwrap();
            }
            assert_eq!(battles, ["OPP_GIOVANNI2"]);
            assert!(engine.get_flag("EVENT_BEAT_SILPH_CO_GIOVANNI"));
        }
    }

    #[test]
    fn cinnabar_trainer_victories_unlock_only_the_matching_gate() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("CinnabarGym").unwrap();
        for n in 0..7 {
            for won in [false, true] {
                let mut engine = NativeScriptEngine::new();
                engine.load_map("CinnabarGym", &scene);
                let mut next = engine.call_function_no_args(&format!("talkSuperNerd{}", n+1)).unwrap();
                let mut battles = 0;
                for _ in 0..100 {
                    let Some(cmd) = next else { break };
                    let result = if matches!(cmd, ScriptCommand::StartBattle { .. }) {
                        battles += 1;
                        CommandResult::Text(if won { "win" } else { "loss" }.into())
                    } else { CommandResult::Void };
                    next = engine.signal_done(result).unwrap();
                }
                assert_eq!(battles, 1);
                for gate in 0..7 {
                    assert_eq!(engine.get_flag(&format!("EVENT_CINNABAR_GYM_GATE{gate}_UNLOCKED")), won && gate == n);
                }
            }
        }
    }

    #[test]
    fn brock_victory_expires_the_optional_route22_battle() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("PewterGym").unwrap();
        for outcome in ["win", "lose"] {
            let mut engine = NativeScriptEngine::new();
            engine.load_map("PewterGym", &scene);
            engine.set_flag("EVENT_1ST_ROUTE22_RIVAL_BATTLE", true);
            engine.set_flag("EVENT_ROUTE22_RIVAL_WANTS_BATTLE", true);
            let mut next = engine.call_function_no_args("talkBrock").unwrap();
            for _ in 0..100 {
                let Some(cmd) = next else { break };
                let result = if matches!(cmd, ScriptCommand::StartBattle { .. }) {
                    CommandResult::Text(outcome.into())
                } else { CommandResult::Void };
                next = engine.signal_done(result).unwrap();
            }
            assert_eq!(engine.get_flag("EVENT_1ST_ROUTE22_RIVAL_BATTLE"), outcome != "win");
            assert_eq!(engine.get_flag("EVENT_ROUTE22_RIVAL_WANTS_BATTLE"), outcome != "win");
        }
    }

    #[test]
    fn route22_entry_repairs_expired_early_encounter_without_clearing_final_encounter() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("Route22").unwrap();
        for final_encounter in [false, true] {
            let mut engine = NativeScriptEngine::new();
            engine.load_map("Route22", &scene);
            engine.set_flag("EVENT_BEAT_BROCK", true);
            engine.set_flag("EVENT_1ST_ROUTE22_RIVAL_BATTLE", true);
            engine.set_flag("EVENT_2ND_ROUTE22_RIVAL_BATTLE", final_encounter);
            engine.set_flag("EVENT_ROUTE22_RIVAL_WANTS_BATTLE", true);
            let mut next = engine.call_function_no_args("Route22OnLoad").unwrap();
            for _ in 0..30 {
                if next.is_none() { break; }
                next = engine.signal_done(CommandResult::Void).unwrap();
            }
            assert!(!engine.get_flag("EVENT_1ST_ROUTE22_RIVAL_BATTLE"));
            assert_eq!(engine.get_flag("EVENT_ROUTE22_RIVAL_WANTS_BATTLE"), final_encounter);
            if final_encounter {
                engine.set_player_position(29, 4);
                next = engine.call_function_no_args("coordRivalBattle").unwrap();
                let mut battle = None;
                for _ in 0..100 {
                    let Some(cmd) = next else { break };
                    if matches!(&cmd, ScriptCommand::Custom { name, .. } if name == "startBattleSet") {
                        battle = Some(cmd);
                        break;
                    }
                    next = engine.signal_done(CommandResult::Void).unwrap();
                }
                assert_eq!(battle, Some(custom("startBattleSet", vec![json!("OPP_RIVAL2"), json!(9)])));
            }
        }
    }

    #[test]
    fn route22_rivals_complete_only_after_winning_the_correct_stage() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("Route22").unwrap();
        for (stage, class, base) in [("1ST", "OPP_RIVAL1", 3), ("2ND", "OPP_RIVAL2", 9)] {
            for outcome in ["win", "lose"] {
                let mut engine = NativeScriptEngine::new();
                engine.load_map("Route22", &scene);
                engine.set_player_position(29, 4);
                engine.set_flag("EVENT_ROUTE22_RIVAL_WANTS_BATTLE", true);
                engine.set_flag(&format!("EVENT_{stage}_ROUTE22_RIVAL_BATTLE"), true);
                let mut next = engine.call_function_no_args("coordRivalBattle").unwrap();
                let mut battles = Vec::new();
                for _ in 0..150 {
                    let Some(cmd) = next else { break };
                    let result = if matches!(&cmd, ScriptCommand::Custom { name, .. } if name == "startBattleSet") {
                        battles.push(cmd);
                        CommandResult::Text(outcome.into())
                    } else { CommandResult::Void };
                    next = engine.signal_done(result).unwrap();
                }
                assert_eq!(battles, vec![custom("startBattleSet", vec![json!(class), json!(base)])]);
                assert_eq!(engine.get_flag(&format!("EVENT_BEAT_ROUTE22_RIVAL_{stage}_BATTLE")), outcome == "win");
                assert_eq!(engine.get_flag("EVENT_ROUTE22_RIVAL_WANTS_BATTLE"), outcome != "win");
            }
        }
    }

    #[test]
    fn route22_lazy_encounters_match_full_script_for_all_flags_results_and_rows() {
        fn run(
            bits: u8,
            y: u8,
            outcome: &str,
            lang: &str,
            lazy: bool,
        ) -> (Vec<ScriptCommand>, HashMap<String, bool>) {
            let mut engine = NativeScriptEngine::new();
            engine.load_embedded_map("Route22", pokered_data::embedded_scenes::scene_functions());
            if !lazy {
                engine.functions.remove("__native_coordRivalBattle_noop");
            }
            for (bit, flag) in [
                "EVENT_ROUTE22_RIVAL_WANTS_BATTLE",
                "EVENT_1ST_ROUTE22_RIVAL_BATTLE",
                "EVENT_2ND_ROUTE22_RIVAL_BATTLE",
                "EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE",
                "EVENT_BEAT_ROUTE22_RIVAL_2ND_BATTLE",
            ]
            .iter()
            .enumerate()
            {
                engine.set_flag(flag, bits & (1 << bit) != 0);
            }
            engine.set_player_position(29, y);
            engine.set_lang(lang);
            let mut commands = Vec::new();
            let mut command = engine.call_function_no_args("coordRivalBattle").unwrap();
            for _ in 0..256 {
                let Some(next) = command else {
                    return (commands, engine.get_all_flags());
                };
                let result = if matches!(&next, ScriptCommand::Custom { name, .. } if name == "startBattleSet")
                {
                    CommandResult::Text(outcome.into())
                } else {
                    CommandResult::Void
                };
                commands.push(next);
                command = engine.signal_done(result).unwrap();
            }
            panic!("Route22 script did not complete");
        }
        for bits in 0..32 {
            for y in [4, 5] {
                for outcome in ["win", "lose"] {
                    for lang in ["en", "zh"] {
                        assert_eq!(
                            run(bits, y, outcome, lang, true),
                            run(bits, y, outcome, lang, false),
                            "flags={bits}, y={y}, result={outcome}, lang={lang}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn champion_scene_completes_only_after_victory() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("ChampionsRoom").unwrap();
        for outcome in ["win", "lose"] {
            let mut engine = NativeScriptEngine::new();
            engine.load_map("ChampionsRoom", &scene);
            let mut next = engine.call_function_no_args("ChampionsRoomOnLoad").unwrap();
            let mut battles = 0;
            for _ in 0..150 {
                let Some(cmd) = next else { break };
                let result = if let ScriptCommand::StartBattle { trainer_id } = cmd {
                    assert_eq!(trainer_id, "OPP_RIVAL3");
                    battles += 1;
                    CommandResult::Text(outcome.into())
                } else { CommandResult::Void };
                next = engine.signal_done(result).unwrap();
            }
            assert_eq!(battles, 1);
            assert_eq!(engine.get_flag("EVENT_BEAT_CHAMPION_RIVAL"), outcome == "win");
        }
    }

    #[test]
    fn npc_rewards_keep_their_completion_flags_unset_until_item_is_accepted() {
        let cases = [
            ("BikeShop", "talkBikeShopClerk", "", "EVENT_GOT_BICYCLE", "BIKE_VOUCHER"),
            ("CeruleanGym", "talkMisty", "EVENT_BEAT_MISTY", "EVENT_GOT_TM11", ""),
            ("CinnabarGym", "talkBlaine", "EVENT_BEAT_BLAINE", "EVENT_GOT_TM38", ""),
            ("ViridianCity", "talkFisher", "", "EVENT_GOT_TM42", ""),
            ("CeladonDiner", "talkGymGuide", "", "EVENT_GOT_COIN_CASE", ""),
            ("CinnabarLabMetronomeRoom", "talkScientist1", "", "EVENT_GOT_TM35", ""),
            ("Route11Gate2F", "talkOaksAide", "", "EVENT_GOT_ITEMFINDER", ""),
            ("Route12Gate2F", "talkBrunetteGirl", "", "EVENT_GOT_TM39", ""),
            ("Route12SuperRodHouse", "talkFishingGuru", "", "EVENT_GOT_SUPER_ROD", ""),
            ("Route15Gate2F", "talkOaksAide", "", "EVENT_GOT_EXP_ALL", ""),
            ("SafariZoneSecretHouse", "talkFishingGuru", "", "EVENT_GOT_HM03", ""),
            ("WardensHouse", "talkWarden", "EVENT_GAVE_GOLD_TEETH", "EVENT_GOT_HM04", ""),
        ];
        for (map, function, prerequisite, completed, bag_item) in cases {
            let scene = pokered_data::embedded_scenes::get_scene_ast(map).unwrap();
            let mut engine = NativeScriptEngine::new();
            engine.load_map(map, &scene);
            engine.seed_number("pokedexOwned", 50.0);
            if !prerequisite.is_empty() { engine.set_flag(prerequisite, true); }
            if !bag_item.is_empty() { engine.seed_set("bag", &[bag_item.to_string()]); }
            for accepted in [false, true] {
                let mut next = engine.call_function_no_args(function).unwrap();
                let mut offered = 0;
                let mut voucher_removed = false;
                for _ in 0..100 {
                    let Some(command) = next else { break };
                    let result = match command {
                        ScriptCommand::GiveItem { .. } => { offered += 1; CommandResult::Bool(accepted) },
                        ScriptCommand::ShowChoice { .. } => CommandResult::Number(0.0),
                        ScriptCommand::TakeItem { .. } => { voucher_removed = true; CommandResult::Void },
                        _ => CommandResult::Void,
                    };
                    next = engine.signal_done(result).unwrap();
                }
                assert_eq!(offered, 1, "{map}: reward was not re-offered");
                assert_eq!(engine.get_flag(completed), accepted, "{map}: wrong completion flag");
                if map == "BikeShop" { assert_eq!(voucher_removed, accepted); }
            }
        }
    }

    #[test]
    fn rocket_keeps_stolen_tm_and_does_not_disappear_when_bag_rejects_it() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("CeruleanCity").unwrap();
        let mut engine = NativeScriptEngine::new();
        engine.load_map("CeruleanCity", &scene);
        engine.set_flag("EVENT_BEAT_CERULEAN_ROCKET_THIEF", true);
        for accepted in [false, true] {
            let mut next = engine.call_function_no_args("talkRocket").unwrap();
            let mut hidden = false;
            for _ in 0..50 {
                let Some(command) = next else { break };
                let result = if matches!(command, ScriptCommand::GiveItem { .. }) {
                    CommandResult::Bool(accepted)
                } else {
                    hidden |= matches!(command, ScriptCommand::HideObjectByName { .. });
                    CommandResult::Void
                };
                next = engine.signal_done(result).unwrap();
            }
            assert_eq!(hidden, accepted);
        }
    }

    #[test]
    fn magikarp_sale_uses_gift_acceptance_for_a_full_party_and_charges_once() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("MtMoonPokecenter").unwrap();
        for accepted in [false, true] {
            let mut engine = NativeScriptEngine::new();
            engine.load_map("MtMoonPokecenter", &scene);
            engine.seed_number("money", 500.0);
            engine.seed_number("partyCount", 6.0);
            let mut next = engine.call_function_no_args("talkMagikarpSalesman").unwrap();
            let mut offers = 0;
            let mut charged = 0;
            for _ in 0..50 {
                let Some(command) = next else { break };
                let result = match command {
                    ScriptCommand::GiveMonster { .. } => { offers += 1; CommandResult::Bool(accepted) },
                    ScriptCommand::ShowChoice { .. } => CommandResult::Number(0.0),
                    ScriptCommand::TakeMoney { amount } => { charged += amount; CommandResult::Void },
                    _ => CommandResult::Void,
                };
                next = engine.signal_done(result).unwrap();
            }
            assert_eq!(offers, 1);
            assert_eq!(charged, if accepted { 500 } else { 0 });
            assert_eq!(engine.get_flag("EVENT_BOUGHT_MAGIKARP"), accepted);
        }
    }

    #[test]
    fn vgym_original_mask_can_put_second_switch_in_can_zero() {
        // Original mask/DEC underflow reads zero padding, even far from can 0.
        assert_eq!(VgymTrashState::second_index(14, 0), 0);
        assert_eq!(VgymTrashState::second_index(14, 0x20), 13);
        // For mask 4, only candidate 4 or the underflow is reachable.
        assert_eq!(VgymTrashState::second_index(4, 0x40), 7);
        assert_eq!(VgymTrashState::second_index(4, 0x10), 0);
    }

    #[test]
    fn vgym_restored_first_lock_flag_checks_saved_second_index() {
        let mut host = NativeHost::new();
        let mut state = VgymTrashState::new();
        state.first = 14;
        state.second = 0;
        host.flags.insert("EVENT_1ST_LOCK_OPENED".into(), true);
        host.player_x = 1;
        host.player_y = 8;
        host.texts.insert("playerFacing".into(), "up".into());
        state.start(&mut host);
        state.next_command(&mut host);
        assert!(host.flags["EVENT_2ND_LOCK_OPENED"]);
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
            vgym.second == 0 || vgym.second == 10,
            "second={}",
            vgym.second
        );
    }

    #[test]
    fn imported_trash_indices_preserve_original_sram_bytes() {
        let mut engine = NativeScriptEngine::new();
        engine.set_gym_trash_indices(255, 254);
        assert_eq!(engine.gym_trash_indices(), (255, 254));
        let snapshot = engine.snapshot();
        let mut restored = NativeScriptEngine::new();
        restored.restore_snapshot(&snapshot);
        assert_eq!(restored.gym_trash_indices(), (255, 254));
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
        // Default saved index is can 0, so this opens the first lock.
        assert!(matches!(cmd, Some(ScriptCommand::PlaySound { .. })));

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
        e.set_player_position(9, 12);
        e.seed_text("playerFacing", "up"); // can 14 differs from either original result (0 or 10)
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
        // Pin the restored second switch at can 4 and
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
    // Drive real embedded map handlers, including command-return values, rather
    // than asserting their source spelling. The caller chooses menu responses.
    fn drive_fidelity_scene(
        engine: &mut NativeScriptEngine,
        handler: &str,
        gift_ok: bool,
        outcome: &str,
        choices: &[usize],
    ) -> Vec<ScriptCommand> {
        let mut next = engine.call_function_no_args(handler).unwrap();
        let mut commands = Vec::new();
        let mut choices = choices.iter();
        for _ in 0..150 {
            let Some(command) = next else {
                assert!(engine.is_idle());
                return commands;
            };
            let result = match &command {
                ScriptCommand::GiveMonster { .. } | ScriptCommand::GiveItem { .. } => CommandResult::Bool(gift_ok),
                ScriptCommand::StartBattle { .. } | ScriptCommand::StartWildBattle { .. } => {
                    CommandResult::Text(outcome.into())
                }
                ScriptCommand::ShowChoice { .. } => {
                    CommandResult::Number(*choices.next().expect("menu response") as f64)
                }
                _ => CommandResult::Void,
            };
            commands.push(command);
            next = engine.signal_done(result).unwrap();
        }
        panic!("handler {handler} did not finish");
    }

    #[test]
    fn fidelity_gifts_commit_choice_and_lapras_only_after_delivery() {
        for (map, handler, flag, toggle) in [
            ("SilphCo7F", "talkSilphWorkerM1", "EVENT_GOT_LAPRAS", ""),
            ("FightingDojo", "talkHitmonleeBall", "EVENT_GOT_HITMONLEE", "FIGHTINGDOJO_HITMONLEE_POKE_BALL"),
            ("FightingDojo", "talkHitmonchanBall", "EVENT_GOT_HITMONCHAN", "FIGHTINGDOJO_HITMONCHAN_POKE_BALL"),
        ] {
            for success in [false, true] {
                let scene = pokered_data::embedded_scenes::get_scene_ast(map).unwrap();
                let mut engine = NativeScriptEngine::new();
                engine.load_map(map, &scene);
                let commands = drive_fidelity_scene(&mut engine, handler, success, "", &[0]);
                assert_eq!(commands.iter().filter(|c| matches!(c, ScriptCommand::GiveMonster { .. })).count(), 1);
                assert_eq!(engine.get_flag(flag), success, "{map} {handler}");
                if !toggle.is_empty() {
                    assert_eq!(commands.iter().any(|c| matches!(c, ScriptCommand::HideObjectByName { toggle_id } if toggle_id == toggle)), success);
                    assert_eq!(engine.get_flag("EVENT_DEFEATED_FIGHTING_DOJO"), success);
                } else {
                    assert_eq!(commands.iter().any(|c| matches!(c, ScriptCommand::ShowText { text } if text.starts_with("It's LAPRAS."))), success);
                }
            }
        }
    }

    #[test]
    fn fidelity_fossils_remain_ready_after_a_full_box_and_can_be_retried() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("CinnabarLabFossilRoom").unwrap();
        for species in ["KABUTO", "OMANYTE", "AERODACTYL"] {
            let mut engine = NativeScriptEngine::new();
            engine.load_map("CinnabarLabFossilRoom", &scene);
            let reviving = format!("EVENT_REVIVING_{species}");
            engine.set_flag("EVENT_GAVE_FOSSIL_TO_LAB", true);
            engine.set_flag(&reviving, true);
            let commands = drive_fidelity_scene(&mut engine, "talkScientist1", false, "", &[]);
            assert!(commands.contains(&ScriptCommand::GiveMonster { species: species.into(), level: 30 }));
            assert!(engine.get_flag("EVENT_GAVE_FOSSIL_TO_LAB"));
            assert!(engine.get_flag(&reviving));
            // The original sets HANDING_OVER before the failed GivePokemon.
            assert!(engine.get_flag("EVENT_LAB_HANDING_OVER_FOSSIL_MON"));
            drive_fidelity_scene(&mut engine, "talkScientist1", true, "", &[]);
            assert!(!engine.get_flag("EVENT_GAVE_FOSSIL_TO_LAB"));
            assert!(!engine.get_flag(&reviving));
            assert!(!engine.get_flag("EVENT_LAB_HANDING_OVER_FOSSIL_MON"));
        }
    }

    fn drive_fossil_commands(
        engine: &mut NativeScriptEngine,
        mut next: Option<ScriptCommand>,
        selected: &str,
        choice: usize,
        gift_ok: bool,
    ) -> Vec<ScriptCommand> {
        let mut commands = Vec::new();
        for _ in 0..64 {
            let Some(command) = next else {
                assert!(engine.is_idle());
                return commands;
            };
            let result = match &command {
                ScriptCommand::Custom { name, .. } if name == "filterBag" => CommandResult::Text(selected.into()),
                ScriptCommand::ShowChoice { .. } => CommandResult::Number(choice as f64),
                ScriptCommand::GiveMonster { .. } => CommandResult::Bool(gift_ok),
                _ => CommandResult::Void,
            };
            commands.push(command);
            next = engine.signal_done(result).unwrap();
        }
        panic!("fossil doctor did not finish");
    }

    #[test]
    fn fossil_lab_lazy_branches_match_full_dialogue_menus_flags_and_delivery() {
        fn run(bits: u8, bag_mask: u8, selected: &str, choice: usize, gift_ok: bool, lang: &str, lazy: bool)
            -> (Vec<ScriptCommand>, HashMap<String, bool>)
        {
            let mut engine = NativeScriptEngine::new();
            engine.load_embedded_map("CinnabarLabFossilRoom", pokered_data::embedded_scenes::scene_functions());
            if !lazy {
                engine.functions.remove("__native_talkScientist1_entry");
            }
            for (bit, flag) in [
                "EVENT_GAVE_FOSSIL_TO_LAB", "EVENT_LAB_STILL_REVIVING_FOSSIL",
                "EVENT_REVIVING_KABUTO", "EVENT_REVIVING_OMANYTE", "EVENT_REVIVING_AERODACTYL",
            ].iter().enumerate() {
                engine.set_flag(flag, bits & (1 << bit) != 0);
            }
            let bag = ["DOME_FOSSIL", "HELIX_FOSSIL", "OLD_AMBER"].iter().enumerate()
                .filter(|(bit, _)| bag_mask & (1 << bit) != 0)
                .map(|(_, name)| (*name).to_string()).collect::<Vec<_>>();
            engine.seed_set("bag", &bag);
            engine.set_lang(lang);
            let next = engine.call_function_no_args("talkScientist1").unwrap();
            let commands = drive_fossil_commands(&mut engine, next, selected, choice, gift_ok);
            (commands, engine.get_all_flags())
        }
        for lang in ["en", "zh"] {
            for bag in 0..8 {
                for selected in ["DOME_FOSSIL", "HELIX_FOSSIL", "OLD_AMBER", "", "UNKNOWN"] {
                    for choice in [0, 1] {
                        assert_eq!(run(0, bag, selected, choice, true, lang, true),
                            run(0, bag, selected, choice, true, lang, false),
                            "entry {lang}, bag {bag}, selection {selected}, choice {choice}");
                    }
                }
            }
            // Include no ready flag and all corrupt multiple-ready states:
            // the original independently tests each flag after each result.
            for ready in 0..8 {
                for still in [0, 2] {
                    for gift_ok in [false, true] {
                        let bits = 1 | still | (ready << 2);
                        assert_eq!(run(bits, 0, "", 0, gift_ok, lang, true),
                            run(bits, 0, "", 0, gift_ok, lang, false),
                            "ready {ready}, still {still}, gift {gift_ok}, {lang}");
                    }
                }
            }
        }
        for species in ["KABUTO", "OMANYTE", "AERODACTYL"] {
            let mut engine = NativeScriptEngine::new();
            engine.load_embedded_map("CinnabarLabFossilRoom", pokered_data::embedded_scenes::scene_functions());
            engine.set_flag("EVENT_GAVE_FOSSIL_TO_LAB", true);
            let ready = format!("EVENT_REVIVING_{species}");
            engine.set_flag(&ready, true);
            for success in [false, true] {
                let next = engine.call_function_no_args("talkScientist1").unwrap();
                let commands = drive_fossil_commands(&mut engine, next, "", 0, success);
                assert!(commands.contains(&ScriptCommand::GiveMonster { species: species.into(), level: 30 }));
                assert_eq!(engine.get_flag("EVENT_GAVE_FOSSIL_TO_LAB"), !success);
                assert_eq!(engine.get_flag(&ready), !success);
                assert_eq!(engine.get_flag("EVENT_LAB_HANDING_OVER_FOSSIL_MON"), !success);
            }
        }
    }

    #[test]
    fn fossil_lab_menu_wait_and_snapshot_resume_use_actual_returned_selection() {
        let mut engine = NativeScriptEngine::new();
        engine.load_embedded_map("CinnabarLabFossilRoom", pokered_data::embedded_scenes::scene_functions());
        engine.seed_set("bag", &["DOME_FOSSIL".into(), "HELIX_FOSSIL".into(), "OLD_AMBER".into()]);
        let intro = engine.call_function_no_args("talkScientist1").unwrap();
        assert!(matches!(intro, Some(ScriptCommand::ShowText { .. })));
        let menu = engine.signal_done(CommandResult::Void).unwrap();
        assert_eq!(menu, Some(PokemonScriptCommand::FilterBag {
            item_ids: vec!["DOME_FOSSIL".into(), "HELIX_FOSSIL".into(), "OLD_AMBER".into()],
        }.into_script_command()));
        assert!(engine.split_fossil_waiting);
        let snapshot = engine.snapshot();
        // The pending menu is held by the overworld, not re-emitted by the VM.
        for _ in 0..100 {
            assert!(engine.tick().is_none());
            assert!(engine.is_waiting());
            assert!(!engine.get_flag("EVENT_GAVE_FOSSIL_TO_LAB"));
        }
        for (item, expected) in [("DOME_FOSSIL", "KABUTO"), ("HELIX_FOSSIL", "OMANYTE"), ("OLD_AMBER", "AERODACTYL"), ("", "")] {
            let mut restored = NativeScriptEngine::new();
            restored.load_embedded_map("CinnabarLabFossilRoom", pokered_data::embedded_scenes::scene_functions());
            restored.restore_snapshot(&snapshot);
            let next = restored.signal_done(CommandResult::Text(item.into())).unwrap();
            let commands = drive_fossil_commands(&mut restored, next, "", 0, true);
            assert!(!commands.iter().any(|c| matches!(c, ScriptCommand::Custom { name, .. } if name == "filterBag")));
            if item.is_empty() {
                assert_eq!(commands, vec![ScriptCommand::ShowText { text: "Aiyah! You come\nagain!".into() }]);
                assert!(!restored.get_flag("EVENT_GAVE_FOSSIL_TO_LAB"));
            } else {
                assert!(commands.contains(&ScriptCommand::TakeItem { item_id: item.into(), quantity: 1 }));
                assert!(restored.get_flag(&format!("EVENT_REVIVING_{expected}")));
                assert!(restored.get_flag("EVENT_GAVE_FOSSIL_TO_LAB"));
                assert!(restored.get_flag("EVENT_LAB_STILL_REVIVING_FOSSIL"));
            }
        }
        // Earlier serialized debug snapshots remain readable.
        let mut old = serde_json::to_value(snapshot).unwrap();
        old.as_object_mut().unwrap().remove("split_fossil_active");
        old.as_object_mut().unwrap().remove("split_fossil_waiting");
        serde_json::from_value::<NativeScriptEngineSnapshot>(old).unwrap();
    }

    #[test]
    fn fossil_lab_lazy_normal_paths_decode_small_independent_fragments() {
        let mut count = 0;
        for (map, name, bytes) in pokered_data::embedded_scenes::scene_functions() {
            if *map == "CinnabarLabFossilRoom" && name.starts_with("__native_talkScientist1_") {
                assert!(bytes.len() < 3000, "{name} grew to {} bytes", bytes.len());
                serde_json::from_slice::<Vec<StoryStmt>>(bytes).unwrap();
                count += 1;
            }
        }
        assert_eq!(count, 11);
    }

    #[test]
    fn fossil_lab_cancel_prints_original_come_again_without_consuming_the_fossil() {
        // Independent original expectation: engine/events/cinnabar_lab.asm
        // 30-31 branches B to .cancelledGivingFossil (70-73), which prints
        // _CinnabarLabFossilRoomScientist1ComeAgainText. Its English original
        // is "Aiyah! You come\nagain!", also used by the NO response.
        for lazy in [false, true] {
          for selected in ["", "DOME_FOSSIL", "HELIX_FOSSIL", "OLD_AMBER"] {
            let mut engine = NativeScriptEngine::new();
            engine.load_embedded_map("CinnabarLabFossilRoom", pokered_data::embedded_scenes::scene_functions());
            if !lazy { engine.functions.remove("__native_talkScientist1_entry"); }
            engine.seed_set("bag", &["DOME_FOSSIL".into(), "HELIX_FOSSIL".into(), "OLD_AMBER".into()]);
            let next = engine.call_function_no_args("talkScientist1").unwrap();
            let commands = drive_fossil_commands(&mut engine, next, selected, 1, true);
            assert!(matches!(commands.last(), Some(ScriptCommand::ShowText { text }) if text == "Aiyah! You come\nagain!"));
            assert_eq!(commands.len(), if selected.is_empty() { 3 } else { 5 });
            assert!(!commands.iter().any(|c| matches!(c, ScriptCommand::TakeItem { .. } | ScriptCommand::GiveMonster { .. })));
            assert_eq!(commands.iter().any(|c| matches!(c, ScriptCommand::ShowChoice { .. })), !selected.is_empty());
            assert!(!engine.get_flag("EVENT_GAVE_FOSSIL_TO_LAB"));
            assert!(!engine.get_flag("EVENT_LAB_STILL_REVIVING_FOSSIL"));
            for species in ["KABUTO", "OMANYTE", "AERODACTYL"] {
                assert!(!engine.get_flag(&format!("EVENT_REVIVING_{species}")));
            }
          }
        }
    }

    #[test]
    fn fidelity_game_corner_coin_thresholds_preserve_unclaimed_gifts() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("GameCorner").unwrap();
        for coins in [0, 9949, 9950, 9989, 9990, 9991, 9999] {
            for (handler, flag, threshold) in [
                ("talkFishingGuru", "EVENT_GOT_10_COINS", coins >= 9990),
                ("talkClerk2", "EVENT_GOT_20_COINS_2", coins >= 9990),
                ("talkGentleman", "EVENT_GOT_20_COINS", coins == 9990),
            ] {
                for has_case in [false, true] {
                    let mut engine = NativeScriptEngine::new();
                    engine.load_map("GameCorner", &scene);
                    if has_case {
                        engine.seed_set("bag", &["COIN_CASE".into()]);
                    }
                    engine.seed_number("coins", coins as f64);
                    let commands = drive_fidelity_scene(&mut engine, handler, false, "", &[]);
                    let received = has_case && !threshold;
                    assert_eq!(
                        engine.get_flag(flag),
                        received,
                        "{handler}: {coins}, case={has_case}"
                    );
                    assert_eq!(commands.iter().any(|c| matches!(c, ScriptCommand::Custom { name, .. } if name == "giveCoins")), received);
                    if !received && has_case {
                        engine.seed_number("coins", 0.0);
                        drive_fidelity_scene(&mut engine, handler, false, "", &[]);
                        assert!(
                            engine.get_flag(flag),
                            "refusal must leave the gift available"
                        );
                    }
                }
            }
            for (has_case, money, answer) in [
                (true, 1000, 0),
                (true, 999, 0),
                (false, 1000, 0),
                (true, 1000, 1),
            ] {
                let mut engine = NativeScriptEngine::new();
                engine.load_map("GameCorner", &scene);
                if has_case {
                    engine.seed_set("bag", &["COIN_CASE".into()]);
                }
                engine.seed_number("coins", coins as f64);
                engine.seed_number("money", money as f64);
                let commands =
                    drive_fidelity_scene(&mut engine, "talkClerk1", false, "", &[answer]);
                let bought = has_case && money >= 1000 && answer == 0 && coins < 9990;
                for name in ["giveCoins", "takeMoney"] {
                    assert_eq!(
                        commands.iter().any(|c| match c {
                            ScriptCommand::Custom { name: n, .. } => n == name,
                            ScriptCommand::TakeMoney { .. } => name == "takeMoney",
                            _ => false,
                        }),
                        bought,
                        "{name}: {coins}"
                    );
                }
            }
        }
    }

    #[test]
    fn fidelity_game_corner_rocket_walks_around_player_before_hiding() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("GameCorner").unwrap();
        for (x, y, length, end) in [(9, 6, 5, (14, 5)), (8, 5, 5, (14, 5)), (9, 4, 8, (15, 5))] {
            let mut engine = NativeScriptEngine::new();
            engine.load_map("GameCorner", &scene);
            engine.set_player_position(x, y);
            let commands = drive_fidelity_scene(&mut engine, "talkRocket", false, "win", &[]);
            let move_at = commands
                .iter()
                .position(|c| matches!(c, ScriptCommand::MoveNpc { .. }))
                .unwrap();
            let hide_at = commands
                .iter()
                .position(|c| matches!(c, ScriptCommand::HideObjectByName { .. }))
                .unwrap();
            assert!(move_at < hide_at);
            if let ScriptCommand::MoveNpc { path, .. } = &commands[move_at] {
                assert_eq!(path.len(), length);
                assert_eq!(path.last(), Some(&end));
                assert!(!path.contains(&(x, y)));
            }
            assert!(engine.get_flag("EVENT_BEAT_GAME_CORNER_ROCKET"));
        }
        let mut engine = NativeScriptEngine::new();
        engine.load_map("GameCorner", &scene);
        let commands = drive_fidelity_scene(&mut engine, "talkRocket", false, "lose", &[]);
        assert!(!commands.iter().any(|c| matches!(
            c,
            ScriptCommand::MoveNpc { .. } | ScriptCommand::HideObjectByName { .. }
        )));
        assert!(!engine.get_flag("EVENT_BEAT_GAME_CORNER_ROCKET"));
    }

    #[test]
    fn fidelity_indigo_reception_has_only_welcome_before_link_flow() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("IndigoPlateauLobby").unwrap();
        let mut engine = NativeScriptEngine::new();
        engine.load_map("IndigoPlateauLobby", &scene);
        let commands = drive_fidelity_scene(&mut engine, "talkLinkReceptionist", false, "", &[]);
        assert_eq!(commands.len(), 2);
        assert!(
            matches!(&commands[0], ScriptCommand::ShowText { text } if text == "Welcome to the\nCable Club!")
        );
        assert!(matches!(&commands[1], ScriptCommand::Custom { name, .. } if name == "linkStart"));
    }

    #[test]
    fn fidelity_pokemon_prizes_require_confirmation_and_charge_only_after_delivery() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("GameCornerPrizeRoom").unwrap();
        for version in [0.0, 1.0] {
            for handler in ["prizeVendor1", "prizeVendor2"] {
                for selection in 0..3 {
                    for (confirm, success) in [(1, false), (0, false), (0, true)] {
                        let mut engine = NativeScriptEngine::new();
                        engine.load_map("GameCornerPrizeRoom", &scene);
                        engine.seed_set("bag", &["COIN_CASE".into()]);
                        engine.seed_number("coins", 9999.0);
                        engine.seed_number("gameVersion", version);
                        let commands = drive_fidelity_scene(&mut engine, handler, success, "", &[selection, confirm]);
                        let delivered = commands.iter().any(|c| matches!(c, ScriptCommand::GiveMonster { .. }));
                        let charged = commands.iter().any(|c| matches!(c, ScriptCommand::Custom { name, .. } if name == "takeCoins"));
                        assert_eq!(delivered, confirm == 0);
                        assert_eq!(charged, confirm == 0 && success, "version {version}, {handler}, selection {selection}");
                    }
                }
            }
        }
    }

    #[test]
    fn fidelity_tm_prizes_require_confirmation_and_success_before_payment() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("GameCornerPrizeRoom").unwrap();
        for selection in 0..3 {
            for (confirm, success) in [(1, false), (0, false), (0, true)] {
                let mut engine = NativeScriptEngine::new();
                engine.load_map("GameCornerPrizeRoom", &scene);
                engine.seed_set("bag", &["COIN_CASE".into()]);
                engine.seed_number("coins", 9999.0);
                let commands = drive_fidelity_scene(&mut engine, "prizeVendor3", success, "", &[selection, confirm]);
                assert_eq!(commands.iter().any(|c| matches!(c, ScriptCommand::GiveItem { .. })), confirm == 0);
                assert_eq!(commands.iter().any(|c| matches!(c, ScriptCommand::Custom { name, .. } if name == "takeCoins")), confirm == 0 && success);
            }
        }
    }

    #[test]
    fn fidelity_static_monsters_are_consumed_by_run_and_poke_doll() {
        let mut cases: Vec<(&str, String, String)> = vec![
            ("PowerPlant", "talkZapdos".into(), "EVENT_BEAT_ZAPDOS".into()),
            ("SeafoamIslandsB4F", "talkArticuno".into(), "EVENT_BEAT_ARTICUNO".into()),
            ("VictoryRoad2F", "talkMoltres".into(), "EVENT_BEAT_MOLTRES".into()),
            ("CeruleanCaveB1F", "talkMewtwo".into(), "EVENT_BEAT_MEWTWO".into()),
        ];
        for (i, handler) in ["talkVoltorb1", "talkVoltorb2", "talkVoltorb3", "talkElectrode1", "talkVoltorb4", "talkVoltorb5", "talkElectrode2", "talkVoltorb6"].iter().enumerate() {
            cases.push(("PowerPlant", (*handler).into(), format!("EVENT_BEAT_POWER_PLANT_VOLTORB_{i}")));
        }
        for (map, handler, flag) in cases {
            for outcome in ["win", "caught", "ran", "fled", "lose"] {
                let scene = pokered_data::embedded_scenes::get_scene_ast(map).unwrap();
                let mut engine = NativeScriptEngine::new();
                engine.load_map(map, &scene);
                let commands = drive_fidelity_scene(&mut engine, &handler, false, outcome, &[]);
                assert_eq!(engine.get_flag(&flag), outcome != "lose", "{map} {handler} {outcome}");
                assert_eq!(commands.iter().any(|c| matches!(c, ScriptCommand::HideObjectByName { .. })), outcome != "lose");
            }
        }
    }

    #[test]
    fn fidelity_snorlax_is_hidden_before_battle_even_after_blackout() {
        for map in ["Route12", "Route16"] {
            for outcome in ["win", "caught", "ran", "fled", "lose"] {
                let scene = pokered_data::embedded_scenes::get_scene_ast(map).unwrap();
                let mut engine = NativeScriptEngine::new();
                engine.load_map(map, &scene);
                engine.seed_set("bag", &["POKE_FLUTE".into()]);
                let commands = drive_fidelity_scene(&mut engine, "talkSnorlax", false, outcome, &[]);
                let hide = commands.iter().position(|c| matches!(c, ScriptCommand::HideObjectByName { .. })).unwrap();
                let battle = commands.iter().position(|c| matches!(c, ScriptCommand::StartWildBattle { .. })).unwrap();
                assert!(hide < battle);
                assert!(!engine.get_flag(&format!("EVENT_FIGHT_{}_SNORLAX", map.to_uppercase())));
                assert_eq!(engine.get_flag(&format!("EVENT_BEAT_{}_SNORLAX", map.to_uppercase())), outcome != "lose");
                let calmed = commands.iter().any(|c| matches!(c, ScriptCommand::ShowText { text } if text.contains("mountains!")));
                assert_eq!(calmed, outcome == "win" || outcome == "fled");
            }
        }
    }

    #[test]
    fn fidelity_lance_walks_reverse_rle_and_keeps_the_exit_locked_after_victory() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("LancesRoom").unwrap();
        let mut engine = NativeScriptEngine::new();
        engine.load_map("LancesRoom", &scene);
        engine.set_player_position(24, 16);
        let commands = drive_fidelity_scene(&mut engine, "LancesRoomOnLoad", false, "", &[]);
        let path = commands.iter().find_map(|c| match c { ScriptCommand::MovePlayerRelative { steps } => Some(steps), _ => None }).unwrap();
        let expected: Vec<_> = core::iter::repeat((-1,0)).take(6)
            .chain(core::iter::repeat((0,1)).take(7))
            .chain(core::iter::repeat((-1,0)).take(12))
            .chain(core::iter::repeat((0,-1)).take(12)).collect();
        assert_eq!(*path, expected);
        assert!(engine.get_flag("EVENT_LANCES_ROOM_LOCK_DOOR"));
        for handler in ["talkLance", "lanceStep"] {
            engine.set_flag("EVENT_BEAT_LANCE", false);
            let commands = drive_fidelity_scene(&mut engine, handler, false, "win", &[]);
            assert!(engine.get_flag("EVENT_BEAT_LANCE"));
            assert!(engine.get_flag("EVENT_LANCES_ROOM_LOCK_DOOR"));
            assert!(!commands.contains(&custom("replaceTileBlock", vec![json!(2), json!(6), json!(49)])));
        }
        engine.set_flag("EVENT_BEAT_LANCE", false);
        engine.set_player_position(6, 11);
        let commands = drive_fidelity_scene(&mut engine, "LancesRoomOnLoad", false, "", &[]);
        assert!(!commands.iter().any(|c| matches!(c, ScriptCommand::MovePlayerRelative { .. })));
    }

    #[test]
    fn fidelity_cinnabar_entry_resets_mansion_switch_and_finishes_fossils() {
        let scene = pokered_data::embedded_scenes::get_scene_ast("CinnabarIsland").unwrap();
        let mut engine = NativeScriptEngine::new();
        engine.load_map("CinnabarIsland", &scene);
        engine.set_flag("EVENT_MANSION_SWITCH_ON", true);
        engine.set_flag("EVENT_LAB_STILL_REVIVING_FOSSIL", true);
        drive_fidelity_scene(&mut engine, "CinnabarIslandOnLoad", false, "", &[]);
        assert!(!engine.get_flag("EVENT_MANSION_SWITCH_ON"));
        assert!(!engine.get_flag("EVENT_LAB_STILL_REVIVING_FOSSIL"));
    }

}
