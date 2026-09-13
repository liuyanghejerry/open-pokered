//! AST walker: `GameScene` → [`ScriptSemantics`] per storyline, with
//! coverage accounting. Every `Command` statement and every `Call`
//! expression is either recognized (typed predicate/effect, or a known
//! no-state command/query) or reported as `Unknown`.

use std::collections::BTreeMap;

use dotzuki_engine_dsl::ast::{Expression, GameScene, StoryStmt};

use super::types::{
    BattleKind, CoverageReport, MapSemantics, ScriptSemantics, StateEffect, StatePredicate,
    UnknownConstruct,
};

/// Normalize a command/callee name: some scenes invoke through the
/// `game.` object (`game.setFlag(...)`), others bare (`setFlag(...)`).
fn normalize_name(name: &str) -> &str {
    name.strip_prefix("game.").unwrap_or(name)
}

/// Commands that produce a typed [`StateEffect`] — whether they appear
/// as statements or as call expressions. Anything else is either a
/// recognized no-state command or `Unknown`.
const EFFECT_COMMANDS: &[&str] = &[
    "setFlag",
    "resetFlag",
    "giveItem",
    "takeItem",
    "givePokemon",
    "giveBadge",
    "startBattle",
    "startBattleSet",
    "startWildBattle",
    "oldManTutorial",
    "warpTo",
    "moveNpc",
    "startNpcMove",
    "moveNpcTo",
    "startNpcMoveTo",
    "setNpcPosition",
    "followNpc",
    "movePlayer",
    "movePlayerTo",
    "movePlayerRelative",
    "showObject",
    "showObjectByName",
    "hideObject",
    "hideObjectByName",
    "replaceTileBlock",
    "giveMoney",
    "takeMoney",
    "giveCoins",
    "takeCoins",
];

/// Map an effect-command invocation to a typed [`StateEffect`].
fn command_effect(name: &str, args: &[Expression]) -> StateEffect {
    let str_arg = |i: usize| -> Option<String> {
        match args.get(i) {
            Some(Expression::StringLit(s)) => Some(s.clone()),
            _ => None,
        }
    };
    let num_arg = |i: usize| -> Option<u32> {
        match args.get(i) {
            Some(Expression::NumberLit(n)) => Some(*n as u32),
            _ => None,
        }
    };
    let dynamic = || StateEffect::Unknown {
        construct: format!("{name}(dynamic_arg)"),
    };
    match name {
        "setFlag" => match str_arg(0) {
            Some(flag) => StateEffect::FlagSet { flag },
            None => dynamic(),
        },
        "resetFlag" => match str_arg(0) {
            Some(flag) => StateEffect::FlagCleared { flag },
            None => dynamic(),
        },
        "giveItem" => match str_arg(0) {
            Some(item) => StateEffect::ItemGiven {
                item,
                qty: num_arg(1).map(|n| n as u8),
            },
            None => dynamic(),
        },
        "takeItem" => match str_arg(0) {
            Some(item) => StateEffect::ItemTaken {
                item,
                qty: num_arg(1).map(|n| n as u8),
            },
            None => dynamic(),
        },
        "givePokemon" => match str_arg(0) {
            Some(species) => StateEffect::PokemonGiven {
                species,
                level: num_arg(1).map(|n| n as u8),
            },
            None => dynamic(),
        },
        "giveBadge" => {
            let badge = str_arg(0).or_else(|| num_arg(0).map(|n| n.to_string()));
            match badge {
                Some(badge) => StateEffect::BadgeGiven { badge },
                None => dynamic(),
            }
        }
        "startBattle" | "startBattleSet" => match str_arg(0) {
            Some(trainer_id) => {
                let (class, set) = trainer_parts(&trainer_id);
                StateEffect::BattleStarted {
                    battle: BattleKind::Trainer {
                        trainer_id,
                        class,
                        set,
                    },
                }
            }
            None => dynamic(),
        },
        "startWildBattle" => match str_arg(0) {
            Some(species) => StateEffect::BattleStarted {
                battle: BattleKind::Wild {
                    species,
                    level: num_arg(1).map(|n| n as u8),
                },
            },
            None => dynamic(),
        },
        "oldManTutorial" => StateEffect::BattleStarted {
            battle: BattleKind::OldManTutorial,
        },
        "warpTo" => match str_arg(0) {
            Some(map) => StateEffect::PlayerWarped {
                map,
                x: num_arg(1).map(|n| n as u8),
                y: num_arg(2).map(|n| n as u8),
            },
            None => dynamic(),
        },
        "moveNpc" | "startNpcMove" | "moveNpcTo" | "startNpcMoveTo" | "setNpcPosition"
        | "followNpc" => match str_arg(0) {
            Some(npc_id) => StateEffect::NpcMoved { npc_id },
            None => dynamic(),
        },
        "movePlayer" | "movePlayerTo" | "movePlayerRelative" => StateEffect::PlayerMoved,
        "showObject" | "showObjectByName" => match str_arg(0) {
            Some(toggle_id) => StateEffect::ObjectShown { toggle_id },
            // Numeric object-index form: resolvable only with the map's
            // object list — kept factual via the numeric id.
            None => match num_arg(0) {
                Some(index) => StateEffect::ObjectShown {
                    toggle_id: format!("#{index}"),
                },
                None => dynamic(),
            },
        },
        "hideObject" | "hideObjectByName" => match str_arg(0) {
            Some(toggle_id) => StateEffect::ObjectHidden { toggle_id },
            None => match num_arg(0) {
                Some(index) => StateEffect::ObjectHidden {
                    toggle_id: format!("#{index}"),
                },
                None => dynamic(),
            },
        },
        "replaceTileBlock" => StateEffect::TileReplaced {
            x: num_arg(0).map(|n| n as u8),
            y: num_arg(1).map(|n| n as u8),
            block: num_arg(2).map(|n| n as u8),
        },
        "giveMoney" => StateEffect::MoneyGiven {
            amount: num_arg(0),
        },
        "takeMoney" => StateEffect::MoneyTaken {
            amount: num_arg(0),
        },
        "giveCoins" => StateEffect::CoinsGiven {
            amount: num_arg(0),
        },
        "takeCoins" => StateEffect::CoinsTaken {
            amount: num_arg(0),
        },
        _ => StateEffect::Unknown {
            construct: name.to_string(),
        },
    }
}

