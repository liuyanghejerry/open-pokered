# Transform result caption

Successful Transform now announces the original user's name and the copied
species after the move announcement. The previous build returned to the battle
menu without this result. A target nickname must not replace the species name.

## Original evidence

Reference: pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c.
`engine/battle/move_effects/transform.asm` plays the move animation, copies
species and stats, obtains the copied species name with GetMonName, then calls
PrintText with TransformedText. `data/text/text_3.asm:290-296` contains
`<USER>` followed by `transformed into`, wNameBuffer and `!`.

This is source-level evidence. An optional prepared-ROM text-tail probe did not
reach its entry hook and observed zero cases; it is not counted as verification.
The game's existing Transformed marker distinguishes a successful action from
blocked moves. Original invulnerability bugs remain unchanged.

## Regression and validation

Three regression tests execute and drain 26 prepared public FIGHT turns:
16 successful cases across both sides, action orders and nickname combinations;
six frozen/self-invulnerable/target-invulnerable cases; and four turns covering
Transform followed by the copied move. HP and random draws are checked as well.
On the parent, two tests fail with 19 missing expected result captions and the
following-turn control passes. The candidate passes all three.

- Full core: 3,764 passed, one existing ignored.
- App library: 393 passed, 38 existing ignored.
- Fresh desktop NEW GAME through m06: 37.6s; all 11 subsystem
  scenarios: 153.3s, source/binary identities unchanged.
- Seven GBA performance and 13 memory cases passed with unchanged thresholds,
  fixtures and timeouts. Minimum heap 4,928 B; stack 8,720 B.
- Same input and frame 347 through actual BattleVisualEffects/draw_battle:
  parent has returned to the menu; candidate shows Enemy CLONE transformed
  into SNORLAX, despite the target being nicknamed TANK. Both images inspected.

The captures use prepared combatants and the actual renderer. They are not a
full PokemonGame/audio-clock or original-renderer comparison. No new m49 claim.
Premature sprite/status/level publication remains a separate finding and is
not changed by this caption repair. No engine or battle-mechanics changes.
