# Confirmation-sound output ownership

PressAB ends while a covered music note still has time remaining. Previously the retained music trigger caused another five-register pulse attack at release; it now emits no pulse register writes until an actual music command/effect produces them. Music parsing and deadlines continue during the sound effect.

The normal and development audio pins select merged upstream [dotzuki PR101](https://github.com/liuyanghejerry/dotzuki/pull/101), source `5c7f7cd5ed1c21e40cff6c142135aaffb7ffefca`. Upstream sink regressions cover both pulse voices and wave, release, unchanged sustain, partial vibrato, next-note attack, and two-byte slide updates. All three regressions fail before the fix; 176 default and 201 all-feature audio tests pass after it.

Game tests use real DUNGEON3 and SILPH_CO streams and the normal PressAB constructor. Both fail on the previous game audio pin and pass here. Each enters a covered sustain whose note/vibrato delays are positive when PressAB ends, so the original paired branch cannot have produced a music register write. The selected before/after records are in `register_cases.json`; the diagnostic source is included.

```sh
cargo test -p pokered-audio confirmation_output_owner_tests
mkdir -p crates/pokered-audio/examples
cp docs/validation/audio-output-1153/real_song_probe.rs crates/pokered-audio/examples/audio_output_probe.rs
cargo run -p pokered-audio --example audio_output_probe
rm crates/pokered-audio/examples/audio_output_probe.rs
```

The original instruction controls and verifier are retained in [PR320's merged evidence](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/4aa07af12283645e8d618ea8dfe551727975d84a/docs/validation/music-saved-pitch-1151/README.md). Source references: [paired music gate](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/audio/engine_3.asm#L31), [SFX return](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/audio/engine_3.asm#L163), [two-byte slide output](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/audio/engine_3.asm#L1119).

These are selected state/write tests and synthetic original controls, not full waveform, common-interrupt, FLASH or whole-game parity. SFX-only envelope tails, rest/panning and wave-return behavior remain separate audit targets. Audio-only change; rendered output is unchanged.

Local checks: 105 audio tests, 381 application-library tests (39 ignored), and the audio no-default-features check pass.
