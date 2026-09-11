use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScriptCommand {
    ShowText {
        text: String,
    },
    ShowChoice {
        options: Vec<String>,
    },
    GiveItem {
        item_id: String,
        quantity: u8,
    },
    GiveMonster {
        species: String,
        level: u8,
    },
    TakeItem {
        item_id: String,
        quantity: u8,
    },
    SetFlag {
        flag: String,
    },
    ResetFlag {
        flag: String,
    },
    CheckFlag {
        flag: String,
    },
    ShowObject {
        object_index: u8,
    },
    HideObject {
        object_index: u8,
    },
    ShowObjectByName {
        toggle_id: String,
    },
    HideObjectByName {
        toggle_id: String,
    },
    MoveNpc {
        npc_id: String,
        path: Vec<(u8, u8)>,
    },
    StartNpcMove {
        npc_id: String,
        path: Vec<(u8, u8)>,
    },
    AwaitNpcMove {
        npc_id: String,
    },
    MovePlayer {
        path: Vec<(u8, u8)>,
    },
    /// Relative player steps: each entry is a (dx, dy) delta applied
    /// cumulatively from the player's current position. Direction
    /// strings ("up"/"down"/"left"/"right") are converted to unit
    /// deltas at parse time.
    MovePlayerRelative {
        steps: Vec<(i16, i16)>,
    },
    MoveNpcTo {
        npc_id: String,
        x: u8,
        y: u8,
    },
    StartNpcMoveTo {
        npc_id: String,
        x: u8,
        y: u8,
    },
    MovePlayerTo {
        x: u8,
        y: u8,
    },
    FaceNpc {
        npc_id: String,
        direction: String,
    },
    FacePlayer {
        direction: String,
    },
    PlayMusic {
        music_id: String,
    },
    PlaySound {
        sound_id: String,
    },
    StopMusic,
    FadeOutMusic,
    StartBattle {
        trainer_id: String,
    },
    /// Start a wild/static battle against a single generated opponent of the
    /// given species and level (catchable, like a random encounter). Resolves
    /// to the battle outcome string ("win" | "lose" | "caught" | "fled" | ...).
    StartWildBattle {
        species: String,
        level: u8,
    },
    /// Arm the battle-local weather for the NEXT battle (`Some(id)` names a
    /// `kind: Weather` rules.ron record; `None` clears a previously armed
    /// one). Runner-local: registered by the jrpg runner's scene engine next
    /// to `startBattle`, consumed before the battle starts, cleared when the
    /// battle ends. Never saved.
    SetWeather {
        weather: Option<String>,
    },
    Delay {
        frames: u16,
    },
    WarpTo {
        map: String,
        x: u8,
        y: u8,
    },
    Heal,
    FadeScreen {
        fade_type: String,
    },
    SetJoyIgnore {
        mask: u8,
    },
    ClearJoyIgnore,
    FollowNpc {
        npc_id: String,
        target_x: u8,
        target_y: u8,
    },
    OpenShop {
        items: Vec<String>,
    },
    ShowEmotionBubble {
        npc_id: String,
        emotion: String,
    },
    SetNpcPosition {
        npc_id: String,
        x: u8,
        y: u8,
    },
    SetNpcFrame {
        npc_id: String,
        frame: u8,
    },
    ShowScene {
        scene_name: String,
        layout_json: Option<String>,
    },
    HideScene {
        scene_name: String,
    },
    UpdateUI {
        scene_name: String,
        data_json: String,
    },
    GiveMoney {
        amount: u32,
    },
    TakeMoney {
        amount: u32,
    },
    PlayCry {
        species: String,
    },
    GiveBadge {
        badge: u8,
    },
    /// A game-defined command outside the generic JRPG protocol.
    ///
    /// The game registers the JS verb through its `ScriptApiRegistrar`
    /// (returning this variant with the verb's name and arguments) and
    /// dispatches on `name`/`args` in its own app layer.
    Custom {
        name: String,
        args: Vec<serde_json::Value>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandResult {
    Void,
    Bool(bool),
    Number(f64),
    Text(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_text_command() {
        let cmd = ScriptCommand::ShowText {
            text: "Hello".to_string(),
        };
        assert_eq!(
            cmd,
            ScriptCommand::ShowText {
                text: "Hello".to_string()
            }
        );
    }

    #[test]
    fn test_show_choice_command() {
        let cmd = ScriptCommand::ShowChoice {
            options: vec!["Yes".to_string(), "No".to_string()],
        };
        assert_eq!(
            cmd,
            ScriptCommand::ShowChoice {
                options: vec!["Yes".to_string(), "No".to_string()]
            }
        );
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let cmds: Vec<ScriptCommand> = vec![
            ScriptCommand::ShowText {
                text: "Hello".into(),
            },
            ScriptCommand::ShowChoice {
                options: vec!["A".into(), "B".into()],
            },
            ScriptCommand::GiveItem {
                item_id: "POTION".into(),
                quantity: 5,
            },
            ScriptCommand::GiveMonster {
                species: "SPARKIT".into(),
                level: 5,
            },
            ScriptCommand::Heal,
            ScriptCommand::StopMusic,
            ScriptCommand::ClearJoyIgnore,
            ScriptCommand::WarpTo {
                map: "START_TOWN".into(),
                x: 5,
                y: 3,
            },
            ScriptCommand::Delay { frames: 60 },
            ScriptCommand::PlayMusic {
                music_id: "START_TOWN".into(),
            },
            ScriptCommand::PlaySound {
                sound_id: "SFX_BALL".into(),
            },
            ScriptCommand::FadeOutMusic,
            ScriptCommand::FadeScreen {
                fade_type: "out".into(),
            },
            ScriptCommand::SetJoyIgnore { mask: 0xFF },
            ScriptCommand::MoveNpc {
                npc_id: "prof".into(),
                path: vec![(2, 3), (4, 5)],
            },
            ScriptCommand::FollowNpc {
                npc_id: "rival".into(),
                target_x: 10,
                target_y: 8,
            },
            ScriptCommand::OpenShop {
                items: vec!["POTION".into()],
            },
            ScriptCommand::GiveMoney { amount: 500 },
            ScriptCommand::TakeMoney { amount: 100 },
            ScriptCommand::GiveBadge { badge: 0 },
            ScriptCommand::Custom {
                name: "tradeMonster".into(),
                args: vec![serde_json::json!("SPARKIT")],
            },
            ScriptCommand::ShowObject { object_index: 1 },
            ScriptCommand::HideObjectByName {
                toggle_id: "HIDDEN_ITEM".into(),
            },
            ScriptCommand::SetWeather {
                weather: Some("sandstorm".into()),
            },
            ScriptCommand::SetWeather { weather: None },
        ];

        for cmd in &cmds {
            let json = serde_json::to_string(cmd).unwrap();
            let deserialized: ScriptCommand = serde_json::from_str(&json).unwrap();
            assert_eq!(*cmd, deserialized, "round-trip failed for {cmd:?}");
        }
    }

    #[test]
    fn test_command_result_equality() {
        assert_eq!(CommandResult::Void, CommandResult::Void);
        assert_eq!(CommandResult::Bool(true), CommandResult::Bool(true));
        assert_eq!(CommandResult::Bool(false), CommandResult::Bool(false));
        assert_ne!(CommandResult::Bool(true), CommandResult::Bool(false));
        assert_eq!(CommandResult::Number(42.0), CommandResult::Number(42.0));
        assert_ne!(CommandResult::Number(1.0), CommandResult::Number(2.0));
        assert_eq!(
            CommandResult::Text("hello".into()),
            CommandResult::Text("hello".into())
        );
        assert_ne!(
            CommandResult::Text("hello".into()),
            CommandResult::Text("world".into())
        );
    }

    #[test]
    fn test_command_result_debug() {
        let void = CommandResult::Void;
        assert!(!format!("{void:?}").is_empty());

        let b = CommandResult::Bool(true);
        assert_eq!(format!("{b:?}"), "Bool(true)");

        let n = CommandResult::Number(2.5);
        assert!(format!("{n:?}").contains("2.5"));

        let t = CommandResult::Text("result".into());
        assert_eq!(format!("{t:?}"), "Text(\"result\")");
    }

    #[test]
    fn test_give_take_money() {
        let give = ScriptCommand::GiveMoney { amount: 999 };
        let take = ScriptCommand::TakeMoney { amount: 50 };
        assert_ne!(give, take);
        if let ScriptCommand::GiveMoney { amount } = &give {
            assert_eq!(*amount, 999);
        } else {
            panic!("expected GiveMoney");
        }
    }

    #[test]
    fn test_show_hide_scene() {
        let show = ScriptCommand::ShowScene {
            scene_name: "shop".into(),
            layout_json: None,
        };
        let hide = ScriptCommand::HideScene {
            scene_name: "shop".into(),
        };
        assert_ne!(show, hide);
        assert_eq!(
            show,
            ScriptCommand::ShowScene {
                scene_name: "shop".into(),
                layout_json: None
            }
        );
    }

    #[test]
    fn test_update_ui_command() {
        let cmd = ScriptCommand::UpdateUI {
            scene_name: "bag".into(),
            data_json: r#"{"gold":500}"#.into(),
        };
        assert!(format!("{cmd:?}").contains("bag"));
        assert!(format!("{cmd:?}").contains("gold"));
    }
}
