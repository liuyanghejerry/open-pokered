#!/usr/bin/env python3
"""
Convert pokered music .asm files to Rust byte arrays.

Parses assembly macros from audio/music/*.asm and outputs
Rust source with static byte slices for each channel of each track.
"""

import os
import re
import sys
from pathlib import Path

NOTE_NAMES = {
    "C_": 0, "C#": 1, "D_": 2, "D#": 3, "E_": 4, "F_": 5,
    "F#": 6, "G_": 7, "G#": 8, "A_": 9, "A#": 10, "B_": 11,
}

MUSIC_HEADER_FILES = [
    "audio/headers/musicheaders1.asm",
    "audio/headers/musicheaders2.asm",
    "audio/headers/musicheaders3.asm",
]


def parse_int(s):
    s = s.strip()
    if s.startswith("$"):
        return int(s[1:], 16)
    if s.startswith("0x"):
        return int(s, 16)
    if s.startswith("%"):
        return int(s[1:], 2)
    return int(s)


def parse_signed_fade(s):
    val = parse_int(s)
    if val < 0:
        return 0x08 | (-val)
    return val


def resolve_note_or_int(s):
    s = s.strip()
    if s in NOTE_NAMES:
        return NOTE_NAMES[s]
    return parse_int(s)


class ChannelAssembler:
    def __init__(self):
        self.data = bytearray()
        self.labels = {}
        self.fixups = []

    def pos(self):
        return len(self.data)

    def define_label(self, name):
        self.labels[name] = self.pos()

    def emit(self, *bts):
        for b in bts:
            self.data.append(b & 0xFF)

    def emit_label_ref(self, label):
        self.fixups.append((self.pos(), label))
        self.emit(0x00, 0x00)

    def resolve_fixups(self, external_labels=None):
        all_labels = dict(self.labels)
        if external_labels:
            all_labels.update(external_labels)
        for offset, label in self.fixups:
            if label not in all_labels:
                raise ValueError(f"Unresolved label: {label}")
            addr = all_labels[label]
            self.data[offset] = addr & 0xFF
            self.data[offset + 1] = (addr >> 8) & 0xFF

    def has_unresolved(self):
        for _, label in self.fixups:
            if label not in self.labels:
                return True
        return False

    def get_bytes(self, external_labels=None):
        self.resolve_fixups(external_labels)
        return bytes(self.data)


def parse_music_headers(pokered_root):
    tracks = {}
    for hf in MUSIC_HEADER_FILES:
        path = pokered_root / hf
        if not path.exists():
            continue
        current_track = None
        with open(path) as f:
            for line in f:
                line = line.split(";")[0].strip()
                if not line:
                    continue
                if line.endswith("::"):
                    current_track = line[:-2].strip()
                    tracks[current_track] = {"channels": []}
                    continue
                if current_track and line.startswith("channel_count"):
                    m = re.match(r"channel_count\s+(\d+)", line)
                    if m:
                        tracks[current_track]["num_channels"] = int(m.group(1))
                elif current_track and line.startswith("channel "):
                    m = re.match(r"channel\s+(\d+)\s*,\s*(\S+)", line)
                    if m:
                        ch_id = int(m.group(1))
                        ch_label = m.group(2)
                        tracks[current_track]["channels"].append((ch_id, ch_label))
    return tracks


def find_channel_file(pokered_root, ch_label):
    for asm_file in (pokered_root / "audio" / "music").glob("*.asm"):
        with open(asm_file) as f:
            content = f.read()
        if f"{ch_label}::" in content:
            return asm_file, content
    return None, None


