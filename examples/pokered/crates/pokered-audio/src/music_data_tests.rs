use crate::music_data::*;

#[test]
fn test_music_id_count() {
    assert_eq!(NUM_MUSIC_TRACKS, 45);
}

#[test]
fn test_music_id_from_u8_valid() {
    assert_eq!(MusicId::from_u8(0), Some(MusicId::PALLET_TOWN));
    assert_eq!(MusicId::from_u8(23), Some(MusicId::FINAL_BATTLE));
    assert_eq!(MusicId::from_u8(31), Some(MusicId::JIGGLYPUFF_SONG));
    assert_eq!(MusicId::from_u8(44), Some(MusicId::MEET_MALE_TRAINER));
}

#[test]
fn test_music_id_from_u8_invalid() {
    assert_eq!(MusicId::from_u8(45), None);
    assert_eq!(MusicId::from_u8(255), None);
}

#[test]
fn test_all_tracks_have_at_least_one_channel() {
    for track in MUSIC_TRACKS.iter() {
        let has_channel = track.channels.iter().any(|ch| ch.is_some());
        assert!(has_channel, "Track {:?} has no channel data", track.id);
    }
}

#[test]
fn test_all_channel_data_non_empty() {
    for track in MUSIC_TRACKS.iter() {
        for (i, ch) in track.channels.iter().enumerate() {
            if let Some(data) = ch {
                assert!(
                    !data.is_empty(),
                    "Track {:?} ch{} is empty",
                    track.id,
                    i + 1
                );
            }
        }
    }
}

#[test]
fn test_all_channel_data_ends_with_loop_or_ret() {
    for track in MUSIC_TRACKS.iter() {
        for (i, ch) in track.channels.iter().enumerate() {
            if let Some(data) = ch {
                let last = data[data.len() - 1];
                let valid_end = last == 0xFF  // sound_ret
                    || last == 0xFE           // part of sound_loop (rare, last byte is hi addr)
                    || data.len() >= 4 && data[data.len() - 4] == 0xFE; // sound_loop: FE cnt lo hi
                assert!(
                    valid_end || (data.len() >= 3 && data[data.len() - 3] == 0xFD), // sound_call as tail
                    "Track {:?} ch{} ends with 0x{:02X}, expected sound_ret/sound_loop",
                    track.id,
                    i + 1,
                    last
                );
            }
        }
    }
}

#[test]
fn test_pallet_town_structure() {
    let track = get_music_track(MusicId::PALLET_TOWN);
    assert_eq!(track.num_channels, 3);
    assert!(track.channels[0].is_some());
    assert!(track.channels[1].is_some());
    assert!(track.channels[2].is_some());
    assert!(track.channels[3].is_none());
    assert!(track.tempo > 0);
}

#[test]
fn test_jigglypuff_two_channels() {
    let track = get_music_track(MusicId::JIGGLYPUFF_SONG);
    assert_eq!(track.num_channels, 2);
    assert!(track.channels[0].is_some());
    assert!(track.channels[1].is_some());
    assert!(track.channels[2].is_none());
    assert!(track.channels[3].is_none());
}

#[test]
fn test_final_battle_has_all_channels_parsed() {
    let track = get_music_track(MusicId::FINAL_BATTLE);
    assert!(
        track.channels[0].is_some(),
        "Ch1 should be present (cross-channel ref test)"
    );
    assert!(track.channels[1].is_some());
    assert!(track.channels[2].is_some());
    let ch1_data = track.channels[0].unwrap();
    assert!(
        ch1_data.len() > 100,
        "Ch1 should have substantial data, got {}",
        ch1_data.len()
    );
}

#[test]
fn test_battle_tracks_have_four_channels() {
    for id in [
        MusicId::TITLE_SCREEN,
        MusicId::INTRO_BATTLE,
        MusicId::DUNGEON1,
    ] {
        let track = get_music_track(id);
        assert_eq!(
            track.num_channels, 4,
            "Track {:?} should have 4 channels",
            id
        );
        for (i, ch) in track.channels.iter().enumerate() {
            assert!(ch.is_some(), "Track {:?} ch{} should be present", id, i + 1);
        }
    }
}

#[test]
fn test_tempos_are_reasonable() {
    for track in MUSIC_TRACKS.iter() {
        assert!(track.tempo > 0, "Track {:?} has zero tempo", track.id);
    }
}

#[test]
fn test_pallet_town_ch1_starts_with_tempo_command() {
    let track = get_music_track(MusicId::PALLET_TOWN);
    let ch1 = track.channels[0].unwrap();
    assert_eq!(ch1[0], 0xED, "First byte should be tempo command (0xED)");
}

#[test]
fn test_channel_data_contains_notes() {
    let track = get_music_track(MusicId::PALLET_TOWN);
    let ch1 = track.channels[0].unwrap();
    let has_notes = ch1.iter().any(|&b| b < 0xB0);
    assert!(has_notes, "Channel data should contain note bytes (< 0xB0)");
}

