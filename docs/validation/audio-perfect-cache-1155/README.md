# Perfect-pitch saved modulation base

The tuning command already adjusts the hardware onset by one, but the previous sequencer saved the unadjusted low byte for later modulation. [Merged dotzuki PR103](https://github.com/liuyanghejerry/dotzuki/pull/103), audio source `245ca8f3fe9737571e72b8a683cd966e6fa84252`, stores the emitted low byte after tuning. Slide initialization still uses the untuned origin; a paired one-shot continues to hold the previous music base. Note timing and hardware onset are unchanged.

Two real-song regressions require at least three adjusted onsets and subsequent vibrato in DUNGEON3/SILPH_CO. Both fail on the old audio source. The included eight native records show six unchanged onset writes/deadlines with corrected saved bytes, and two subsequent modulation writes changed by one. The upstream sink test covers 576 tonal onsets: three voices, twelve notes, eight encoded octaves, and tuning on/off; a separate test covers slides and paired output gating. Default 181/all-feature 206 audio tests pass upstream.

```sh
cargo test -p pokered-audio real_perfect_pitch_base_tests
mkdir -p crates/pokered-audio/examples
cp docs/validation/audio-perfect-cache-1155/real_perfect_probe.rs crates/pokered-audio/examples/perfect_probe.rs
cargo run -p pokered-audio --example perfect_probe
rm crates/pokered-audio/examples/perfect_probe.rs
```

The portable verifier executes the pinned original ROM with constructed RAM. Its 144 controls cover tonal voices 0/1/2, tuning on/off, corresponding SFX active/inactive, selected notes 0/4/11 and encoded octaves 0/3/6/7. Uncovered notes must save their actual low-byte onset write; covered notes retain 77 and issue no note write. This is a register/state control, not an ordinary original-ROM song recording or APU oscillator/envelope/sample emulation. No ROM is included.

```sh
python3 docs/validation/audio-perfect-cache-1155/verify_original.py --rom /path/to/pinned/reference.gbc --output /tmp/perfect-controls.json
cmp /tmp/perfect-controls.json docs/validation/audio-perfect-cache-1155/original_controls.json
```

[Original tune and saved-byte store](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/audio/engine_3.asm#L825) supports the ordering. Complete waveform, source IRQ ownership, FLASH timing and wave sound-ret remain separate audits. Audio-only change, no screenshot change.

Local validation: 109 audio tests, 381 application-library tests (39 ignored), and the audio no-default-features check pass. Lock-file semantics match exactly after substituting only this audio source and its declared dependency edge (including the unchanged other pinned audio package).