def parse_channel_data(pokered_root, ch_label):
    asm_file, content = find_channel_file(pokered_root, ch_label)
    if content is None:
        raise ValueError(f"Could not find channel label {ch_label}")

    lines = content.split("\n")
    asm = ChannelAssembler()

    in_channel = False
    channel_base_label = ch_label

    for line in lines:
        raw = line.split(";")[0]
        stripped = raw.strip()
        if not stripped:
            continue

        if stripped == f"{ch_label}::":
            in_channel = True
            asm.define_label(ch_label)
            continue

        if not in_channel:
            continue

        if stripped.endswith("::") and stripped != f"{ch_label}::":
            break

        if stripped.startswith("."):
            if stripped.endswith(":"):
                local_label = f"{channel_base_label}{stripped[:-1]}"
                asm.define_label(local_label)
                continue

        parts = stripped.replace("\t", " ").split(None, 1)
        if not parts:
            continue
        cmd = parts[0]
        args_str = parts[1] if len(parts) > 1 else ""
        args = [a.strip() for a in args_str.split(",") if a.strip()] if args_str else []

        if cmd == "tempo":
            val = parse_int(args[0])
            asm.emit(0xED)
            asm.emit((val >> 8) & 0xFF)
            asm.emit(val & 0xFF)
        elif cmd == "volume":
            l = parse_int(args[0])
            r = parse_int(args[1])
            asm.emit(0xF0)
            asm.emit((l << 4) | r)
        elif cmd == "duty_cycle":
            asm.emit(0xEC)
            asm.emit(parse_int(args[0]))
        elif cmd == "duty_cycle_pattern":
            a, b, c, d = [parse_int(x) for x in args]
            asm.emit(0xFC)
            asm.emit((a << 6) | (b << 4) | (c << 2) | d)
        elif cmd == "note_type":
            speed = parse_int(args[0])
            vol = parse_int(args[1])
            fade = parse_signed_fade(args[2])
            asm.emit(0xD0 | speed)
            asm.emit((vol << 4) | (fade & 0x0F))
        elif cmd == "drum_speed":
            speed = parse_int(args[0])
            asm.emit(0xD0 | speed)
        elif cmd == "note":
            pitch = resolve_note_or_int(args[0])
            length = parse_int(args[1])
            asm.emit((pitch << 4) | (length - 1))
        elif cmd == "drum_note":
            instrument = parse_int(args[0])
            length = parse_int(args[1])
            asm.emit(0xB0 | (length - 1))
            asm.emit(instrument)
        elif cmd == "rest":
            length = parse_int(args[0])
            asm.emit(0xC0 | (length - 1))
        elif cmd == "octave":
            n = parse_int(args[0])
            asm.emit(0xE0 | (8 - n))
        elif cmd == "toggle_perfect_pitch":
            asm.emit(0xE8)
        elif cmd == "vibrato":
            delay = parse_int(args[0])
            depth = parse_int(args[1])
            rate = parse_int(args[2])
            asm.emit(0xEA)
            asm.emit(delay)
            asm.emit((depth << 4) | rate)
        elif cmd == "pitch_slide":
            length = parse_int(args[0])
            octave = parse_int(args[1])
            pitch = resolve_note_or_int(args[2])
            asm.emit(0xEB)
            asm.emit(length - 1)
            asm.emit(((8 - octave) << 4) | pitch)
        elif cmd == "stereo_panning":
            l = parse_int(args[0])
            r = parse_int(args[1])
            asm.emit(0xEE)
            asm.emit((l << 4) | r)
        elif cmd == "unknownmusic0xef":
            asm.emit(0xEF)
            asm.emit(parse_int(args[0]))
        elif cmd == "execute_music":
            asm.emit(0xF8)
        elif cmd == "sound_call":
            label = args[0].strip()
            if label.startswith("."):
                label = f"{channel_base_label}{label}"
            asm.emit(0xFD)
            asm.emit_label_ref(label)
        elif cmd == "sound_loop":
            count = parse_int(args[0])
            label = args[1].strip()
            if label.startswith("."):
                label = f"{channel_base_label}{label}"
            asm.emit(0xFE)
            asm.emit(count)
            asm.emit_label_ref(label)
        elif cmd == "sound_ret":
            asm.emit(0xFF)
        elif cmd == "pitch_sweep":
            length = parse_int(args[0])
            change = parse_int(args[1]) if len(args) > 1 else 0
            asm.emit(0x10)
            if change < 0:
                asm.emit((length << 4) | (0x08 | (-change)))
            else:
                asm.emit((length << 4) | change)
        elif cmd in ("square_note", "sfx_note"):
            length = parse_int(args[0])
            vol = parse_int(args[1])
            fade = parse_signed_fade(args[2])
            freq = parse_int(args[3])
            asm.emit(0x20 | length)
            asm.emit((vol << 4) | (fade & 0x0F))
            asm.emit(freq & 0xFF)
            asm.emit((freq >> 8) & 0xFF)
        elif cmd == "noise_note":
            length = parse_int(args[0])
            vol = parse_int(args[1])
            fade = parse_signed_fade(args[2])
            param = parse_int(args[3])
            asm.emit(0x20 | length)
            asm.emit((vol << 4) | (fade & 0x0F))
            asm.emit(param)
        else:
            print(f"  WARNING: Unknown command '{cmd}' in {ch_label}", file=sys.stderr)

    return asm


