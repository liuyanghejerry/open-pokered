//! Controlled ordinary-sequencer states for independent original drum-frame
//! traces. This is not a saved game, Game input trace or physical clock proof.
use pokered_audio::{audio_manager::AudioManager, music_data::MusicId, sfx_data::SfxId};

fn main() {
    let output = std::env::args().nth(1).expect("output path");
    let mut rows = Vec::new();
    let mut coverage = [0; 8];
    for (music, bank) in [
        (MusicId::CITIES1, 2),
        (MusicId::VERMILION, 2),
        (MusicId::LAVENDER, 2),
        (MusicId::ROUTES1, 2),
        (MusicId::ROUTES2, 2),
        (MusicId::ROUTES3, 2),
        (MusicId::ROUTES4, 2),
        (MusicId::INDIGO_PLATEAU, 2),
        (MusicId::BIKE_RIDING, 31),
        (MusicId::DUNGEON1, 31),
        (MusicId::DUNGEON2, 31),
        (MusicId::CINNABAR_MANSION, 31),
    ] {
        for confirmation in [false, true] {
            let mut manager = AudioManager::new();
            manager.play_music(music);
            manager.update_frame();
            let mut selected = [false; 8];
            for frame in 0..1024 {
                if confirmation && frame % 37 == 0 {
                    manager.play_sfx(SfxId::PressAB);
                }
                let channels = &manager.sequencer.channels;
                let beat = &channels[3];
                let noise = &channels[7];
                let drum = beat.active
                    && beat.delay_counter == 1
                    && beat.data.get(beat.ptr).is_some_and(|b| b & 0xf0 == 0xb0);
                let conditions = [
                    frame == 0,
                    drum && !noise.active,
                    drum && noise.active,
                    noise.active
                        && noise.delay_counter == 1
                        && noise.data.get(noise.ptr) == Some(&0xff),
                    noise.active
                        && noise.delay_counter == 1
                        && noise.data.get(noise.ptr).is_some_and(|b| b & 0xf0 == 0x20),
                    noise.active && noise.delay_counter > 1,
                    beat.active && beat.delay_counter == 1 && !drum,
                    channels[4].active
                        && channels[4].delay_counter == 1
                        && channels[4].data.get(channels[4].ptr) == Some(&0xff)
                        && noise.active,
                ];
                if conditions
                    .iter()
                    .enumerate()
                    .any(|(i, yes)| *yes && !selected[i])
                {
                    for (i, yes) in conditions.iter().enumerate() {
                        if *yes && !selected[i] {
                            selected[i] = true;
                            coverage[i] += 1;
                        }
                    }
                    let work = manager
                        .begin_source_audio_frame(bank, 0)
                        .expect("percussion frame admitted");
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
    // Ordinary music did not overlap two drums in these trajectories. Exercise
    // the busy branch explicitly, with a documented canonical pending CHAN8
    // note; do not label these four rows ordinary playback observations.
    for (music, bank, frame) in [(MusicId::CITIES1, 2, 39), (MusicId::DUNGEON1, 31, 93)] {
        for confirmation in [false, true] {
            let mut manager = AudioManager::new();
            manager.play_music(music);
            manager.update_frame();
            for n in 0..=frame {
                if confirmation && n % 37 == 0 {
                    manager.play_sfx(SfxId::PressAB);
                }
                if n < frame {
                    manager.update_frame();
                }
            }
            let beat = &manager.sequencer.channels[3];
            assert_eq!(beat.delay_counter, 1);
            assert_eq!(beat.data[beat.ptr] & 0xf0, 0xb0);
            let instrument = 19;
            let noise = &mut manager.sequencer.channels[7];
            noise.reset();
            noise.data =
                pokered_audio::sfx_data::get_sfx_track(SfxId::from_u8(instrument - 1).unwrap())
                    .channels[3]
                    .unwrap()
                    .to_vec();
            noise.active = true;
            noise.sound_id = instrument;
            noise.note_speed = 1;
            noise.delay_counter = 1;
            noise
                .flags1
                .insert(pokered_audio::ChannelFlags1::NOISE_OR_SFX);
            manager.sequencer.sfx_playing = true;
            let work = manager.begin_source_audio_frame(bank, 0).unwrap();
            let data: Vec<_> = manager
                .sequencer
                .channels
                .iter()
                .map(|c| c.data.clone())
                .collect();
            rows.push(serde_json::json!({"frame":frame,"confirmation":confirmation,"work":work,"data":data,"initial_noise_override":instrument}));
            coverage[2] += 1;
        }
    }
    assert!(
        coverage.iter().all(|n| *n != 0),
        "missing drum branch: {coverage:?}"
    );
    println!("{} frames; categories {coverage:?}", rows.len());
    std::fs::write(output, serde_json::to_vec(&serde_json::json!({
        "scope":"Controlled ordinary native states for original whole-frame drum traces; no Game input or physical clock claim.",
        "coverage":coverage,"frames":rows
    })).unwrap()).unwrap();
}