/// Commands the analyzer recognizes as having NO state effect relevant
/// to the event graph (dialogue, presentation, audio, control flow).
/// Listed explicitly so unknown typos still surface as unknown.
const NO_STATE_COMMANDS: &[&str] = &[
    "showText",
    "showRandomText",
    "showChoice",
    "delay",
    "facePlayer",
    "faceNpc",
    "setNpcFrame",
    "playMusic",
    "playSound",
    "playCry",
    "stopMusic",
    "fadeOutMusic",
    "fadeScreen",
    "setJoyIgnore",
    "clearJoyIgnore",
    "heal",
    "animateHealingMachine",
    "openShop",
    "openSlots",
    "elevatorMenu",
    "filterBag",
    "showDiploma",
    "openPC",
    "openItemPC",
    "openBillsPC",
    "linkStart",
    "enterHallOfFame",
    "showPokedexEntry",
    "tradePokemon",
    "openNamingScreen",
    "choosePartyPokemon",
    "setPartyNickname",
    "depositDaycare",
    "withdrawDaycare",
    "showEmotionBubble",
    "showScene",
    "hideScene",
    "playShipDeparture",
    // DSL early-return control flow (no state effect).
    "return",
];

/// Expression-level calls that map to a typed [`StatePredicate`];
/// anything else is either a known query (`Query`) or `Unknown`.
fn call_predicate(callee: &str, args: &[Expression]) -> StatePredicate {
    let str_arg = |i: usize| -> Option<String> {
        match args.get(i) {
            Some(Expression::StringLit(s)) => Some(s.clone()),
            _ => None,
        }
    };
    let num_arg = |i: usize| -> Option<u32> {
        match args.get(i) {
            Some(Expression::NumberLit(n)) => Some(*n as u32),
            _ => None,
        }
    };
    match callee {
        "getFlag" => match str_arg(0) {
            Some(flag) => StatePredicate::FlagRead { flag },
            None => StatePredicate::Unknown {
                construct: "getFlag(dynamic_arg)".to_string(),
            },
        },
        "hasItem" => match str_arg(0) {
            Some(item) => StatePredicate::ItemHeld { item },
            None => StatePredicate::Unknown {
                construct: "hasItem(dynamic_arg)".to_string(),
            },
        },
        "hasBadge" => match str_arg(0) {
            Some(badge) => StatePredicate::BadgeHeld { badge },
            None => StatePredicate::Unknown {
                construct: "hasBadge(dynamic_arg)".to_string(),
            },
        },
        "hasMoney" => match num_arg(0) {
            Some(amount) => StatePredicate::MoneyAtLeast { amount },
            None => StatePredicate::Unknown {
                construct: "hasMoney(dynamic_arg)".to_string(),
            },
        },
        "hasCoins" => match num_arg(0) {
            Some(amount) => StatePredicate::CoinsAtLeast { amount },
            None => StatePredicate::Unknown {
                construct: "hasCoins(dynamic_arg)".to_string(),
            },
        },
        _ if KNOWN_QUERIES.contains(&callee) => StatePredicate::Query {
            name: callee.to_string(),
        },
        _ => StatePredicate::Unknown {
            construct: callee.to_string(),
        },
    }
}

