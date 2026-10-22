//! Controlled native fixtures, not a Game/playthrough/snapshot-restoration tool.
use pokered_audio::{audio_manager::AudioManager, music_data::MusicId, sfx_data::SfxId};
fn main() {
    let output = std::env::args().nth(1).expect("output path");
    let mut rows = Vec::new();
    for (music, bank) in [
        (MusicId::POKECENTER, 2),
        (MusicId::SILPH_CO, 31),
        (MusicId::DUNGEON3, 31),
    ] {
        for confirmation in [false, true] {
            let mut manager = AudioManager::new();
            manager.play_music(music);
            manager.update_frame();
            for frame in 0..256 {
                if confirmation && frame % 37 == 0 {
                    manager.play_sfx(SfxId::PressAB);
                }
                if frame < 8
                    || [12, 16, 36, 37, 38, 39, 50, 51, 52, 73, 74, 75, 89, 95].contains(&frame)
                    || manager
                        .sequencer
                        .channels
                        .iter()
                        .any(|c| c.active && c.delay_counter <= 1)
                {
                    let work = manager
                        .begin_source_audio_frame(bank, 0)
                        .expect("supported native fixture");
                    let data: Vec<_> = manager
                        .sequencer
                        .channels
                        .iter()
                        .map(|c| c.data.clone())
                        .collect();
                    rows.push(serde_json::json!({"frame":frame,"confirmation":confirmation,"work":work,"data":data}));
                }
                manager.update_frame();
            }
        }
    }
    std::fs::write(output,serde_json::to_vec(&serde_json::json!({"scope":"Controlled native-manager fixtures; no Game input or physical APU timing claim.","frames":rows})).unwrap()).unwrap();
}
