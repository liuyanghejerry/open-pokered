//! Socket-free transport for the existing debug protocol, used only by tests.
//! Frames and commands use the production PokemonGame handlers unchanged.
use super::*;
use std::io::{BufRead, Write};

#[test]
#[ignore = "interactive JSON-lines test transport; run via fidelity_stdio.py"]
fn driver() {
    let stdin = std::io::stdin();
    let mut game: Option<PokemonGame> = None;
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        let response = match value["cmd"].as_str().unwrap() {
            "initialize_fixture" => {
                let path = |key: &str| value[key].as_str().map(PathBuf::from);
                let mut g = PokemonGame::new_with_options(
                    GameVersion::Red, path("save"), path("snapshot"), None,
                    false, None, false, true, None,
                );
                g.handle_debug_command(serde_json::from_value(serde_json::json!({
                    "cmd": "set_seed", "seed": value["seed"].as_u64().unwrap_or(0),
                })).unwrap());
                game = Some(g);
                serde_json::json!({ "ok": true })
            }
            "export_fixture" => {
                serde_json::json!({ "ok": true, "data": game.as_ref().unwrap().save_data })
            }
            _ => serde_json::to_value(game.as_mut().unwrap().handle_debug_command(
                serde_json::from_value(value).unwrap(),
            )).unwrap(),
        };
        println!("{response}");
        std::io::stdout().flush().unwrap();
    }
}
