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
    /// Give a Pokémon to the player's party.
    GivePokemon { species: String, level: u8 },
    /// Start a wild battle against the given species/level (for testing catch
    /// and battle flow without walking into a random encounter).
    StartWildBattle { species: String, level: u8 },
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
    }

    /// An unknown command string is an error (not silently misparsed).
    #[test]
    fn unknown_command_is_an_error() {
        assert!(serde_json::from_str::<DebugCommand>(r#"{"cmd":"fly_to_moon"}"#).is_err());
    }
}
