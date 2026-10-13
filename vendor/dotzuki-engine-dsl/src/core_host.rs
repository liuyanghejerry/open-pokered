//! Canonical native-AST adapter for dotzuki's generic asynchronous `game.*`
//! protocol.
//!
//! Games own synchronous state queries and game-specific extensions through
//! [`crate::interpreter::ScriptHost`]. They should call
//! [`dispatch_core_async`] first, then handle only their own verbs.

use crate::ast::{Expression, GameScene, SourceSpan, StoryStmt};
use crate::interpreter::Value;
use dotzuki_engine_script::ScriptCommand;

/// Every asynchronous `game.*` verb owned by the generic dotzuki protocol.
///
/// This catalog is also suitable for build-time capability validation by DSL
/// consumers. Stateful synchronous verbs (`getFlag`, `setFlag`, `lang`, ...)
/// intentionally are not listed: those are supplied by each host.
pub use dotzuki_engine_script::command::CORE_ASYNC_FUNCTIONS;

/// Generic verbs whose implementation needs mutable/read-only host state and
/// therefore cannot be handled by [`dispatch_core_async`].
pub const CORE_STATEFUL_FUNCTIONS: &[&str] = &[
    "lang",
    "t",
    "showRandomText",
    "getFlag",
    "setFlag",
    "resetFlag",
    "getPlayerPosition",
    "getPlayerX",
    "getPlayerY",
];

pub fn is_core_function(name: &str) -> bool {
    CORE_ASYNC_FUNCTIONS.contains(&name) || CORE_STATEFUL_FUNCTIONS.contains(&name)
}

/// A DSL call for which neither dotzuki nor the game declared a capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownHostFunction {
    pub name: String,
    pub span: SourceSpan,
}

/// Find undeclared `game.*` calls before a scene is serialized or code-generated.
///
/// `is_extension` describes the consuming game's APIs. Raw `@run` JavaScript is
/// intentionally opaque and must be governed separately by consumers that allow
/// it; every structured DSL command and nested expression is checked here.
pub fn unknown_host_functions(
    scene: &GameScene,
    is_extension: impl Fn(&str) -> bool,
) -> Vec<UnknownHostFunction> {
    let mut unknown = Vec::new();
    for storyline in &scene.storylines {
        collect_unknown(&storyline.statements, &is_extension, &mut unknown);
    }
    if let Some(on_load) = &scene.on_load {
        collect_unknown(&on_load.statements, &is_extension, &mut unknown);
    }
    unknown
}

fn collect_unknown(
    statements: &[StoryStmt],
    is_extension: &impl Fn(&str) -> bool,
    unknown: &mut Vec<UnknownHostFunction>,
) {
    for statement in statements {
        match statement {
            StoryStmt::Speaker { name, span, .. } | StoryStmt::Say { name, span, .. } => {
                collect_expression(name, span, is_extension, unknown);
                continue;
            }
            StoryStmt::Choice { options, .. } => {
                for option in options {
                    collect_unknown(&option.body, is_extension, unknown);
                }
                continue;
            }
            StoryStmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                collect_expression(condition, span, is_extension, unknown);
                collect_unknown(then_branch, is_extension, unknown);
                collect_unknown(else_branch, is_extension, unknown);
                continue;
            }
            StoryStmt::Each {
                source, body, span, ..
            } => {
                collect_expression(source, span, is_extension, unknown);
                collect_unknown(body, is_extension, unknown);
                continue;
            }
            StoryStmt::Assign { value, span, .. } => {
                collect_expression(value, span, is_extension, unknown);
                continue;
            }
            StoryStmt::Command { name, args, span } => {
                if !is_core_function(name) && !is_extension(name) {
                    unknown.push(UnknownHostFunction {
                        name: name.clone(),
                        span: span.clone(),
                    });
                }
                for argument in args {
                    collect_expression(argument, span, is_extension, unknown);
                }
                continue;
            }
            StoryStmt::Run { .. } | StoryStmt::Return { .. } => continue,
        };
    }
}

