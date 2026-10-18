# Music rests: pulse envelope and wave routing

The previous implementation treated a rest as a normal attack at frequency zero, retaining the note volume and reloading the wave. The merged [dotzuki PR102](https://github.com/liuyanghejerry/dotzuki/pull/102), audio source `f8784e127d5051bef762100882320fd56dbd463e`, emits the original rest operations: pulse envelope $08 and high byte $80; wave routing bits are cleared, without waveform loading, DAC toggling or a new frequency write. Routing stays hardware state until the next actual note. A rest uses the common next-note entry to reload vibrato delay and clear previous slide flags. Logical rest state survives clone/rebuild.

Two game regressions use normal AudioManager APIs and real DUNGEON3/SILPH_CO byte streams. Each must reach a rest on all three tonal voices within 2000 updates; all six cases have identical command positions, deadlines and cached volumes before/after, with corrected register requests and vibrato reload. Both tests fail on the previous audio source and pass on the new source. Selected native records and the diagnostic program are included. Upstream generic tests additionally cover rest sustain, the following note, paired-SFX coverage and clone/rebuild without advancing.

```sh
cargo test -p pokered-audio real_music_rest_output_tests
mkdir -p crates/pokered-audio/examples
cp docs/validation/audio-rest-1154/real_rest_probe.rs crates/pokered-audio/examples/rest_probe.rs
cargo run -p pokered-audio --example rest_probe
rm crates/pokered-audio/examples/rest_probe.rs
```

The portable original verifier executes pinned ROM instructions with artificial RAM, not an ordinary original-ROM song. Twelve controls cover music voices 0/1/2, their paired SFX active/inactive, and delay fraction 0/255. Entry flags1=$30 must clear to 0; vibrato counter 2 must reload to 7. Initial NR51=$ff: a wave rest writes $bb, pulse rests write only envelope 8/high 128, and paired rests issue no channel write. The separately logged `write_intents` are actual MMIO calls; `registers` is final memory and includes initialized NR51. The executor supports only these paths and records Z/C flags; it does not emulate physical APU power behavior, oscillators, envelopes or samples. No ROM is shipped.

```sh
python3 docs/validation/audio-rest-1154/verify_original_rest.py --rom /path/to/pinned/reference.gbc --output /tmp/original-rest-controls.json
cmp /tmp/original-rest-controls.json docs/validation/audio-rest-1154/original_rest_controls.json
```

[Original next-note entry](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/audio/engine_3.asm#L147) and [rest output](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/audio/engine_3.asm#L750) support these selected state/write results. Complete waveform/common-IRQ/FLASH parity, SFX-only tails and wave sound-ret remain separate audits. Audio-only change; screenshots are unnecessary.

Local checks: 107 audio tests, 381 application-library tests (39 ignored), and the audio no-default-features check pass. All non-audio lock entries are unchanged; the audio records reorder because the new source SHA sorts after the other pinned audio package.
