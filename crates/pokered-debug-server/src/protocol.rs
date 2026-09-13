use serde::{Deserialize, Serialize};

// The generic JRPG debug protocol (command set + ok/error/data response
// envelope) lives in the engine's platform layer; this crate re-exports it
// and adds only the game-specific commands.
pub use dotzuki_app::debug_server::{CoreDebugCommand, DebugResponse};

/// Game-side debug commands — pokered's extension of the generic JRPG debug
/// protocol ([`CoreDebugCommand`]). Holds the Pokémon-specific commands plus
/// the deterministic dialogue/cutscene stepping commands (`wait_until` /
/// `skip_dialogue`): their concepts are generic, but they drive the game's
/// own overworld/dialogue state, so they live on the game side.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum GameDebugCommand {
    /// Render the current screen to a PNG without advancing simulation.
    /// Native debug harness only; the parent directory must already exist.
    CaptureFrame { path: String },
    /// Queue one button or explicit neutral (`null`) per emulated frame.
    /// `start_at_frame` pads with neutral frames so the first supplied input
    /// lands on the requested absolute game frame.
    PressTimeline {
        buttons: Vec<Option<String>>,
        #[serde(default)]
        start_at_frame: Option<u64>,
    },
    /// Read the live overworld blocks, including script and field-move edits.
    GetMap,
    /// Get the player's party Pokémon data.
    GetParty,
    /// Synchronously step the game until a named condition holds (checked
    /// after each frame), or until `max_frames` elapse. Collapses the
    /// driver's poll-every-N-frames loop into a single round trip. The
    /// response carries `reached`, the number of frames stepped, and the
    /// final state snapshot so a timeout is still inspectable. Unknown
    /// condition names are rejected with an error before any frame is
    /// stepped. Queued Press/PressSequence inputs are consumed one per
    /// stepped frame, as with `step_frames`.
    ///
    /// Conditions (see the `wait_until` handler docs in the app):
    /// `dialogue_done`, `dialogue_ready`, `choice_open`, `choice_closed`,
    /// `script_idle`, `control_ready`, `not_battle`, plus the generic
    /// `screen=<name>` / `battle_phase=<name>` / `script_effect=<name>`
    /// forms (name compared against the Debug variant name).
    WaitUntil { condition: String, max_frames: u32 },
    /// Advance the active dialogue box to completion with engine-internal
    /// A taps — typing is skipped, every page is advanced, and the box is
    /// closed exactly as if a player pressed A through all of it, so a
    /// script suspended on `ShowDialogue` resumes normally. Returns the
    /// number of frames stepped plus a state snapshot. No-op when no
    /// dialogue is showing. Queued (unconsumed) Press/PressSequence inputs
    /// are dropped first — they would override the internal taps.
    SkipDialogue,
    /// Semantic observation snapshot for AI agents (the M1 observation
    /// layer), built by the app on the `pokered-agent` crate. `level`
    /// (1-4) selects a canned observation profile — 1: runtime state
    /// only, 2: +dialogue/battle, 3: +nearby entities, 4: +world-data
    /// allowance; `profile` supplies a complete `ObservationProfile`
    /// document instead. Pass at most one of them; with neither, the
    /// default is the full symbolic level 3. Purely observational: never
    /// steps frames.
    GetAgentState {
        #[serde(default)]
        level: Option<u8>,
        #[serde(default)]
        profile: Option<serde_json::Value>,
    },
    /// Entities near the player (NPCs, trainers, item balls, signs,
    /// warps, hidden items), sorted by Manhattan distance in step units
    /// (1 step = 2 GB tiles). `radius` caps the distance, default 10.
    /// Purely observational: never steps frames.
    GetNearby {
        #[serde(default)]
        radius: Option<u32>,
    },
    /// Closed-loop walk to tile (`x`, `y`) on the current map: the game
    /// BFS-pathfinds reusing its own collision, walks one tile at a time
    /// with real controller input (re-observing after every tile), and
    /// aborts cleanly on interruption. Synchronous like `step_frames`:
    /// the response carries the `NavigationOutcome` (`result`:
    /// `reached` / `blocked` / `interrupted` / `entered_battle` /
    /// `entered_dialogue` / `map_changed`, plus `steps`, `frames`, and
    /// `start`/`target`/`final` positions) and fresh state snapshots.
    MoveTo { x: u16, y: u16 },
    /// Face the adjacent interactable (the faced tile first, otherwise
    /// the player turns toward an adjacent visible NPC / sign / hidden
    /// item, in that priority) and press A, running until a dialogue
    /// opens, a battle starts, a script takes over, or nothing happens.
    Interact,
    /// Pathfind adjacent to a `get_nearby` entity id (`npc:{i}`,
    /// `sign:{i}`, `hidden:{table_index}` — warps are `move_to`'s job),
    /// face it, and press A. The response carries the interaction
    /// result (`dialogue` / `battle` / `nothing` / `interrupted` /
    /// `blocked` / `not_found` / …) and, when navigation ran, its
    /// outcome.
    InteractWith { id: String },
    /// The M3 geographic world graph: every map's connection and warp
    /// edges. With `maps` (a list of PascalCase map names) returns only
    /// edges leaving those maps; without it returns the full graph
    /// (large). Purely observational: never steps frames.
    GetWorldGraph {
        #[serde(default)]
        maps: Option<Vec<String>>,
    },
    /// BFS shortest route between two maps as a leg list (connection /
    /// warp legs with positions where derivable). Purely observational.
    FindWorldRoute { from: String, to: String },
    /// Travel cross-map to `map`: world routing → tile-level execution
    /// per leg (M2 walker) → warp/connection traversal with landing
    /// verification → replan on surprise. Wild battles are auto-resolved
    /// (RUN with a fast lead, FIGHT fallback); trainer battles are
    /// fought with the lead's first move; blackouts and unresolvable
    /// battles abort. Synchronous like `step_frames`. The response
    /// carries the `TravelOutcome` (`result`: `reached` / `blocked` /
    /// `entered_battle` / `interrupted` / `map_mismatch` / `blackout` /
    /// `invalid_target`) plus fresh state snapshots.
    TravelTo { map: String },
    /// M4 static scene semantics. With `map` (PascalCase name) returns
    /// that map's extracted storylines (`reads` state predicates,
    /// `effects` state changes, `triggers`). Without it returns the
    /// coverage summary plus the list of analyzed maps (full per-map
    /// payloads live in `target/agent/world_semantics.json`). Purely
    /// observational: never steps frames.
    GetScriptSemantics {
        #[serde(default)]
        map: Option<String>,
    },
    /// M5: pin determinism by replacing both RNG streams (overworld +
    /// battle) with seeded ChaCha12 streams. Unseeded runs stay
    /// entropy-based; this is the runtime form of `--seed`.
    SetSeed { seed: u64 },
    /// M5: capture the full runtime (save data + screen + overworld/
    /// battle internals + frame counters + RNG state) into an in-memory
    /// slot. Overworld and battle screens only — menus and mid-movie
    /// takeovers error cleanly. The response carries a content hash of
    /// the snapshot for identity assertions.
    SaveState { slot: u8 },
    /// M5: restore a slot captured by `save_state`. Bit-for-bit:
    /// afterwards identical inputs produce identical frames. Queued
    /// debug inputs are dropped; presentation-only state (battle VFX)
    /// restarts.
    RestoreState { slot: u8 },
    /// Give a Pokémon to the player's party.
    GivePokemon { species: String, level: u8 },
    /// Start a wild battle against the given species/level (for testing catch
    /// and battle flow without walking into a random encounter).  A supplied
    /// `start_at_frame` synchronously advances with neutral input first, so
    /// animation captures do not inherit TCP connection timing.
    StartWildBattle {
        species: String,
        level: u8,
        #[serde(default)]
        start_at_frame: Option<u64>,
    },
}

