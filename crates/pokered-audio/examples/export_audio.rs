//! Export pokered's built-in music & SFX to the generic, file-based audio JSON
//! format defined in `dotzuki-audio`'s `format` module.
//!
//! This doubles as a demonstration that the generated pokered tables convert
//! losslessly into the editable format, and as a way to seed real content for
//! the dotzuki-editor audio activity.
//!
//! Usage:
//!   cargo run -p pokered-audio --example export_audio -- <out_dir>
//!
//! Produces `<out_dir>/music/<Name>.json` and `<out_dir>/sfx/<Name>.json`.

use std::fs;
use std::path::{Path, PathBuf};

use dotzuki_audio::format::{TrackDef, TrackKind, DEFAULT_TEMPO};
use dotzuki_audio::HwChannel;
use pokered_audio::music_data::MUSIC_TRACKS;
use pokered_audio::sfx_data::SFX_TRACKS;

fn raw_channels(channels: &[Option<&'static [u8]>; 4]) -> Vec<(HwChannel, &'static [u8])> {
    channels
        .iter()
        .enumerate()
        .filter_map(|(i, ch)| ch.map(|b| (HwChannel::from_u8(i as u8).unwrap(), b)))
        .collect()
}

fn write_track(dir: &Path, def: &TrackDef) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.json", def.id));
    let json = serde_json::to_string_pretty(def).unwrap();
    fs::write(path, json + "\n")
}

fn main() -> std::io::Result<()> {
    let out_dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "audio_export".to_string()),
    );
    let music_dir = out_dir.join("music");
    let sfx_dir = out_dir.join("sfx");

    let mut music_count = 0;
    for track in MUSIC_TRACKS.iter() {
        let raw = raw_channels(&track.channels);
        let def = TrackDef::from_raw_channels(
            format!("{:?}", track.id),
            TrackKind::Music,
            track.tempo,
            &raw,
        );
        write_track(&music_dir, &def)?;
        music_count += 1;
    }

    let mut sfx_count = 0;
    for track in SFX_TRACKS.iter() {
        let raw = raw_channels(&track.channels);
        if raw.is_empty() {
            continue;
        }
        let def = TrackDef::from_raw_channels(
            format!("{:?}", track.id),
            TrackKind::Sfx,
            DEFAULT_TEMPO,
            &raw,
        );
        write_track(&sfx_dir, &def)?;
        sfx_count += 1;
    }

    println!(
        "Exported {music_count} music tracks → {} and {sfx_count} SFX → {}",
        music_dir.display(),
        sfx_dir.display()
    );
    Ok(())
}
