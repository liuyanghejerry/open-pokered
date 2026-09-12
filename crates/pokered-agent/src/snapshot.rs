//! [`AgentSnapshot`]: the typed semantic observation of the live game.

use pokered_core::battle::{BattlePhase, BattleScreen};
use pokered_core::game_state::GameScreen;
use pokered_core::overworld::npc_movement::NpcRuntimeState;
use pokered_core::overworld::script_bridge::PendingChoice;
use pokered_core::overworld::{BedroomDialogue, Direction};
use pokered_core::pokemon::party::Party;
use pokered_data::items::ItemId;
use pokered_data::map_json::MapJson;
use pokered_data::maps::MapId;
use serde::{Deserialize, Serialize};

use crate::mode::{classify_mode, AgentMode, ModeInput, OverworldObs};
use crate::nearby::{nearby_entities, HiddenItemSpot, NearbyEntity, NpcObs, Position};
use crate::profile::ObservationProfile;

/// Map identity (numeric id + PascalCase name, same convention as the
/// existing debug snapshots).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapRef {
    pub id: u8,
    pub name: String,
}

/// One party member, compressed to what an agent plans with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyMonSummary {
    pub species: String,
    pub level: u8,
    pub hp: u16,
    pub max_hp: u16,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BagItemSummary {
    pub item: String,
    pub qty: u32,
}

/// Badge progress decoded from the `obtained_badges` bitfield.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadgeSummary {
    pub count: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<String>,
}

/// Badge names in `wObtainedBadges` bit order (bit 0 = Boulder … bit 7 = Earth).
pub const BADGE_NAMES: [&str; 8] = [
    "Boulder", "Cascade", "Thunder", "Rainbow", "Soul", "Marsh", "Volcano", "Earth",
];

impl BadgeSummary {
    pub fn from_bits(obtained_badges: u8) -> Self {
        let names: Vec<String> = BADGE_NAMES
            .iter()
            .enumerate()
            .filter(|(bit, _)| obtained_badges & (1 << bit) != 0)
            .map(|(_, name)| name.to_string())
            .collect();
        Self {
            count: names.len() as u8,
            names,
        }
    }
}

/// Overworld text box state, plus the script choice prompt when open.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueSummary {
    /// Full text of the current page (not the typewriter-revealed
    /// prefix the raw `get_state` snapshot reports — the typewriter
    /// progress is already conveyed by `waiting_for_input` / `done`).
    pub text: String,
    pub page: usize,
    pub total_pages: usize,
    pub waiting_for_input: bool,
    pub done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<ChoiceSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceSummary {
    pub options: Vec<String>,
    pub selected: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleMonSummary {
    pub species: String,
    pub level: u8,
    pub hp: u16,
    pub max_hp: u16,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleSummary {
    /// Phase variant name (Debug form, e.g. "PlayerMenu").
    pub phase: String,
    /// "wild" or "trainer".
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_class: Option<String>,
    /// Current battle text box message, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub player: BattleMonSummary,
    pub enemy: BattleMonSummary,
    pub player_party_size: usize,
    pub enemy_party_size: usize,
}

/// The typed semantic observation. Sections absent under the current
/// [`ObservationProfile`] (or meaningless for the mode) serialize as
/// omitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub mode: AgentMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<MapRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub party: Vec<PartyMonSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bag: Vec<BagItemSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badges: Option<BadgeSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialogue: Option<DialogueSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battle: Option<BattleSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nearby: Option<Vec<NearbyEntity>>,
}

/// Everything [`build_agent_snapshot`] reads, so the app-side adapter
/// only fills one struct. Lifetimes borrow the live game state; nothing
/// is mutated.
pub struct ObservationSource<'a> {
    pub screen: &'a GameScreen,
    pub map_id: MapId,
    pub player_x: u16,
    pub player_y: u16,
    pub player_facing: Direction,
    pub overworld: OverworldObs,
    /// Present when a battle owns the screen.
    pub battle_phase: Option<&'a BattlePhase>,
    pub party: &'a Party,
    pub bag: &'a [(ItemId, u32)],
    pub obtained_badges: u8,
    pub dialogue: Option<&'a BedroomDialogue>,
    pub choice: Option<&'a PendingChoice>,
    /// Present when a battle owns the screen.
    pub battle: Option<&'a BattleScreen>,
    pub npc_states: &'a [NpcRuntimeState],
    pub map_json: Option<&'a MapJson>,
    /// Hidden items on the current map (see [`crate::hidden_item_spots`]).
    pub hidden_items: &'a [HiddenItemSpot],
    /// Nearby radius override; the profile's `nearby_radius` is copied
    /// here by the adapter.
    pub nearby_radius: i32,
}

