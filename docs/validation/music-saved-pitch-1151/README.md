# Music saved pitch during a paired sound effect

Original source: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, Audio3_note_pitch and its `.noSfx` branch. While a paired sound effect owns a voice, music continues parsing notes but returns before writing its saved frequency-low byte. The native sequencer previously replaced that byte, changing the later modulation base.

The normal and development audio dependency pins select dotzuki `db60028d5bcde1433e0fafd8ae9e560a0f7bd01f` (upstream PR100). Game regression tests use unmodified DUNGEON3 and SILPH_CO music and the ordinary PressAB sound constructor. Each tries 64 early-song insertion points for 18 updates and requires crossing an actual new pitched note under the sound effect. Both fail on the previous dependency and pass on the candidate. These cover two songs and selected state, not all songs, waveforms or a complete original-ROM playthrough.

The portable verifier executes pinned original instructions with constructed RAM. Its 18 music controls include paired and unpaired channels, notes and rests, and fractional delays. The saved byte starts at77; paired channel0 retains77, while unpaired channels can write their note pitch. Twenty pulse-SFX controls are also retained because the same verifier produces both files. It records register write intents in memory, not hardware APU behavior. No ROM is shipped.

```sh
python3 docs/validation/music-saved-pitch-1151/verify_original.py --rom /path/to/reference.gbc --output-dir /tmp/music-saved-pitch-controls
cmp /tmp/music-saved-pitch-controls/paired_music_work_golden.json docs/validation/music-saved-pitch-1151/paired_music_work_golden.json
cmp /tmp/music-saved-pitch-controls/press_ab_work_golden.json docs/validation/music-saved-pitch-1151/press_ab_work_golden.json
```

Local validation: 103 audio tests, 381 application library tests (39 ignored), audio no-default-features check. Audio-only change; no rendered output or screenshot change.
