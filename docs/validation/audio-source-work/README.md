# Bounded Audio3 work accounting

The FLASH display-clock integration needs the CPU work of the existing audio
update, rather than a fixed duration or a second independently advanced player.
`AudioManager::update_frame_with_source_work` prepares a bounded model from the
live sequencer, calls the existing `update_frame` exactly once, validates the
result, and returns the supported original-instruction T-cycle count. The hosted
`AudioOutput` adapter performs the same operation under its existing manager lock.
No game caller is switched to this API by this change.

## Supported scope and limits

- Rock Tunnel (`DUNGEON3`) and Silph Co. (`SILPH_CO`) Audio3 music after its first
  ordinary native initialization tick, including finite loops, noise rests/end,
  and the finite PressAB sound with paired music-channel branches.
- Other music banks, unsupported sound effects/commands, fade and low-health
  alarm paths return an explicit error. An unsupported update still receives
  exactly one ordinary native audio tick. Unprimed native channel constructors
  remain explicitly unsupported; no original phase is inferred for them.
- The arithmetic/command work modules are allocation-compatible with `no_std`;
  the output-lock adapter is hosted only. Existing GBA update/catch-up behavior
  is unchanged. Returned work is not charged to a display clock in this PR.
- This does not resolve FLASH palette timing, GBA latency, common VBlank IRQ
  ownership, APU register write phase, or complete original audio-wave alignment.

## Validation

`source_clock/*_golden.json` contains independent original-subroutine results and
instruction durations from pret/pokered revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`; each fixture records its particular
scope. They test components, not a complete original-game replay. Production
work is derived from live channels and decoded branches, never looked up from
these JSON fixtures, which are included only in test modules.

The two full-song tests exercise 20,000 native ticks per song and compare all
eight channel states against the unchanged update path. Confirmation tests
exercise PressAB at 64 starting phases per song. The output test compares
1,200 ticks for each of the two supported tracks and unsupported Pallet Town,
including PressAB at tick 80: all PCM samples, eight channel states, and NR50
must remain identical. Unsupported results cannot cause a skipped/double tick.

Local validation on master `fbedfcc239f3dfe4d8b7185b4a26cfc5c1f53606` plus this
library change: full workspace `cargo test --workspace --tests` completed with
5,731 passed, 0 failed, and 95 existing ignored tests across 126 test targets.
The audio package has 131 passing tests and no ignored tests. The work-accounting
API has no current game caller, so these results do not claim a changed FLASH
movie or a fresh mainline playthrough.
