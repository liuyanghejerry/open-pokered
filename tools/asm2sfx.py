#!/usr/bin/env python3
"""
Convert pokered SFX .asm files to Rust byte arrays.

Parses assembly macros from audio/sfx/*.asm and outputs
Rust source with static byte slices for each channel of each SFX.
"""

import os
import re
import sys
from pathlib import Path

NOTE_NAMES = {
    "C_": 0, "C#": 1, "D_": 2, "D#": 3, "E_": 4, "F_": 5,
    "F#": 6, "G_": 7, "G#": 8, "A_": 9, "A#": 10, "B_": 11,
}

SFX_HEADER_FILES = [
    "audio/headers/sfxheaders1.asm",
    "audio/headers/sfxheaders2.asm",
    "audio/headers/sfxheaders3.asm",
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

    def get_bytes(self, external_labels=None):
        self.resolve_fixups(external_labels)
        return bytes(self.data)


def parse_sfx_headers(pokered_root):
    """Parse all 3 SFX header files and build a deduplicated SFX list."""
    all_sfx = {}  # canonical_name -> {channels: [(ch_id, ch_label)], bank: int}
    
    for bank_idx, hf in enumerate(SFX_HEADER_FILES, start=1):
        path = pokered_root / hf
        if not path.exists():
            continue
        current_sfx = None
        with open(path) as f:
            for line in f:
                line = line.split(";")[0].strip()
                if not line:
                    continue
                # Skip the SFX_Headers_N:: line and padding
                if line.startswith("SFX_Headers_") and line.endswith("::"):
                    continue
                if line.startswith("db "):
                    continue
                if line.endswith("::"):
                    label = line[:-2].strip()
                    current_sfx = label
                    if current_sfx not in all_sfx:
                        all_sfx[current_sfx] = {"channels": [], "bank": bank_idx, "label": label}
                    continue
                if current_sfx and line.startswith("channel_count"):
                    m = re.match(r"channel_count\s+(\d+)", line)
                    if m and "num_channels" not in all_sfx[current_sfx]:
                        all_sfx[current_sfx]["num_channels"] = int(m.group(1))
                elif current_sfx and line.startswith("channel "):
                    m = re.match(r"channel\s+(\d+)\s*,\s*(\S+)", line)
                    if m and not all_sfx[current_sfx]["channels"]:
                        ch_id = int(m.group(1))
                        ch_label = m.group(2)
                        all_sfx[current_sfx]["channels"].append((ch_id, ch_label))
                    elif m and all_sfx[current_sfx]["channels"]:
                        # Already have channels from first encounter; only add if this is the same header
                        # (multi-channel SFX have multiple channel lines)
                        ch_id = int(m.group(1))
                        ch_label = m.group(2)
                        # Check if we already have enough channels
                        num_ch = all_sfx[current_sfx].get("num_channels", 0)
                        if len(all_sfx[current_sfx]["channels"]) < num_ch:
                            all_sfx[current_sfx]["channels"].append((ch_id, ch_label))
    
    return all_sfx


def find_sfx_channel_file(pokered_root, ch_label):
    """Find the .asm file containing the given SFX channel label."""
    sfx_dir = pokered_root / "audio" / "sfx"
    for asm_file in sfx_dir.glob("*.asm"):
        with open(asm_file) as f:
            content = f.read()
        # SFX labels use single colon (not double)
        if f"{ch_label}:" in content:
            return asm_file, content
    return None, None


def parse_sfx_channel_data(pokered_root, ch_label, fall_through=False):
    """Parse a single SFX channel's data from its .asm file.

    With fall_through=True, parsing continues past the next top-level label
    instead of stopping — used for SFX_Pokeflute_Ch5, whose ROM stream begins
    with `tempo 256` and then falls through into SFX_Pokeflute_Ch6's data
    (audio/sfx/pokeflute_ch5_ch6.asm).
    """
    asm_file, content = find_sfx_channel_file(pokered_root, ch_label)
    if content is None:
        raise ValueError(f"Could not find SFX channel label {ch_label}")

    lines = content.split("\n")
    asm = ChannelAssembler()

    in_channel = False
    channel_base_label = ch_label

    for line in lines:
        raw = line.split(";")[0]
        stripped = raw.strip()
        if not stripped:
            continue

        # SFX labels use single colon
        if stripped == f"{ch_label}:" or stripped == f"{ch_label}::":
            in_channel = True
            asm.define_label(ch_label)
            continue

        if not in_channel:
            continue

        # Stop at next top-level label (either : or ::)
        if re.match(r'^[A-Z_][A-Za-z0-9_]*:{1,2}\s*$', stripped):
            if fall_through:
                continue
            break

        # Handle local labels (.label:)
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


def sfx_name_to_enum(label):
    """Convert SFX label like 'SFX_Ball_Toss' to Rust enum 'BallToss'."""
    name = label
    # Strip SFX_ prefix
    if name.startswith("SFX_"):
        name = name[4:]
    # Strip bank suffix _1, _2, _3
    if re.match(r'.*_[123]$', name):
        name = name[:-2]
    # Convert underscores to nothing (PascalCase)
    parts = name.split("_")
    result = "".join(p.capitalize() if p.islower() else p for p in parts)
    if result and result[0].isdigit():
        result = "Sfx" + result
    return result


def sfx_name_to_const(label):
    """Convert SFX label to SCREAMING_SNAKE_CASE for const names."""
    name = label
    if name.startswith("SFX_"):
        name = name[4:]
    # Strip bank suffix
    if re.match(r'.*_[123]$', name):
        name = name[:-2]
    return name.upper()


def canonical_sfx_name(label):
    """Get canonical name without bank suffix."""
    name = label
    # Strip _1, _2, _3 suffix but be careful not to strip things like _1E, _1B etc.
    # Only strip if it's exactly _1, _2, or _3 at the end
    # Check: is the part after last _ a single digit 1, 2, or 3?
    if re.match(r'^.*_[123]$', name):
        # But we need to be careful: SFX_59_1 should strip to SFX_59, not SFX_5
        # The suffix is the last _N where N is 1,2,3
        name = name[:-2]
    return name


def deduplicate_sfx(all_sfx):
    """Group SFX by canonical name, preferring bank 1 data."""
    canonical = {}
    for label, info in all_sfx.items():
        cname = canonical_sfx_name(label)
        if cname not in canonical:
            canonical[cname] = info.copy()
            canonical[cname]["original_label"] = label
    return canonical


def generate_rust(pokered_root, sfx_list):
    """Generate Rust source file with SFX data."""
    out = []
    out.append("// Auto-generated by tools/asm2sfx.py — DO NOT EDIT")
    out.append("")
    out.append("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")
    out.append("#[repr(u8)]")
    out.append("#[allow(non_camel_case_types)]")
    out.append("pub enum SfxId {")

    sfx_names = list(sfx_list.keys())
    for i, cname in enumerate(sfx_names):
        enum_name = sfx_name_to_enum(cname)
        out.append(f"    {enum_name} = {i},")
    out.append("}")
    out.append("")

    out.append(f"pub const NUM_SFX: usize = {len(sfx_names)};")
    out.append("")

    out.append("impl SfxId {")
    out.append("    pub fn from_u8(val: u8) -> Option<Self> {")
    out.append(f"        if (val as usize) < NUM_SFX {{")
    out.append("            Some(unsafe { core::mem::transmute(val) })")
    out.append("        } else {")
    out.append("            None")
    out.append("        }")
    out.append("    }")
    out.append("}")
    out.append("")

    # Emit static byte arrays for each channel
    sfx_data_names = {}
    for cname in sfx_names:
        info = sfx_list[cname]
        channels = info["channels"]
        for ch_id, ch_label in channels:
            data = info.get(f"ch{ch_id}_data")
            if data is None:
                continue
            const_name = f"SFX_{sfx_name_to_const(cname)}_CH{ch_id}"
            sfx_data_names.setdefault(cname, []).append((ch_id, const_name))
            out.append(f"pub static {const_name}: &[u8] = &[")
            out.append(f"    {to_rust_hex(data)}")
            out.append("];")
            out.append("")

    # SfxTrack struct
    out.append("pub struct SfxTrack {")
    out.append("    pub id: SfxId,")
    out.append("    pub num_channels: u8,")
    out.append("    /// Channel assignments: index is 0-3 for hw channels (pulse1, pulse2, wave, noise).")
    out.append("    /// SFX channels 5-8 map to hw channels 0-3.")
    out.append("    pub channels: [Option<&'static [u8]>; 4],")
    out.append("}")
    out.append("")

    # SFX_TRACKS array
    out.append("pub static SFX_TRACKS: [SfxTrack; NUM_SFX] = [")
    for cname in sfx_names:
        info = sfx_list[cname]
        enum_name = sfx_name_to_enum(cname)
        num_ch = info.get("num_channels", len(info["channels"]))
        chans = sfx_data_names.get(cname, [])
        ch_arr = ["None"] * 4
        for ch_id, const_name in chans:
            # Map SFX channels 5-8 to indices 0-3; channel 3 maps to index 2 (wave)
            if ch_id >= 5:
                hw_idx = ch_id - 5
            else:
                hw_idx = ch_id - 1  # For SFX_Pokeflute which uses channel 3
            if 0 <= hw_idx < 4:
                ch_arr[hw_idx] = f"Some({const_name})"
        out.append(f"    SfxTrack {{")
        out.append(f"        id: SfxId::{enum_name},")
        out.append(f"        num_channels: {num_ch},")
        out.append(f"        channels: [{', '.join(ch_arr)}],")
        out.append(f"    }},")
    out.append("];")
    out.append("")

    out.append("pub fn get_sfx_track(id: SfxId) -> &'static SfxTrack {")
    out.append("    &SFX_TRACKS[id as usize]")
    out.append("}")
    out.append("")

    out.extend(generate_pokeflute_in_battle(pokered_root))

    return "\n".join(out)


def generate_pokeflute_in_battle(pokered_root):
    """Emit the in-battle POKé FLUTE streams (audio/poke_flute.asm).

    Music_PokeFluteInBattle plays SFX_CAUGHT_MON, then overwrites the live
    command pointers of SFX channels 5/6/7 with SFX_Pokeflute_Ch5/6/7
    (audio/sfx/pokeflute_ch5_ch6.asm, audio/sfx/pokeflute_ch7.asm). These are
    not referenced by any SFX header, so they are converted separately here.
    CH5's stream begins with `tempo 256` and falls through into CH6's data,
    exactly as it does in the ROM.
    """
    ch5 = parse_sfx_channel_data(
        pokered_root, "SFX_Pokeflute_Ch5", fall_through=True
    ).get_bytes()
    ch6 = parse_sfx_channel_data(pokered_root, "SFX_Pokeflute_Ch6").get_bytes()
    ch7 = parse_sfx_channel_data(pokered_root, "SFX_Pokeflute_Ch7").get_bytes()

    out = []
    out.append("// ── In-battle POKé FLUTE channel streams (audio/poke_flute.asm) ───────")
    out.append("// Music_PokeFluteInBattle overwrites the live command pointers of SFX")
    out.append("// channels 5-7 with these streams; see AudioManager::play_flute_in_battle.")
    for name, data in (("CH5", ch5), ("CH6", ch6), ("CH7", ch7)):
        out.append(f"pub static POKEFLUTE_IN_BATTLE_{name}: &[u8] = &[")
        out.append(f"    {to_rust_hex(data)}")
        out.append("];")
        out.append("")
    return out


def main():
    if len(sys.argv) < 2:
        pokered_root = Path(__file__).parent.parent.parent
    else:
        pokered_root = Path(sys.argv[1])

    if not (pokered_root / "audio" / "sfx").exists():
        print(f"Error: {pokered_root / 'audio' / 'sfx'} not found", file=sys.stderr)
        sys.exit(1)

    # Parse headers
    all_sfx = parse_sfx_headers(pokered_root)
    print(f"Found {len(all_sfx)} SFX entries across all headers", file=sys.stderr)

    # Deduplicate (bank 1/2/3 copies)
    sfx_list = deduplicate_sfx(all_sfx)
    print(f"After dedup: {len(sfx_list)} unique SFX", file=sys.stderr)

    errors = 0
    for cname, info in sfx_list.items():
        channels = info["channels"]
        # First pass: parse all channels
        assemblers = {}
        for ch_id, ch_label in channels:
            try:
                asm = parse_sfx_channel_data(pokered_root, ch_label)
                assemblers[ch_id] = (ch_label, asm)
            except Exception as e:
                print(f"  ERROR parsing {ch_label}: {e}", file=sys.stderr)
                errors += 1

        # Build combined label map
        combined_labels = {}
        for ch_id, (ch_label, asm) in assemblers.items():
            combined_labels.update(asm.labels)

        # Second pass: resolve and extract bytes
        for ch_id, (ch_label, asm) in assemblers.items():
            try:
                data = asm.get_bytes(external_labels=combined_labels)
                # In the original ROM, channels without an explicit sound_ret
                # fall through to the next channel's sound_ret byte.
                # Since we store each channel separately, we must append 0xFF
                # if the channel doesn't end with sound_ret (0xFF) or sound_loop (0xFE).
                if data and data[-1] not in (0xFF, 0xFE):
                    data = data + bytes([0xFF])
                    print(f"  {ch_label}: {len(data)} bytes (appended sound_ret)", file=sys.stderr)
                else:
                    print(f"  {ch_label}: {len(data)} bytes", file=sys.stderr)
                info[f"ch{ch_id}_data"] = data
            except Exception as e:
                print(f"  ERROR resolving {ch_label}: {e}", file=sys.stderr)
                info[f"ch{ch_id}_data"] = bytes([0xFF])
                errors += 1

    print(f"\nTotal errors: {errors}", file=sys.stderr)

    rust_code = generate_rust(pokered_root, sfx_list)

    # Output goes to this repo's workspace (the asm root given on the command
    # line is only the data source).
    out_path = (
        Path(__file__).resolve().parent.parent
        / "crates/pokered-audio/src/sfx_data.rs"
    )
    with open(out_path, "w") as f:
        f.write(rust_code)
    print(f"Wrote {out_path}", file=sys.stderr)


if __name__ == "__main__":
    main()
