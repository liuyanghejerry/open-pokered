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
                if value["pcm_audio"].as_bool().unwrap_or(false) {
                    g.audio = Some(AudioOutput::new_pcm());
                }
                game = Some(g);
                serde_json::json!({ "ok": true })
            }
            "stats_state" => {
                let stats = game.as_ref().unwrap().stats_screen.as_ref().unwrap();
                serde_json::json!({ "ok": true, "pokemon": stats.pokemon() })
            }
            "audio_state" => {
                let audio = game.as_ref().unwrap().audio.as_ref().unwrap();
                let manager = audio.manager.lock().unwrap();
                serde_json::json!({ "ok": true, "sound_playing": manager.is_sfx_playing(),
                    "sfx_channels": (0..4).map(|ch| manager.sequencer.is_sfx_channel_active(ch)).collect::<Vec<_>>(),
                    "registers": (0xFF10..=0xFF25).map(|address| manager.apu.read_register(address)).collect::<Vec<_>>() })
            }
            "audio_reference_cry" => {
                let species: pokered_data::species::Species = serde_json::from_value(value["species"].clone()).unwrap();
                let audio = AudioOutput::new_pcm();
                play_species_cry(&audio, species);
                audio.update_frame();
                let registers = {
                    let manager = audio.manager.lock().unwrap();
                    (0xFF10..=0xFF25).map(|address| manager.apu.read_register(address)).collect::<Vec<_>>()
                };
                let mut ticks = 1;
                while audio.is_sfx_playing() && ticks < 600 { audio.update_frame(); ticks += 1; }
                serde_json::json!({ "ok": true, "registers": registers, "ticks": ticks })
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
