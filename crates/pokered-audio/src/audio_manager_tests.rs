use crate::audio_manager::{AudioManager, FadeState};
use crate::music_data::MusicId;
use crate::sfx_data::SfxId;

#[test]
fn test_new_defaults() {
    let mgr = AudioManager::new();
    assert_eq!(mgr.master_volume_left(), 7);
    assert_eq!(mgr.master_volume_right(), 7);
    assert_eq!(mgr.fade_state(), FadeState::None);
    assert!(!mgr.is_fading());
    assert!(!mgr.is_music_playing());
    assert!(!mgr.is_sfx_playing());
    assert_eq!(mgr.last_music_id(), None);
    assert_eq!(mgr.nr50(), 0x77);
}

#[test]
fn test_set_master_volume() {
    let mut mgr = AudioManager::new();
    mgr.set_master_volume(5, 3);
    assert_eq!(mgr.master_volume_left(), 5);
    assert_eq!(mgr.master_volume_right(), 3);
    assert_eq!(mgr.nr50(), 0x53);
}

#[test]
fn test_set_master_volume_clamps_to_7() {
    let mut mgr = AudioManager::new();
    mgr.set_master_volume(15, 10);
    assert_eq!(mgr.master_volume_left(), 7);
    assert_eq!(mgr.master_volume_right(), 7);
}

#[test]
fn test_play_music() {
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);
    assert!(mgr.is_music_playing());
    assert_eq!(mgr.last_music_id(), Some(MusicId::PALLET_TOWN));
    assert_eq!(mgr.fade_state(), FadeState::None);
}

#[test]
fn test_stop_music() {
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);
    mgr.stop_music();
    assert!(!mgr.is_music_playing());
    assert_eq!(mgr.last_music_id(), None);
}

#[test]
fn test_stop_all() {
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);
    mgr.play_sfx(SfxId::PressAB);
    mgr.stop_all();
    assert!(!mgr.is_music_playing());
    assert!(!mgr.is_sfx_playing());
    assert_eq!(mgr.last_music_id(), None);
}

#[test]
fn test_play_sfx() {
    let mut mgr = AudioManager::new();
    mgr.play_sfx(SfxId::PressAB);
    assert!(mgr.is_sfx_playing());
}

#[test]
fn test_stop_sfx() {
    let mut mgr = AudioManager::new();
    mgr.play_sfx(SfxId::PressAB);
    mgr.stop_sfx();
    assert!(!mgr.is_sfx_playing());
}

#[test]
fn test_play_music_with_fade_no_current_music_plays_immediately() {
    let mut mgr = AudioManager::new();
    mgr.play_music_with_fade(MusicId::PALLET_TOWN, 10);
    assert!(mgr.is_music_playing());
    assert_eq!(mgr.last_music_id(), Some(MusicId::PALLET_TOWN));
    assert_eq!(mgr.fade_state(), FadeState::None);
}

#[test]
fn test_play_music_with_fade_same_id_noop() {
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);
    mgr.play_music_with_fade(MusicId::PALLET_TOWN, 10);
    assert_eq!(mgr.fade_state(), FadeState::None);
}

#[test]
fn test_play_music_with_fade_starts_fading() {
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);
    mgr.play_music_with_fade(MusicId::BIKE_RIDING, 2);
    assert!(mgr.is_fading());
    assert_eq!(mgr.fade_state(), FadeState::FadingOut);
}

#[test]
fn test_fade_decrements_volume_over_time() {
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);
    mgr.play_music_with_fade(MusicId::BIKE_RIDING, 0);
    assert_eq!(mgr.master_volume_left(), 7);

    mgr.update_frame();
    assert_eq!(mgr.master_volume_left(), 6);
    assert_eq!(mgr.master_volume_right(), 6);

    mgr.update_frame();
    assert_eq!(mgr.master_volume_left(), 5);
}

#[test]
fn test_fade_with_counter_delays_volume_decrement() {
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);
    mgr.play_music_with_fade(MusicId::BIKE_RIDING, 2);

    // First two frames: counter counts down (2->1, 1->0)
    mgr.update_frame();
    assert_eq!(mgr.master_volume_left(), 7);
    mgr.update_frame();
    assert_eq!(mgr.master_volume_left(), 7);

    // Third frame: counter reload, volume decrements
    mgr.update_frame();
    assert_eq!(mgr.master_volume_left(), 6);
}

