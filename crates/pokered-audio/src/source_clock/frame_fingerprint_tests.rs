use super::*;
use crate::audio_manager::AudioManager;
use crate::sfx_data::SfxId;
use alloc::vec::Vec;

fn wide_product(value: u64) -> u64 {
    (u128::from(value) * 0x0000_0100_0000_01b3u128) as u64
}

fn wide_fingerprint(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        wide_product(hash ^ u64::from(byte))
    })
}

#[test]
fn arm32_fingerprint_product_preserves_carries_and_wrapping() {
    for high in [
        0u32,
        1,
        0xffff,
        0x1_0000,
        0x7fff_ffff,
        0x8000_0000,
        u32::MAX,
    ] {
        for middle in [0u32, 1, 255, 65534, 65535] {
            for low in 0..=65535u32 {
                let value = (u64::from(high) << 32) | (u64::from(middle) << 16) | u64::from(low);
                assert_eq!(fnv_multiply_arm32(value), wide_product(value));
            }
        }
    }
    let mut value = 0x98f7_24ae_1cc3_018bu64;
    for _ in 0..1_000_000 {
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        assert_eq!(fnv_multiply_arm32(value), wide_product(value));
    }
}

fn check_program(data: &[u8]) {
    let mut arm32 = 0xcbf2_9ce4_8422_2325;
    let mut wide = arm32;
    for &byte in data {
        arm32 = fnv_multiply_arm32(arm32 ^ u64::from(byte));
        wide = wide_product(wide ^ u64::from(byte));
        assert_eq!(arm32, wide);
    }
    assert_eq!(program_fingerprint(data), wide);
}

#[test]
fn all_music_and_sound_programs_keep_the_exact_fingerprint() {
    check_program(&[]);
    for track in &crate::music_data::MUSIC_TRACKS {
        for data in track.channels.iter().flatten() {
            check_program(data);
        }
    }
    for track in &crate::sfx_data::SFX_TRACKS {
        for data in track.channels.iter().flatten() {
            check_program(data);
        }
    }
    let bytes: Vec<u8> = (0..=255).cycle().take(4096).collect();
    for size in [0, 1, 2, 4, 11, 19, 38, 64, 255, 4096] {
        check_program(&bytes[..size]);
    }
}

#[test]
fn restored_frame_keeps_full_program_guards_for_all_eight_channels() {
    for song in [
        SourceSong::Pokecenter,
        SourceSong::Dungeon3,
        SourceSong::SilphCo,
    ] {
        let mut manager = AudioManager::new();
        manager.play_music(song.music());
        manager.update_frame();
        manager.play_sfx(SfxId::PressAB);
        let frame =
            SourceAudioFrame::new_with_bus(&manager.sequencer, song, song.bank(), 0).unwrap();
        let restored: SourceAudioFrame =
            serde_json::from_slice(&serde_json::to_vec(&frame).unwrap()).unwrap();
        assert_eq!(frame, restored);
        assert!(restored.matches_native(&manager.sequencer));
        for i in 0..8 {
            let original = manager.sequencer.channels[i].data.clone();
            assert_eq!(
                restored.start.voices[i].data_fingerprint,
                wide_fingerprint(&original)
            );
            if !original.is_empty() {
                for index in [0, original.len() / 2, original.len() - 1] {
                    manager.sequencer.channels[i].data[index] ^= 1;
                    assert!(!restored.matches_native(&manager.sequencer));
                    manager.sequencer.channels[i].data[index] ^= 1;
                    assert!(restored.matches_native(&manager.sequencer));
                }
                manager.sequencer.channels[i].data.pop();
                assert!(!restored.matches_native(&manager.sequencer));
                manager.sequencer.channels[i].data = original.clone();
            }
            manager.sequencer.channels[i].data.push(0);
            assert!(!restored.matches_native(&manager.sequencer));
            manager.sequencer.channels[i].data = original;
            assert!(restored.matches_native(&manager.sequencer));
        }
    }
}