fn collect_expression(
    expression: &Expression,
    span: &SourceSpan,
    is_extension: &impl Fn(&str) -> bool,
    unknown: &mut Vec<UnknownHostFunction>,
) {
    match expression {
        Expression::Call { callee, args } => {
            if !is_core_function(callee) && !is_extension(callee) {
                unknown.push(UnknownHostFunction {
                    name: callee.clone(),
                    span: span.clone(),
                });
            }
            for argument in args {
                collect_expression(argument, span, is_extension, unknown);
            }
        }
        Expression::ArrayLit(values) => {
            for value in values {
                collect_expression(value, span, is_extension, unknown);
            }
        }
        Expression::ObjectLit(fields) => {
            for (_, value) in fields {
                collect_expression(value, span, is_extension, unknown);
            }
        }
        Expression::UnaryOp { operand, .. } => {
            collect_expression(operand, span, is_extension, unknown);
        }
        Expression::BinaryOp { left, right, .. } => {
            collect_expression(left, span, is_extension, unknown);
            collect_expression(right, span, is_extension, unknown);
        }
        Expression::TernaryOp {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expression(condition, span, is_extension, unknown);
            collect_expression(then_expr, span, is_extension, unknown);
            collect_expression(else_expr, span, is_extension, unknown);
        }
        Expression::StringLit(_)
        | Expression::Localized(_)
        | Expression::NumberLit(_)
        | Expression::BoolLit(_)
        | Expression::Variable(_) => {}
    }
}

