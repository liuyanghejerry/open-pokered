# Music modulation during a paired sound effect

Original `audio/engine_3.asm` (`Audio3_ApplyMusicAffects`) decrements a music channel's note delay, checks the corresponding SFX sound ID, and returns before duty/pitch-slide/vibrato effects if that SFX channel is occupied. The native sequencer was advancing those effects underneath the SFX.

The shared fix is dotzuki PR #99, exact source `547e3e6aa7136aeb527e70534168bf01d48a7464`, merged as `61f753fbd2e13ace2650e649b4d39441d248f3b6` after all 12 original CI checks passed. Both runtime and dev audio dependencies use this source. All non-audio package identities in the lockfile are unchanged.

The two game regressions use actual DUNGEON3 and SILPH_CO streams. After ordinary updates reach a delayed vibrato note, ordinary PressAB playback must continue the note countdown while holding the vibrato delay. Both failed on the previous pin (8 became 7) and pass with the shared fix. No runtime state setters or synthetic replacement streams are used in those two tests.

`verify_original.py` independently executes a restricted subset of original ROM instructions for 24 constructed controls: four music channels, note delays 2/13/255, paired SFX on/off. The vibrato delay starts at 5 and the extent is 0x22. Note delay always decrements; vibrato delay stays 5 under the paired SFX and becomes 4 otherwise. Unsupported instructions fail explicitly. These are synthetic RAM controls; they are not an ordinary original-ROM scene or waveform comparison.

Original source pin: `pret/pokered` commit `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`. ROM SHA-256: `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`. Only the required symbol addresses and the resulting numeric instruction traces are included; no ROM is shipped.

```sh
python3 docs/validation/music-modulation-sfx-1148/verify_original.py \
  --rom /path/to/pinned/reference.gbc --output-file /tmp/recreated.json
cmp /tmp/recreated.json docs/validation/music-modulation-sfx-1148/original-controls.json
```

This audio-only change does not establish full sound/animation parity. FLASH shared-IRQ timing remains separate unfinished work.