def to_rust_hex(data):
    parts = []
    for i, b in enumerate(data):
        if i > 0 and i % 16 == 0:
            parts.append("\n        ")
        parts.append(f"0x{b:02X}")
        if i < len(data) - 1:
            parts.append(", ")
    return "".join(parts)


def track_name_to_enum(name):
    name = name.replace("Music_", "")
    result = []
    for i, c in enumerate(name):
        if c.isupper() and i > 0 and name[i-1].islower():
            result.append("_")
        result.append(c.upper())
    s = "".join(result)
    s = s.replace("S_S_", "SS_")
    s = s.replace("PKMN_", "PKMN")
    return s


def generate_rust(pokered_root, tracks):
    out = []
    out.append("// Auto-generated by tools/asm2music.py — DO NOT EDIT")
    out.append("")
    out.append("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")
    out.append("#[repr(u8)]")
    out.append("#[allow(non_camel_case_types)]")
    out.append("pub enum MusicId {")

    track_names = list(tracks.keys())
    for i, tname in enumerate(track_names):
        enum_name = track_name_to_enum(tname)
        out.append(f"    {enum_name} = {i},")
    out.append("}")
    out.append("")

    out.append(f"pub const NUM_MUSIC_TRACKS: usize = {len(track_names)};")
    out.append("")

    out.append("impl MusicId {")
    out.append("    pub fn from_u8(val: u8) -> Option<Self> {")
    out.append(f"        if (val as usize) < NUM_MUSIC_TRACKS {{")
    out.append("            Some(unsafe { core::mem::transmute(val) })")
    out.append("        } else {")
    out.append("            None")
    out.append("        }")
    out.append("    }")
    out.append("}")
    out.append("")

    track_data_names = {}
    for tname in track_names:
        info = tracks[tname]
        channels = info["channels"]
        for ch_id, ch_label in channels:
            data = info.get(f"ch{ch_id}_data")
            if data is None:
                continue
            const_name = f"{tname.upper()}_CH{ch_id}"
            const_name = const_name.replace("MUSIC_", "")
            track_data_names.setdefault(tname, []).append((ch_id, const_name))
            out.append(f"pub static {const_name}: &[u8] = &[")
            out.append(f"    {to_rust_hex(data)}")
            out.append("];")
            out.append("")

    out.append("pub struct MusicTrack {")
    out.append("    pub id: MusicId,")
    out.append("    pub num_channels: u8,")
    out.append("    pub channels: [Option<&'static [u8]>; 4],")
    out.append("    pub tempo: u16,")
    out.append("}")
    out.append("")

    out.append("pub static MUSIC_TRACKS: [MusicTrack; NUM_MUSIC_TRACKS] = [")
    for tname in track_names:
        info = tracks[tname]
        enum_name = track_name_to_enum(tname)
        num_ch = info.get("num_channels", len(info["channels"]))
        chans = track_data_names.get(tname, [])
        ch_arr = ["None"] * 4
        for ch_id, const_name in chans:
            ch_arr[ch_id - 1] = f"Some({const_name})"
        tempo = info.get("default_tempo", 0x0100)
        out.append(f"    MusicTrack {{")
        out.append(f"        id: MusicId::{enum_name},")
        out.append(f"        num_channels: {num_ch},")
        out.append(f"        channels: [{', '.join(ch_arr)}],")
        out.append(f"        tempo: 0x{tempo:04X},")
        out.append(f"    }},")
    out.append("];")
    out.append("")

    out.append("pub fn get_music_track(id: MusicId) -> &'static MusicTrack {")
    out.append("    &MUSIC_TRACKS[id as usize]")
    out.append("}")
    out.append("")

    out.extend(generate_alternate_tempo_streams(pokered_root))

    return "\n".join(out)