/// Recognized query functions with no typed predicate form.
const KNOWN_QUERIES: &[&str] = &[
    "getMoney",
    "getCoins",
    "getPokedexOwnedCount",
    "getPokedexSeenCount",
    "getPlayerFacing",
    "getRivalStarter",
    "getBadgeCount",
    "isDaycareInUse",
    "getDaycareMonName",
    "getDaycareLevelsGrown",
    "getDaycareCost",
    "getPartyCount",
    "getPartyMonName",
    "partyMonKnowsHm",
    "getPlayerX",
    "getPlayerY",
    "getPlayerPosition",
    "getGameVersion",
    "lang",
    "t",
];

/// `(class, set)` for a trainer id, resolved via trainer data; `None`
/// when the id does not parse.
fn trainer_parts(trainer_id: &str) -> (Option<String>, Option<usize>) {
    pokered_data::trainer_data::parse_trainer_id(trainer_id)
        .map(|(class, set)| (Some(format!("{:?}", class)), Some(set)))
        .unwrap_or((None, None))
}

/// Per-extraction state: the accumulated semantics plus coverage counts.
pub(crate) struct Extractor<'a> {
    map: &'a str,
    reads: Vec<StatePredicate>,
    effects: Vec<StateEffect>,
    coverage: &'a mut CoverageAccum,
}

#[derive(Default)]
pub(crate) struct CoverageAccum {
    pub maps_analyzed: usize,
    pub maps_without_scene: usize,
    pub storylines_analyzed: usize,
    pub commands_recognized: usize,
    pub commands_unknown: usize,
    pub calls_recognized: usize,
    pub calls_unknown: usize,
    /// construct name → (count, first map seen).
    pub unknown: BTreeMap<String, (usize, String)>,
}

impl CoverageAccum {
    fn note_unknown(&mut self, construct: &str, map: &str) {
        self.unknown
            .entry(construct.to_string())
            .and_modify(|(count, _)| *count += 1)
            .or_insert_with(|| (1, map.to_string()));
    }

    pub(crate) fn into_report(self) -> CoverageReport {
        CoverageReport {
            maps_analyzed: self.maps_analyzed,
            maps_without_scene: self.maps_without_scene,
            storylines_analyzed: self.storylines_analyzed,
            commands_recognized: self.commands_recognized,
            commands_unknown: self.commands_unknown,
            calls_recognized: self.calls_recognized,
            calls_unknown: self.calls_unknown,
            unknown_constructs: self
                .unknown
                .into_iter()
                .map(|(name, (count, example_map))| UnknownConstruct {
                    name,
                    count,
                    example_map,
                })
                .collect(),
        }
    }
}

impl<'a> Extractor<'a> {
    fn effect(&mut self, effect: StateEffect) {
        match &effect {
            StateEffect::Unknown { construct } => {
                self.coverage.commands_unknown += 1;
                self.coverage.note_unknown(construct, self.map);
            }
            _ => self.coverage.commands_recognized += 1,
        }
        self.effects.push(effect);
    }

    fn predicate(&mut self, predicate: StatePredicate) {
        match &predicate {
            StatePredicate::Unknown { construct } => {
                self.coverage.calls_unknown += 1;
                self.coverage.note_unknown(construct, self.map);
            }
            _ => self.coverage.calls_recognized += 1,
        }
        self.reads.push(predicate);
    }

    fn walk_statements(&mut self, statements: &[StoryStmt]) {
        for stmt in statements {
            self.walk_statement(stmt);
        }
    }

