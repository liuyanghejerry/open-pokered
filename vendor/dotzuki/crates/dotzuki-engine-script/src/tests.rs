use crate::command::{CommandResult, ScriptCommand};
use crate::cutscene::CutsceneManager;
use crate::engine::ScriptEngine;
use crate::ScriptApiRegistrar;
use boa_engine::js_string;
use boa_engine::{Context, JsArgs, JsResult, JsValue};

struct TestMonsterApi;
impl ScriptApiRegistrar for TestMonsterApi {
    fn register_api(&self, engine: &mut ScriptEngine) {
        engine.register_async_fn(
            "giveItem",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let item_id = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                let quantity = args.get_or_undefined(1).to_u32(ctx)? as u8;
                Ok(ScriptCommand::GiveItem { item_id, quantity })
            },
        );
        engine.register_async_fn(
            "takeItem",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let item_id = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                let quantity = args.get_or_undefined(1).to_u32(ctx)? as u8;
                Ok(ScriptCommand::TakeItem { item_id, quantity })
            },
        );
        engine.register_async_fn(
            "giveMonster",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let species = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                let level = args.get_or_undefined(1).to_u32(ctx)? as u8;
                Ok(ScriptCommand::GiveMonster { species, level })
            },
        );
        engine.register_async_fn(
            "startBattle",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let trainer_id = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                Ok(ScriptCommand::StartBattle { trainer_id })
            },
        );
        engine.register_async_fn(
            "showDexEntry",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let species = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                Ok(ScriptCommand::Custom {
                    name: "showDexEntry".into(),
                    args: vec![serde_json::json!(species)],
                })
            },
        );
    }
}

fn new_engine_with_monster() -> ScriptEngine {
    ScriptEngine::with_api(&TestMonsterApi)
}

#[test]
fn test_engine_creation() {
    let engine = ScriptEngine::new();
    assert!(engine.is_idle());
}

#[test]
fn test_load_and_call_simple_script() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function onEnter() {
            await game.showText("Hello world!");
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("onEnter", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Hello world!".to_string()
        })
    );
    assert!(engine.is_waiting());
}

#[test]
fn test_signal_done_continues_script() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function onEnter() {
            await game.showText("Line 1");
            await game.showText("Line 2");
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("onEnter", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Line 1".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Line 2".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

#[test]
fn test_show_choice_returns_number() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        var chosen = -1;
        export async function onEnter() {
            chosen = await game.showChoice(["Yes", "No"]);
            await game.showText("You picked: " + chosen);
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("onEnter", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowChoice {
            options: vec!["Yes".to_string(), "No".to_string()]
        })
    );

    let cmd = engine.signal_done(CommandResult::Number(1.0)).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "You picked: 1".to_string()
        })
    );
}

#[test]
fn test_flag_operations() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        var result = false;
        export async function checkFlags() {
            result = game.getFlag("TEST_FLAG");
            game.setFlag("TEST_FLAG");
            result = game.getFlag("TEST_FLAG");
            if (result) {
                await game.showText("Flag is set!");
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("checkFlags", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Flag is set!".to_string()
        })
    );
}