# ── Alternate tempo/start override streams (audio/alternate_tempo.asm) ──────
#
# Music_RivalAlternateStart / Music_RivalAlternateTempo /
# Music_RivalAlternateStartAndTempo / Music_Cities1AlternateTempo play a song
# and then poke new command pointers into the LIVE music channels
# (Audio1_OverwriteChannelPointer, audio/alternate_tempo.asm:13-18). The
# alternate streams (audio/music/meetrival.asm, audio/music/cities1.asm) end
# in `sound_loop 0, <normal channel label>` — in the ROM those are absolute
# pointers into the normal channel's data. Our sequencer uses per-channel
# byte slices with slice-relative offsets, so each override stream is emitted
# as the alternate prefix followed by a copy of the full normal channel data,
# with every loop/call offset shifted by the combined prefix length.
ALTERNATE_STREAMS = [
    # (const name, [asm segment labels, in concatenation order])
    # Music_RivalAlternateStart (audio/alternate_tempo.asm:2-12): CH1+CH2+CH3.
    (
        "MEETRIVAL_CH1_ALTERNATE_START",
        ["Music_MeetRival_Ch1_AlternateStart", "Music_MeetRival_Ch1"],
    ),
    (
        "MEETRIVAL_CH2_ALTERNATE_START",
        ["Music_MeetRival_Ch2_AlternateStart", "Music_MeetRival_Ch2"],
    ),
    (
        "MEETRIVAL_CH3_ALTERNATE_START",
        ["Music_MeetRival_Ch3_AlternateStart", "Music_MeetRival_Ch3"],
    ),
    # Music_RivalAlternateTempo (audio/alternate_tempo.asm:21-27): CH1 only.
    (
        "MEETRIVAL_CH1_ALTERNATE_TEMPO",
        ["Music_MeetRival_Ch1_AlternateTempo", "Music_MeetRival_Ch1"],
    ),
    # Music_RivalAlternateStartAndTempo (audio/alternate_tempo.asm:30-34):
    # CH1 loops into the AlternateStart body, which loops into normal CH1.
    (
        "MEETRIVAL_CH1_ALTERNATE_START_AND_TEMPO",
        [
            "Music_MeetRival_Ch1_AlternateStartAndTempo",
            "Music_MeetRival_Ch1_AlternateStart",
            "Music_MeetRival_Ch1",
        ],
    ),
    # Music_Cities1AlternateTempo (audio/alternate_tempo.asm:37-50): CH1 only.
    (
        "CITIES1_CH1_ALTERNATE_TEMPO",
        ["Music_Cities1_Ch1_AlternateTempo", "Music_Cities1_Ch1"],
    ),
]


def build_override_stream(pokered_root, segment_labels):
    """Concatenate channel segments into one self-contained override stream.

    Each segment is parsed independently (parse_channel_data stops at the
    next top-level label). Label offsets from every segment are rebased into
    the concatenated address space, then all sound_call/sound_loop fixups are
    resolved against that map.
    """
    segments = [parse_channel_data(pokered_root, lbl) for lbl in segment_labels]

    label_addr = {}
    seg_bases = []
    offset = 0
    for asm in segments:
        seg_bases.append(offset)
        for name, pos in asm.labels.items():
            if name in label_addr:
                raise ValueError(f"Duplicate label across segments: {name}")
            label_addr[name] = offset + pos
        offset += len(asm.data)

    data = bytearray()
    for asm in segments:
        data += asm.data

    for base, asm in zip(seg_bases, segments):
        for fixup_offset, label in asm.fixups:
            if label not in label_addr:
                raise ValueError(f"Unresolved label: {label}")
            addr = label_addr[label]
            pos = base + fixup_offset
            data[pos] = addr & 0xFF
            data[pos + 1] = (addr >> 8) & 0xFF

    return bytes(data)


