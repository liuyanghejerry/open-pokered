use super::*;
use crate::{audio_manager::AudioManager, music_data, music_data::MusicId, sfx_data::SfxId};

fn manager() -> AudioManager {
    let mut value = AudioManager::new();
    value.apu.write_register(0xff26, 0x80);
    value
}
fn tick(value: &mut AudioManager, count: usize) {
    for _ in 0..count {
        value.update_frame();
    }
}
fn roundtrip(mut original: AudioManager, follow: usize) {
    // Audio callbacks advance independently of song commands. Preserve a
    // nonaligned software phase, then compare actual future PCM as well.
    original.apu.tick_n(9 * 8192 + 157);
    let saved = original.audio_snapshot().unwrap();
    let bytes = serde_json::to_vec(&saved).unwrap();
    let saved: AudioSnapshot = serde_json::from_slice(&bytes).unwrap();
    let mut restored = manager();
    restored.play_music(MusicId::POKECENTER);
    tick(&mut restored, 120);
    restored.play_music(MusicId::WILD_BATTLE);
    tick(&mut restored, 50);
    restored.restore_audio_snapshot(&saved);
    assert_eq!(restored.audio_snapshot().unwrap(), saved);
    assert_eq!(original.audio_snapshot(), restored.audio_snapshot());
    for frame in 0..follow {
        original.update_frame();
        restored.update_frame();
        let samples = 37 + frame % 173;
        let mut expected = vec![0.0; samples * 2];
        let mut actual = vec![0.0; samples * 2];
        dotzuki_audio::output::render_apu_stereo(&mut original.apu, &mut expected, 48_000);
        dotzuki_audio::output::render_apu_stereo(&mut restored.apu, &mut actual, 48_000);
        assert_eq!(
            expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            "PCM frame{frame}"
        );
        assert_eq!(
            original.audio_snapshot(),
            restored.audio_snapshot(),
            "frame{frame}"
        );
    }
}

#[test]
fn map_four_channel_noise_alternate_and_silence_roundtrip_from_another_future_song() {
    for id in [MusicId::PALLET_TOWN, MusicId::CITIES1, MusicId::SILPH_CO] {
        for phase in [0, 1, 50, 300, 1400] {
            let mut original = manager();
            original.play_music(id);
            tick(&mut original, phase);
            roundtrip(original, 180);
        }
    }
    let mut drums = manager();
    drums.play_music(MusicId::CITIES1);
    for _ in 0..300 {
        drums.update_frame();
        if drums.sequencer.channels[7].active {
            break;
        }
    }
    assert!(drums.sequencer.channels[3].active && drums.sequencer.channels[7].active);
    roundtrip(drums, 180);
    let mut alternate = manager();
    alternate.play_meet_rival_alternate_start_and_tempo();
    tick(&mut alternate, 7);
    assert_eq!(alternate.last_music_id(), Some(MusicId::MEET_RIVAL));
    assert_eq!(
        alternate.sequencer.channels[0].data,
        music_data::MEETRIVAL_CH1_ALTERNATE_START_AND_TEMPO
    );
    assert_ne!(
        alternate.sequencer.channels[0].data,
        music_data::MEETRIVAL_CH1
    );
    assert!(alternate.sequencer.channels[0].ptr > 0);
    roundtrip(alternate, 180);
    let mut quiet = manager();
    quiet.play_music(MusicId::CITIES1);
    tick(&mut quiet, 30);
    quiet.stop_all();
    quiet.update_frame();
    assert_eq!(quiet.last_music_id(), None);
    assert!(!quiet.sequencer.is_playing());
    roundtrip(quiet, 180);
}

#[test]
fn cache_returns_to_saved_alternate_stream_instead_of_future_resume_position() {
    let mut original = manager();
    original.play_meet_rival_alternate_start_and_tempo();
    tick(&mut original, 7);
    original.play_music(MusicId::PALLET_TOWN);
    tick(&mut original, 13);
    original.set_master_volume(3, 5);
    original.set_no_audio_fade_out(true);
    let saved = original.audio_snapshot().unwrap();
    assert_eq!(saved.saved_music.len(), 1);
    assert_eq!(saved.saved_music[0].0, MusicId::MEET_RIVAL);
    let mut restored = manager();
    restored.play_music(MusicId::MEET_RIVAL);
    tick(&mut restored, 100);
    restored.play_music(MusicId::PALLET_TOWN);
    restored.restore_audio_snapshot(&saved);
    assert_eq!(original.audio_snapshot(), restored.audio_snapshot());
    original.play_music(MusicId::MEET_RIVAL);
    restored.play_music(MusicId::MEET_RIVAL);
    assert_eq!(
        restored.sequencer.channels[0].data,
        music_data::MEETRIVAL_CH1_ALTERNATE_START_AND_TEMPO
    );
    for _ in 0..180 {
        assert_eq!(original.audio_snapshot(), restored.audio_snapshot());
        original.update_frame();
        restored.update_frame();
    }
}