#[test]
fn test_conditional_branching() {
    let mut engine = ScriptEngine::new();
    engine.set_flag("GOT_STARTER", true);
    engine
        .load_script(
            r#"
        export async function onEnter() {
            if (game.getFlag("GOT_STARTER")) {
                await game.showText("You already have a starter!");
            } else {
                await game.showText("Choose your starter!");
                await game.giveMonster("LEAFKIT", 5);
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("onEnter", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "You already have a starter!".to_string()
        })
    );
}

#[test]
fn test_conditional_branching_else() {
    let mut engine = new_engine_with_monster();
    engine
        .load_script(
            r#"
        export async function onEnter() {
            if (game.getFlag("GOT_STARTER")) {
                await game.showText("You already have a starter!");
            } else {
                await game.showText("Choose your starter!");
                await game.giveMonster("LEAFKIT", 5);
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("onEnter", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Choose your starter!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::GiveMonster {
            species: "LEAFKIT".to_string(),
            level: 5
        })
    );
}

#[test]
fn test_move_npc_command() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function onEnter() {
            await game.moveNpc("prof", [[2, 3], [2, 5], [4, 5]]);
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("onEnter", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::MoveNpc {
            npc_id: "prof".to_string(),
            path: vec![(2, 3), (2, 5), (4, 5)]
        })
    );
}

#[test]
fn test_show_random_text_picks_from_pool() {
    // showRandomText emits a ShowText command whose text is one of the options.
    // seed_rng pins the stream so the pick is deterministic in tests.
    let mut engine = ScriptEngine::new();
    engine.seed_rng(1);
    engine
        .load_script(
            r#"
        export async function onEnter() {
            await game.showRandomText("a", "b", "c");
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("onEnter", &[]).unwrap();
    match cmd {
        Some(ScriptCommand::ShowText { text }) => {
            assert!(
                ["a", "b", "c"].contains(&text.as_str()),
                "unexpected pick: {text}"
            );
        }
        other => panic!("expected ShowText, got {other:?}"),
    }
    // Resolves like showText once the box is dismissed.
    assert_eq!(engine.signal_done(CommandResult::Void).unwrap(), None);
}

#[test]
fn test_show_random_text_accepts_array_and_covers_pool() {
    // Array form is also accepted, and over many draws every option is reachable.
    let mut engine = ScriptEngine::new();
    engine.seed_rng(0xDEAD_BEEF);
    engine
        .load_script(
            r#"
        export async function pick() {
            await game.showRandomText(["x", "y", "z"]);
        }
    "#,
        )
        .unwrap();

    let mut seen = std::collections::HashSet::new();
    for _ in 0..200 {
        engine.mix_rng(0x1234_5678);
        if let Some(ScriptCommand::ShowText { text }) = engine.call_function("pick", &[]).unwrap() {
            seen.insert(text);
        }
        engine.signal_done(CommandResult::Void).unwrap();
    }
    assert_eq!(
        seen.len(),
        3,
        "all three options should be reachable, saw {seen:?}"
    );
}

#[test]
fn test_auto_path_commands() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function onEnter() {
            await game.startNpcMoveTo("prof", 12, 11);
            await game.movePlayerTo(12, 11);
            await game.moveNpcTo("prof", 8, 8);
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("onEnter", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::StartNpcMoveTo {
            npc_id: "prof".to_string(),
            x: 12,
            y: 11,
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, Some(ScriptCommand::MovePlayerTo { x: 12, y: 11 }));

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::MoveNpcTo {
            npc_id: "prof".to_string(),
            x: 8,
            y: 8,
        })
    );
}

#[test]
fn test_multiple_commands_sequence() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function onEnter() {
            await game.playMusic("START_TOWN");
            await game.showText("Welcome!");
            await game.delay(30);
            await game.heal();
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("onEnter", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::PlayMusic {
            music_id: "START_TOWN".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Welcome!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, Some(ScriptCommand::Delay { frames: 30 }));

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, Some(ScriptCommand::Heal));

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

#[test]
fn test_tick_returns_pending_command() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function onEnter() {
            await game.showText("Tick test");
        }
    "#,
        )
        .unwrap();

    engine.call_function("onEnter", &[]).unwrap();

    let cmd = engine.tick();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Tick test".to_string()
        })
    );
    assert!(engine.is_waiting());
}

#[test]
fn test_function_not_found() {
    let mut engine = ScriptEngine::new();
    engine.load_script("export function foo() {}").unwrap();

    let result = engine.call_function("nonExistent", &[]);
    assert!(result.is_err());
}

#[test]
fn test_warp_command() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function doWarp() {
            await game.warpTo("PROF_LAB", 5, 3);
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("doWarp", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::WarpTo {
            map: "PROF_LAB".to_string(),
            x: 5,
            y: 3
        })
    );
}