    fn walk_statement(&mut self, stmt: &StoryStmt) {
        match stmt {
            StoryStmt::Speaker { texts, name, .. } | StoryStmt::Say { texts, name, .. } => {
                let _ = texts;
                self.walk_expression(name);
            }
            StoryStmt::Choice { options, .. } => {
                for option in options {
                    self.walk_statements(&option.body);
                }
            }
            StoryStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.walk_expression(condition);
                self.walk_statements(then_branch);
                self.walk_statements(else_branch);
            }
            StoryStmt::Each { source, body, .. } => {
                self.walk_expression(source);
                self.walk_statements(body);
            }
            StoryStmt::Run { .. } => {
                self.effect(StateEffect::Unknown {
                    construct: "run_js".to_string(),
                });
            }
            StoryStmt::Return { .. } => {
                // Control flow (end the storyline early) — no state
                // reads or effects to record.
            }
            StoryStmt::Assign { value, .. } => {
                self.walk_expression(value);
            }
            StoryStmt::Command { name, args, .. } => {
                // Args may themselves contain calls (e.g. moveNpcTo with
                // getPlayerX()); walk them for predicates first.
                for arg in args {
                    self.walk_expression(arg);
                }
                let name = normalize_name(name);
                if NO_STATE_COMMANDS.contains(&name) {
                    self.coverage.commands_recognized += 1;
                } else {
                    self.effect(command_effect(name, args));
                }
            }
        }
    }

    fn walk_expression(&mut self, expr: &Expression) {
        match expr {
            Expression::StringLit(_)
            | Expression::Localized(_)
            | Expression::NumberLit(_)
            | Expression::BoolLit(_)
            | Expression::Variable(_) => {}
            Expression::ArrayLit(items) => {
                for item in items {
                    self.walk_expression(item);
                }
            }
            Expression::ObjectLit(entries) => {
                for (_, value) in entries {
                    self.walk_expression(value);
                }
            }
            Expression::Call { callee, args } => {
                for arg in args {
                    self.walk_expression(arg);
                }
                // Effect commands are invoked as calls too
                // (`result = startBattle("OPP_RIVAL1")`, bare
                // `setFlag(...)` expressions) — produce effects for
                // them here as well as in statement position.
                let callee = normalize_name(callee);
                if NO_STATE_COMMANDS.contains(&callee) {
                    self.coverage.commands_recognized += 1;
                } else if EFFECT_COMMANDS.contains(&callee) {
                    self.effect(command_effect(callee, args));
                } else {
                    self.predicate(call_predicate(callee, args));
                }
            }
            Expression::UnaryOp { operand, .. } => self.walk_expression(operand),
            Expression::BinaryOp { left, right, .. } => {
                self.walk_expression(left);
                self.walk_expression(right);
            }
            Expression::TernaryOp {
                condition,
                then_expr,
                else_expr,
            } => {
                self.walk_expression(condition);
                self.walk_expression(then_expr);
                self.walk_expression(else_expr);
            }
        }
    }

    fn finish(mut self, storyline: &str, triggers: Vec<String>) -> ScriptSemantics {
        self.reads.sort();
        self.reads.dedup();
        self.effects.sort();
        self.effects.dedup();
        let storyline = storyline.to_string();
        ScriptSemantics {
            id: format!("{}:{}", self.map, storyline),
            map: self.map.to_string(),
            storyline,
            triggers,
            reads: self.reads,
            effects: self.effects,
        }
    }
}

/// Extract one storyline's semantics (coverage not tracked — used by
/// tests and single-storyline queries).
pub fn extract_storyline(
    map: &str,
    storyline: &str,
    triggers: &[String],
    statements: &[StoryStmt],
) -> ScriptSemantics {
    let mut coverage = CoverageAccum::default();
    let mut ex = Extractor {
        map,
        reads: Vec::new(),
        effects: Vec::new(),
        coverage: &mut coverage,
    };
    ex.walk_statements(statements);
    ex.finish(storyline, triggers.to_vec())
}

