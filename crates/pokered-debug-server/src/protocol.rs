use serde::{Deserialize, Serialize};

/// Commands that can be sent to the debug server via JSON-line protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum DebugCommand {
    /// Get a full game state snapshot.
    GetState,
    /// Get the player's current position (map, coordinates, facing).
    GetPosition,
    /// Get the player's party Pokémon data.
    GetParty,
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
    /// Give a Pokémon to the player's party.
    GivePokemon { species: String, level: u8 },
    /// Start a wild battle against the given species/level (for testing catch
    /// and battle flow without walking into a random encounter).
    StartWildBattle { species: String, level: u8 },
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