#[test]
fn test_battle_with_result() {
    let mut engine = new_engine_with_monster();
    engine
        .load_script(
            r#"
        var battleResult = "";
        export async function onEnter() {
            battleResult = await game.startBattle("RIVAL_1");
            if (battleResult === "won") {
                await game.showText("You won!");
            } else {
                await game.showText("You lost...");
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("onEnter", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::StartBattle {
            trainer_id: "RIVAL_1".to_string()
        })
    );

    let cmd = engine
        .signal_done(CommandResult::Text("won".to_string()))
        .unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "You won!".to_string()
        })
    );
}

#[test]
fn test_reset_flag() {
    let mut engine = ScriptEngine::new();
    engine.set_flag("MY_FLAG", true);
    assert!(engine.get_flag("MY_FLAG"));

    engine
        .load_script(
            r#"
        export async function doReset() {
            game.resetFlag("MY_FLAG");
            if (!game.getFlag("MY_FLAG")) {
                await game.showText("Flag was reset!");
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("doReset", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Flag was reset!".to_string()
        })
    );
    assert!(!engine.get_flag("MY_FLAG"));
}

#[test]
fn test_set_joy_ignore() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function onEnter() {
            await game.setJoyIgnore(0xFF);
            await game.showText("Input disabled");
            await game.clearJoyIgnore();
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("onEnter", &[]).unwrap();
    assert_eq!(cmd, Some(ScriptCommand::SetJoyIgnore { mask: 255 }));

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Input disabled".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, Some(ScriptCommand::ClearJoyIgnore));

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

#[test]
fn test_open_shop_command() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function onEnter() {
            await game.openShop(["POTION", "ANTIDOTE"]);
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("onEnter", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::OpenShop {
            items: vec!["POTION".to_string(), "ANTIDOTE".to_string()]
        })
    );
    assert!(engine.is_waiting());

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

// ── Cutscene Integration Tests ─────────────────────────────────────────

#[test]
fn test_cutscene_simple_dialog() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function profLabCutscene() {
            await game.showText("Prof: Welcome to my lab!");
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("profLabCutscene", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Prof: Welcome to my lab!".to_string()
        })
    );
    assert!(engine.is_waiting());

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

#[test]
fn test_cutscene_with_multiple_steps() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function labIntro() {
            await game.fadeScreen("out");
            await game.showText("Prof: Hello!");
            await game.delay(30);
            await game.fadeScreen("in");
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("labIntro", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::FadeScreen {
            fade_type: "out".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Prof: Hello!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, Some(ScriptCommand::Delay { frames: 30 }));

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::FadeScreen {
            fade_type: "in".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

#[test]
fn test_cutscene_with_player_movement() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function walkToLab() {
            await game.movePlayer([[4, 5], [4, 6], [4, 7]]);
            await game.facePlayer("up");
            await game.showText("I'm at the lab door.");
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("walkToLab", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::MovePlayer {
            path: vec![(4, 5), (4, 6), (4, 7)]
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::FacePlayer {
            direction: "up".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "I'm at the lab door.".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

#[test]
fn test_move_player_relative_deltas_and_direction_strings() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function pushBack() {
            await game.movePlayerRelative([[0, -1], [0, -1], "down", "left", "right", "up"]);
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("pushBack", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::MovePlayerRelative {
            steps: vec![(0, -1), (0, -1), (0, 1), (-1, 0), (1, 0), (0, -1)]
        })
    );
}

#[test]
fn test_move_player_relative_rejects_unknown_direction() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function bad() {
            await game.movePlayerRelative(["sideways"]);
        }
    "#,
        )
        .unwrap();

    // The thrown TypeError rejects the async function; the engine logs
    // it and yields no command (rather than a garbage movement).
    let cmd = engine.call_function("bad", &[]).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

#[test]
fn test_cutscene_with_sound_and_music() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function encounterCutscene() {
            await game.playSound("SFX_STOP_ALL_MUSIC");
            await game.playMusic("MUSIC_MEET_PROF");
            await game.showText("A wild PROF appeared!");
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("encounterCutscene", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::PlaySound {
            sound_id: "SFX_STOP_ALL_MUSIC".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::PlayMusic {
            music_id: "MUSIC_MEET_PROF".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "A wild PROF appeared!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

#[test]
fn test_cutscene_with_warp() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function warpInside() {
            await game.fadeScreen("out");
            await game.warpTo("ProfLab", 4, 3);
            await game.fadeScreen("in");
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("warpInside", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::FadeScreen {
            fade_type: "out".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::WarpTo {
            map: "ProfLab".to_string(),
            x: 4,
            y: 3
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::FadeScreen {
            fade_type: "in".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

#[test]
fn test_cutscene_with_emotion_bubble() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function surpriseCutscene() {
            await game.showEmotionBubble("rival", "exclamation");
            await game.delay(30);
            await game.showText("<RIVAL>: Wait!");
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("surpriseCutscene", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowEmotionBubble {
            npc_id: "rival".to_string(),
            emotion: "exclamation".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, Some(ScriptCommand::Delay { frames: 30 }));

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "<RIVAL>: Wait!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

#[test]
fn test_cutscene_manager_start_to_script_execution() {
    let mut cm = CutsceneManager::new();
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function introCutscene() {
            await game.showText("Hello!");
            await game.delay(60);
            await game.showText("Goodbye!");
        }
    "#,
        )
        .unwrap();

    cm.start_cutscene("introCutscene", true);
    assert!(cm.is_active());
    assert!(cm.is_blocking());
    assert!(cm.needs_start());

    let script_name = cm.current_script_name().unwrap().to_string();
    let cmd = engine.call_function(&script_name, &[]).unwrap();
    cm.mark_started();

    assert!(!cm.needs_start());
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Hello!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, Some(ScriptCommand::Delay { frames: 60 }));

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Goodbye!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());

    let next = cm.end_cutscene();
    assert_eq!(next, None);
    assert!(!cm.is_active());
}

#[test]
fn test_cutscene_queue_execution_order() {
    let mut cm = CutsceneManager::new();
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function first() {
            await game.showText("First");
        }
        export async function second() {
            await game.showText("Second");
        }
        export async function third() {
            await game.showText("Third");
        }
    "#,
        )
        .unwrap();

    cm.start_cutscene("first", true);
    cm.queue_script("second");
    cm.queue_script("third");

    assert_eq!(cm.current_script_name(), Some("first"));
    assert_eq!(cm.queue_len(), 2);

    let script_name = cm.current_script_name().unwrap().to_string();
    let cmd = engine.call_function(&script_name, &[]).unwrap();
    cm.mark_started();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "First".to_string()
        })
    );
    engine.signal_done(CommandResult::Void).unwrap();

    let next = cm.end_cutscene();
    assert_eq!(next, Some("second".to_string()));
    assert!(cm.needs_start());

    let script_name = cm.current_script_name().unwrap().to_string();
    let cmd = engine.call_function(&script_name, &[]).unwrap();
    cm.mark_started();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Second".to_string()
        })
    );
    engine.signal_done(CommandResult::Void).unwrap();

    let next = cm.end_cutscene();
    assert_eq!(next, Some("third".to_string()));

    let script_name = cm.current_script_name().unwrap().to_string();
    let cmd = engine.call_function(&script_name, &[]).unwrap();
    cm.mark_started();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Third".to_string()
        })
    );
    engine.signal_done(CommandResult::Void).unwrap();

    let next = cm.end_cutscene();
    assert_eq!(next, None);
    assert!(!cm.is_active());
}

#[test]
fn test_cutscene_non_blocking_allows_input() {
    let mut cm = CutsceneManager::new();
    cm.start_cutscene("ambient_bgm", false);
    assert!(cm.is_active());
    assert!(!cm.is_blocking());
}

#[test]
fn test_cutscene_force_stop_during_execution() {
    let mut cm = CutsceneManager::new();
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function longCutscene() {
            await game.showText("Step 1");
            await game.delay(30);
            await game.showText("Step 2");
            await game.delay(30);
            await game.showText("Step 3");
        }
    "#,
        )
        .unwrap();

    cm.start_cutscene("longCutscene", true);
    let script_name = cm.current_script_name().unwrap().to_string();
    let cmd = engine.call_function(&script_name, &[]).unwrap();
    cm.mark_started();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Step 1".to_string()
        })
    );

    cm.force_stop();
    assert!(!cm.is_active());
    assert!(cm.queue.is_empty());
    assert!(!cm.started);
}