/// Parse a generic asynchronous `game.*` call into the engine protocol.
///
/// `Ok(None)` means that `name` is not a generic dotzuki verb and should be
/// offered to the game-specific host. A recognized verb with invalid arguments
/// returns a descriptive error instead of falling through to an extension with
/// the same spelling.
pub fn dispatch_core_async(name: &str, args: &[Value]) -> Result<Option<ScriptCommand>, String> {
    let command = match name {
        "showText" => ScriptCommand::ShowText {
            text: text(required(args, 0, name, "text")?, name)?,
        },
        "showChoice" => ScriptCommand::ShowChoice {
            options: string_array(required(args, 0, name, "options")?, name)?,
        },
        "moveNpc" => ScriptCommand::MoveNpc {
            npc_id: text(required(args, 0, name, "npc")?, name)?,
            path: path(required(args, 1, name, "path")?, name)?,
        },
        "startNpcMove" => ScriptCommand::StartNpcMove {
            npc_id: text(required(args, 0, name, "npc")?, name)?,
            path: path(required(args, 1, name, "path")?, name)?,
        },
        "awaitNpcMove" => ScriptCommand::AwaitNpcMove {
            npc_id: text(required(args, 0, name, "npc")?, name)?,
        },
        "movePlayer" => ScriptCommand::MovePlayer {
            path: path(required(args, 0, name, "path")?, name)?,
        },
        "movePlayerRelative" => ScriptCommand::MovePlayerRelative {
            steps: relative_steps(required(args, 0, name, "steps")?, name)?,
        },
        "moveNpcTo" => ScriptCommand::MoveNpcTo {
            npc_id: text(required(args, 0, name, "npc")?, name)?,
            x: u8_arg(required(args, 1, name, "x")?, name)?,
            y: u8_arg(required(args, 2, name, "y")?, name)?,
        },
        "startNpcMoveTo" => ScriptCommand::StartNpcMoveTo {
            npc_id: text(required(args, 0, name, "npc")?, name)?,
            x: u8_arg(required(args, 1, name, "x")?, name)?,
            y: u8_arg(required(args, 2, name, "y")?, name)?,
        },
        "movePlayerTo" => ScriptCommand::MovePlayerTo {
            x: u8_arg(required(args, 0, name, "x")?, name)?,
            y: u8_arg(required(args, 1, name, "y")?, name)?,
        },
        "faceNpc" => ScriptCommand::FaceNpc {
            npc_id: text(required(args, 0, name, "npc")?, name)?,
            direction: text(required(args, 1, name, "direction")?, name)?,
        },
        "facePlayer" => ScriptCommand::FacePlayer {
            direction: text(required(args, 0, name, "direction")?, name)?,
        },
        "setNpcFrame" => ScriptCommand::SetNpcFrame {
            npc_id: text(required(args, 0, name, "npc")?, name)?,
            frame: u8_arg(required(args, 1, name, "frame")?, name)?,
        },
        "playMusic" => ScriptCommand::PlayMusic {
            music_id: text(required(args, 0, name, "music")?, name)?,
        },
        "playSound" => ScriptCommand::PlaySound {
            sound_id: text(required(args, 0, name, "sound")?, name)?,
        },
        "stopMusic" => ScriptCommand::StopMusic,
        "fadeOutMusic" => ScriptCommand::FadeOutMusic,
        "delay" => ScriptCommand::Delay {
            frames: u16_arg(required(args, 0, name, "frames")?, name)?,
        },
        "warpTo" => ScriptCommand::WarpTo {
            map: text(required(args, 0, name, "map")?, name)?,
            x: u8_arg(required(args, 1, name, "x")?, name)?,
            y: u8_arg(required(args, 2, name, "y")?, name)?,
        },
        "heal" => ScriptCommand::Heal,
        "fadeScreen" => ScriptCommand::FadeScreen {
            fade_type: text(required(args, 0, name, "type")?, name)?,
        },
        "showObject" => object_toggle(required(args, 0, name, "object")?, true)?,
        "hideObject" => object_toggle(required(args, 0, name, "object")?, false)?,
        "showObjectByName" => ScriptCommand::ShowObjectByName {
            toggle_id: text(required(args, 0, name, "id")?, name)?,
        },
        "hideObjectByName" => ScriptCommand::HideObjectByName {
            toggle_id: text(required(args, 0, name, "id")?, name)?,
        },
        "setJoyIgnore" => ScriptCommand::SetJoyIgnore {
            mask: u8_arg(required(args, 0, name, "mask")?, name)?,
        },
        "clearJoyIgnore" => ScriptCommand::ClearJoyIgnore,
        "followNpc" => ScriptCommand::FollowNpc {
            npc_id: text(required(args, 0, name, "npc")?, name)?,
            target_x: u8_arg(required(args, 1, name, "x")?, name)?,
            target_y: u8_arg(required(args, 2, name, "y")?, name)?,
        },
        "openShop" => ScriptCommand::OpenShop {
            items: string_array(required(args, 0, name, "items")?, name)?,
        },
        "showEmotionBubble" => ScriptCommand::ShowEmotionBubble {
            npc_id: text(required(args, 0, name, "npc")?, name)?,
            emotion: text(required(args, 1, name, "emotion")?, name)?,
        },
        "setNpcPosition" => ScriptCommand::SetNpcPosition {
            npc_id: text(required(args, 0, name, "npc")?, name)?,
            x: u8_arg(required(args, 1, name, "x")?, name)?,
            y: u8_arg(required(args, 2, name, "y")?, name)?,
        },
        "showScene" => ScriptCommand::ShowScene {
            scene_name: text(required(args, 0, name, "name")?, name)?,
            layout_json: None,
        },
        "hideScene" => ScriptCommand::HideScene {
            scene_name: text(required(args, 0, name, "name")?, name)?,
        },
        "updateUI" => ScriptCommand::UpdateUI {
            scene_name: text(required(args, 0, name, "name")?, name)?,
            data_json: json(required(args, 1, name, "data")?),
        },
        _ => return Ok(None),
    };
    Ok(Some(command))
}

fn required<'a>(
    args: &'a [Value],
    index: usize,
    name: &str,
    field: &str,
) -> Result<&'a Value, String> {
    args.get(index)
        .ok_or_else(|| format!("{name}: missing {field}"))
}

fn text(value: &Value, what: &str) -> Result<String, String> {
    match value {
        Value::Text(value) => Ok(value.clone()),
        Value::Number(value) => Ok(format!("{value}")),
        Value::Bool(value) => Ok(if *value { "true" } else { "false" }.to_string()),
        other => Err(format!(
            "{what}: expected string, got {}",
            other.type_name()
        )),
    }
}

