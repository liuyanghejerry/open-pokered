//! Generate a map's `script_config.json` **from** its `.scene` source.
//!
//! In the unified DSL design the `.scene` file is the single source of truth:
//! it carries both the script bodies (storylines) and the routing/binding data
//! (`@trigger(npc = N, sign = N, coord = [x,y], toggle = ..., script = ...,
//! hidden = true, no_talk = true)` + the `@load` block). The runtime still
//! consumes the familiar `script_config.json` (`MapScriptConfig`), so this
//! module re-derives that JSON from the parsed scene. A round-trip test then
//! asserts the generated config matches the committed one — the no-drift
//! guarantee between the DSL source and the runtime binding contract.

use crate::ast;
use crate::{lexer, parser};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// Build the `script_config.json` value for an already-parsed scene.
pub fn scene_to_script_config(scene: &ast::GameScene) -> Value {
    // NPC entries keyed by id so a talk handler and object-only metadata for
    // the same id merge into one entry.
    let mut npcs: BTreeMap<u8, Map<String, Value>> = BTreeMap::new();
    let mut signs: BTreeMap<u8, String> = BTreeMap::new();
    let mut coord_events: Vec<Value> = Vec::new();

    for storyline in &scene.storylines {
        let fn_name = storyline.name.clone();
        for trigger in &storyline.triggers {
            if let Some(id) = trigger.npc_id {
                let entry = npcs.entry(id).or_insert_with(Map::new);
                entry.insert("id".into(), json!(id));
                if !trigger.no_talk {
                    entry.insert("talk".into(), json!(fn_name));
                }
                if let Some(t) = &trigger.toggle_id {
                    entry.insert("toggleId".into(), json!(t));
                }
                if let Some(s) = &trigger.script_id {
                    entry.insert("scriptId".into(), json!(s));
                }
                if trigger.default_hidden {
                    entry.insert("defaultHidden".into(), json!(true));
                }
            }

            if let Some(id) = trigger.sign_id {
                signs.insert(id, fn_name.clone());
            }

            for (idx, (x, y)) in trigger.coords.iter().enumerate() {
                let coord_name = if trigger.name.is_empty() {
                    format!("{}_{}_{}", fn_name, x, y)
                } else if trigger.coords.len() == 1 {
                    trigger.name.clone()
                } else {
                    // Multiple coords with a named trigger: the .scene `name`
                    // already carries the first coord's suffix (e.g. "northExit1",
                    // "cardKeyDoor11"). Strip only the LAST digit so
                    // "cardKeyDoor11" → base "cardKeyDoor1", not "cardKeyDoor".
                    let base = if trigger
                        .name
                        .as_bytes()
                        .last()
                        .map_or(false, |b| b.is_ascii_digit())
                    {
                        &trigger.name[..trigger.name.len() - 1]
                    } else {
                        trigger.name.as_str()
                    };
                    if idx == 0 {
                        trigger.name.clone()
                    } else {
                        format!("{}{}", base, idx + 1)
                    }
                };
                coord_events
                    .push(json!({ "name": coord_name, "position": [*x, *y], "trigger": fn_name }));
            }
        } // for trigger
    } // for storyline

    let mut root = Map::new();
    root.insert(
        "$schema".into(),
        json!("../../schemas/script_config.schema.json"),
    );
    if scene.on_load.is_some() {
        root.insert("onLoad".into(), json!(format!("{}OnLoad", scene.name)));
    }
    root.insert(
        "npcs".into(),
        Value::Array(npcs.into_values().map(Value::Object).collect()),
    );
    root.insert(
        "signs".into(),
        Value::Array(
            signs
                .into_iter()
                .map(|(id, talk)| json!({ "id": id, "talk": talk }))
                .collect(),
        ),
    );
    root.insert("coordEvents".into(), Value::Array(coord_events));
    Value::Object(root)
}