// ── Prof's Lab Intro Cutscene Tests ─────────────────────────────────────

#[test]
fn test_prof_lab_intro_full_cutscene_flambit() {
    let mut engine = new_engine_with_monster();
    engine
        .load_script(
            r#"
        export async function profLabIntro() {
            await game.fadeScreen("out");
            await game.warpTo("ProfLab", 2, 6);
            await game.fadeScreen("in");
            await game.showText("PROF: Hello there, young trainer!");
            await game.showText("PROF: The world of monsters is vast...");
            await game.showText("PROF: Before you go, choose a partner!");

            const choice = await game.showChoice([
                "FLAMBIT", "AQUAKIT", "LEAFKIT"
            ]);

            if (choice === 0) {
                await game.giveMonster("FLAMBIT", 5);
                await game.showText("PROF: FLAMBIT is a fire type!");
            } else if (choice === 1) {
                await game.giveMonster("AQUAKIT", 5);
                await game.showText("PROF: AQUAKIT is a water type!");
            } else {
                await game.giveMonster("LEAFKIT", 5);
                await game.showText("PROF: LEAFKIT is a grass type!");
            }

            game.setFlag("HIDE_PROF_LAB_STARTER");
            await game.delay(30);
        }
    "#,
        )
        .unwrap();

    let cm = &mut CutsceneManager::new();
    cm.start_cutscene("profLabIntro", true);
    assert!(cm.is_active());
    assert!(cm.needs_start());

    let cmd = engine.call_function("profLabIntro", &[]).unwrap();
    cm.mark_started();
    assert!(!cm.needs_start());

    assert_eq!(
        cmd,
        Some(ScriptCommand::FadeScreen {
            fade_type: "out".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::WarpTo {
            map: "ProfLab".to_string(),
            x: 2,
            y: 6
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::FadeScreen {
            fade_type: "in".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "PROF: Hello there, young trainer!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "PROF: The world of monsters is vast...".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "PROF: Before you go, choose a partner!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowChoice {
            options: vec![
                "FLAMBIT".to_string(),
                "AQUAKIT".to_string(),
                "LEAFKIT".to_string()
            ]
        })
    );

    let cmd = engine.signal_done(CommandResult::Number(0.0)).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::GiveMonster {
            species: "FLAMBIT".to_string(),
            level: 5
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "PROF: FLAMBIT is a fire type!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, Some(ScriptCommand::Delay { frames: 30 }));

    assert!(engine.get_flag("HIDE_PROF_LAB_STARTER"));

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());

    cm.end_cutscene();
    assert!(!cm.is_active());
}

#[test]
fn test_prof_lab_intro_aquakit_path() {
    let mut engine = new_engine_with_monster();
    engine
        .load_script(
            r#"
        export async function profLabIntro() {
            await game.fadeScreen("out");
            await game.showText("PROF: Hello there, young trainer!");
            const choice = await game.showChoice([
                "FLAMBIT", "AQUAKIT", "LEAFKIT"
            ]);
            if (choice === 0) {
                await game.giveMonster("FLAMBIT", 5);
            } else if (choice === 1) {
                await game.giveMonster("AQUAKIT", 5);
            } else {
                await game.giveMonster("LEAFKIT", 5);
            }
            game.setFlag("HIDE_PROF_LAB_STARTER");
            await game.delay(30);
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("profLabIntro", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::FadeScreen {
            fade_type: "out".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "PROF: Hello there, young trainer!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowChoice {
            options: vec![
                "FLAMBIT".to_string(),
                "AQUAKIT".to_string(),
                "LEAFKIT".to_string()
            ]
        })
    );

    let cmd = engine.signal_done(CommandResult::Number(1.0)).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::GiveMonster {
            species: "AQUAKIT".to_string(),
            level: 5
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, Some(ScriptCommand::Delay { frames: 30 }));

    assert!(engine.get_flag("HIDE_PROF_LAB_STARTER"));

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

#[test]
fn test_prof_lab_intro_leafkit_fallback() {
    let mut engine = new_engine_with_monster();
    engine
        .load_script(
            r#"
        export async function profLabIntro() {
            await game.showText("PROF: Hello there, young trainer!");
            const choice = await game.showChoice([
                "FLAMBIT", "AQUAKIT", "LEAFKIT"
            ]);
            if (choice === 0) {
                await game.giveMonster("FLAMBIT", 5);
            } else if (choice === 1) {
                await game.giveMonster("AQUAKIT", 5);
            } else {
                await game.giveMonster("LEAFKIT", 5);
            }
            game.setFlag("HIDE_PROF_LAB_STARTER");
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("profLabIntro", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "PROF: Hello there, young trainer!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowChoice {
            options: vec![
                "FLAMBIT".to_string(),
                "AQUAKIT".to_string(),
                "LEAFKIT".to_string()
            ]
        })
    );

    let cmd = engine.signal_done(CommandResult::Number(2.0)).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::GiveMonster {
            species: "LEAFKIT".to_string(),
            level: 5
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
    assert!(engine.get_flag("HIDE_PROF_LAB_STARTER"));
}

#[test]
fn test_cutscene_via_script_loader_registry() {
    let mut loader = crate::loader::ScriptLoader::new();
    loader.register_script(
        "events/prof_lab_intro",
        r#"
        export async function profLabIntro() {
            await game.showText("PROF: Hello!");
            await game.giveMonster("FLAMBIT", 5);
            game.setFlag("STARTER_CHOSEN");
        }
    "#,
    );

    assert!(loader.has_script("events/prof_lab_intro"));

    let mut engine = new_engine_with_monster();
    let source = loader.get_script("events/prof_lab_intro").unwrap();
    engine.load_script(source).unwrap();

    let cmd = engine.call_function("profLabIntro", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "PROF: Hello!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::GiveMonster {
            species: "FLAMBIT".to_string(),
            level: 5
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
    assert!(engine.get_flag("STARTER_CHOSEN"));
}

// ── BridgeView & Seed Tests ───────────────────────────────────────────────

struct BridgeViewRegistrar;

impl ScriptApiRegistrar for BridgeViewRegistrar {
    fn register_api(&self, engine: &mut ScriptEngine) {
        engine.register_sync_fn("getTestNumber", |args, ctx, view| {
            let k = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(JsValue::from(view.number(&k)))
        });
        engine.register_sync_fn("getTestText", |args, ctx, view| {
            let k = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(JsValue::from(js_string!(view.text(&k))))
        });
        engine.register_sync_fn("testSetContains", |args, ctx, view| {
            let k = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            let v = args
                .get_or_undefined(1)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(JsValue::from(view.set_contains(&k, &v)))
        });
        engine.register_sync_fn("getTestFlag", |args, _ctx, view| {
            let k = args
                .get_or_undefined(0)
                .to_string(_ctx)
                .ok()
                .map(|s| s.to_std_string_lossy())
                .unwrap_or_default();
            Ok(JsValue::from(view.flag(&k)))
        });
    }
}

fn new_engine_with_bridge_view() -> ScriptEngine {
    let mut engine = ScriptEngine::with_api(&BridgeViewRegistrar);
    engine.seed_number("PLAYER_GOLD", 999.0);
    engine.seed_text("PLAYER_NAME", "KAI");
    engine.seed_set("BADGES", &["ROCKBADGE".into(), "CASCADEBADGE".into()]);
    engine.set_flag("GOT_STARTER", true);
    engine.set_player_position(5, 3);
    engine.set_lang("zh");
    engine
}

#[test]
fn test_bridge_view_number() {
    let mut engine = new_engine_with_bridge_view();
    engine
        .load_script(
            r#"
        export async function check() {
            const gold = game.getTestNumber("PLAYER_GOLD");
            if (gold === 999) {
                await game.showText("Gold is 999");
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("check", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Gold is 999".to_string()
        })
    );
}

#[test]
fn test_bridge_view_text() {
    let mut engine = new_engine_with_bridge_view();
    engine
        .load_script(
            r#"
        export async function check() {
            const name = game.getTestText("PLAYER_NAME");
            if (name === "KAI") {
                await game.showText("Name is KAI");
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("check", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Name is KAI".to_string()
        })
    );
}

#[test]
fn test_bridge_view_set_contains() {
    let mut engine = new_engine_with_bridge_view();
    engine
        .load_script(
            r#"
        export async function check() {
            const hasRock = game.testSetContains("BADGES", "ROCKBADGE");
            const hasEarth = game.testSetContains("BADGES", "EARTHBADGE");
            if (hasRock && !hasEarth) {
                await game.showText("Correct badges");
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("check", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Correct badges".to_string()
        })
    );
}

#[test]
fn test_bridge_view_flag() {
    let mut engine = new_engine_with_bridge_view();
    engine
        .load_script(
            r#"
        export async function check() {
            const hasStarter = game.getTestFlag("GOT_STARTER");
            const hasBadge = game.getTestFlag("NONEXISTENT");
            if (hasStarter && !hasBadge) {
                await game.showText("Flag check ok");
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("check", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Flag check ok".to_string()
        })
    );
}

#[test]
fn test_get_all_flags_snapshot() {
    let mut engine = ScriptEngine::new();
    engine.set_flag("FLAG_A", true);
    engine.set_flag("FLAG_B", false);

    let snapshot = engine.get_all_flags();
    assert_eq!(snapshot.get("FLAG_A"), Some(&true));
    assert_eq!(snapshot.get("FLAG_B"), Some(&false));
    assert_eq!(snapshot.len(), 2);

    // Modify original — snapshot should be isolated
    engine.set_flag("FLAG_A", false);
    assert_eq!(snapshot.get("FLAG_A"), Some(&true));
}

#[test]
fn test_seed_flags_additive() {
    let mut engine = ScriptEngine::new();
    engine.set_flag("PRE_EXISTING", true);

    let mut additional = std::collections::HashMap::new();
    additional.insert("NEW_FLAG".to_string(), true);
    engine.seed_flags(&additional);

    assert!(engine.get_flag("PRE_EXISTING"));
    assert!(engine.get_flag("NEW_FLAG"));
}

#[test]
fn test_has_function_exists() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export function foo() {}
        export async function bar() {}
    "#,
        )
        .unwrap();

    assert!(engine.has_function("foo"));
    assert!(engine.has_function("bar"));
    assert!(!engine.has_function("nonexistent"));
    assert!(!engine.has_function("undefinedExport"));
}

#[test]
fn test_has_function_no_module() {
    let mut engine = ScriptEngine::new();
    assert!(!engine.has_function("anything"));
}

#[test]
fn test_signal_done_when_idle() {
    let mut engine = ScriptEngine::new();
    // signal_done should be a no-op if not waiting
    let result = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(result, None);
    assert!(engine.is_idle());
}

#[test]
fn test_script_syntax_error() {
    let mut engine = ScriptEngine::new();
    let result = engine.load_script("this is not valid js @@@");
    assert!(result.is_err());
}

#[test]
fn test_call_with_convenience_methods() {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(
            r#"
        export async function showMsg() { await game.showText("msg"); }
        export async function showLevel(lvl) { await game.showText("Level " + lvl); }
        export async function showPos(x, y) { await game.showText(x + "," + y); }
        export async function showName(name) { await game.showText(name); }
    "#,
        )
        .unwrap();

    // call_function_no_args
    let cmd = engine.call_function_no_args("showMsg").unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "msg".to_string()
        })
    );

    // signal done
    engine.signal_done(CommandResult::Void).unwrap();

    // call_function_with_u8
    let cmd = engine.call_function_with_u8("showLevel", 42).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Level 42".to_string()
        })
    );

    engine.signal_done(CommandResult::Void).unwrap();

    // call_function_with_xy
    let cmd = engine.call_function_with_xy("showPos", 5, 3).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "5,3".to_string()
        })
    );

    engine.signal_done(CommandResult::Void).unwrap();

    // call_function_with_str
    let cmd = engine.call_function_with_str("showName", "KAI").unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "KAI".to_string()
        })
    );
}

