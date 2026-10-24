# Secondary effects cannot affect an immune target

Body Slam and Thunderbolt could paralyze an immune target despite reporting
`It doesn't affect…`; Low Kick could make that target flinch and lose its turn.
The damage remained zero, but the secondary chance gate still ran.

Ordinary secondary effects now stop before their chance draw when the attack
has missed, including type immunity. Successful hits retain their existing
chance, knockout, Substitute, status and type checks.

Original reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm:3218–3227,5615–5624`: the missed-move path returns before
secondary-effect dispatch.

## Verification

- Three public-FIGHT tests resolve and drain 126 prepared turns. Twelve moves
  cover both sides, immune targets with/without Substitute, ordinary misses,
  RNG controls, and positive paralysis/flinch cases. Immune targets stay healthy
  and execute their selected Splash.
- The identical final tests give two passes/one failure on the parent; all
  three pass on the candidate. Full core: 3,727 passed, one existing ignored.
- Fresh native NEW GAME through m06 and all 11 subsystem scenarios passed with
  source and executable identities unchanged.
- Seven GBA benchmark cases and 13 memory cases passed with unchanged fixtures,
  limits and timeouts. Minimum free heap: 7,816 B; stack margin: 8,704 B.

`capture.rs` uses prepared parties, deterministic RNG, public FIGHT and the
actual `BattleVisualEffects`/`draw_battle` renderer. Both versions capture frame
208 after Body Slam against a Ghost-type target. The parent HUD incorrectly
shows `PAR` and its longer result sequence is still running; the candidate is
healthy and has returned to the menu. Both retain all 450 target HP. Both PNGs
were visually inspected; `verification.json` records traces and identities.

The capture does not test the full game/audio clock or the original ROM, and
these fixtures are not a new full m49 playthrough. RNG tests use an immune
ordinary attack as a relative control; they do not certify the complete
original RNG pipeline. Critical/damage byte rotations and phase order,
single-hit Rage and per-hit narration remain separate findings.