fn number(value: &Value, what: &str) -> Result<f64, String> {
    match value {
        Value::Number(value) => Ok(*value),
        other => Err(format!(
            "{what}: expected number, got {}",
            other.type_name()
        )),
    }
}

fn integer_arg(value: &Value, what: &str, min: f64, max: f64) -> Result<f64, String> {
    let value = number(value, what)?;
    if !value.is_finite() || (value as i32) as f64 != value || value < min || value > max {
        return Err(format!(
            "{what}: expected an integer in {min:.0}..={max:.0}, got {value}"
        ));
    }
    Ok(value)
}

fn u8_arg(value: &Value, what: &str) -> Result<u8, String> {
    integer_arg(value, what, u8::MIN as f64, u8::MAX as f64).map(|value| value as u8)
}

fn u16_arg(value: &Value, what: &str) -> Result<u16, String> {
    integer_arg(value, what, u16::MIN as f64, u16::MAX as f64).map(|value| value as u16)
}

fn i16_arg(value: &Value, what: &str) -> Result<i16, String> {
    integer_arg(value, what, i16::MIN as f64, i16::MAX as f64).map(|value| value as i16)
}

fn string_array(value: &Value, what: &str) -> Result<Vec<String>, String> {
    match value {
        Value::Array(values) => values
            .iter()
            .map(|value| text(value, &format!("{what} element")))
            .collect(),
        other => Err(format!("{what}: expected array, got {}", other.type_name())),
    }
}

fn path(value: &Value, what: &str) -> Result<Vec<(u8, u8)>, String> {
    match value {
        Value::Array(points) => points
            .iter()
            .enumerate()
            .map(|(index, point)| match point {
                Value::Array(pair) if pair.len() == 2 => Ok((
                    u8_arg(&pair[0], &format!("{what}[{index}].x"))?,
                    u8_arg(&pair[1], &format!("{what}[{index}].y"))?,
                )),
                other => Err(format!(
                    "{what}[{index}]: expected [x, y] pair, got {}",
                    other.type_name()
                )),
            })
            .collect(),
        other => Err(format!("{what}: expected array, got {}", other.type_name())),
    }
}

fn relative_steps(value: &Value, what: &str) -> Result<Vec<(i16, i16)>, String> {
    match value {
        Value::Array(steps) => steps
            .iter()
            .enumerate()
            .map(|(index, step)| match step {
                Value::Text(direction) => match direction.to_ascii_lowercase().as_str() {
                    "up" | "north" => Ok((0, -1)),
                    "down" | "south" => Ok((0, 1)),
                    "left" | "west" => Ok((-1, 0)),
                    "right" | "east" => Ok((1, 0)),
                    other => Err(format!("{what}[{index}]: unknown direction '{other}'")),
                },
                Value::Array(pair) if pair.len() == 2 => Ok((
                    i16_arg(&pair[0], &format!("{what}[{index}].dx"))?,
                    i16_arg(&pair[1], &format!("{what}[{index}].dy"))?,
                )),
                other => Err(format!(
                    "{what}[{index}]: expected direction string or [dx, dy], got {}",
                    other.type_name()
                )),
            })
            .collect(),
        other => Err(format!("{what}: expected array, got {}", other.type_name())),
    }
}

fn object_toggle(value: &Value, show: bool) -> Result<ScriptCommand, String> {
    match value {
        Value::Text(toggle_id) if show => Ok(ScriptCommand::ShowObjectByName {
            toggle_id: toggle_id.clone(),
        }),
        Value::Text(toggle_id) => Ok(ScriptCommand::HideObjectByName {
            toggle_id: toggle_id.clone(),
        }),
        Value::Number(_) if show => Ok(ScriptCommand::ShowObject {
            object_index: u8_arg(value, "showObject")?,
        }),
        Value::Number(_) => Ok(ScriptCommand::HideObject {
            object_index: u8_arg(value, "hideObject")?,
        }),
        other => Err(format!(
            "{}: expected number or string, got {}",
            if show { "showObject" } else { "hideObject" },
            other.type_name()
        )),
    }
}

