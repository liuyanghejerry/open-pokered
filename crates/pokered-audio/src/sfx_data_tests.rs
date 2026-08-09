use crate::sfx_data::*;

#[test]
fn test_sfx_id_count() {
    assert_eq!(NUM_SFX, 161);
}

#[test]
fn test_sfx_id_from_u8_valid() {
    assert_eq!(SfxId::from_u8(0), Some(SfxId::NoiseInstrument01));
    assert_eq!(SfxId::from_u8(19), Some(SfxId::Cry00));
    assert_eq!(SfxId::from_u8(160), Some(SfxId::ShootingStar));
}

#[test]
fn test_sfx_id_from_u8_invalid() {
    assert_eq!(SfxId::from_u8(161), None);
    assert_eq!(SfxId::from_u8(255), None);
}

#[test]
fn test_all_sfx_tracks_have_at_least_one_channel() {
    for track in SFX_TRACKS.iter() {
        let has_channel = track.channels.iter().any(|ch| ch.is_some());
        assert!(has_channel, "SFX {:?} has no channel data", track.id);
    }
}

#[test]
fn test_all_channel_data_non_empty() {
    for track in SFX_TRACKS.iter() {
        for (i, ch) in track.channels.iter().enumerate() {
            if let Some(data) = ch {
                assert!(!data.is_empty(), "SFX {:?} hw_ch{} is empty", track.id, i);
            }
        }
    }
}

#[test]
fn test_all_channel_data_ends_with_ret_or_loop() {
    for track in SFX_TRACKS.iter() {
        for (i, ch) in track.channels.iter().enumerate() {
            if let Some(data) = ch {
                let last = data[data.len() - 1];
                // Most SFX end with sound_ret (0xFF).
                // Some end with sound_loop (0xFE, count, lo, hi) — last byte is hi addr.
                let has_ret = last == 0xFF;
                let has_loop = data.len() >= 4 && data[data.len() - 4] == 0xFE;
                assert!(
                    has_ret || has_loop,
                    "SFX {:?} hw_ch{} ends with 0x{:02X}, expected sound_ret(0xFF) or sound_loop(0xFE)",
                    track.id,
                    i,
                    last
                );
            }
        }
    }
}

#[test]
fn test_num_channels_matches_actual_channels() {
    for track in SFX_TRACKS.iter() {
        let actual = track.channels.iter().filter(|ch| ch.is_some()).count();
        assert_eq!(
            actual, track.num_channels as usize,
            "SFX {:?}: num_channels={} but actual={}",
            track.id, track.num_channels, actual
        );
    }
}

#[test]
fn test_noise_instruments_are_single_channel_noise() {
    // NoiseInstrument01-19 (ids 0-18) should all be 1-channel noise (hw channel 3)
    for i in 0..19u8 {
        let id = SfxId::from_u8(i).unwrap();
        let track = get_sfx_track(id);
        assert_eq!(
            track.num_channels,
            1,
            "NoiseInstrument{:02} should have 1 channel",
            i + 1
        );
        assert!(
            track.channels[3].is_some(),
            "NoiseInstrument{:02} should use noise channel (hw3)",
            i + 1
        );
        assert!(
            track.channels[0].is_none(),
            "NoiseInstrument{:02} should not use pulse1",
            i + 1
        );
        assert!(
            track.channels[1].is_none(),
            "NoiseInstrument{:02} should not use pulse2",
            i + 1
        );
        assert!(
            track.channels[2].is_none(),
            "NoiseInstrument{:02} should not use wave",
            i + 1
        );
    }
}

#[test]
fn test_noise_instrument01_byte_data() {
    // NoiseInstrument01: noise_note 0, 12, 1, $33 + sound_ret
    // Expected: [0x20, 0xC1, 0x33, 0xFF]
    let track = get_sfx_track(SfxId::NoiseInstrument01);
    let data = track.channels[3].unwrap();
    assert_eq!(data, &[0x20, 0xC1, 0x33, 0xFF]);
}

