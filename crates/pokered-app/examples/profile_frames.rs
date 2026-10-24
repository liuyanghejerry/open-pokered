//! Bounded profiling of the shared hosted update, draw, and PCM paths.
//! Run with `cargo run --release -p pokered-app --example profile_frames -- 3600`.
use pokered_app::PokemonGame;
use pokered_core::{
    data::wild_data::GameVersion, game_state::GameScreen, pokemon::stats::create_pokemon,
};
use pokered_data::species::Species;
use pokered_renderer::{input::InputState, FrameBuffer, RenderConfig, Rgba};
use std::{hint::black_box, time::Instant};

fn stats(values: &mut [f64]) -> serde_json::Value {
    values.sort_by(f64::total_cmp);
    serde_json::json!({"mean_ms":values.iter().sum::<f64>() / values.len() as f64,
        "p50_ms":values[values.len()/2], "p95_ms":values[values.len()*95/100],
        "p99_ms":values[values.len()*99/100], "max_ms":values[values.len()-1]})
}

fn main() {
    let frames = std::env::args()
        .nth(1)
        .map(|v| v.parse::<usize>().unwrap())
        .unwrap_or(3600);
    assert!(frames > 0);
    let mut reports = Vec::new();
    for phase in ["idle", "walk", "battle"] {
        let mut game = PokemonGame::new_mobile(GameVersion::Red, None).unwrap();
        game.set_seed(42);
        game.handle_transition(GameScreen::Overworld);
        game.save_data
            .party
            .add(create_pokemon(Species::Bulbasaur, 25, [0x9a, 0x78]).unwrap())
            .unwrap();
        if phase == "battle" {
            game.debug_start_wild_battle(Species::Snorlax, 30);
        }
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        let mut input = InputState::new();
        let mut pcm = vec![0.0; 740 * 2];
        let mut rgba = vec![0; 160 * 144 * 4];
        let (mut updates, mut draws, mut audio, mut copies, mut totals) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for frame in 0..frames + 120 {
            input.begin_frame();
            input.set_from_bitmask(if phase == "walk" {
                if frame % 128 < 64 {
                    16
                } else {
                    32
                }
            } else {
                0
            });
            let a = Instant::now();
            game.update(&input);
            let b = Instant::now();
            game.draw(&mut fb);
            let c = Instant::now();
            game.audio.as_ref().unwrap().render_pcm(&mut pcm);
            let d = Instant::now();
            fb.to_rgba(&mut rgba);
            black_box((&pcm, &rgba));
            let e = Instant::now();
            if frame >= 120 {
                updates.push((b - a).as_secs_f64() * 1000.0);
                draws.push((c - b).as_secs_f64() * 1000.0);
                audio.push((d - c).as_secs_f64() * 1000.0);
                copies.push((e - d).as_secs_f64() * 1000.0);
                totals.push((e - a).as_secs_f64() * 1000.0);
            }
        }
        reports.push(serde_json::json!({"scenario":phase,"frames":frames,
            "screen":format!("{:?}", game.state.screen), "update":stats(&mut updates),
            "draw":stats(&mut draws), "pcm":stats(&mut audio), "copy":stats(&mut copies),
            "total":stats(&mut totals)}));
    }
    println!("{}", serde_json::to_string_pretty(&reports).unwrap());
}