/// Commands that can be sent to the debug server via JSON-line protocol:
/// the engine's generic JRPG set plus the game-side extension.
///
/// serde internally-tagged enums cannot be extended, so the composition is
/// an untagged wrapper — the wire format stays exactly
/// `{"cmd": "<snake_case>", ...}` either way (core variants are tried
/// first, so a core command can never fall through to the game set).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DebugCommand {
    Core(CoreDebugCommand),
    Game(GameDebugCommand),
}

/// Snapshot of the current game state (returned by GetState).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateSnapshot {
    pub screen: String,
    pub map_id: u8,
    pub map_name: String,
    pub player_x: u16,
    pub player_y: u16,
    pub player_facing: String,
    pub player_name: String,
    pub frame_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wire compat guard: the exact JSON lines scripts/debug_drive.py sends
    /// must keep parsing, core and game commands alike.
    #[test]
    fn wire_json_parses_into_core_and_game_commands() {
        let cmd: DebugCommand = serde_json::from_str(r#"{"cmd":"get_state"}"#).unwrap();
        assert!(matches!(cmd, DebugCommand::Core(CoreDebugCommand::GetState)));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"press_sequence","buttons":["up","a"]}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Core(CoreDebugCommand::PressSequence { .. })
        ));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"step_frames","count":40}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Core(CoreDebugCommand::StepFrames { count: 40 })
        ));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"warp","map":"pallet_town","x":3,"y":4}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Core(CoreDebugCommand::Warp { x: 3, y: 4, .. })
        ));

        let cmd: DebugCommand = serde_json::from_str(r#"{"cmd":"get_party"}"#).unwrap();
        assert!(matches!(cmd, DebugCommand::Game(GameDebugCommand::GetParty)));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"give_pokemon","species":"Pikachu","level":5}"#)
                .unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::GivePokemon { level: 5, .. })
        ));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"start_wild_battle","species":"Rattata","level":3}"#)
                .unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::StartWildBattle { level: 3, .. })
        ));

        let cmd: DebugCommand = serde_json::from_str(
            r#"{"cmd":"start_wild_battle","species":"Rattata","level":3,"start_at_frame":300}"#,
        )
        .unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::StartWildBattle {
                level: 3,
                start_at_frame: Some(300),
                ..
            })
        ));

        let cmd: DebugCommand = serde_json::from_str(r#"{"cmd":"get_agent_state"}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::GetAgentState {
                level: None,
                profile: None,
            })
        ));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"get_agent_state","level":2}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::GetAgentState {
                level: Some(2),
                profile: None,
            })
        ));

        let cmd: DebugCommand = serde_json::from_str(
            r#"{"cmd":"get_agent_state","profile":{"level":"interaction","include_nearby":true}}"#,
        )
        .unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::GetAgentState {
                level: None,
                profile: Some(_),
            })
        ));

        let cmd: DebugCommand = serde_json::from_str(r#"{"cmd":"get_nearby"}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::GetNearby { radius: None })
        ));

        let cmd: DebugCommand = serde_json::from_str(r#"{"cmd":"get_nearby","radius":5}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::GetNearby { radius: Some(5) })
        ));

        let cmd: DebugCommand = serde_json::from_str(r#"{"cmd":"move_to","x":12,"y":11}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::MoveTo { x: 12, y: 11 })
        ));

        let cmd: DebugCommand = serde_json::from_str(r#"{"cmd":"interact"}"#).unwrap();
        assert!(matches!(cmd, DebugCommand::Game(GameDebugCommand::Interact)));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"interact_with","id":"npc:0"}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::InteractWith { ref id }) if id == "npc:0"
        ));

        let cmd: DebugCommand = serde_json::from_str(r#"{"cmd":"get_world_graph"}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::GetWorldGraph { maps: None })
        ));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"get_world_graph","maps":["PalletTown","Route1"]}"#)
                .unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::GetWorldGraph { maps: Some(_) })
        ));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"find_world_route","from":"PalletTown","to":"PewterCity"}"#)
                .unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::FindWorldRoute { .. })
        ));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"travel_to","map":"ViridianCity"}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::TravelTo { ref map }) if map == "ViridianCity"
        ));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"get_script_semantics"}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::GetScriptSemantics { map: None })
        ));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"get_script_semantics","map":"OaksLab"}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::GetScriptSemantics { map: Some(_) })
        ));

        let cmd: DebugCommand = serde_json::from_str(r#"{"cmd":"set_seed","seed":42}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::SetSeed { seed: 42 })
        ));

        let cmd: DebugCommand = serde_json::from_str(r#"{"cmd":"save_state","slot":1}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::SaveState { slot: 1 })
        ));

        let cmd: DebugCommand =
            serde_json::from_str(r#"{"cmd":"restore_state","slot":1}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::RestoreState { slot: 1 })
        ));
    }

    /// The game-side dialogue/cutscene stepping commands (wait_until /
    /// skip_dialogue) keep their wire format from before the protocol split.
    #[test]
    fn wire_json_parses_stepping_commands() {
        let cmd: DebugCommand = serde_json::from_str(
            r#"{"cmd":"wait_until","condition":"dialogue_done","max_frames":600}"#,
        )
        .unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::WaitUntil {
                ref condition,
                max_frames: 600
            }) if condition == "dialogue_done"
        ));

        let cmd: DebugCommand = serde_json::from_str(r#"{"cmd":"skip_dialogue"}"#).unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::SkipDialogue)
        ));

        let cmd: DebugCommand = serde_json::from_str(
            r#"{"cmd":"press_timeline","buttons":["a",null,"down"],"start_at_frame":240}"#,
        )
        .unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::PressTimeline {
                ref buttons,
                start_at_frame: Some(240),
            }) if buttons == &vec![Some("a".into()), None, Some("down".into())]
        ));

        let cmd: DebugCommand = serde_json::from_str(
            r#"{"cmd":"press_timeline","buttons":[null]}"#,
        )
        .unwrap();
        assert!(matches!(
            cmd,
            DebugCommand::Game(GameDebugCommand::PressTimeline {
                start_at_frame: None,
                ..
            })
        ));
    }

    /// Serialization must reproduce the same `{"cmd": ...}` documents (the
    /// server logs and may round-trip commands).
    #[test]
    fn commands_serialize_back_to_wire_json() {
        let json = serde_json::to_string(&DebugCommand::Core(CoreDebugCommand::Press {
            button: "a".into(),
        }))
        .unwrap();
        assert_eq!(json, r#"{"cmd":"press","button":"a"}"#);

        let json = serde_json::to_string(&DebugCommand::Game(GameDebugCommand::GivePokemon {
            species: "Pikachu".into(),
            level: 5,
        }))
        .unwrap();
        assert_eq!(json, r#"{"cmd":"give_pokemon","species":"Pikachu","level":5}"#);

        let json = serde_json::to_string(&DebugCommand::Game(GameDebugCommand::WaitUntil {
            condition: "control_ready".into(),
            max_frames: 120,
        }))
        .unwrap();
        assert_eq!(
            json,
            r#"{"cmd":"wait_until","condition":"control_ready","max_frames":120}"#
        );

        let json = serde_json::to_string(&DebugCommand::Game(GameDebugCommand::GetAgentState {
            level: Some(3),
            profile: None,
        }))
        .unwrap();
        assert_eq!(json, r#"{"cmd":"get_agent_state","level":3,"profile":null}"#);

        let json = serde_json::to_string(&DebugCommand::Game(GameDebugCommand::GetAgentState {
            level: None,
            profile: None,
        }))
        .unwrap();
        assert_eq!(json, r#"{"cmd":"get_agent_state","level":null,"profile":null}"#);

        let json = serde_json::to_string(&DebugCommand::Game(GameDebugCommand::GetNearby {
            radius: None,
        }))
        .unwrap();
        assert_eq!(json, r#"{"cmd":"get_nearby","radius":null}"#);
    }

    /// An unknown command string is an error (not silently misparsed).
    #[test]
    fn unknown_command_is_an_error() {
        assert!(serde_json::from_str::<DebugCommand>(r#"{"cmd":"fly_to_moon"}"#).is_err());
    }

    /// The agent observation commands round-trip through the wire format.
    #[test]
    fn agent_commands_round_trip() {
        let cmd = DebugCommand::Game(GameDebugCommand::GetAgentState {
            level: None,
            profile: Some(serde_json::json!({"level": "full_symbolic"})),
        });
        let line = serde_json::to_string(&cmd).unwrap();
        let back: DebugCommand = serde_json::from_str(&line).unwrap();
        assert!(matches!(
            back,
            DebugCommand::Game(GameDebugCommand::GetAgentState {
                level: None,
                profile: Some(_),
            })
        ));

        let cmd = DebugCommand::Game(GameDebugCommand::GetNearby { radius: Some(12) });
        let line = serde_json::to_string(&cmd).unwrap();
        assert_eq!(line, r#"{"cmd":"get_nearby","radius":12}"#);
        let back: DebugCommand = serde_json::from_str(&line).unwrap();
        assert!(matches!(
            back,
            DebugCommand::Game(GameDebugCommand::GetNearby { radius: Some(12) })
        ));

        let cmd = DebugCommand::Game(GameDebugCommand::MoveTo { x: 12, y: 11 });
        let line = serde_json::to_string(&cmd).unwrap();
        assert_eq!(line, r#"{"cmd":"move_to","x":12,"y":11}"#);
        let back: DebugCommand = serde_json::from_str(&line).unwrap();
        assert!(matches!(
            back,
            DebugCommand::Game(GameDebugCommand::MoveTo { x: 12, y: 11 })
        ));

        let cmd = DebugCommand::Game(GameDebugCommand::InteractWith { id: "sign:1".into() });
        let line = serde_json::to_string(&cmd).unwrap();
        assert_eq!(line, r#"{"cmd":"interact_with","id":"sign:1"}"#);
        let back: DebugCommand = serde_json::from_str(&line).unwrap();
        assert!(matches!(
            back,
            DebugCommand::Game(GameDebugCommand::InteractWith { ref id }) if id == "sign:1"
        ));

        let cmd = DebugCommand::Game(GameDebugCommand::TravelTo {
            map: "PewterCity".into(),
        });
        let line = serde_json::to_string(&cmd).unwrap();
        assert_eq!(line, r#"{"cmd":"travel_to","map":"PewterCity"}"#);
        let back: DebugCommand = serde_json::from_str(&line).unwrap();
        assert!(matches!(
            back,
            DebugCommand::Game(GameDebugCommand::TravelTo { .. })
        ));

        let cmd = DebugCommand::Game(GameDebugCommand::SetSeed { seed: 42 });
        let line = serde_json::to_string(&cmd).unwrap();
        assert_eq!(line, r#"{"cmd":"set_seed","seed":42}"#);
        let back: DebugCommand = serde_json::from_str(&line).unwrap();
        assert!(matches!(
            back,
            DebugCommand::Game(GameDebugCommand::SetSeed { seed: 42 })
        ));

        let cmd = DebugCommand::Game(GameDebugCommand::SaveState { slot: 3 });
        let line = serde_json::to_string(&cmd).unwrap();
        assert_eq!(line, r#"{"cmd":"save_state","slot":3}"#);
        let back: DebugCommand = serde_json::from_str(&line).unwrap();
        assert!(matches!(
            back,
            DebugCommand::Game(GameDebugCommand::SaveState { slot: 3 })
        ));
    }
}
