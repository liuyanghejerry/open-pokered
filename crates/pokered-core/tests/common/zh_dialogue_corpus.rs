//! Read current embedded scene ASTs, including every branch and showText expression.
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
pub struct DialogueCase {
    pub source: String,
    pub text: String,
}

fn localized(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.into());
    }
    for key in ["Plain", "StringLit"] {
        if let Some(s) = value.get(key).and_then(Value::as_str) {
            return Some(s.into());
        }
    }
    value
        .get("Localized")
        .and_then(Value::as_array)
        .and_then(|pairs| {
            pairs
                .iter()
                .find(|p| p[0] == "zh")
                .and_then(|p| p[1].as_str())
                .map(str::to_owned)
        })
}

fn expression(value: &Value) -> String {
    if let Some(text) = localized(value) {
        return text;
    }
    if let Some(binary) = value.get("BinaryOp") {
        if binary["op"] == "Add" {
            return expression(&binary["left"]) + &expression(&binary["right"]);
        }
    }
    if let Some(call) = value.get("Call") {
        return match call["callee"].as_str().unwrap_or("") {
            "getPlayerName" => "<PLAYER>",
            "getRivalName" => "<RIVAL>",
            "getDaycareMonName" | "getPartyMonName" | "getPokemonName" => "妙蛙种子",
            "getDaycareCost" | "getMoney" => "999999",
            "getDaycareLevelsGrown" => "99",
            _ => "7",
        }
        .into();
    }
    if let Some(n) = value.get("NumberLit") {
        return n.to_string();
    }
    // Runtime values in report expressions (counts, prices, nicknames) get
    // representative values; tests additionally expand both name placeholders.
    "7".into()
}

fn walk(value: &Value, map: &str, out: &mut Vec<DialogueCase>) {
    if let Some(object) = value.as_object() {
        if let Some(body) = object.get("Run") {
            let mut rest = body["js"].as_str().unwrap_or("");
            let mut index = 0;
            while let Some(at) = rest.find("game.t(") {
                rest = &rest[at + "game.t(".len()..];
                let mut first = serde_json::Deserializer::from_str(rest).into_iter::<String>();
                if !matches!(first.next(), Some(Ok(_))) {
                    break;
                }
                rest = &rest[first.byte_offset()..];
                rest = rest
                    .trim_start()
                    .strip_prefix(',')
                    .unwrap_or(rest)
                    .trim_start();
                let mut second = serde_json::Deserializer::from_str(rest).into_iter::<String>();
                let Some(Ok(text)) = second.next() else {
                    break;
                };
                rest = &rest[second.byte_offset()..];
                out.push(DialogueCase {
                    source: format!("{map}:{}:Run[{index}]", body["span"]["line_start"]),
                    text,
                });
                index += 1;
            }
        }
        for kind in ["Speaker", "Say"] {
            if let Some(body) = object.get(kind) {
                let mut text = body["texts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(localized)
                    .collect::<Vec<_>>()
                    .join("\n");
                let speaker = expression(&body["name"]);
                if !speaker.is_empty() {
                    text = format!("{speaker}: {text}");
                }
                out.push(DialogueCase {
                    source: format!("{map}:{}:{kind}", body["span"]["line_start"]),
                    text,
                });
                return;
            }
        }
        if let Some(body) = object.get("Command") {
            if body["name"] == "showText" {
                if let Some(arg) = body["args"].as_array().and_then(|a| a.first()) {
                    out.push(DialogueCase {
                        source: format!("{map}:{}:showText", body["span"]["line_start"]),
                        text: expression(arg),
                    });
                }
            }
            if body["name"] == "showRandomText" {
                for (index, arg) in body["args"].as_array().unwrap().iter().enumerate() {
                    out.push(DialogueCase {
                        source: format!(
                            "{map}:{}:showRandomText[{index}]",
                            body["span"]["line_start"]
                        ),
                        text: expression(arg),
                    });
                }
            }
        }
        for child in object.values() {
            walk(child, map, out);
        }
    } else if let Some(array) = value.as_array() {
        for child in array {
            walk(child, map, out);
        }
    }
}

pub fn corpus() -> Vec<DialogueCase> {
    let mut out = Vec::new();
    for (map, bytes) in pokered_data::embedded_scenes::scene_asts() {
        let scene: Value = serde_json::from_slice(bytes).unwrap();
        walk(&scene, map, &mut out);
    }
    for (index, (_, zh)) in pokered_data::dialog_text::static_translations()
        .iter()
        .enumerate()
    {
        out.push(DialogueCase {
            source: format!("dialog_text::EXACT[{index}]"),
            text: (*zh).into(),
        });
    }
    out
}
