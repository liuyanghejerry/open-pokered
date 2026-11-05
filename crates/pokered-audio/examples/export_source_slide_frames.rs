//! Ordinary Safari music states for independent whole-frame source traces.
//! These are sequencer observations, not gameplay or physical clock evidence.
use pokered_audio::{audio_manager::AudioManager, music_data::MusicId, sfx_data::SfxId};

fn main() {
    let output = std::env::args().nth(1).expect("output path");
    let mut rows = Vec::new();
    for confirmation in [false, true] {
        let mut manager = AudioManager::new();
        manager.play_music(MusicId::SAFARI_ZONE);
        manager.update_frame();
        for frame in 0..64 {
            if confirmation && frame % 37 == 0 {
                manager.play_sfx(SfxId::PressAB);
            }
            let work = manager
                .begin_source_audio_frame(2, 0)
                .expect("Safari frame admission");
            let data: Vec<_> = manager
                .sequencer
                .channels
                .iter()
                .map(|c| c.data.clone())
                .collect();
            rows.push(serde_json::json!({"frame":frame,"confirmation":confirmation,"work":work,"data":data}));
            manager.update_frame();
        }
    }
    println!("{} ordinary Safari starting states", rows.len());
    std::fs::write(output, serde_json::to_vec(&serde_json::json!({
        "scope":"Ordinary native Safari music states, both with and without PressAB; no Game or physical timing claim.",
        "frames":rows
    })).unwrap()).unwrap();
}