def generate_alternate_tempo_streams(pokered_root):
    """Emit the alternate tempo/start channel streams (audio/alternate_tempo.asm).

    These overwrite LIVE music channel pointers after PlayMusic; see
    AudioManager::play_meet_rival_alternate_start & friends.
    """
    out = []
    out.append("// ── Alternate tempo/start override streams (audio/alternate_tempo.asm) ──")
    out.append("// Each stream is the alternate prefix followed by a copy of the normal")
    out.append("// channel data, with loop offsets rebased — in the ROM the prefix's")
    out.append("// `sound_loop 0, <normal label>` points straight into the normal data.")
    for const_name, segment_labels in ALTERNATE_STREAMS:
        data = build_override_stream(pokered_root, segment_labels)
        out.append(f"pub static {const_name}: &[u8] = &[")
        out.append(f"    {to_rust_hex(data)}")
        out.append("];")
        out.append("")
    return out


def extract_default_tempo(data):
    i = 0
    while i < len(data) - 2:
        if data[i] == 0xED:
            return (data[i+1] << 8) | data[i+2]
        if data[i] >= 0x00 and data[i] <= 0xAF:
            break
        if data[i] >= 0xB0 and data[i] <= 0xBF:
            break
        i += 1
    return 0x0100


def main():
    if len(sys.argv) < 2:
        pokered_root = Path(__file__).parent.parent.parent
    else:
        pokered_root = Path(sys.argv[1])

    if not (pokered_root / "audio" / "music").exists():
        print(f"Error: {pokered_root / 'audio' / 'music'} not found", file=sys.stderr)
        sys.exit(1)

    tracks = parse_music_headers(pokered_root)
    print(f"Found {len(tracks)} music tracks", file=sys.stderr)

    skip_tracks = {"Music_UnusedSong"}

    for tname, info in tracks.items():
        if tname in skip_tracks:
            continue
        # First pass: parse all channels, collect assembler objects
        assemblers = {}
        for ch_id, ch_label in info["channels"]:
            try:
                asm = parse_channel_data(pokered_root, ch_label)
                assemblers[ch_id] = (ch_label, asm)
            except Exception as e:
                print(f"  ERROR parsing {ch_label}: {e}", file=sys.stderr)

        # Build combined label map from all channels in this track
        combined_labels = {}
        for ch_id, (ch_label, asm) in assemblers.items():
            combined_labels.update(asm.labels)

        # Second pass: resolve with combined labels and extract bytes
        for ch_id, (ch_label, asm) in assemblers.items():
            try:
                data = asm.get_bytes(external_labels=combined_labels)
                info[f"ch{ch_id}_data"] = data
                print(f"  {ch_label}: {len(data)} bytes", file=sys.stderr)
            except Exception as e:
                print(f"  ERROR resolving {ch_label}: {e}", file=sys.stderr)
                info[f"ch{ch_id}_data"] = bytes([0xFF])

        ch1_data = info.get("ch1_data", b"")
        info["default_tempo"] = extract_default_tempo(ch1_data)

    filtered = {k: v for k, v in tracks.items() if k not in skip_tracks}
    rust_code = generate_rust(pokered_root, filtered)

    # Output goes to this repo's workspace (the asm root given on the command
    # line is only the data source).
    out_path = (
        Path(__file__).resolve().parent.parent
        / "examples/pokered/crates/pokered-audio/src/music_data.rs"
    )
    with open(out_path, "w") as f:
        f.write(rust_code)
    print(f"\nWrote {out_path}", file=sys.stderr)


if __name__ == "__main__":
    main()