#[test]
fn test_get_music_track_all_ids() {
    for i in 0..NUM_MUSIC_TRACKS {
        let id = MusicId::from_u8(i as u8).unwrap();
        let track = get_music_track(id);
        assert_eq!(track.id, id);
    }
}

#[test]
fn test_sequencer_can_load_pallet_town() {
    use crate::apu::Apu;
    use crate::sequencer::Sequencer;

    let track = get_music_track(MusicId::PALLET_TOWN);
    let channel_data: Vec<Vec<u8>> = track
        .channels
        .iter()
        .filter_map(|ch| ch.map(|d| d.to_vec()))
        .collect();

    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    seq.play_music(0, &channel_data, track.tempo);

    for _ in 0..100 {
        seq.update_frame(&mut apu);
    }
}

#[test]
fn test_sequencer_can_load_final_battle() {
    use crate::apu::Apu;
    use crate::sequencer::Sequencer;

    let track = get_music_track(MusicId::FINAL_BATTLE);
    let channel_data: Vec<Vec<u8>> = track
        .channels
        .iter()
        .filter_map(|ch| ch.map(|d| d.to_vec()))
        .collect();

    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    seq.play_music(0, &channel_data, track.tempo);

    for _ in 0..200 {
        seq.update_frame(&mut apu);
    }
}

// ── Alternate tempo/start override streams (audio/alternate_tempo.asm) ─────
// The alternate prefixes were byte-verified against the original ROM (bank
// 2: Music_MeetRival_Ch1_AlternateTempo $7119, _StartAndTempo $719B,
// _AlternateStart $71A2, Ch2 $721D, Ch3 $72B5; Music_Cities1_Ch1_Alternate-
// Tempo $6A6F). Loop/call offsets are slice-relative here — the ROM stores
// bank addresses; each override stream is the alternate prefix followed by
// the full normal channel data with offsets rebased by the prefix length.

#[test]
fn test_meet_rival_ch1_alternate_tempo() {
    // audio/music/meetrival.asm:1-3: `tempo 100; sound_loop 0, Ch1.body`.
    // Ch1.body is at offset 3, so the rebased loop target is 7 + 3 = 0x0A.
    assert_eq!(
        &MEETRIVAL_CH1_ALTERNATE_TEMPO[..7],
        &[0xED, 0x00, 0x64, 0xFE, 0x00, 0x0A, 0x00]
    );
    // Tail = normal CH1 with its sound_loop offset rebased (+7):
    // .mainloop (32) -> 39 = 0x27. CH1's only fixup is the trailing
    // sound_loop address at offset 121.
    let tail = &MEETRIVAL_CH1_ALTERNATE_TEMPO[7..];
    assert_eq!(tail.len(), MEETRIVAL_CH1.len());
    assert_eq!(&tail[..121], &MEETRIVAL_CH1[..121]);
    assert_eq!(&tail[121..], &[0x27, 0x00]);
}

#[test]
fn test_meet_rival_ch1_alternate_start() {
    // audio/music/meetrival.asm:124-140: same header as CH1 but a different
    // first measure, then `sound_loop 0, Ch1.mainloop`.
    // .mainloop is at offset 32 -> 25 + 32 = 57 = 0x39.
    assert_eq!(
        &MEETRIVAL_CH1_ALTERNATE_START[..25],
        &[
            0xED, 0x00, 0x70, // tempo 112
            0xF0, 0x77, // volume 7,7
            0xEC, 0x03, // duty_cycle 3
            0xEA, 0x06, 0x34, // vibrato 6,3,4
            0xE8, // toggle_perfect_pitch
            0xDC, 0xB3, // note_type 12,11,3
            0xE5, // octave 3
            0x20, 0xC2, // note D_,1 / rest 3
            0x20, 0xC4, // note D_,1 / rest 5
            0x91, 0x71, 0x91, // note A_,2 / G_,2 / A_,2
            0xFE, 0x00, 0x39, 0x00, // sound_loop 0, .mainloop (rebased)
        ]
    );
    let tail = &MEETRIVAL_CH1_ALTERNATE_START[25..];
    assert_eq!(tail.len(), MEETRIVAL_CH1.len());
    assert_eq!(&tail[..121], &MEETRIVAL_CH1[..121]);
    assert_eq!(&tail[121..], &[0x39, 0x00]);
}

