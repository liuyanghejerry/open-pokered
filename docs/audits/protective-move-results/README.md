# Protective move guards and results

Reflect, Light Screen, Mist and Focus Energy checked the opponent's existing
effect while applying the effect to the user. Thus the opponent's Reflect could
prevent the user's own Reflect from taking effect. Their success and duplicate-
use failure captions were also absent, and duplicates played a success animation.

Bind these four authored hooks to the user for their existing guard and effect.
Record the result using the existing per-action presentation receipt, so later
Haze cannot change an earlier result. Print the original success caption or
`But, it failed!`; duplicate failures suppress the move animation.
The original Focus Energy critical-rate bug and NPC PP behavior remain intact.
The guard reuses authored operations and the existing action receipt; there
are no engine changes or additional effect records.

## Original evidence

Reference: pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c,
`engine/battle/move_effects/reflect_light_screen.asm`, `mist.asm`, and
`focus_energy.asm`. Each checks its user's flag. If absent it sets the flag,
plays the move animation and prints its result; if present it prints failure.
The literal captions are in `data/text/text_3.asm:204-205,244-247,298-312`.

32 original ROM slices cover four effects, both sides, and absent/already-active/
unrelated/opponent-active flags. Read-only hooks observe the flag at animation
entry or the duplicate veto. All consume zero random bytes. These observations
do not execute complete original turns, animation playback, text timing or Haze.

## Regression and validation

Four integration tests execute 72 prepared public FIGHT turns and drain text:
first/repeated use in both orders (32), opponent already protected (16), Haze
before/after the action (16), and frozen-user controls (8). They check flags,
captions, action order, animation miss markers, unchanged HP/random draws and
the player's PP charge. EnemyCanExecuteMove omits DecrementPP in the original.
The final parent run fails three groups and passes the frozen control; the
candidate passes all four. Earlier fixture diagnostics are not counted.

- Full core: 3,776 passed, one existing ignored.
- App library: 393 passed, 38 existing ignored.
- Fresh desktop NEW GAME through m06: 49.9s; all 11 subsystem
  scenarios: 140.6s, source/binary identities unchanged.
- Seven GBA performance and 13 memory cases passed with unchanged thresholds,
  fixtures and timeouts. Minimum heap 4,928 B; stack 8,768 B.
- Same input and frame 187 through actual BattleVisualEffects/draw_battle:
  the target already has Reflect; the candidate now announces WARD gained armor.
  The parent has no success result. Both images inspected.

Prepared combatants are used; this is not a full PokemonGame/audio-clock or
original-renderer comparison. No new m49 claim. Conversion/recovery captions,
status HUD timing and other unrelated battle presentation remain separate work.
Run original_oracle.py with --rom, --symbols, --state and --output pointing to
matching local assets. No ROM is distributed.
