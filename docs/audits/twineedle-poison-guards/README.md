# Twineedle poison preconditions

Twineedle could poison the real Pokemon when its second hit broke a Substitute.
It also drew a poison chance byte for targets already statused, Poison-typed,
or still protected by a doll. The original skips that draw on these paths.

Keep the authored two-hit RepeatHits operation, then dispatch the existing
side.poison_1 hook through the game's pre-roll guards. Include Twineedle in the
Poison-type/status precheck. The canonical poison probability and operations
remain data; no engine change, new per-hit allocation or copied operation list is needed.

## Original evidence

Reference: pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c.
PoisonEffect (engine/battle/effects.asm:78-118) checks Substitute, existing status,
and both types before its secondary chance. The Twineedle branch at line 968
replaces the move effect with POISON_SIDE_EFFECT1. AttackSubstitute
(core.asm:4849-4902) clears that effect when the doll breaks; the final-hit tails
at lines 3259-3269 and 5657-5665 do not dispatch a zeroed effect.

36 original ROM slices observe PoisonEffect with prepared RAM, registers, stack
and link seeds. Both sides cover raw 51/raw 52, five existing statuses, Poison in
either type slot, and Substitute. Eligible targets consume one chance byte;
blocked targets consume none. Hooks observe infliction entry, veto or return;
this is not a complete original multi-hit turn or second-hit break runtime.
An attempted full damage-sink helper observation did not return from its prepared
text/interrupt environment and is excluded from passing evidence.

## Regression and validation

Three tests execute all 48 prepared public FIGHT turns and drain presentations.
They check HP, statuses, draws, hit-count/poison/break captions, and whether the
doll remains. The parent passes the 52/256 boundary control but fails the two
guard groups. The final candidate passes all three. The existing data-shape
assertion now checks the reused canonical poison hook; behavior assertions remain.

- Full core: 3,761 passed, one existing ignored.
- App library: 393 passed, 38 existing ignored.
- Fresh desktop NEW GAME through m06: 40.9s; all 11 subsystem
  scenarios: 159.8s, source/binary identities unchanged.
- Seven GBA performance and 13 memory cases passed with unchanged thresholds,
  fixtures and timeouts. Minimum heap 4,928 B; stack 8,704 B.
- Same input and frame 427 through actual BattleVisualEffects/draw_battle:
  parent poisons Snorlax through the broken doll, candidate does not.
  Both images were inspected. Prepared combatants are used; this is not a
  full PokemonGame/audio-clock or original-renderer comparison. No new m49 claim.

Run original_oracle.py with --rom, --symbols, --state and --output pointing to
matching local assets. verification.json preserves identities, observations,
red tests and validation receipts; no ROM is distributed.