#[test]
fn test_meet_rival_ch2_ch3_alternate_start() {
    // meetrival.asm:228-243: Ch2 prefix (22 bytes), `sound_loop 0,
    // Ch2.mainloop` (offset 29 -> 22 + 29 = 51 = 0x33).
    assert_eq!(
        &MEETRIVAL_CH2_ALTERNATE_START[..22],
        &[
            0xEC, 0x03, 0xEA, 0x0A, 0x26, 0xDC, 0xC7, 0xE5, 0x70, 0xC2, 0x70, 0xC2, 0x20, 0xC0,
            0xE4, 0x21, 0x51, 0x61, 0xFE, 0x00, 0x33, 0x00,
        ]
    );
    let tail2 = &MEETRIVAL_CH2_ALTERNATE_START[22..];
    assert_eq!(tail2.len(), MEETRIVAL_CH2.len());
    assert_eq!(&tail2[..96], &MEETRIVAL_CH2[..96]);
    assert_eq!(&tail2[96..], &[0x33, 0x00]);

    // meetrival.asm:374-389: Ch3 prefix (19 bytes), `sound_loop 0,
    // Ch3.mainloop` (offset 24 -> 19 + 24 = 43 = 0x2B).
    assert_eq!(
        &MEETRIVAL_CH3_ALTERNATE_START[..19],
        &[
            0xDC, 0x14, 0xE4, 0x70, 0xC2, 0x70, 0xC2, 0x70, 0xC0, 0x70, 0xC0, 0x70, 0xC0, 0x70,
            0xC0, 0xFE, 0x00, 0x2B, 0x00,
        ]
    );
    let tail3 = &MEETRIVAL_CH3_ALTERNATE_START[19..];
    assert_eq!(tail3.len(), MEETRIVAL_CH3.len());
    assert_eq!(&tail3[..128], &MEETRIVAL_CH3[..128]);
    assert_eq!(&tail3[128..], &[0x2B, 0x00]);
}

#[test]
fn test_meet_rival_ch1_alternate_start_and_tempo() {
    // meetrival.asm:120-122: `tempo 100; sound_loop 0,
    // Ch1_AlternateStart.body` (.body at offset 3 -> 7 + 3 = 0x0A).
    assert_eq!(
        &MEETRIVAL_CH1_ALTERNATE_START_AND_TEMPO[..7],
        &[0xED, 0x00, 0x64, 0xFE, 0x00, 0x0A, 0x00]
    );
    // Middle = the AlternateStart stream, its loop rebased into the appended
    // normal CH1: 7 + 25 + 32 = 64 = 0x40.
    let middle = &MEETRIVAL_CH1_ALTERNATE_START_AND_TEMPO[7..32];
    assert_eq!(&middle[..23], &MEETRIVAL_CH1_ALTERNATE_START[..23]);
    assert_eq!(&middle[23..], &[0x40, 0x00]);
    // Tail = normal CH1, loop rebased by 7 + 25 = 32: 32 + 32 = 64 = 0x40.
    let tail = &MEETRIVAL_CH1_ALTERNATE_START_AND_TEMPO[32..];
    assert_eq!(tail.len(), MEETRIVAL_CH1.len());
    assert_eq!(&tail[..121], &MEETRIVAL_CH1[..121]);
    assert_eq!(&tail[121..], &[0x40, 0x00]);
}

#[test]
fn test_cities1_ch1_alternate_tempo() {
    // audio/music/cities1.asm:1-3: `tempo 232; sound_loop 0, Ch1.body`
    // (.body at offset 3 -> 7 + 3 = 0x0A).
    assert_eq!(
        &CITIES1_CH1_ALTERNATE_TEMPO[..7],
        &[0xED, 0x00, 0xE8, 0xFE, 0x00, 0x0A, 0x00]
    );
    // Tail = normal CH1 with all six loop/call fixups rebased (+7).
    let tail = &CITIES1_CH1_ALTERNATE_TEMPO[7..];
    assert_eq!(tail.len(), CITIES1_CH1.len());
    // Fixup positions in CH1 (sound_call sub1 x2, sound_call sub2 x2,
    // sound_loop loop1, sound_loop mainloop) with rebased targets:
    // sub1 263 -> 270 = 0x010E, sub2 276 -> 283 = 0x011B,
    // loop1 156 -> 163 = 0x00A3, mainloop 10 -> 17 = 0x0011.
    let fixups: [(usize, [u8; 2]); 6] = [
        (42, [0x0E, 0x01]),
        (99, [0x0E, 0x01]),
        (127, [0x1B, 0x01]),
        (167, [0xA3, 0x00]),
        (196, [0x1B, 0x01]),
        (261, [0x11, 0x00]),
    ];
    let mut masked: Vec<u8> = tail.to_vec();
    let mut masked_normal: Vec<u8> = CITIES1_CH1.to_vec();
    for (pos, expected) in fixups {
        assert_eq!(&tail[pos..pos + 2], &expected, "fixup at {}", pos);
        masked[pos] = 0;
        masked[pos + 1] = 0;
        masked_normal[pos] = 0;
        masked_normal[pos + 1] = 0;
    }
    assert_eq!(masked, masked_normal);
}