#[test]
fn test_fade_completes_and_switches_music() {
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);
    mgr.play_music_with_fade(MusicId::BIKE_RIDING, 0);

    // 8 frames to go from volume 7 to 0 (decrement each frame), then one more for completion
    for _ in 0..7 {
        assert!(mgr.is_fading());
        mgr.update_frame();
    }
    assert_eq!(mgr.master_volume_left(), 0);
    assert_eq!(mgr.master_volume_right(), 0);

    // Next frame triggers fade_complete
    mgr.update_frame();
    assert_eq!(mgr.fade_state(), FadeState::None);
    assert_eq!(mgr.last_music_id(), Some(MusicId::BIKE_RIDING));
    assert_eq!(mgr.master_volume_left(), 7);
    assert_eq!(mgr.master_volume_right(), 7);
}

#[test]
fn test_fade_with_no_queued_music_just_stops() {
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);

    mgr.fade_state = FadeState::FadingOut;
    mgr.fade_queued_music = None;
    mgr.sequencer.fade_counter = 0;
    mgr.fade_counter_reload = 0;

    for _ in 0..8 {
        mgr.update_frame();
    }

    assert_eq!(mgr.fade_state(), FadeState::None);
    assert!(!mgr.is_music_playing());
}

#[test]
fn test_update_frame_no_crash_when_idle() {
    let mut mgr = AudioManager::new();
    for _ in 0..100 {
        mgr.update_frame();
    }
    assert_eq!(mgr.fade_state(), FadeState::None);
}

#[test]
fn test_update_frame_with_music() {
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);
    for _ in 0..60 {
        mgr.update_frame();
    }
    assert_eq!(mgr.master_volume_left(), 7);
}

#[test]
fn test_no_audio_fade_out_flag() {
    let mut mgr = AudioManager::new();
    assert!(!mgr.no_audio_fade_out());
    mgr.set_no_audio_fade_out(true);
    assert!(mgr.no_audio_fade_out());
}

#[test]
fn test_play_music_resets_fade_state() {
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);
    mgr.play_music_with_fade(MusicId::BIKE_RIDING, 5);
    assert!(mgr.is_fading());

    mgr.play_music(MusicId::SURFING);
    assert_eq!(mgr.fade_state(), FadeState::None);
    assert_eq!(mgr.last_music_id(), Some(MusicId::SURFING));
}

#[test]
fn test_play_music_restores_volume() {
    let mut mgr = AudioManager::new();
    mgr.set_master_volume(2, 2);
    mgr.play_music(MusicId::PALLET_TOWN);
    assert_eq!(mgr.master_volume_left(), 7);
    assert_eq!(mgr.master_volume_right(), 7);
}

#[test]
fn test_nr50_reflects_volume() {
    let mut mgr = AudioManager::new();
    assert_eq!(mgr.nr50(), 0x77);
    mgr.set_master_volume(3, 5);
    assert_eq!(mgr.nr50(), 0x35);
}

#[test]
fn test_play_sfx_noise_instrument_single_channel() {
    let mut mgr = AudioManager::new();
    mgr.play_sfx(SfxId::NoiseInstrument01);
    assert!(mgr.is_sfx_playing());
}

#[test]
fn test_play_sfx_cry_multi_channel() {
    let mut mgr = AudioManager::new();
    mgr.play_sfx(SfxId::Cry00);
    assert!(mgr.is_sfx_playing());
}

#[test]
fn test_default_impl() {
    let mgr = AudioManager::default();
    assert_eq!(mgr.master_volume_left(), 7);
    assert_eq!(mgr.nr50(), 0x77);
}

#[test]
fn test_play_cry_applies_modifiers() {
    // Audio1_SetSfxTempo: cry tempo = 0x0080 + wTempoModifier;
    // Audio1_ApplyFrequencyModifier: wFrequencyModifier reaches the sequencer.
    let mut mgr = AudioManager::new();
    mgr.play_cry(SfxId::Cry00, 0x20, 0xC0);
    assert!(mgr.is_sfx_playing());
    assert_eq!(mgr.sequencer.sfx_tempo, 0x0140);
    assert_eq!(mgr.sequencer.frequency_modifier, 0x20);
}

#[test]
fn test_play_sfx_resets_cry_modifiers() {
    // Plain SFX run at 0x0100 with no frequency modifier.
    let mut mgr = AudioManager::new();
    mgr.play_cry(SfxId::Cry00, 0x20, 0xC0);
    mgr.stop_sfx();
    mgr.play_sfx(SfxId::Pound);
    assert_eq!(mgr.sequencer.sfx_tempo, 0x0100);
    assert_eq!(mgr.sequencer.frequency_modifier, 0);
}

