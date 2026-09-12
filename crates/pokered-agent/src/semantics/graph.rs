//! Event graph over the extracted semantics: scripts (storylines),
//! flags, items, trainers, Pokémon, badges, maps, and resources as
//! nodes (id-convention strings), typed edges between them.
//!
//! Node id conventions:
//! - `flag:EVENT_X`, `item:POKE_BALL`, `badge:BOULDERBADGE`,
//!   `pokemon:CHARMANDER`, `trainer:OPP_RIVAL1`, `map:PalletTown`,
//!   `resource:money|coins`, `object:TOGGLE_ID`,
//!   `script:{map}:{storyline}`, `objective:{id}` (reserved).

use serde::{Deserialize, Serialize};

use super::types::{BattleKind, MapSemantics, StateEffect, StatePredicate};

/// Edge vocabulary. `Requires`/`Sets`/`Clears`/`Gives`/`StartsBattle`/
/// `WarpsTo`/`TriggeredAt` are the M4 core set; `Takes`/`Shows`/`Hides`
/// are factual extensions for take-item and show/hide effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Script reads flag/item/badge/resource.
    Requires,
    /// Script sets a flag.
    Sets,
    /// Script clears a flag.
    Clears,
    /// Script grants item/badge/Pokémon/money/coins.
    Gives,
    /// Script removes item/money/coins.
    Takes,
    /// Script starts a battle (trainer, wild, tutorial).
    StartsBattle,
    /// Script warps the player to a map.
    WarpsTo,
    /// Script fires on a map (its trigger binding).
    TriggeredAt,
    /// Script shows an object (extension).
    Shows,
    /// Script hides an object (extension).
    Hides,
}

/// One typed edge between two node ids.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EventEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// The whole-world event graph. Edge vector is sorted for deterministic
/// serialization.
pub struct EventGraph {
    edges: Vec<EventEdge>,
}

fn flag_node(flag: &str) -> String {
    format!("flag:{flag}")
}
fn item_node(item: &str) -> String {
    format!("item:{item}")
}
fn badge_node(badge: &str) -> String {
    format!("badge:{badge}")
}
fn script_node(id: &str) -> String {
    format!("script:{id}")
}