fn json(value: &Value) -> String {
    match value {
        Value::Undefined => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Text(value) => serde_json::to_string(value).unwrap_or_else(|_| "null".to_string()),
        Value::Array(values) => {
            let mut output = "[".to_string();
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                output.push_str(&json(value));
            }
            output.push(']');
            output
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Storyline;

    fn text_value(value: &str) -> Value {
        Value::Text(value.to_string())
    }

    #[test]
    fn dispatches_core_commands_and_leaves_extensions_to_the_game() {
        assert_eq!(
            dispatch_core_async("movePlayerTo", &[Value::Number(3.0), Value::Number(4.0)]),
            Ok(Some(ScriptCommand::MovePlayerTo { x: 3, y: 4 }))
        );
        assert_eq!(dispatch_core_async("startBattle", &[]), Ok(None));
    }

    #[test]
    fn recognized_command_errors_do_not_fall_through() {
        let error = dispatch_core_async("warpTo", &[text_value("PALLET_TOWN")]).unwrap_err();
        assert_eq!(error, "warpTo: missing x");
    }

    #[test]
    fn integer_arguments_reject_fractional_non_finite_and_out_of_range_values() {
        for value in [-1.0, 1.5, 256.0, f64::NAN, f64::INFINITY] {
            let error =
                dispatch_core_async("movePlayerTo", &[Value::Number(value), Value::Number(4.0)])
                    .unwrap_err();
            assert!(error.contains("expected an integer in 0..=255"), "{error}");
        }

        let relative = Value::Array(vec![Value::Array(vec![
            Value::Number(32_768.0),
            Value::Number(0.0),
        ])]);
        let error = dispatch_core_async("movePlayerRelative", &[relative]).unwrap_err();
        assert!(
            error.contains("expected an integer in -32768..=32767"),
            "{error}"
        );
    }

    #[test]
    fn parses_paths_relative_steps_and_object_names() {
        let path = Value::Array(vec![Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
        ])]);
        assert_eq!(
            dispatch_core_async("movePlayer", &[path]),
            Ok(Some(ScriptCommand::MovePlayer { path: vec![(1, 2)] }))
        );
        let steps = Value::Array(vec![
            text_value("north"),
            Value::Array(vec![Value::Number(2.0), Value::Number(0.0)]),
        ]);
        assert_eq!(
            dispatch_core_async("movePlayerRelative", &[steps]),
            Ok(Some(ScriptCommand::MovePlayerRelative {
                steps: vec![(0, -1), (2, 0)]
            }))
        );
        assert_eq!(
            dispatch_core_async("showObject", &[text_value("door")]),
            Ok(Some(ScriptCommand::ShowObjectByName {
                toggle_id: "door".to_string()
            }))
        );
    }

    #[test]
    fn catalog_and_dispatch_stay_in_sync() {
        for name in CORE_ASYNC_FUNCTIONS {
            assert_ne!(
                dispatch_core_async(name, &[]),
                Ok(None),
                "missing dispatcher arm for {name}"
            );
        }
    }

    #[test]
    fn validates_structured_calls_with_game_extensions() {
        let span = SourceSpan::point("test.scene", 7, 3);
        let scene = GameScene {
            name: "test".to_string(),
            variables: None,
            storylines: vec![Storyline {
                name: "main".to_string(),
                triggers: vec![],
                statements: vec![
                    StoryStmt::Command {
                        name: "showText".to_string(),
                        args: vec![Expression::StringLit("hello".to_string())],
                        span: span.clone(),
                    },
                    StoryStmt::Assign {
                        name: "result".to_string(),
                        value: Expression::Call {
                            callee: "gameExtension".to_string(),
                            args: vec![],
                        },
                        span: span.clone(),
                    },
                    StoryStmt::Command {
                        name: "typoedFunction".to_string(),
                        args: vec![],
                        span: span.clone(),
                    },
                ],
                span: span.clone(),
            }],
            on_load: None,
            ui: None,
            themes: vec![],
            styles: vec![],
            atlases: vec![],
            span: span.clone(),
        };

        assert_eq!(
            unknown_host_functions(&scene, |name| name == "gameExtension"),
            vec![UnknownHostFunction {
                name: "typoedFunction".to_string(),
                span,
            }]
        );
    }
}
