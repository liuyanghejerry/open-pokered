# Rotate the critical-hit random byte like the original

The native battle handler compared the raw random byte with the critical-hit
threshold. Original `CriticalHitTest` first executes three `RLC A` instructions.
For Mewtwo's ordinary threshold of 65, raw 16 must not be critical, while raw
130 must be critical. The old handler produced the opposite results.

The handler now rotates the byte left by three before comparing it. Thresholds
and the number of random draws are unchanged.

Original reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm:4478–4546` (`CriticalHitTest`).

## Verification

- `original_oracle.py` executes the real ROM subroutine in PyBoy with prepared
  Mewtwo/Tackle operands and the shared link seed list. Read-only hooks observe
  calls and results. All 256 raw bytes were measured on both sides, plus two
  zero-power controls: 514 original cases. The producer does not implement a
  replacement critical-hit formula. Its flags match the earlier independent
  run, and each side has 65 critical outcomes.
- One native public-FIGHT regression resolves and drains 514 prepared turns:
  the same 512 byte/side cases plus two RNG controls. The identical final test
  fails on the parent at raw byte 9 and passes on the repair.
- Full core: 3,728 passed, one existing ignored. Fresh native NEW GAME through
  m06 and all 11 subsystem scenarios passed with unchanged source/binary identity.
- Seven GBA benchmark cases and 13 memory cases passed with unchanged fixtures,
  limits and timeouts. Minimum free heap: 7,832 B; stack margin: 8,704 B.

`capture.rs` uses prepared parties, raw byte 130, public FIGHT and the actual
`BattleVisualEffects`/`draw_battle` renderer. Both versions capture frame 139:
the parent has returned to the menu with target HP 446; the candidate shows
`Critical hit!` with target HP 444. Both began at 450 HP. The PNGs were visually
inspected; `verification.json` records hashes, result flags and traces.

Run the original producer with matching assets and PyBoy installed:

```sh
python original_oracle.py --rom RED_ROM --symbols POKERED_SYM --state BOOTED_STATE --output result.json
```

The original measurement executes an isolated subroutine with prepared inputs,
not a full original battle. The native capture does not run the full game/audio
clock, and these fixtures are not a new m49 playthrough. The public-FIGHT seed
layout still follows the current accuracy/damage phase order. Damage-byte
rotation and the overall RNG pipeline remain separate confirmed findings.