/// Extract all storylines of one map (including its `@load` block),
/// updating the coverage accumulator.
pub(crate) fn extract_map_with_coverage(
    scene: &GameScene,
    coverage: &mut CoverageAccum,
) -> MapSemantics {
    let map = scene.name.as_str();
    let mut storylines = Vec::new();
    if let Some(on_load) = &scene.on_load {
        let mut ex = Extractor {
            map,
            reads: Vec::new(),
            effects: Vec::new(),
            coverage,
        };
        ex.walk_statements(&on_load.statements);
        storylines.push(ex.finish("@load", vec!["load".to_string()]));
        coverage.storylines_analyzed += 1;
    }
    for storyline in &scene.storylines {
        let triggers: Vec<String> = storyline
            .triggers
            .iter()
            .flat_map(|t| {
                let mut v: Vec<String> = Vec::new();
                if let Some(npc) = t.npc_id {
                    v.push(format!("npc:{npc}"));
                }
                if let Some(sign) = t.sign_id {
                    v.push(format!("sign:{sign}"));
                }
                v.extend(t.coords.iter().map(|(x, y)| format!("coord:({x},{y})")));
                if !t.name.is_empty() {
                    v.push(format!("name:{}", t.name));
                }
                v
            })
            .collect();
        let mut ex = Extractor {
            map,
            reads: Vec::new(),
            effects: Vec::new(),
            coverage,
        };
        ex.walk_statements(&storyline.statements);
        storylines.push(ex.finish(&storyline.name, triggers));
        coverage.storylines_analyzed += 1;
    }
    coverage.maps_analyzed += 1;
    MapSemantics {
        map: map.to_string(),
        storylines,
    }
}