#[test]
fn test_noise_instruments_registered() {
    // The 19 noise-instrument (drum) streams are registered with the
    // sequencer at startup; instrument N (1-19) → slot N-1.
    let mgr = AudioManager::new();
    assert_eq!(mgr.sequencer.noise_instruments.len(), 19);
    assert_eq!(
        mgr.sequencer.noise_instruments[0].as_slice(),
        crate::sfx_data::SFX_NOISE_INSTRUMENT01_CH8
    );
    assert_eq!(
        mgr.sequencer.noise_instruments[18].as_slice(),
        crate::sfx_data::SFX_NOISE_INSTRUMENT19_CH8
    );
}

#[test]
fn test_drum_note_triggers_noise_instrument_sfx() {
    // ROUTES3 CH4 opens with `drum_speed 6` then `drum_note 19, 1` — the
    // drum_note must start the instrument-19 noise SFX on CHAN8
    // (audio/engine_1.asm:673-688).
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::ROUTES3);
    mgr.update_frame();

    assert!(mgr.sequencer.channels[7].active); // CHAN8
    assert_eq!(mgr.sequencer.channels[7].sound_id, 19);
    assert_eq!(
        mgr.sequencer.channels[7].data.as_slice(),
        crate::sfx_data::SFX_NOISE_INSTRUMENT19_CH8
    );
    // Music noise channel CHAN4 keeps running on its own timing.
    assert!(mgr.sequencer.channels[3].active);
}

#[test]
fn test_volume_command_overrides_master_volume_for_frame() {
    // PALLET_TOWN CH1's second command is `volume $77`. The fade machinery
    // applies the master volume first (home/vblank.asm:53 FadeOutAudio
    // before :62 Audio1_UpdateMusic), then the in-song volume command
    // overrides NR50 for the rest of the frame.
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);
    mgr.set_master_volume(5, 3); // NR50 = $53
    assert_eq!(mgr.nr50(), 0x53);

    mgr.update_frame();
    assert_eq!(mgr.nr50(), 0x77);
}

// ── Low-health alarm (audio/low_health_alarm.asm) ───────────────────────

/// `Music_DoLowHealthAlarm` (low_health_alarm.asm:9-32): timer 0 plays the
/// hi tone (freq $750) and reloads the timer to 30; timer 20 plays the lo
/// tone (freq $6EE); all other frames just count down.
#[test]
fn test_low_health_alarm_cadence() {
    let mut mgr = AudioManager::new();
    mgr.apu.write_register(0xFF26, 0x80); // NR52 power on
    mgr.set_low_health_alarm(true);
    assert!(mgr.low_health_alarm_active());

    // VBlank 1 (timer 0): hi tone, timer reloaded to 30.
    mgr.update_frame();
    assert_eq!(mgr.apu.ch1.freq_reg, 0x750);

    // VBlanks 2-11 (timer 30→21): no register writes — hi tone holds.
    for _ in 0..10 {
        mgr.update_frame();
    }
    assert_eq!(mgr.apu.ch1.freq_reg, 0x750);

    // VBlank 12 (timer 20): lo tone.
    mgr.update_frame();
    assert_eq!(mgr.apu.ch1.freq_reg, 0x6EE);

    // VBlanks 13-31 (timer 19→0): no writes — lo tone holds.
    for _ in 0..19 {
        mgr.update_frame();
    }
    assert_eq!(mgr.apu.ch1.freq_reg, 0x6EE);

    // VBlank 32 (timer 0 again): hi tone — the cycle repeats.
    mgr.update_frame();
    assert_eq!(mgr.apu.ch1.freq_reg, 0x750);
}

/// The tone data is written to NR11-NR14 with NR10 zeroed
/// (`.playTone`, low_health_alarm.asm:50-62; `.toneDataHi`: `alarm_tone
/// $A0, $E2, $8750`).
#[test]
fn test_low_health_alarm_tone_registers() {
    let mut mgr = AudioManager::new();
    mgr.apu.write_register(0xFF26, 0x80);
    mgr.set_low_health_alarm(true);
    mgr.update_frame();
    assert_eq!(mgr.apu.read_register(0xFF11), 0xA0 | 0x3F); // duty 2, length $20
    assert_eq!(mgr.apu.read_register(0xFF12), 0xE2); // envelope
    assert!(mgr.apu.ch1.enabled); // NR14 bit 7 retriggered the channel
}