/// Parse a `.scene` source string and produce its `script_config.json` text.
pub fn compile_scene_to_config(source: &str, file_path: &str) -> Result<String, String> {
    let tokens = lexer::Lexer::new(source, file_path)
        .tokenize()
        .map_err(|errors| {
            errors
                .iter()
                .map(|e| format!("{}:{}: {}", e.line, e.col, e.message))
                .collect::<Vec<_>>()
                .join("; ")
        })?;

    let (doc, parse_errors, semantic_errors) = parser::parse_and_validate(tokens, source);
    if !parse_errors.is_empty() {
        return Err(parse_errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "));
    }
    if !semantic_errors.is_empty() {
        return Err(semantic_errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "));
    }

    let doc = doc.ok_or_else(|| "parser returned no document".to_string())?;
    match doc {
        ast::Document::Scene(scene) => {
            let value = scene_to_script_config(&scene);
            serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
        }
        _ => Err("only .scene files are supported by this function".to_string()),
    }
}

/// Normalize a `script_config.json` value for **semantic** comparison
/// (sort npcs/signs by id, sort coord events, drop `$schema`, treat an absent
/// `defaultHidden`/`talk` as its default). Used by round-trip verification so
/// formatting/ordering differences don't cause false mismatches.
pub fn normalize_config(value: &Value) -> Value {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return value.clone(),
    };
    let mut out = Map::new();

    out.insert(
        "onLoad".into(),
        obj.get("onLoad").cloned().unwrap_or(Value::Null),
    );

    let mut npcs: Vec<Value> = obj
        .get("npcs")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().map(normalize_npc).collect())
        .unwrap_or_default();
    npcs.sort_by_key(npc_sort_key);
    out.insert("npcs".into(), Value::Array(npcs));

    let mut signs: Vec<Value> = obj
        .get("signs")
        .and_then(|v| v.as_array())
        .map(|a| a.to_vec())
        .unwrap_or_default();
    signs.sort_by_key(|s| s.get("id").and_then(|v| v.as_u64()).unwrap_or(0));
    out.insert("signs".into(), Value::Array(signs));

    let mut coords: Vec<Value> = obj
        .get("coordEvents")
        .and_then(|v| v.as_array())
        .map(|a| a.to_vec())
        .unwrap_or_default();
    coords.sort_by_key(|c| {
        let pos = c.get("position").and_then(|v| v.as_array());
        let x = pos
            .and_then(|p| p.first())
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let y = pos
            .and_then(|p| p.get(1))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let trig = c
            .get("trigger")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = c
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        (x, y, trig, name)
    });
    out.insert("coordEvents".into(), Value::Array(coords));

    Value::Object(out)
}

fn npc_sort_key(v: &Value) -> u64 {
    v.get("id").and_then(|x| x.as_u64()).unwrap_or(0)
}

fn normalize_npc(v: &Value) -> Value {
    let obj = match v.as_object() {
        Some(o) => o,
        None => return v.clone(),
    };
    let mut out = Map::new();
    out.insert("id".into(), obj.get("id").cloned().unwrap_or(Value::Null));
    out.insert(
        "talk".into(),
        obj.get("talk").cloned().unwrap_or(Value::Null),
    );
    out.insert(
        "toggleId".into(),
        obj.get("toggleId").cloned().unwrap_or(Value::Null),
    );
    out.insert(
        "scriptId".into(),
        obj.get("scriptId").cloned().unwrap_or(Value::Null),
    );
    out.insert(
        "defaultHidden".into(),
        json!(obj
            .get("defaultHidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)),
    );
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_config_from_trigger_bindings() {
        let src = r#"
game_scene StartTown {
  @load {
    @run { game.setFlag("X"); }
  }
  @storyline("talkProf") {
    @trigger(map = "StartTown", npc = 1, toggle = "START_TOWN_OBJ_1", script = "STARTTOWN_PROF", hidden = true)
    @speaker("") { "Hi" }
  }
  @storyline("talkGirl") {
    @trigger(map = "StartTown", npc = 2)
    @speaker("") { "Hello" }
  }
  @storyline("signProfLab") {
    @trigger(map = "StartTown", sign = 1)
    @speaker("") { "LAB" }
  }
  @storyline("coordNorthExit") {
    @trigger(map = "StartTown", coords = [[10, 1], [11, 1]], name = "northExit1")
    @speaker("") { "wait" }
  }
}
"#;
        let json_text = compile_scene_to_config(src, "StartTown/script.scene").unwrap();
        let got: Value = serde_json::from_str(&json_text).unwrap();

        let expected = json!({
            "$schema": "../../schemas/script_config.schema.json",
            "onLoad": "StartTownOnLoad",
            "npcs": [
                {"id": 1, "talk": "talkProf", "toggleId": "START_TOWN_OBJ_1", "scriptId": "STARTTOWN_PROF", "defaultHidden": true},
                {"id": 2, "talk": "talkGirl"}
            ],
            "signs": [ {"id": 1, "talk": "signProfLab"} ],
            "coordEvents": [
                {"name": "northExit1", "position": [10, 1], "trigger": "coordNorthExit"},
                {"name": "northExit2", "position": [11, 1], "trigger": "coordNorthExit"}
            ]
        });

        assert_eq!(
            normalize_config(&got),
            normalize_config(&expected),
            "generated config:\n{}",
            json_text
        );
    }

    #[test]
    fn object_only_binding_has_no_talk() {
        let src = r#"
game_scene SilphCo7F {
  @storyline("obj12") {
    @trigger(map = "SilphCo7F", npc = 12, toggle = "SILPH_CO_7F_OBJ_12", no_talk = true)
  }
}
"#;
        let json_text = compile_scene_to_config(src, "SilphCo7F/script.scene").unwrap();
        let got: Value = serde_json::from_str(&json_text).unwrap();
        let npc = &got["npcs"][0];
        assert_eq!(npc["id"], json!(12));
        assert_eq!(npc["toggleId"], json!("SILPH_CO_7F_OBJ_12"));
        assert!(
            npc.get("talk").is_none(),
            "object-only npc must have no talk fn"
        );
    }
}