#[test]
fn test_call_function_without_module() {
    let mut engine = ScriptEngine::new();
    let result = engine.call_function("anything", &[]);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err().to_string(), s if s.contains("initialized")));
}

#[test]
fn test_set_player_position_and_lang() {
    let mut engine = ScriptEngine::new();
    engine.set_player_position(7, 4);
    engine.set_lang("en");

    engine
        .load_script(
            r#"
        export async function check() {
            const pos = game.getPlayerPosition();
            const lang = game.lang();
            if (pos.x === 7 && pos.y === 4 && lang === "en") {
                await game.showText("Position and lang OK");
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("check", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Position and lang OK".to_string()
        })
    );
}

#[test]
fn test_game_t_i18n() {
    let mut engine = ScriptEngine::new();
    engine.set_lang("zh");
    engine
        .load_script(
            r#"
        export async function check() {
            const greeting = game.t("Hello", "你好");
            if (greeting === "你好") {
                await game.showText("Chinese OK");
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("check", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Chinese OK".to_string()
        })
    );

    // Switch to English
    engine.set_lang("en");
    engine
        .load_script(
            r#"
        export async function checkEn() {
            const greeting = game.t("Hello", "你好");
            if (greeting === "Hello") {
                await game.showText("English OK");
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("checkEn", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "English OK".to_string()
        })
    );
}

#[test]
fn test_seed_number_text_set_roundtrip() {
    let mut engine = ScriptEngine::new();
    engine.seed_number("KEY_NUM", 42.5);
    engine.seed_text("KEY_TXT", "hello");
    engine.seed_set("KEY_SET", &["A".into(), "B".into()]);

    // Verify seeds are accessible via BridgeView through JS sync functions
    engine.register_sync_fn("getNum", |args, ctx, view| {
        let k = args
            .get_or_undefined(0)
            .to_string(ctx)
            .unwrap()
            .to_std_string_lossy();
        Ok(JsValue::from(view.number(&k)))
    });
    engine.register_sync_fn("getTxt", |args, ctx, view| {
        let k = args
            .get_or_undefined(0)
            .to_string(ctx)
            .unwrap()
            .to_std_string_lossy();
        Ok(JsValue::from(js_string!(view.text(&k))))
    });
    engine.register_sync_fn("checkSet", |args, ctx, view| {
        let k = args
            .get_or_undefined(0)
            .to_string(ctx)
            .unwrap()
            .to_std_string_lossy();
        let v = args
            .get_or_undefined(1)
            .to_string(ctx)
            .unwrap()
            .to_std_string_lossy();
        Ok(JsValue::from(view.set_contains(&k, &v)))
    });

    engine
        .load_script(
            r#"
        export async function verify() {
            const n = game.getNum("KEY_NUM");
            const t = game.getTxt("KEY_TXT");
            const hasA = game.checkSet("KEY_SET", "A");
            const hasB = game.checkSet("KEY_SET", "B");
            const hasC = game.checkSet("KEY_SET", "C");
            if (n === 42.5 && t === "hello" && hasA && hasB && !hasC) {
                await game.showText("All seeds OK");
            }
        }
    "#,
        )
        .unwrap();

    let cmd = engine.call_function("verify", &[]).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "All seeds OK".to_string()
        })
    );
}

#[test]
fn test_default_flag_is_false() {
    let engine = ScriptEngine::new();
    assert!(!engine.get_flag("ANY_NONEXISTENT_FLAG"));
}
