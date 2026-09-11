//! Debug-server protocol: the wire types of the JSON-line debug protocol.
//!
//! [`CoreDebugCommand`] is the generic JRPG debug command set — state and
//! position inspection, script flags, warps, button injection,
//! deterministic frame stepping, NPC inspection, save, and bag access are
//! all engine-level concepts any game on the engine can implement.
//!
//! ## Extending the command set
//!
//! Games add their own commands (party/creature inspection, starting
//! encounters, …) by defining their own top-level command type and running
//! [`super::server::DebugServer`] over it. serde internally-tagged enums
//! cannot be extended, so the usual pattern is an `#[serde(untagged)]`
//! wrapper — the wire format stays exactly `{"cmd": "<snake_case>", ...}`:
//!
//! ```text
//! #[serde(untagged)]
//! enum DebugCommand {
//!     Core(CoreDebugCommand),      // the generic set, from this module
//!     Game(MyGameDebugCommand),    // the game's own tagged enum
//! }
//! ```
//!
//! [`DebugResponse`] is game-agnostic as-is: the three-part
//! ok/error/data envelope with the payload as a `serde_json::Value`.

use serde::{Deserialize, Serialize};

/// The generic JRPG debug commands, sent to the debug server via the
/// JSON-line protocol. Games extend this set with their own command enums
/// (see the [module docs](self)).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum CoreDebugCommand {
    /// Get a full game state snapshot.
    GetState,
    /// Get the player's current position (map, coordinates, facing).
    GetPosition,
    /// Get the player's bag items with quantities.
    GetBag,
    /// Get all script flags.
    GetFlags,
    /// Warp to a specific map and coordinates.
    Warp { map: String, x: u16, y: u16 },
    /// Press a single button for one frame.
    Press { button: String },
    /// Press a sequence of buttons, one per frame.
    PressSequence { buttons: Vec<String> },
    /// Run the game for N frames without processing player input.
    RunFrames { count: u32 },
    /// Synchronously step the game forward N frames before responding.
    /// Unlike `RunFrames` (which only schedules frames on the real-time
    /// loop), this drives `update()` in a tight loop inside the command
    /// handler, so the game state is fully advanced (and deterministic)
    /// when the response arrives. Queued Press/PressSequence inputs are
    /// consumed one per stepped frame.
    StepFrames { count: u32 },
    /// Get all NPC runtime states on the current map (position,
    /// visibility, facing, scripted-move progress).
    GetNpcs,
    /// Save the game to file.
    Save,
    /// Set a script flag value.
    SetFlag { name: String, value: bool },
    /// Give an item to the player's bag.
    GiveItem { item: String, qty: u32 },
}

/// Response to a debug command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugResponse {
    /// Whether the command succeeded.
    pub ok: bool,
    /// Error message if the command failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Optional JSON data payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl DebugResponse {
    pub fn ok() -> Self {
        Self {
            ok: true,
            error: None,
            data: None,
        }
    }

    pub fn ok_with_data(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            error: None,
            data: Some(data),
        }
    }

    pub fn err(msg: String) -> Self {
        Self {
            ok: false,
            error: Some(msg),
            data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire format is `{"cmd": "<snake_case>", ...fields}` — the exact
    /// JSON-line convention drivers speak.
    #[test]
    fn core_commands_parse_from_wire_json() {
        let cmd: CoreDebugCommand = serde_json::from_str(r#"{"cmd":"get_state"}"#).unwrap();
        assert!(matches!(cmd, CoreDebugCommand::GetState));

        let cmd: CoreDebugCommand =
            serde_json::from_str(r#"{"cmd":"warp","map":"pallet","x":3,"y":4}"#).unwrap();
        assert!(matches!(
            cmd,
            CoreDebugCommand::Warp { ref map, x: 3, y: 4 } if map == "pallet"
        ));

        let cmd: CoreDebugCommand =
            serde_json::from_str(r#"{"cmd":"press_sequence","buttons":["up","a"]}"#).unwrap();
        assert!(matches!(
            cmd,
            CoreDebugCommand::PressSequence { ref buttons } if buttons == &["up", "a"]
        ));

        let cmd: CoreDebugCommand =
            serde_json::from_str(r#"{"cmd":"step_frames","count":40}"#).unwrap();
        assert!(matches!(cmd, CoreDebugCommand::StepFrames { count: 40 }));

        let cmd: CoreDebugCommand =
            serde_json::from_str(r#"{"cmd":"set_flag","name":"got_starter","value":true}"#)
                .unwrap();
        assert!(matches!(
            cmd,
            CoreDebugCommand::SetFlag { ref name, value: true } if name == "got_starter"
        ));

        let cmd: CoreDebugCommand =
            serde_json::from_str(r#"{"cmd":"give_item","item":"potion","qty":3}"#).unwrap();
        assert!(matches!(
            cmd,
            CoreDebugCommand::GiveItem { ref item, qty: 3 } if item == "potion"
        ));
    }

    #[test]
    fn response_serializes_three_part_envelope() {
        let json = serde_json::to_string(&DebugResponse::ok()).unwrap();
        assert_eq!(json, r#"{"ok":true}"#);

        let json = serde_json::to_string(&DebugResponse::err("boom".into())).unwrap();
        assert_eq!(json, r#"{"ok":false,"error":"boom"}"#);

        let json =
            serde_json::to_string(&DebugResponse::ok_with_data(serde_json::json!({"x": 1})))
                .unwrap();
        assert_eq!(json, r#"{"ok":true,"data":{"x":1}}"#);
    }
}
