use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MapScriptConfig {
    #[serde(default)]
    pub on_load: Option<String>,
    #[serde(default)]
    pub npcs: Vec<NpcBinding>,
    #[serde(default)]
    pub signs: Vec<SignBinding>,
    #[serde(default)]
    pub coord_events: Vec<CoordEventBinding>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcBinding {
    pub id: u8,
    #[serde(default)]
    pub talk: Option<String>,
    /// Named toggle identifier for script showObject/hideObject (e.g. "START_TOWN_PROF").
    #[serde(default)]
    pub toggle_id: Option<String>,
    /// Script-facing NPC identifier used by moveNpc/startNpcMove (e.g. "STARTTOWN_PROF").
    #[serde(default)]
    pub script_id: Option<String>,
    /// If true, this NPC is hidden when the map first loads (until a script shows it).
    #[serde(default)]
    pub default_hidden: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignBinding {
    pub id: u8,
    pub talk: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordEventBinding {
    pub name: String,
    pub position: (u16, u16),
    pub trigger: String,
    /// If false, the event re-fires every time the player steps onto the
    /// tile (the storyline's own flag checks gate re-entry). Defaults to
    /// true (fire once per map entry).
    #[serde(default = "default_one_shot")]
    pub one_shot: bool,
}

fn default_one_shot() -> bool {
    true
}

impl MapScriptConfig {
    pub fn on_load(&self) -> Option<&str> {
        self.on_load.as_deref()
    }

    pub fn npc_talk_fn(&self, npc_text_id: u8) -> Option<&str> {
        self.npcs
            .iter()
            .find(|n| n.id == npc_text_id)
            .and_then(|n| n.talk.as_deref())
    }

    pub fn sign_talk_fn(&self, sign_text_id: u8) -> Option<&str> {
        self.signs
            .iter()
            .find(|s| s.id == sign_text_id)
            .map(|s| s.talk.as_str())
    }

    pub fn coord_event_fn(&self, x: u16, y: u16) -> Option<&str> {
        self.coord_events
            .iter()
            .find(|c| c.position == (x, y))
            .map(|c| c.trigger.as_str())
    }

    pub fn coord_event_by_name(&self, name: &str) -> Option<&str> {
        self.coord_events
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.trigger.as_str())
    }

    pub fn hidden_npc_ids(&self) -> Vec<u8> {
        self.npcs
            .iter()
            .filter(|n| n.default_hidden)
            .map(|n| n.id)
            .collect()
    }

    pub fn npc_id_by_toggle(&self, toggle_id: &str) -> Option<u8> {
        self.npcs
            .iter()
            .find(|n| n.toggle_id.as_deref() == Some(toggle_id))
            .map(|n| n.id)
    }

    pub fn npc_id_by_script_id(&self, script_id: &str) -> Option<u8> {
        self.npcs
            .iter()
            .find(|n| n.script_id.as_deref() == Some(script_id))
            .map(|n| n.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> MapScriptConfig {
        MapScriptConfig {
            on_load: Some("onEnter".into()),
            npcs: vec![
                NpcBinding {
                    id: 1,
                    talk: Some("talkProf".into()),
                    toggle_id: Some("START_TOWN_PROF".into()),
                    script_id: Some("STARTTOWN_PROF".into()),
                    default_hidden: false,
                },
                NpcBinding {
                    id: 2,
                    talk: Some("talkRival".into()),
                    toggle_id: None,
                    script_id: None,
                    default_hidden: true,
                },
                NpcBinding {
                    id: 3,
                    talk: None,
                    toggle_id: None,
                    script_id: None,
                    default_hidden: false,
                },
            ],
            signs: vec![SignBinding {
                id: 1,
                talk: "signLab".into(),
            }],
            coord_events: vec![
                CoordEventBinding {
                    name: "northExit".into(),
                    position: (4, 1),
                    trigger: "enterRoute1".into(),
                    one_shot: true,
                },
                CoordEventBinding {
                    name: "southExit".into(),
                    position: (4, 11),
                    trigger: "enterStartTown".into(),
                    one_shot: true,
                },
            ],
        }
    }

    #[test]
    fn test_on_load() {
        let config = sample_config();
        assert_eq!(config.on_load(), Some("onEnter"));
    }

    #[test]
    fn test_on_load_none() {
        let config = MapScriptConfig::default();
        assert_eq!(config.on_load(), None);
    }

    #[test]
    fn test_npc_talk_fn_found() {
        let config = sample_config();
        assert_eq!(config.npc_talk_fn(1), Some("talkProf"));
        assert_eq!(config.npc_talk_fn(2), Some("talkRival"));
    }

    #[test]
    fn test_npc_talk_fn_not_found() {
        let config = sample_config();
        assert_eq!(config.npc_talk_fn(99), None);
    }

    #[test]
    fn test_npc_talk_fn_no_talk_field() {
        let config = sample_config();
        assert_eq!(config.npc_talk_fn(3), None);
    }

    #[test]
    fn test_sign_talk_fn_found() {
        let config = sample_config();
        assert_eq!(config.sign_talk_fn(1), Some("signLab"));
    }

    #[test]
    fn test_sign_talk_fn_not_found() {
        let config = sample_config();
        assert_eq!(config.sign_talk_fn(99), None);
    }

    #[test]
    fn test_coord_event_fn_found() {
        let config = sample_config();
        assert_eq!(config.coord_event_fn(4, 1), Some("enterRoute1"));
        assert_eq!(config.coord_event_fn(4, 11), Some("enterStartTown"));
    }

    #[test]
    fn test_coord_event_fn_not_found() {
        let config = sample_config();
        assert_eq!(config.coord_event_fn(0, 0), None);
    }

    #[test]
    fn test_coord_event_by_name_found() {
        let config = sample_config();
        assert_eq!(config.coord_event_by_name("northExit"), Some("enterRoute1"));
        assert_eq!(
            config.coord_event_by_name("southExit"),
            Some("enterStartTown")
        );
    }

    #[test]
    fn test_coord_event_by_name_not_found() {
        let config = sample_config();
        assert_eq!(config.coord_event_by_name("nonexistent"), None);
    }

    #[test]
    fn test_hidden_npc_ids() {
        let config = sample_config();
        let hidden = config.hidden_npc_ids();
        assert_eq!(hidden, vec![2]);
    }

    #[test]
    fn test_hidden_npc_ids_empty() {
        let config = MapScriptConfig::default();
        let hidden = config.hidden_npc_ids();
        assert!(hidden.is_empty());
    }

    #[test]
    fn test_npc_id_by_toggle_found() {
        let config = sample_config();
        assert_eq!(config.npc_id_by_toggle("START_TOWN_PROF"), Some(1));
    }

    #[test]
    fn test_npc_id_by_toggle_not_found() {
        let config = sample_config();
        assert_eq!(config.npc_id_by_toggle("NONEXISTENT"), None);
    }

    #[test]
    fn test_npc_id_by_script_id_found() {
        let config = sample_config();
        assert_eq!(config.npc_id_by_script_id("STARTTOWN_PROF"), Some(1));
    }

    #[test]
    fn test_npc_id_by_script_id_not_found() {
        let config = sample_config();
        assert_eq!(config.npc_id_by_script_id("NONEXISTENT"), None);
    }

    #[test]
    fn test_empty_config() {
        let config = MapScriptConfig::default();
        assert!(config.on_load.is_none());
        assert!(config.npcs.is_empty());
        assert!(config.signs.is_empty());
        assert!(config.coord_events.is_empty());
    }

    #[test]
    fn test_deserialize_full_config() {
        let json = r#"{
            "onLoad": "onEnter",
            "npcs": [
                {"id": 1, "talk": "talkProf", "toggleId": "T1", "scriptId": "S1", "defaultHidden": true}
            ],
            "signs": [
                {"id": 2, "talk": "signLab"}
            ],
            "coordEvents": [
                {"name": "exit", "position": [5, 3], "trigger": "onExit"}
            ]
        }"#;
        let config: MapScriptConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.on_load(), Some("onEnter"));
        assert_eq!(config.npc_talk_fn(1), Some("talkProf"));
        assert_eq!(config.npc_id_by_toggle("T1"), Some(1));
        assert_eq!(config.npc_id_by_script_id("S1"), Some(1));
        assert!(config.hidden_npc_ids().contains(&1));
        assert_eq!(config.sign_talk_fn(2), Some("signLab"));
        assert_eq!(config.coord_event_fn(5, 3), Some("onExit"));
        assert_eq!(config.coord_event_by_name("exit"), Some("onExit"));
    }
}