#[test]
fn test_cries_are_three_channel() {
    // All 38 cries (Cry00-Cry25) use channels 5, 6, 8 → hw 0, 1, 3
    for i in 19..57u8 {
        let id = SfxId::from_u8(i).unwrap();
        let track = get_sfx_track(id);
        assert_eq!(track.num_channels, 3, "Cry {:?} should have 3 channels", id);
        assert!(
            track.channels[0].is_some(),
            "Cry {:?} should use pulse1 (ch5→hw0)",
            id
        );
        assert!(
            track.channels[1].is_some(),
            "Cry {:?} should use pulse2 (ch6→hw1)",
            id
        );
        assert!(
            track.channels[2].is_none(),
            "Cry {:?} should not use wave",
            id
        );
        assert!(
            track.channels[3].is_some(),
            "Cry {:?} should use noise (ch8→hw3)",
            id
        );
    }
}

#[test]
fn test_cry00_channel_data_lengths() {
    let track = get_sfx_track(SfxId::Cry00);
    assert_eq!(track.channels[0].unwrap().len(), 15, "Cry00 ch5 length");
    assert_eq!(track.channels[1].unwrap().len(), 15, "Cry00 ch6 length");
    assert_eq!(track.channels[3].unwrap().len(), 10, "Cry00 ch8 length");
}

#[test]
fn test_cry00_ch5_starts_with_duty_cycle_pattern() {
    let data = get_sfx_track(SfxId::Cry00).channels[0].unwrap();
    // duty_cycle_pattern encodes as 0xFC
    assert_eq!(
        data[0], 0xFC,
        "Cry00 ch5 should start with duty_cycle_pattern (0xFC)"
    );
}

#[test]
fn test_pokeflute_uses_wave_channel() {
    // Pokeflute is special: uses channel 3 → hw wave (index 2)
    let track = get_sfx_track(SfxId::Pokeflute);
    assert_eq!(track.num_channels, 1);
    assert!(
        track.channels[0].is_none(),
        "Pokeflute should not use pulse1"
    );
    assert!(
        track.channels[1].is_none(),
        "Pokeflute should not use pulse2"
    );
    assert!(
        track.channels[2].is_some(),
        "Pokeflute should use wave channel"
    );
    assert!(
        track.channels[3].is_none(),
        "Pokeflute should not use noise"
    );
}

#[test]
fn test_pokeflute_data_length() {
    let data = get_sfx_track(SfxId::Pokeflute).channels[2].unwrap();
    assert_eq!(data.len(), 23, "Pokeflute wave channel should be 23 bytes");
    assert_eq!(*data.last().unwrap(), 0xFF, "Should end with sound_ret");
}

#[test]
fn test_level_up_three_channels_pulse_pulse_wave() {
    let track = get_sfx_track(SfxId::LevelUp);
    assert_eq!(track.num_channels, 3);
    assert!(
        track.channels[0].is_some(),
        "LevelUp should use pulse1 (ch5)"
    );
    assert!(
        track.channels[1].is_some(),
        "LevelUp should use pulse2 (ch6)"
    );
    assert!(track.channels[2].is_some(), "LevelUp should use wave (ch7)");
    assert!(track.channels[3].is_none(), "LevelUp should not use noise");
}

#[test]
fn test_level_up_ch5_starts_with_execute_music() {
    let data = get_sfx_track(SfxId::LevelUp).channels[0].unwrap();
    // execute_music encodes as 0xF8
    assert_eq!(
        data[0], 0xF8,
        "LevelUp ch5 should start with execute_music (0xF8)"
    );
    assert_eq!(data.len(), 27, "LevelUp ch5 should be 27 bytes");
}

#[test]
fn test_ball_toss_two_channels() {
    let track = get_sfx_track(SfxId::BallToss);
    assert_eq!(track.num_channels, 2);
    assert!(track.channels[0].is_some(), "BallToss should use pulse1");
    assert!(track.channels[1].is_some(), "BallToss should use pulse2");
    assert_eq!(track.channels[0].unwrap().len(), 9, "BallToss ch5 length");
    assert_eq!(track.channels[1].unwrap().len(), 7, "BallToss ch6 length");
}