/// Extract all storylines of one map by name (no coverage output —
/// covers the `get_script_semantics {map}` wire path).
pub fn extract_map_semantics(map: &str) -> Option<MapSemantics> {
    let scene = pokered_data::embedded_scenes::get_scene_ast(map)?;
    let mut coverage = CoverageAccum::default();
    Some(extract_map_with_coverage(&scene, &mut coverage))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pallet() -> GameScene {
        pokered_data::embedded_scenes::get_scene_ast("PalletTown").unwrap()
    }

    fn sem(map: &GameScene, storyline: &str) -> ScriptSemantics {
        let mut coverage = CoverageAccum::default();
        extract_map_with_coverage(map, &mut coverage)
            .storylines
            .into_iter()
            .find(|s| s.storyline == storyline)
            .unwrap_or_else(|| panic!("storyline {storyline}"))
    }

    #[test]
    fn pallet_load_reads_and_hides() {
        let load = sem(&pallet(), "@load");
        assert!(
            load.reads.contains(&StatePredicate::FlagRead {
                flag: "EVENT_FOLLOWED_OAK_INTO_LAB".to_string()
            }),
            "{:?}",
            load.reads
        );
        assert!(
            load.reads.contains(&StatePredicate::FlagRead {
                flag: "EVENT_GOT_POKEBALLS_FROM_OAK".to_string()
            }),
            "{:?}",
            load.reads
        );
        assert!(
            load.effects.contains(&StateEffect::ObjectHidden {
                toggle_id: "PALLET_TOWN_OBJ_1".to_string()
            }),
            "{:?}",
            load.effects
        );
        assert!(
            load.effects.contains(&StateEffect::FlagSet {
                flag: "EVENT_PALLET_AFTER_GETTING_POKEBALLS".to_string()
            }),
            "{:?}",
            load.effects
        );
    }

    #[test]
    fn pallet_north_exit_semantics() {
        let exit = sem(&pallet(), "coordNorthExit");
        // Reads the escort flag (twice in the scene — deduped).
        assert!(
            exit.reads.contains(&StatePredicate::FlagRead {
                flag: "EVENT_FOLLOWED_OAK_INTO_LAB".to_string()
            }),
            "{:?}",
            exit.reads
        );
        // Sets the appearance flag, shows then hides Oak, moves him,
        // walks the player onto the door tile.
        assert!(
            exit.effects.contains(&StateEffect::FlagSet {
                flag: "EVENT_OAK_APPEARED_IN_PALLET".to_string()
            }),
            "{:?}",
            exit.effects
        );
        assert!(
            exit
                .effects
                .contains(&StateEffect::ObjectShown { toggle_id: "PALLET_TOWN_OBJ_1".into() })
        );
        assert!(
            exit
                .effects
                .contains(&StateEffect::ObjectHidden { toggle_id: "PALLET_TOWN_OBJ_1".into() })
        );
        assert!(
            exit.effects.contains(&StateEffect::NpcMoved {
                npc_id: "PALLET_TOWN_OBJ_1".to_string()
            }),
            "{:?}",
            exit.effects
        );
        assert!(exit.effects.contains(&StateEffect::PlayerMoved));
        // Nothing unknown in this storyline.
        assert!(
            !exit
                .effects
                .iter()
                .any(|e| matches!(e, StateEffect::Unknown { .. })),
            "{:?}",
            exit.effects
        );
        assert!(
            !exit
                .reads
                .iter()
                .any(|r| matches!(r, StatePredicate::Unknown { .. })),
            "{:?}",
            exit.reads
        );
    }

    #[test]
    fn oaks_lab_starter_and_rival_battle() {
        let lab = pokered_data::embedded_scenes::get_scene_ast("OaksLab").unwrap();
        let ball = sem(&lab, "talkCharmanderBall");
        assert!(
            ball.effects.contains(&StateEffect::PokemonGiven {
                species: "CHARMANDER".to_string(),
                level: Some(5)
            }),
            "{:?}",
            ball.effects
        );
        assert!(
            ball.effects.contains(&StateEffect::FlagSet {
                flag: "EVENT_GOT_STARTER".to_string()
            }),
            "{:?}",
            ball.effects
        );

        // The first rival battle is a Trainer battle with class resolved.
        let mut coverage = CoverageAccum::default();
        let semantics = extract_map_with_coverage(&lab, &mut coverage);
        let rival = semantics
            .storylines
            .iter()
            .flat_map(|s| s.effects.iter())
            .find_map(|e| match e {
                StateEffect::BattleStarted {
                    battle: BattleKind::Trainer { trainer_id, .. },
                } if trainer_id == "OPP_RIVAL1" => Some(e),
                _ => None,
            })
            .expect("rival battle");
        assert_eq!(
            rival,
            &StateEffect::BattleStarted {
                battle: BattleKind::Trainer {
                    trainer_id: "OPP_RIVAL1".to_string(),
                    class: Some("Rival1".to_string()),
                    set: Some(0),
                }
            }
        );
    }

    #[test]
    fn oaks_lab_parcel_and_pokedex() {
        let lab = pokered_data::embedded_scenes::get_scene_ast("OaksLab").unwrap();
        let oak = sem(&lab, "talkOak1");
        assert!(
            oak.effects.contains(&StateEffect::FlagSet {
                flag: "EVENT_OAK_GOT_PARCEL".to_string()
            }),
            "{:?}",
            oak.effects
        );
        assert!(
            oak.effects.contains(&StateEffect::FlagSet {
                flag: "EVENT_GOT_POKEDEX".to_string()
            }),
            "{:?}",
            oak.effects
        );
        assert!(
            oak.effects.contains(&StateEffect::ItemGiven {
                item: "POKE_BALL".to_string(),
                qty: Some(5)
            }),
            "{:?}",
            oak.effects
        );
    }

    #[test]
    fn unknown_constructs_are_reported_not_guessed() {
        // A synthetic storyline with an unrecognized command and a
        // dynamic-flag call.
        let stmts = vec![
            StoryStmt::Command {
                name: "teleportPlayerToMoon".to_string(),
                args: vec![],
                span: SourceSpan::point("test", 1, 1),
            },
            StoryStmt::If {
                condition: Expression::Call {
                    callee: "getFlag".to_string(),
                    args: vec![Expression::Variable("some_var".to_string())],
                },
                then_branch: vec![],
                else_branch: vec![],
                span: SourceSpan::point("test", 2, 1),
            },
            StoryStmt::Run {
                js: "game.anything()".to_string(),
                span: SourceSpan::point("test", 3, 1),
            },
        ];
        let sem = extract_storyline("TestMap", "main", &[], &stmts);
        assert!(
            sem.effects.contains(&StateEffect::Unknown {
                construct: "teleportPlayerToMoon".to_string()
            }),
            "{:?}",
            sem.effects
        );
        assert!(
            sem.effects.contains(&StateEffect::Unknown {
                construct: "run_js".to_string()
            }),
            "{:?}",
            sem.effects
        );
        assert!(
            sem.reads.contains(&StatePredicate::Unknown {
                construct: "getFlag(dynamic_arg)".to_string()
            }),
            "{:?}",
            sem.reads
        );
    }

    use dotzuki_engine_dsl::ast::SourceSpan;
}