#[test]
fn active_sfx_alarm_and_pending_flute_retain_their_existing_owner_lifecycle() {
    for phase in [1, 6, 17] {
        let mut original = manager();
        original.play_music(MusicId::CITIES1);
        tick(&mut original, 50);
        original.play_sfx(SfxId::CaughtMon);
        tick(&mut original, phase);
        assert!(original.is_sfx_playing());
        roundtrip(original, 180);
    }
    for phase in [0, 1, 5, 10, 11, 12, 30, 31, 35] {
        let mut alarm = manager();
        alarm.play_music(MusicId::WILD_BATTLE);
        tick(&mut alarm, 50);
        alarm.set_low_health_alarm(true);
        tick(&mut alarm, phase);
        assert!(alarm.low_health_alarm_active());
        roundtrip(alarm, 90);
    }
    let mut disabled = manager();
    disabled.play_music(MusicId::WILD_BATTLE);
    disabled.set_low_health_alarm(true);
    tick(&mut disabled, 5);
    disabled.set_low_health_alarm(false);
    roundtrip(disabled, 90);
    let mut flute = manager();
    flute.play_music(MusicId::PALLET_TOWN);
    tick(&mut flute, 70);
    flute.play_flute_overworld(MusicId::PALLET_TOWN);
    tick(&mut flute, 8);
    let saved = flute.audio_snapshot().unwrap();
    assert_eq!(saved.current_music, None);
    assert_eq!(saved.music_after_sfx, Some(MusicId::PALLET_TOWN));
    assert!(flute.is_sfx_playing());
    roundtrip(flute, 300);
}

#[test]
fn native_post_sfx_controls_and_between_drum_tail_are_not_guessed_from_music() {
    for (music, sfx, phase) in [
        (MusicId::PALLET_TOWN, SfxId::PressAB, 1),
        (MusicId::GYM, SfxId::Cry00, 1),
    ] {
        let mut original = manager();
        original.play_music(music);
        tick(&mut original, phase);
        original.play_sfx(sfx);
        for _ in 0..500 {
            if !original.is_sfx_playing() {
                break;
            }
            original.update_frame();
        }
        assert!(!original.is_sfx_playing());
        let saved = original.audio_snapshot().unwrap();
        if music == MusicId::PALLET_TOWN {
            assert_ne!(
                saved.apu.pulse1.envelope.register,
                original.sequencer.channels[0].volume_envelope
            );
        } else {
            assert_ne!(
                saved.apu.pulse2.envelope.register,
                original.sequencer.channels[1].volume_envelope
            );
        }
        roundtrip(original, 120);
    }
    let mut drums = manager();
    drums.play_music(MusicId::CITIES1);
    let mut played = false;
    for _ in 0..500 {
        drums.update_frame();
        played |= drums.sequencer.channels[7].active;
        if played && !drums.sequencer.channels[7].active {
            break;
        }
    }
    assert!(played && !drums.sequencer.channels[7].active);
    assert!(drums.sequencer.channels[3].active);
    roundtrip(drums, 120);
}

#[test]
fn fade_callback_is_explicitly_unsupported_and_native_music_ids_are_validated() {
    let mut music = manager();
    music.play_music(MusicId::PALLET_TOWN);
    music.play_cities1_alternate_tempo();
    assert!(music.is_fading());
    assert!(music.audio_snapshot().is_none());
    for _ in 0..300 {
        music.update_frame();
        if !music.is_fading() {
            break;
        }
    }
    assert!(!music.is_fading());
    assert!(music.audio_snapshot().is_some());
    assert!(serde_json::from_str::<MusicId>("255").is_err());
    let current: Option<MusicId> = serde_json::from_str("null").unwrap();
    assert_eq!(current, None);
}

#[test]
fn restored_native_voices_render_pcm_and_a_quiet_snapshot_stays_quiet() {
    for voice in 0..4 {
        let mut original = manager();
        original.play_music(MusicId::CITIES1);
        let mut found = false;
        for _ in 0..1000 {
            original.update_frame();
            found = match voice {
                0 => original.apu.ch1.enabled && original.apu.ch1.envelope.current_volume > 0,
                1 => original.apu.ch2.enabled && original.apu.ch2.envelope.current_volume > 0,
                2 => original.apu.ch3.enabled && original.apu.ch3.volume_code > 0,
                _ => original.apu.ch4.enabled && original.apu.ch4.envelope.current_volume > 0,
            };
            if found {
                break;
            }
        }
        assert!(found, "native music never started voice{voice}");
        let saved = original.audio_snapshot().unwrap();
        let mut restored = manager();
        restored.play_music(MusicId::WILD_BATTLE);
        tick(&mut restored, 300);
        restored.restore_audio_snapshot(&saved);
        assert_eq!(restored.audio_snapshot().unwrap(), saved);
        restored.apu.write_register(0xff25, 0x11 << voice);
        let mut samples = [0.0; 1600];
        dotzuki_audio::output::render_apu_stereo(&mut restored.apu, &mut samples, 48_000);
        assert!(samples.iter().any(|&sample| sample != 0.0), "voice{voice}");
    }
    let mut quiet = manager();
    quiet.play_music(MusicId::CITIES1);
    tick(&mut quiet, 100);
    quiet.stop_all();
    quiet.update_frame();
    let saved = quiet.audio_snapshot().unwrap();
    let mut restored = manager();
    restored.play_music(MusicId::CITIES1);
    tick(&mut restored, 100);
    restored.restore_audio_snapshot(&saved);
    assert_eq!(restored.audio_snapshot().unwrap(), saved);
    let mut samples = [1.0; 1600];
    dotzuki_audio::output::render_apu_stereo(&mut restored.apu, &mut samples, 48_000);
    assert!(samples.iter().all(|&sample| sample == 0.0));
}