#[test]
fn test_psybeam_uses_sound_loop() {
    let track = get_sfx_track(SfxId::Psybeam);
    assert_eq!(track.num_channels, 3);
    // Psybeam ch5 should contain sound_loop (0xFE)
    let ch5 = track.channels[0].unwrap();
    let has_loop = ch5.windows(1).any(|w| w[0] == 0xFE);
    assert!(has_loop, "Psybeam ch5 should contain sound_loop (0xFE)");
    // ch8 (noise) should also contain sound_loop
    let ch8 = track.channels[3].unwrap();
    let has_loop_noise = ch8.windows(1).any(|w| w[0] == 0xFE);
    assert!(
        has_loop_noise,
        "Psybeam ch8 should contain sound_loop (0xFE)"
    );
}

#[test]
fn test_psybeam_data_lengths() {
    let track = get_sfx_track(SfxId::Psybeam);
    assert_eq!(track.channels[0].unwrap().len(), 23, "Psybeam ch5 length");
    assert_eq!(track.channels[1].unwrap().len(), 23, "Psybeam ch6 length");
    assert_eq!(track.channels[3].unwrap().len(), 11, "Psybeam ch8 length");
}

#[test]
fn test_get_sfx_track_all_ids() {
    for i in 0..NUM_SFX {
        let id = SfxId::from_u8(i as u8).unwrap();
        let track = get_sfx_track(id);
        assert_eq!(track.id, id);
    }
}

#[test]
fn test_sfx_tracks_array_length() {
    assert_eq!(SFX_TRACKS.len(), NUM_SFX);
}

#[test]
fn test_sfx_tracks_ids_sequential() {
    for (i, track) in SFX_TRACKS.iter().enumerate() {
        assert_eq!(
            track.id as usize, i,
            "SFX_TRACKS[{}] has id {:?} ({}), expected {}",
            i, track.id, track.id as usize, i
        );
    }
}

// ── In-battle POKé FLUTE streams (audio/poke_flute.asm) ─────────────────
// Music_PokeFluteInBattle overwrites the live command pointers of SFX
// channels 5-7 with these. Converted by tools/asm2sfx.py from
// audio/sfx/pokeflute_ch5_ch6.asm and audio/sfx/pokeflute_ch7.asm.

#[test]
fn test_pokeflute_in_battle_ch5_stream() {
    // SFX_Pokeflute_Ch5 (pokeflute_ch5_ch6.asm:1-2) is only `tempo 256`;
    // in the ROM the stream falls through into SFX_Pokeflute_Ch6's data,
    // so the converted bytes are the tempo command followed by all of CH6.
    assert_eq!(
        POKEFLUTE_IN_BATTLE_CH5,
        &[
            0xED, 0x01, 0x00, // tempo 256
            0xF8, // execute_music
            0xD8, 0x00, // note_type 8, 0, 0
            0xC4, 0xC6, 0xC4, 0xC4, 0xC4, 0xC4, // rest 5, 7, 5, 5, 5, 5
            0xFF, // sound_ret
        ]
    );
}

#[test]
fn test_pokeflute_in_battle_ch6_stream() {
    // SFX_Pokeflute_Ch6 (pokeflute_ch5_ch6.asm:4-13): both pulse channels
    // only rest while CHAN7 plays the melody.
    assert_eq!(
        POKEFLUTE_IN_BATTLE_CH6,
        &[
            0xF8, // execute_music
            0xD8, 0x00, // note_type 8, 0, 0
            0xC4, 0xC6, 0xC4, 0xC4, 0xC4, 0xC4, // rest 5, 7, 5, 5, 5, 5
            0xFF, // sound_ret
        ]
    );
}

#[test]
fn test_pokeflute_in_battle_ch7_stream() {
    // SFX_Pokeflute_Ch7 (pokeflute_ch7.asm:1-13): the flute melody on the
    // wave channel — E3 F3 G7 A3 G3 (oct 5), then C13 (oct 6).
    assert_eq!(
        POKEFLUTE_IN_BATTLE_CH7,
        &[
            0xF8, // execute_music
            0xEA, 0x10, 0x14, // vibrato 16, 1, 4
            0xD8, 0x10, // note_type 8, 1, 0 (wave instrument 1)
            0xE3, // octave 5
            0x42, 0x52, 0x76, 0x92, 0x72, // E_3, F_3, G_7, A_3, G_3
            0xE2, // octave 6
            0x0C, // C_13
            0xFF, // sound_ret
        ]
    );
}