impl EventGraph {
    /// Build from every map's extracted semantics.
    pub fn build(maps: &[MapSemantics]) -> Self {
        let mut edges = Vec::new();
        for map in maps {
            for script in &map.storylines {
                let from = script_node(&script.id);
                let mut push = |to: String, kind: EdgeKind, detail: Option<String>| {
                    edges.push(EventEdge {
                        from: from.clone(),
                        to,
                        kind,
                        detail,
                    });
                };
                if !script.triggers.is_empty() {
                    let map_node = format!("map:{}", script.map);
                    push(map_node, EdgeKind::TriggeredAt, None);
                }
                for read in &script.reads {
                    match read {
                        StatePredicate::FlagRead { flag } => {
                            push(flag_node(flag), EdgeKind::Requires, None);
                        }
                        StatePredicate::ItemHeld { item } => {
                            push(item_node(item), EdgeKind::Requires, None);
                        }
                        StatePredicate::BadgeHeld { badge } => {
                            push(badge_node(badge), EdgeKind::Requires, None);
                        }
                        StatePredicate::MoneyAtLeast { amount } => {
                            push(
                                "resource:money".to_string(),
                                EdgeKind::Requires,
                                Some(format!(">={amount}")),
                            );
                        }
                        StatePredicate::CoinsAtLeast { amount } => {
                            push(
                                "resource:coins".to_string(),
                                EdgeKind::Requires,
                                Some(format!(">={amount}")),
                            );
                        }
                        StatePredicate::Query { .. } | StatePredicate::Unknown { .. } => {}
                    }
                }
                for effect in &script.effects {
                    match effect {
                        StateEffect::FlagSet { flag } => {
                            push(flag_node(flag), EdgeKind::Sets, None);
                        }
                        StateEffect::FlagCleared { flag } => {
                            push(flag_node(flag), EdgeKind::Clears, None);
                        }
                        StateEffect::ItemGiven { item, qty } => {
                            push(item_node(item), EdgeKind::Gives, qty.map(|q| format!("x{q}")));
                        }
                        StateEffect::ItemTaken { item, qty } => {
                            push(item_node(item), EdgeKind::Takes, qty.map(|q| format!("x{q}")));
                        }
                        StateEffect::PokemonGiven { species, level } => {
                            push(
                                format!("pokemon:{species}"),
                                EdgeKind::Gives,
                                level.map(|l| format!("lv{l}")),
                            );
                        }
                        StateEffect::BadgeGiven { badge } => {
                            push(badge_node(badge), EdgeKind::Gives, None);
                        }
                        StateEffect::BattleStarted { battle } => match battle {
                            BattleKind::Trainer { trainer_id, .. } => {
                                push(
                                    format!("trainer:{trainer_id}"),
                                    EdgeKind::StartsBattle,
                                    None,
                                );
                            }
                            BattleKind::Wild { species, level } => {
                                push(
                                    format!("pokemon:{species}"),
                                    EdgeKind::StartsBattle,
                                    level.map(|l| format!("lv{l}")),
                                );
                            }
                            BattleKind::OldManTutorial => {
                                push(
                                    "trainer:OLD_MAN_TUTORIAL".to_string(),
                                    EdgeKind::StartsBattle,
                                    Some("tutorial".to_string()),
                                );
                            }
                        },
                        StateEffect::PlayerWarped { map: dest, .. } => {
                            push(format!("map:{dest}"), EdgeKind::WarpsTo, None);
                        }
                        StateEffect::MoneyGiven { amount } => {
                            push(
                                "resource:money".to_string(),
                                EdgeKind::Gives,
                                amount.map(|a| a.to_string()),
                            );
                        }
                        StateEffect::MoneyTaken { amount } => {
                            push(
                                "resource:money".to_string(),
                                EdgeKind::Takes,
                                amount.map(|a| a.to_string()),
                            );
                        }
                        StateEffect::CoinsGiven { amount } => {
                            push(
                                "resource:coins".to_string(),
                                EdgeKind::Gives,
                                amount.map(|a| a.to_string()),
                            );
                        }
                        StateEffect::CoinsTaken { amount } => {
                            push(
                                "resource:coins".to_string(),
                                EdgeKind::Takes,
                                amount.map(|a| a.to_string()),
                            );
                        }
                        StateEffect::ObjectShown { toggle_id } => {
                            push(format!("object:{toggle_id}"), EdgeKind::Shows, None);
                        }
                        StateEffect::ObjectHidden { toggle_id } => {
                            push(format!("object:{toggle_id}"), EdgeKind::Hides, None);
                        }
                        StateEffect::NpcMoved { .. }
                        | StateEffect::PlayerMoved
                        | StateEffect::TileReplaced { .. }
                        | StateEffect::Unknown { .. } => {}
                    }
                }
            }
        }
        edges.sort();
        edges.dedup();
        Self { edges }
    }

    pub fn edges(&self) -> &[EventEdge] {
        &self.edges
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Every edge touching `node` in either direction.
    pub fn edges_of<'a>(&'a self, node: &str) -> impl Iterator<Item = &'a EventEdge> + 'a {
        let node = node.to_string();
        self.edges
            .iter()
            .filter(move |e| e.from == node || e.to == node)
    }

