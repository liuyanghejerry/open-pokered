//! Round-trip validation of the generic `jrpg-audio` file-based audio format
//! against the full corpus of real pokered data (45 music tracks + 161 SFX).
//!
//! If the byte-code encoder is a faithful inverse of the decoder, then
//! `encode_channel(decode_channel(bytes)) == bytes` for every channel of every
//! track. This is a far stronger test than hand-written unit cases — it covers
//! every command shape that actually occurs in the original game's audio.

use jrpg_audio::commands::{decode_channel, encode_channel};
use jrpg_audio::format::{TrackDef, TrackKind};
use jrpg_audio::HwChannel;

use crate::music_data::MUSIC_TRACKS;
use crate::sfx_data::SFX_TRACKS;

/// Every music channel byte stream survives decode → encode unchanged.
#[test]
fn all_music_channels_roundtrip_bytes() {
    for track in MUSIC_TRACKS.iter() {
        for (hw_idx, ch) in track.channels.iter().enumerate() {
            let Some(bytes) = ch else { continue };
            let is_noise = hw_idx == 3;
            let cmds = decode_channel(bytes, is_noise, false);
            let reencoded = encode_channel(&cmds, is_noise);
            assert_eq!(
                &reencoded,
                bytes,
                "music {:?} channel {hw_idx} did not round-trip ({} cmds)",
                track.id,
                cmds.len()
            );
        }
    }
}

/// Every SFX channel byte stream survives decode → encode unchanged.
#[test]
fn all_sfx_channels_roundtrip_bytes() {
    for track in SFX_TRACKS.iter() {
        for (hw_idx, ch) in track.channels.iter().enumerate() {
            let Some(bytes) = ch else { continue };
            let is_noise = hw_idx == 3;
            let cmds = decode_channel(bytes, is_noise, true);
            let reencoded = encode_channel(&cmds, is_noise);
            assert_eq!(
                &reencoded,
                bytes,
                "sfx {:?} channel {hw_idx} did not round-trip ({} cmds)",
                track.id,
                cmds.len()
            );
        }
    }
}

/// A full `TrackDef` reconstructed from the generated music table reproduces the
/// original channel byte streams (in hw order) and survives a JSON round-trip.
#[test]
fn music_trackdef_reproduces_channels_and_json() {
    for track in MUSIC_TRACKS.iter() {
        let raw: Vec<(HwChannel, &[u8])> = track
            .channels
            .iter()
            .enumerate()
            .filter_map(|(i, ch)| ch.map(|b| (HwChannel::from_u8(i as u8).unwrap(), b)))
            .collect();

        let def = TrackDef::from_raw_channels(
            format!("{:?}", track.id),
            TrackKind::Music,
            track.tempo,
            &raw,
        );

        // to_music_channels rebuilds exactly the original contiguous streams.
        let rebuilt = def.to_music_channels();
        let expected: Vec<&[u8]> = raw.iter().map(|(_, b)| *b).collect();
        assert_eq!(rebuilt.len(), expected.len(), "channel count for {:?}", track.id);
        for (i, (got, want)) in rebuilt.iter().zip(expected.iter()).enumerate() {
            assert_eq!(got.as_slice(), *want, "music {:?} rebuilt channel {i}", track.id);
        }

        // JSON serialize → deserialize is lossless.
        let json = serde_json::to_string(&def).unwrap();
        let back: TrackDef = serde_json::from_str(&json).unwrap();
        assert_eq!(back, def, "music {:?} JSON round-trip", track.id);
    }
}

/// SFX tracks (including non-contiguous, noise-only ones) reconstruct their
/// channel streams and correct `start_channel`, and survive JSON round-trip.
#[test]
fn sfx_trackdef_reproduces_channels_and_json() {
    for track in SFX_TRACKS.iter() {
        let raw: Vec<(HwChannel, &[u8])> = track
            .channels
            .iter()
            .enumerate()
            .filter_map(|(i, ch)| ch.map(|b| (HwChannel::from_u8(i as u8).unwrap(), b)))
            .collect();
        if raw.is_empty() {
            continue;
        }

        let def = TrackDef::from_raw_channels(
            format!("{:?}", track.id),
            TrackKind::Sfx,
            jrpg_audio::format::DEFAULT_TEMPO,
            &raw,
        );

        // Mirror AudioManager::play_sfx: start_channel = first used hw index,
        // channel data compacted in hw order.
        let expected_start = raw.iter().map(|(hw, _)| *hw as usize).min().unwrap();
        let (data, start) = def.to_sfx_channels();
        assert_eq!(start, expected_start, "sfx {:?} start channel", track.id);
        let expected: Vec<&[u8]> = raw.iter().map(|(_, b)| *b).collect();
        assert_eq!(data.len(), expected.len(), "sfx {:?} channel count", track.id);
        for (i, (got, want)) in data.iter().zip(expected.iter()).enumerate() {
            assert_eq!(got.as_slice(), *want, "sfx {:?} rebuilt channel {i}", track.id);
        }

        let json = serde_json::to_string(&def).unwrap();
        let back: TrackDef = serde_json::from_str(&json).unwrap();
        assert_eq!(back, def, "sfx {:?} JSON round-trip", track.id);
    }
}
