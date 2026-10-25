# Damage calculation before accuracy

A shared random stream could make Tackle hit when the original misses, or miss
when the original hits. The port drew accuracy before ordinary damage variance.
The original calculates critical damage, applies types and variance, then runs
MoveHitTest. This also applies when Dig/Fly invulnerability or Dream Eater's
awake-target check prevents the hit.

Prepare ordinary damage immediately after the critical test, before any
Accuracy hook. The damage stage reuses that value. Preserve charge gathering,
Bide releases, trapping continuations, status moves and separate fixed-damage,
Counter/Psywave/OHKO paths. A hit-test miss clears both prepared and shared
damage; charge strike cleanup still runs on success and miss.

Original reference: pret/pokered
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm`, PlayerCalcMoveDamage, EnemyCalcMoveDamage and MoveHitTest.

## Evidence

32 original calculation slices cover both sides: three Tackle sequences,
invulnerability, X Accuracy, Swift, immunity, Fly/Dig strike hit/miss, and Dream
Eater against awake/asleep targets. Read-only hooks observe CriticalHitTest,
CalculateDamage, type adjustment, RandomizeDamage, MoveHitTest and BattleRandom.
The setup prepares RAM, registers, stack and link seeds and stops the observation
before animation/HP application. It is not an entire original battle turn.

The initial native regression fails on the parent for ordinary ordering and
invulnerability; ten exception controls pass. A separate red test exposed the
Dream Eater veto still running before variance in the first candidate. Five final
tests cover all 32 prepared public-FIGHT turns, drain presentations, and check HP,
RNG draws, miss text, shared-damage clearing and charge/invulnerability cleanup.

Existing HP, status and presentation assertions are preserved. Explicit test
streams now put variance before accuracy, and missed damaging turns account for
the variance draw. Original 256-byte oracle values are unchanged. The long-loop
fixture advances one preparation byte to retain the original variance start index
2 and uses X Accuracy to isolate that loop; its asserted turn count excludes the
preparation byte. It now also covers raw 255.

## Validation

- Full core: 3,758 passed, one existing ignored. Only a RON comment correction
  followed this run; the exact diff is recorded. Remaining checks use that final
  source.
- App library: 393 passed, 38 existing ignored.
- Fresh desktop NEW GAME through m06: 48.1s; all 11 subsystem
  scenarios: 144.2s, with unchanged source/binary identities.
- Seven GBA benchmark and 13 memory cases passed with unchanged fixtures,
  thresholds and timeouts. Minimum heap 7,816 B and stack
  8,704 B.
- Actual battle renderer at identical input trace and frame 75;
  before shows “Enemy MEWTWO used TACKLE!” and after shows the miss caption.
  Both PNGs were inspected. Both display 999/999 HP at this frame because the
  before capture is still at its animation; no numeric HP difference is claimed
  for these PNGs. Prepared combatants and direct
  BattleVisualEffects/draw_battle are used, not a complete game/audio-clock or
  original-renderer comparison. No new m49 claim.

Reproduce original observations with matching local assets and PyBoy:

```sh
python original_oracle.py --rom original.gbc --symbols pokered.sym --state prepared.state --output observed.json
```

`verification.json` records input/source/binary identities, original outcomes,
red tests and validation receipts. Interrupted diagnostic runs and superseded
fixtures are retained locally and are not counted as passing checks.
