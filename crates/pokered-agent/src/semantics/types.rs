//! Semantics schema: predicates (what a storyline reads) and effects
//! (what it changes), plus per-map/per-run coverage reporting.

use serde::{Deserialize, Serialize};

/// A state condition a storyline reads, found in `getFlag`-style calls
/// inside `@if` conditions (and anywhere else an expression reaches).
/// Internally tagged for a self-describing wire form.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StatePredicate {
    /// `getFlag("EVENT_X")` — a story flag read.
    FlagRead { flag: String },
    /// `hasItem("POKE_BALL")` — item presence check.
    ItemHeld { item: String },
    /// `hasBadge("BOULDERBADGE")` — badge check (name or numeric string).
    BadgeHeld { badge: String },
    /// `hasMoney(n)`.
    MoneyAtLeast { amount: u32 },
    /// `hasCoins(n)`.
    CoinsAtLeast { amount: u32 },
    /// Any other recognized query function whose result has no typed
    /// predicate form (`getBadgeCount`, `getPlayerX`, `getRivalStarter`,
    /// `partyMonKnowsHm`, `lang`, …). The call is factual, just not
    /// reducible to a flag/item predicate.
    Query { name: String },
    /// A call/expression the analyzer does not recognize — reported,
    /// never guessed.
    Unknown { construct: String },
}

/// A state change a storyline performs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StateEffect {
    /// `setFlag("EVENT_X")`.
    FlagSet { flag: String },
    /// `resetFlag("EVENT_X")`.
    FlagCleared { flag: String },
    /// `giveItem("ITEM", qty)`.
    ItemGiven { item: String, qty: Option<u8> },
    /// `takeItem("ITEM", qty)`.
    ItemTaken { item: String, qty: Option<u8> },
    /// `givePokemon("SPECIES", level)`.
    PokemonGiven { species: String, level: Option<u8> },
    /// `giveBadge(name|index)`.
    BadgeGiven { badge: String },
    /// `startBattle` / `startBattleSet` / `startWildBattle` / tutorial.
    BattleStarted { battle: BattleKind },
    /// `warpTo("Map", x, y)`.
    PlayerWarped { map: String, x: Option<u8>, y: Option<u8> },
    /// `moveNpc*` / `followNpc` / `setNpcPosition` — an NPC is moved by
    /// the script (position is dynamic or irrelevant to the graph).
    NpcMoved { npc_id: String },
    /// `movePlayer` / `movePlayerTo` / `movePlayerRelative` — scripted
    /// player movement.
    PlayerMoved,
    /// `showObject("TOGGLE_ID")` / `showObjectByName`.
    ObjectShown { toggle_id: String },
    /// `hideObject("TOGGLE_ID")` / `hideObjectByName`.
    ObjectHidden { toggle_id: String },
    /// `replaceTileBlock(x, y, block)`.
    TileReplaced { x: Option<u8>, y: Option<u8>, block: Option<u8> },
    /// `giveMoney(n)`.
    MoneyGiven { amount: Option<u32> },
    /// `takeMoney(n)`.
    MoneyTaken { amount: Option<u32> },
    /// `giveCoins(n)`.
    CoinsGiven { amount: Option<u32> },
    /// `takeCoins(n)`.
    CoinsTaken { amount: Option<u32> },
    /// A command the analyzer does not recognize — reported, never
    /// guessed. `run_js` marks a `@run` escape hatch.
    Unknown { construct: String },
}

/// Battle reference inside [`StateEffect::BattleStarted`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BattleKind {
    /// `startBattle("OPP_RIVAL1")` / `startBattleSet` — class and 0-based
    /// party index resolved via `trainer_data::parse_trainer_id` when
    /// the id parses.
    Trainer {
        trainer_id: String,
        class: Option<String>,
        set: Option<usize>,
    },
    /// `startWildBattle("SPECIES", level)`.
    Wild { species: String, level: Option<u8> },
    /// `oldManTutorial()` — the scripted catch demo.
    OldManTutorial,
}

/// Extracted semantics for one storyline (or a map's `@load` block).
/// `reads`/`effects` are deduplicated sets, sorted for determinism.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptSemantics {
    /// `"{map}:{storyline}"`, or `"{map}:@load"` for the on-load block.
    pub id: String,
    pub map: String,
    pub storyline: String,
    /// Trigger descriptors: `"load"`, `"npc:{id}"`, `"sign:{id}"`,
    /// `"coord:({x},{y})"`, `"name:{trigger_name}"`.
    pub triggers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<StatePredicate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<StateEffect>,
}

/// All storylines of one map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapSemantics {
    pub map: String,
    pub storylines: Vec<ScriptSemantics>,
}

/// One unrecognized construct, counted for coverage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownConstruct {
    /// Command/callee name, `run_js`, or a descriptive tag like
    /// `setFlag(dynamic_arg)`.
    pub name: String,
    pub count: usize,
    /// First map it was seen in (for follow-up work).
    pub example_map: String,
}

/// Global + per-map extraction coverage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageReport {
    pub maps_analyzed: usize,
    pub maps_without_scene: usize,
    pub storylines_analyzed: usize,
    /// Statement-level commands recognized (state effect or known
    /// no-state-effect command like dialogue/audio/delay).
    pub commands_recognized: usize,
    /// Statement-level commands with no recognized semantics.
    pub commands_unknown: usize,
    /// Expression-level calls recognized (typed predicate or known query).
    pub calls_recognized: usize,
    pub calls_unknown: usize,
    /// Every unrecognized construct, with counts and an example map.
    pub unknown_constructs: Vec<UnknownConstruct>,
}