    /// Edges leaving `node` (script-side queries).
    pub fn edges_from<'a>(&'a self, node: &str) -> impl Iterator<Item = &'a EventEdge> + 'a {
        let node = node.to_string();
        self.edges.iter().filter(move |e| e.from == node)
    }

    /// Edges pointing at `node` (flag/item-side queries).
    pub fn edges_to<'a>(&'a self, node: &str) -> impl Iterator<Item = &'a EventEdge> + 'a {
        let node = node.to_string();
        self.edges.iter().filter(move |e| e.to == node)
    }

    /// Script ids with a `Sets` edge to `flag` (accepts the bare flag
    /// name or the `flag:` node id).
    pub fn scripts_setting(&self, flag: &str) -> Vec<&str> {
        let node = if flag.starts_with("flag:") {
            flag.to_string()
        } else {
            flag_node(flag)
        };
        self.edges
            .iter()
            .filter(move |e| e.kind == EdgeKind::Sets && e.to == node)
            .map(|e| e.from.as_str())
            .collect()
    }

    /// `Requires` edges leaving a script (accepts the bare storyline id
    /// or the `script:` node id).
    pub fn prerequisites_of<'a>(&'a self, script_id: &str) -> impl Iterator<Item = &'a EventEdge> + 'a {
        let node = if script_id.starts_with("script:") {
            script_id.to_string()
        } else {
            script_node(script_id)
        };
        self.edges
            .iter()
            .filter(move |e| e.kind == EdgeKind::Requires && e.from == node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantics::gen::generate_world_semantics;

    fn graph() -> EventGraph {
        EventGraph::build(&generate_world_semantics().maps)
    }

    #[test]
    fn pallet_north_exit_edges() {
        let g = graph();
        let script = "script:PalletTown:coordNorthExit";
        assert!(
            g.edges_from(script)
                .any(|e| e.kind == EdgeKind::Sets && e.to == "flag:EVENT_OAK_APPEARED_IN_PALLET"),
            "missing Sets edge"
        );
        assert!(
            g.prerequisites_of(script)
                .any(|e| e.to == "flag:EVENT_FOLLOWED_OAK_INTO_LAB"),
            "missing Requires edge"
        );
        assert!(
            g.edges_from(script)
                .any(|e| e.kind == EdgeKind::TriggeredAt && e.to == "map:PalletTown"),
            "missing TriggeredAt edge"
        );
        assert!(
            g.edges_from(script)
                .any(|e| e.kind == EdgeKind::Shows && e.to == "object:PALLET_TOWN_OBJ_1"),
            "missing Shows edge"
        );
        assert!(
            g.edges_from(script)
                .any(|e| e.kind == EdgeKind::Hides && e.to == "object:PALLET_TOWN_OBJ_1"),
            "missing Hides edge"
        );
    }

    #[test]
    fn oaks_lab_gives_and_battles() {
        let g = graph();
        let script = "script:OaksLab:talkOak1";
        assert!(
            g.edges_from(script)
                .any(|e| e.kind == EdgeKind::Gives && e.to == "item:POKE_BALL"),
            "missing Gives edge"
        );
        assert!(
            g.edges_from(script)
                .any(|e| e.kind == EdgeKind::Sets && e.to == "flag:EVENT_GOT_POKEDEX"),
            "missing Sets edge"
        );
        // Rival battle: script → StartsBattle → trainer:OPP_RIVAL1.
        assert!(
            g.edges()
                .iter()
                .any(|e| e.kind == EdgeKind::StartsBattle
                    && e.to == "trainer:OPP_RIVAL1"
                    && e.from.starts_with("script:OaksLab:")),
            "missing StartsBattle edge"
        );
        // scripts_setting finds the pokedex setter.
        assert!(g.scripts_setting("EVENT_GOT_POKEDEX").contains(&script));
    }

    #[test]
    fn starter_balls_give_starters() {
        let g = graph();
        assert!(
            g.edges()
                .iter()
                .any(|e| e.kind == EdgeKind::Gives && e.to == "pokemon:CHARMANDER"),
            "missing starter gift edge"
        );
        assert!(
            g.edges()
                .iter()
                .any(|e| e.kind == EdgeKind::Sets && e.to == "flag:EVENT_GOT_STARTER"),
            "missing starter flag edge"
        );
    }
}