/// Build the typed snapshot from the observation source, honoring the
/// profile's section toggles. Mode, map, position, and facing are always
/// populated; party/bag/badges follow the level-1 toggles; dialogue and
/// battle follow the level-2 toggles (and are `None` when no dialogue or
/// battle is live); nearby follows the level-3 toggle.
pub fn build_agent_snapshot(
    src: &ObservationSource,
    profile: &ObservationProfile,
) -> AgentSnapshot {
    let mode = classify_mode(&ModeInput {
        screen: src.screen,
        overworld: src.overworld,
        battle_phase: src.battle_phase,
    });
    let position = Position {
        x: src.player_x as i32,
        y: src.player_y as i32,
    };

    let party = if profile.include_party {
        src.party
            .iter()
            .map(|mon| PartyMonSummary {
                species: format!("{:?}", mon.species),
                level: mon.level,
                hp: mon.hp,
                max_hp: mon.max_hp,
                status: format!("{:?}", mon.status),
            })
            .collect()
    } else {
        Vec::new()
    };

    let bag = if profile.include_bag {
        src.bag
            .iter()
            .map(|(id, qty)| BagItemSummary {
                item: format!("{:?}", id),
                qty: *qty,
            })
            .collect()
    } else {
        Vec::new()
    };

    let badges = profile
        .include_badges
        .then(|| BadgeSummary::from_bits(src.obtained_badges));

    let dialogue = if profile.include_dialogue {
        build_dialogue_summary(src.dialogue, src.choice)
    } else {
        None
    };

    let battle = if profile.include_battle {
        src.battle.map(build_battle_summary)
    } else {
        None
    };

    let nearby = if profile.include_nearby {
        let npcs: Vec<NpcObs> = src.npc_states.iter().map(NpcObs::from).collect();
        Some(nearby_entities(
            position,
            &npcs,
            src.map_json,
            src.hidden_items,
            src.nearby_radius,
        ))
    } else {
        None
    };

    AgentSnapshot {
        mode,
        map: Some(MapRef {
            id: src.map_id as u8,
            name: format!("{:?}", src.map_id),
        }),
        position: Some(position),
        facing: Some(format!("{:?}", src.player_facing)),
        party,
        bag,
        badges,
        dialogue,
        battle,
        nearby,
    }
}

fn build_dialogue_summary(
    dialogue: Option<&BedroomDialogue>,
    choice: Option<&PendingChoice>,
) -> Option<DialogueSummary> {
    if dialogue.is_none() && choice.is_none() {
        return None;
    }
    let summary = dialogue.map(|d| {
        let (line1, line2) = d
            .pages()
            .get(d.current_page())
            .map(|page| (page.line1, page.line2))
            .unwrap_or(("", ""));
        DialogueSummary {
            text: format!("{} {}", line1, line2).trim().to_string(),
            page: d.current_page() + 1,
            total_pages: d.pages().len(),
            waiting_for_input: d.waiting_for_input(),
            done: d.is_done(),
            choice: None,
        }
    });
    let choice = choice.map(|c| ChoiceSummary {
        options: c.options.clone(),
        selected: c.selected,
    });
    Some(match summary {
        Some(mut s) => {
            s.choice = choice;
            s
        }
        None => DialogueSummary {
            text: String::new(),
            page: 0,
            total_pages: 0,
            waiting_for_input: true,
            done: false,
            choice,
        },
    })
}

fn build_battle_summary(battle: &BattleScreen) -> BattleSummary {
    BattleSummary {
        phase: format!("{:?}", battle.phase),
        kind: if battle.is_wild { "wild" } else { "trainer" }.to_string(),
        trainer_class: battle.trainer_class.map(|c| format!("{:?}", c)),
        message: battle.current_message.clone(),
        player: BattleMonSummary {
            species: format!("{:?}", battle.player_species),
            level: battle.player_level,
            hp: battle.player_hp,
            max_hp: battle.player_max_hp,
            status: format!("{:?}", battle.player_status),
        },
        enemy: BattleMonSummary {
            species: format!("{:?}", battle.enemy_species),
            level: battle.enemy_level,
            hp: battle.enemy_hp,
            max_hp: battle.enemy_max_hp,
            status: format!("{:?}", battle.enemy_status),
        },
        player_party_size: battle.player_party_size,
        enemy_party_size: battle.enemy_party_size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_bits_decode_in_gen1_order() {
        let badges = BadgeSummary::from_bits(0b0000_0101);
        assert_eq!(badges.count, 2);
        assert_eq!(badges.names, ["Boulder", "Thunder"]);
        let all = BadgeSummary::from_bits(0xFF);
        assert_eq!(all.count, 8);
        assert_eq!(all.names[7], "Earth");
        let none = BadgeSummary::from_bits(0);
        assert_eq!(none.count, 0);
        assert!(none.names.is_empty());
    }

    #[test]
    fn empty_sections_serialize_sparse() {
        let snapshot = AgentSnapshot {
            mode: AgentMode::Overworld,
            map: Some(MapRef {
                id: 0,
                name: "PalletTown".to_string(),
            }),
            position: Some(Position { x: 10, y: 9 }),
            facing: Some("Down".to_string()),
            party: vec![],
            bag: vec![],
            badges: Some(BadgeSummary::from_bits(0)),
            dialogue: None,
            battle: None,
            nearby: None,
        };
        let json = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(json["mode"], "overworld");
        assert_eq!(json["map"]["name"], "PalletTown");
        assert!(json.get("party").is_none(), "empty party is omitted");
        assert!(json.get("bag").is_none(), "empty bag is omitted");
        assert!(json.get("dialogue").is_none(), "absent dialogue is omitted");
        assert!(json.get("nearby").is_none(), "absent nearby is omitted");
        assert_eq!(json["badges"]["count"], 0);
        assert!(
            json["badges"].get("names").is_none(),
            "empty badge names are omitted"
        );
    }
}