/// Disabling writes `.toneDataSilence` (`alarm_tone $00, $00, $8000`,
/// low_health_alarm.asm:79-80): envelope zeroed, channel silenced.
#[test]
fn test_low_health_alarm_disable_silences_channel() {
    let mut mgr = AudioManager::new();
    mgr.apu.write_register(0xFF26, 0x80);
    mgr.set_low_health_alarm(true);
    mgr.update_frame();
    assert_eq!(mgr.apu.read_register(0xFF12), 0xE2);

    mgr.set_low_health_alarm(false);
    assert!(!mgr.low_health_alarm_active());
    assert_eq!(mgr.apu.read_register(0xFF12), 0x00);
    assert!(!mgr.apu.ch1.enabled);

    // No further channel-1 writes once disabled.
    mgr.update_frame();
    assert_eq!(mgr.apu.read_register(0xFF12), 0x00);
}

/// While the alarm bit is set, SFX channel 5 (pulse 1) processing is
/// suppressed (audio/engine_2.asm:162-170) so an SFX cannot fight the
/// alarm's direct register writes. PressAB's only channel is CHAN5.
#[test]
fn test_low_health_alarm_suppresses_pulse1_sfx() {
    let mut mgr = AudioManager::new();
    mgr.apu.write_register(0xFF26, 0x80);
    mgr.set_low_health_alarm(true);
    mgr.play_sfx(SfxId::PressAB);
    assert!(!mgr.is_sfx_playing());

    // SFX on other hardware channels are unaffected (StartMenu: CHAN8).
    mgr.play_sfx(SfxId::StartMenu);
    assert!(mgr.is_sfx_playing());
}

// ── Music_PokeFluteInBattle (audio/poke_flute.asm) ──────────────────────

#[test]
fn test_play_flute_in_battle() {
    use dotzuki_audio::sequencer::{CHAN5, CHAN6, CHAN7};

    let mut mgr = AudioManager::new();
    mgr.play_flute_in_battle();

    // PlaySound(SFX_CAUGHT_MON) initialized the channels...
    assert!(mgr.is_sfx_playing());
    assert_eq!(mgr.sequencer.current_sfx_id, SfxId::CaughtMon as u8);
    // ...then their command pointers were overwritten with the flute streams
    // (audio/poke_flute.asm:5-12), so the caught-mon data never executes.
    assert_eq!(
        mgr.sequencer.channels[CHAN5].data,
        crate::sfx_data::POKEFLUTE_IN_BATTLE_CH5.to_vec()
    );
    assert_eq!(
        mgr.sequencer.channels[CHAN6].data,
        crate::sfx_data::POKEFLUTE_IN_BATTLE_CH6.to_vec()
    );
    assert_eq!(
        mgr.sequencer.channels[CHAN7].data,
        crate::sfx_data::POKEFLUTE_IN_BATTLE_CH7.to_vec()
    );

    // The wave channel (CHAN7) plays the melody: a note frequency appears
    // while the pulse channels (CHAN5/6) only rest (frequency 0).
    mgr.update_frame();
    assert_ne!(mgr.sequencer.channels[CHAN7].frequency, 0);
    assert_eq!(mgr.sequencer.channels[CHAN5].frequency, 0);
    assert_eq!(mgr.sequencer.channels[CHAN6].frequency, 0);

    // The whole jingle is 32 note-lengths at speed 8 = 256 frames; after
    // that every SFX channel has ended (battle music would resume).
    for _ in 0..300 {
        mgr.update_frame();
    }
    assert!(!mgr.is_sfx_playing());
}

#[test]
fn test_play_flute_in_battle_skipped_during_low_health_alarm() {
    // engine/items/item_effects.asm:1735-1737: with the alarm bit set, the
    // in-battle flute music is skipped entirely.
    let mut mgr = AudioManager::new();
    mgr.set_low_health_alarm(true);
    mgr.play_flute_in_battle();
    assert!(!mgr.is_sfx_playing());
}

// ── Alternate tempo/start music (audio/alternate_tempo.asm) ───────────────

