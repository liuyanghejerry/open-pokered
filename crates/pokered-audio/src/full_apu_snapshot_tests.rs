use super::*;
use crate::{audio_manager::AudioManager, music_data::MusicId, sfx_data::SfxId};
use dotzuki_audio::output::render_apu_stereo;
use std::sync::atomic::{AtomicUsize, Ordering};

fn manager() -> AudioManager {
    let mut manager = AudioManager::new();
    manager.apu.write_register(0xFF26, 0x80);
    manager
}

fn sounding_manager() -> AudioManager {
    let mut value = manager();
    value.play_music(MusicId::CITIES1);
    for _ in 0..43 {
        value.update_frame();
        let mut pcm = [0.0; 802];
        render_apu_stereo(&mut value.apu, &mut pcm, 48_000);
    }
    value.apu.tick_n(157);
    value
}

fn json_snapshot(manager: &AudioManager) -> AudioSnapshot {
    serde_json::from_slice(&serde_json::to_vec(&manager.audio_snapshot().unwrap()).unwrap())
        .unwrap()
}

fn assert_pcm_equal(a: &mut AudioManager, b: &mut AudioManager, rate: u32, samples: usize) -> bool {
    let mut expected = vec![0.0; samples * 2];
    let mut actual = vec![0.0; samples * 2];
    render_apu_stereo(&mut a.apu, &mut expected, rate);
    render_apu_stereo(&mut b.apu, &mut actual, rate);
    assert_eq!(
        expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
    assert_eq!(a.audio_snapshot(), b.audio_snapshot());
    expected.iter().any(|v| *v != 0.0)
}

static SOURCE_WRITES: AtomicUsize = AtomicUsize::new(0);
static DESTINATION_WRITES: AtomicUsize = AtomicUsize::new(0);
fn source_sink(_: u16, _: u8) {
    SOURCE_WRITES.fetch_add(1, Ordering::SeqCst);
}
fn destination_sink(_: u16, _: u8) {
    DESTINATION_WRITES.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn full_snapshot_is_authoritative_and_preserves_destination_sink_without_writes_or_ticks() {
    let mut original = sounding_manager();
    original.apu.set_register_sink(Some(source_sink));
    let phase = original.apu.playback_state();
    SOURCE_WRITES.store(0, Ordering::SeqCst);
    let mut saved = json_snapshot(&original);
    assert_eq!(saved.apu_playback.as_ref(), Some(&phase));
    assert_eq!(
        original.apu.playback_state(),
        phase,
        "capture does not tick"
    );
    assert_eq!(SOURCE_WRITES.load(Ordering::SeqCst), 0);

    // Old controls remain in the JSON for compatibility; they cannot override
    // the complete state or cause a second, phase-destroying register install.
    saved.apu.power = false;
    saved.apu.volume = 0;
    saved.apu.wave.wave_ram = [0; 16];
    let mut restored = sounding_manager();
    restored.apu.tick_n(123_456);
    restored.apu.set_register_sink(Some(destination_sink));
    DESTINATION_WRITES.store(0, Ordering::SeqCst);
    restored.restore_audio_snapshot(&saved);
    assert_eq!(restored.apu.playback_state(), phase);
    assert_eq!(restored.audio_snapshot(), original.audio_snapshot());
    assert_eq!(DESTINATION_WRITES.load(Ordering::SeqCst), 0);
    assert_eq!(SOURCE_WRITES.load(Ordering::SeqCst), 0);
    restored.apu.write_register(0xFF25, 0x11);
    assert_eq!(DESTINATION_WRITES.load(Ordering::SeqCst), 1);
    assert_eq!(SOURCE_WRITES.load(Ordering::SeqCst), 0);
}

fn mixed_write(value: &mut AudioManager, step: usize) {
    match step {
        1 => value.play_sfx(SfxId::PressAB),
        3 => value.set_low_health_alarm(true),
        5 => value.apu.write_register(0xFF13, 0xF9),
        6 => value.apu.write_register(0xFF22, 0x0D),
        8 => value.set_low_health_alarm(false),
        9 => value.apu.write_register(0xFF1A, 0),
        10 => {
            value.apu.write_register(0xFF1A, 0x80);
            value.apu.write_register(0xFF1E, 0x86);
        }
        12 => value.set_master_volume(3, 5),
        13 => {
            value.apu.write_register(0xFF26, 0);
            value.apu.write_register(0xFF35, 0xE1);
        }
        14 => {
            value.apu.write_register(0xFF26, 0x80);
            value.apu.write_register(0xFF12, 0xB2);
            value.apu.write_register(0xFF14, 0x84);
            value.apu.write_register(0xFF1A, 0x80);
            value.apu.write_register(0xFF1C, 0x20);
            value.apu.write_register(0xFF1E, 0x86);
            value.apu.write_register(0xFF21, 0xC1);
            value.apu.write_register(0xFF23, 0x80);
        }
        17 => value.apu.write_register(0xFF25, 0),
        18 => value.apu.write_register(0xFF25, 0xFF),
        21 => value.play_music(MusicId::PALLET_TOWN),
        22 => value.play_music(MusicId::CITIES1),
        _ => {}
    }
    value.update_frame();
    value.apu.tick_n((step as u32 * 31 + 7) % 199);
}

#[test]
fn full_snapshot_replays_mixed_real_manager_updates_and_pcm_at_both_sample_rates() {
    for rate in [44_100, 48_000] {
        let mut original = sounding_manager();
        let saved = json_snapshot(&original);
        let mut restored = manager();
        restored.play_music(MusicId::WILD_BATTLE);
        restored.update_frame();
        restored.apu.tick_n(65_719);
        restored.restore_audio_snapshot(&saved);
        assert_eq!(restored.audio_snapshot(), Some(saved));
        let mut audible = false;
        for step in 0..24 {
            mixed_write(&mut original, step);
            mixed_write(&mut restored, step);
            assert_eq!(original.audio_snapshot(), restored.audio_snapshot());
            audible |= assert_pcm_equal(&mut original, &mut restored, rate, 511 + step * 13);
        }
        assert!(audible, "the matching samples include audible output");
    }
}

#[test]
fn full_snapshot_keeps_none_music_flute_alarm_and_real_between_drum_tail_phases() {
    let mut sfx_only = manager();
    sfx_only.play_sfx(SfxId::CaughtMon);
    sfx_only.update_frame();
    sfx_only.apu.tick_n(12_345);
    assert_eq!(sfx_only.last_music_id(), None);
    assert!(sfx_only.is_sfx_playing());

    let mut flute = sounding_manager();
    flute.play_flute_overworld(MusicId::CITIES1);
    flute.update_frame();
    flute.apu.tick_n(7_123);
    assert_eq!(flute.last_music_id(), None);
    assert_eq!(
        flute.audio_snapshot().unwrap().music_after_sfx,
        Some(MusicId::CITIES1)
    );

    let mut alarm = sounding_manager();
    alarm.set_low_health_alarm(true);
    alarm.update_frame();
    alarm.apu.tick_n(9_345);
    assert!(alarm.low_health_alarm_active());

    let mut drums = manager();
    drums.play_music(MusicId::CITIES1);
    let mut played = false;
    let mut found_tail = false;
    for _ in 0..1_000 {
        drums.update_frame();
        drums.apu.tick_n(697);
        played |= drums.sequencer.channels[7].active;
        if played && !drums.sequencer.channels[7].active && drums.apu.ch4.enabled {
            found_tail = true;
            break;
        }
    }
    assert!(
        found_tail,
        "capture a real retained noise tail after its instrument stream returns"
    );

    for mut original in [sfx_only, flute, alarm, drums] {
        let saved = json_snapshot(&original);
        let mut restored = sounding_manager();
        restored.restore_audio_snapshot(&saved);
        assert_eq!(restored.audio_snapshot(), Some(saved));
        for _ in 0..180 {
            original.update_frame();
            restored.update_frame();
            assert_pcm_equal(&mut original, &mut restored, 48_000, 113);
        }
    }
}

#[test]
fn full_snapshot_preserves_silent_off_wave_ram_volume_and_long_retained_timers() {
    let mut original = manager();
    for (address, value) in [
        (0xFF10, 0x70),
        (0xFF12, 0xB7),
        (0xFF14, 0x80),
        (0xFF17, 0xA7),
        (0xFF19, 0x80),
        (0xFF1A, 0x80),
        (0xFF1E, 0x80),
        (0xFF21, 0xC7),
        (0xFF22, 0xF7),
        (0xFF23, 0x80),
    ] {
        original.apu.write_register(address, value);
    }
    original.apu.tick_n(37);
    for (address, value) in [
        (0xFF10, 0x10),
        (0xFF12, 0xB1),
        (0xFF13, 0xFF),
        (0xFF14, 7),
        (0xFF17, 0xA1),
        (0xFF18, 0xFF),
        (0xFF19, 7),
        (0xFF1D, 0xFF),
        (0xFF1E, 7),
        (0xFF21, 0xC1),
        (0xFF22, 0),
    ] {
        original.apu.write_register(address, value);
    }
    original.set_master_volume(3, 5);
    original.apu.write_register(0xFF24, 0x12);
    let saved = json_snapshot(&original);
    let phase = serde_json::to_value(saved.apu_playback.as_ref().unwrap()).unwrap();
    assert_eq!(phase["ch1"]["freq_timer"], 8192 - 37);
    assert_eq!(phase["ch4"]["freq_timer"], (112 << 15) - 37);
    assert_eq!(saved.master_volume, [3, 5]);
    assert_eq!(saved.apu.volume, 0x12);
    let mut restored = sounding_manager();
    restored.restore_audio_snapshot(&saved);
    assert_eq!(restored.audio_snapshot(), Some(saved));
    assert_eq!(
        restored.apu.nr50, 0x12,
        "logical volume setup cannot overwrite actual NR50"
    );
    for _ in 0..12 {
        assert_pcm_equal(&mut original, &mut restored, 44_100, 431);
    }

    for value in [&mut original, &mut restored] {
        value.apu.ch1.enabled = false;
        value.apu.ch2.enabled = false;
        value.apu.ch3.enabled = false;
        value.apu.ch4.enabled = false;
        value.apu.tick_n(713);
    }
    let silent = json_snapshot(&original);
    restored.restore_audio_snapshot(&silent);
    assert_eq!(original.apu.mix_sample(), (0, 0));
    assert_pcm_equal(&mut original, &mut restored, 48_000, 1_024);
    for value in [&mut original, &mut restored] {
        value.apu.write_register(0xFF26, 0);
        value.apu.write_register(0xFF30, 0xBA);
        value.apu.write_register(0xFF3F, 0x17);
    }
    let off = json_snapshot(&original);
    restored.restore_audio_snapshot(&off);
    assert_eq!(restored.apu.ch3.wave_ram, original.apu.ch3.wave_ram);
    assert_pcm_equal(&mut original, &mut restored, 48_000, 1_024);
    assert_eq!(restored.audio_snapshot(), Some(off));
    for value in [&mut original, &mut restored] {
        value.apu.write_register(0xFF26, 0x80);
        value.apu.write_register(0xFF25, 0x44);
        value.apu.write_register(0xFF1A, 0x80);
        value.apu.write_register(0xFF1C, 0x20);
        value.apu.write_register(0xFF1E, 0x86);
    }
    assert_pcm_equal(&mut original, &mut restored, 48_000, 1_024);
}

#[test]
fn legacy_control_only_json_retains_its_weaker_contract_and_partial_phase_is_rejected() {
    let original = sounding_manager();
    let saved = json_snapshot(&original);
    let mut value = serde_json::to_value(&saved).unwrap();
    value.as_object_mut().unwrap().remove("apu_playback");
    let legacy: AudioSnapshot = serde_json::from_value(value).unwrap();
    assert!(legacy.apu_playback.is_none());
    let mut restored = manager();
    restored.restore_audio_snapshot(&legacy);
    let mut recaptured = restored.audio_snapshot().unwrap();
    assert_ne!(
        recaptured.apu_playback, saved.apu_playback,
        "legacy has no original phase"
    );
    recaptured.apu_playback = None;
    assert_eq!(recaptured, legacy);
    let mut pcm = [0.0; 2_048];
    render_apu_stereo(&mut restored.apu, &mut pcm, 48_000);
    assert!(pcm.iter().any(|v| *v != 0.0));

    let mut missing = serde_json::to_value(&saved).unwrap();
    missing["apu_playback"]
        .as_object_mut()
        .unwrap()
        .remove("frame_seq_counter");
    assert!(serde_json::from_value::<AudioSnapshot>(missing).is_err());
}

#[test]
fn numeric_fades_stay_unsupported_until_their_existing_continuation_finishes() {
    let mut original = sounding_manager();
    original.fade_out(1);
    assert!(original.is_fading());
    assert!(original.audio_snapshot().is_none());
    let mut reached_silence_during_fade = false;
    for _ in 0..100 {
        reached_silence_during_fade |= original.is_fading()
            && original.master_volume_left() == 0
            && original.master_volume_right() == 0;
        if !original.is_fading() {
            break;
        }
        assert!(original.audio_snapshot().is_none());
        original.update_frame();
    }
    assert!(reached_silence_during_fade);
    assert!(!original.is_fading());
    assert!(original.audio_snapshot().unwrap().apu_playback.is_some());
}