#[test]
fn test_play_meet_rival_alternate_start() {
    // Music_RivalAlternateStart (alternate_tempo.asm:2-12): PlayMusic
    // MUSIC_MEET_RIVAL, then overwrite the CH1/CH2/CH3 command pointers.
    let mut mgr = AudioManager::new();
    mgr.play_meet_rival_alternate_start();

    assert_eq!(mgr.last_music_id(), Some(MusicId::MEET_RIVAL));
    assert!(mgr.is_music_playing());
    assert_eq!(
        mgr.sequencer.channels[0].data,
        crate::music_data::MEETRIVAL_CH1_ALTERNATE_START.to_vec()
    );
    assert_eq!(
        mgr.sequencer.channels[1].data,
        crate::music_data::MEETRIVAL_CH2_ALTERNATE_START.to_vec()
    );
    assert_eq!(
        mgr.sequencer.channels[2].data,
        crate::music_data::MEETRIVAL_CH3_ALTERNATE_START.to_vec()
    );

    // The overwrite lands before the first tick, so the normal opening
    // measure never executes: the alternate start's first notes differ from
    // the normal theme's on all three channels.
    let mut normal = AudioManager::new();
    normal.play_music(MusicId::MEET_RIVAL);
    mgr.update_frame();
    normal.update_frame();
    for ch in 0..3 {
        assert_ne!(mgr.sequencer.channels[ch].frequency, 0);
        assert_ne!(
            mgr.sequencer.channels[ch].frequency,
            normal.sequencer.channels[ch].frequency,
            "channel {} should play the alternate opening",
            ch + 1
        );
    }
    // Same tempo as the normal theme (112) — only the start differs.
    assert_eq!(mgr.sequencer.music_tempo, 0x0070);
}

#[test]
fn test_play_meet_rival_alternate_tempo() {
    // Music_RivalAlternateTempo (alternate_tempo.asm:21-27): only CH1's
    // pointer is overwritten, with the tempo-100 stream.
    let mut mgr = AudioManager::new();
    mgr.play_meet_rival_alternate_tempo();

    assert_eq!(mgr.last_music_id(), Some(MusicId::MEET_RIVAL));
    assert_eq!(
        mgr.sequencer.channels[0].data,
        crate::music_data::MEETRIVAL_CH1_ALTERNATE_TEMPO.to_vec()
    );
    // CH2/CH3 keep the normal streams.
    assert_eq!(
        mgr.sequencer.channels[1].data,
        crate::music_data::MEETRIVAL_CH2.to_vec()
    );
    assert_eq!(
        mgr.sequencer.channels[2].data,
        crate::music_data::MEETRIVAL_CH3.to_vec()
    );

    // CH1 begins with `tempo 100` (meetrival.asm:2), applied on the first
    // tick — the normal theme runs at 112.
    mgr.update_frame();
    assert_eq!(mgr.sequencer.music_tempo, 0x0064);
}

#[test]
fn test_play_meet_rival_alternate_start_and_tempo() {
    // Music_RivalAlternateStartAndTempo (alternate_tempo.asm:30-34):
    // AlternateStart, then CH1 is overwritten *again* with the
    // start+tempo stream.
    let mut mgr = AudioManager::new();
    mgr.play_meet_rival_alternate_start_and_tempo();

    assert_eq!(mgr.last_music_id(), Some(MusicId::MEET_RIVAL));
    assert_eq!(
        mgr.sequencer.channels[0].data,
        crate::music_data::MEETRIVAL_CH1_ALTERNATE_START_AND_TEMPO.to_vec()
    );
    assert_eq!(
        mgr.sequencer.channels[1].data,
        crate::music_data::MEETRIVAL_CH2_ALTERNATE_START.to_vec()
    );
    assert_eq!(
        mgr.sequencer.channels[2].data,
        crate::music_data::MEETRIVAL_CH3_ALTERNATE_START.to_vec()
    );

    mgr.update_frame();
    assert_eq!(mgr.sequencer.music_tempo, 0x0064);
    // CH1 plays the alternate start's opening (octave 3 D_), same as the
    // plain alternate start — not the normal theme's octave 4 opening.
    let mut alt_start = AudioManager::new();
    alt_start.play_meet_rival_alternate_start();
    alt_start.update_frame();
    assert_eq!(
        mgr.sequencer.channels[0].frequency,
        alt_start.sequencer.channels[0].frequency
    );
}

#[test]
fn test_play_meet_rival_alternate_variants_ignore_saved_state() {
    // The original PlayMusic always restarts the song; a saved resume-state
    // for MEET_RIVAL must not bypass the alternate start.
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::MEET_RIVAL);
    for _ in 0..30 {
        mgr.update_frame();
    }
    mgr.play_music(MusicId::PALLET_TOWN); // saves MEET_RIVAL state
    mgr.play_meet_rival_alternate_start();
    assert_eq!(mgr.sequencer.channels[0].ptr, 0);
    assert_eq!(
        mgr.sequencer.channels[0].data,
        crate::music_data::MEETRIVAL_CH1_ALTERNATE_START.to_vec()
    );
}

#[test]
fn test_play_cities1_alternate_tempo_fades_then_restarts() {
    // Music_Cities1AlternateTempo (alternate_tempo.asm:37-50): fade out
    // (reload value 10), then restart MUSIC_CITIES1 and overwrite CH1's
    // pointer with the slower tempo-232 stream.
    let mut mgr = AudioManager::new();
    mgr.play_music(MusicId::PALLET_TOWN);
    mgr.play_cities1_alternate_tempo();

    // Fade started with reload 10; the restart is queued.
    assert_eq!(mgr.fade_state(), FadeState::FadingOut);
    assert_eq!(mgr.sequencer.fade_counter, 10);
    assert_eq!(mgr.fade_counter_reload, 10);
    assert_eq!(mgr.fade_queued_music, Some(MusicId::CITIES1));
    assert!(mgr.is_music_playing(), "old song still playing during fade");

    // The fade takes 7 volume steps at ~11 frames each; after it completes
    // the queued song starts with CH1 overridden.
    for _ in 0..120 {
        mgr.update_frame();
    }
    assert_eq!(mgr.fade_state(), FadeState::None);
    assert_eq!(mgr.last_music_id(), Some(MusicId::CITIES1));
    assert!(mgr.is_music_playing());
    assert_eq!(
        mgr.sequencer.channels[0].data,
        crate::music_data::CITIES1_CH1_ALTERNATE_TEMPO.to_vec()
    );
    // CH2-4 are the normal streams.
    assert_eq!(
        mgr.sequencer.channels[1].data,
        crate::music_data::CITIES1_CH2.to_vec()
    );

    // CH1 begins with `tempo 232` (cities1.asm:2) — slower than the normal
    // 144 — applied on the next tick after the restart.
    for _ in 0..120 {
        if mgr.sequencer.music_tempo == 0x00E8 {
            break;
        }
        mgr.update_frame();
    }
    assert_eq!(mgr.sequencer.music_tempo, 0x00E8);
}

#[test]
fn test_play_cities1_alternate_tempo_without_music_plays_immediately() {
    let mut mgr = AudioManager::new();
    mgr.play_cities1_alternate_tempo();
    assert_eq!(mgr.fade_state(), FadeState::None);
    assert_eq!(mgr.last_music_id(), Some(MusicId::CITIES1));
    assert_eq!(
        mgr.sequencer.channels[0].data,
        crate::music_data::CITIES1_CH1_ALTERNATE_TEMPO.to_vec()
    );
}

#[test]
fn test_play_script_music_dispatch() {
    let mut mgr = AudioManager::new();
    assert!(!mgr.play_script_music("MUSIC_PALLET_TOWN"));
    assert!(!mgr.play_script_music("MUSIC_MEET_RIVAL"));

    assert!(mgr.play_script_music("MUSIC_MEET_RIVAL_ALTERNATE_START"));
    assert_eq!(mgr.last_music_id(), Some(MusicId::MEET_RIVAL));
    assert_eq!(
        mgr.sequencer.channels[0].data,
        crate::music_data::MEETRIVAL_CH1_ALTERNATE_START.to_vec()
    );

    assert!(mgr.play_script_music("MUSIC_MEET_RIVAL_ALTERNATE_TEMPO"));
    assert_eq!(
        mgr.sequencer.channels[0].data,
        crate::music_data::MEETRIVAL_CH1_ALTERNATE_TEMPO.to_vec()
    );

    assert!(mgr.play_script_music("MUSIC_MEET_RIVAL_ALTERNATE_START_AND_TEMPO"));
    assert_eq!(
        mgr.sequencer.channels[0].data,
        crate::music_data::MEETRIVAL_CH1_ALTERNATE_START_AND_TEMPO.to_vec()
    );

    assert!(mgr.play_script_music("MUSIC_CITIES1_ALTERNATE_TEMPO"));
    // No music was playing (MEET_RIVAL was stopped by the direct dispatch? —
    // it was playing), so this fades out first.
    assert!(
        mgr.fade_state() == FadeState::FadingOut || mgr.last_music_id() == Some(MusicId::CITIES1)
    );
}
