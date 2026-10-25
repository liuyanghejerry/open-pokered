# Copying move result captions

Conversion applied its types but omitted its result. Failed Conversion and
Transform used the generic attack-missed caption. Restore ConvertedTypeText
with the target's party name/nickname, and PrintButItFailedText for failures.
The existing successful Transform caption still names the copied species.
No mechanics, random draws, animation rules or engine code are changed.

## Original reference

pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c:
`engine/battle/move_effects/conversion.asm` and `transform.asm`, and
`data/text/text_3.asm` (_ButItFailedText, _ConvertedTypeText).
Conversion checks the target's invulnerability. The original Transform bugs
check the player's own invulnerability and overload the enemy's test register;
those behaviors are deliberately preserved. This is a source comparison,
not an original ROM runtime/timing observation.

## Validation

Four tests drain 44 prepared public FIGHT turns: Conversion's two sides,
both action orders and four nickname combinations (16); both copying moves'
self/target invulnerability guards (16); repeated Conversion (4); frozen
users in both orders (8). They check exact result text and order, copied
species/types, unchanged HP/RNG and PP for unchanged move slots. Successful
Transform replaces its move slots. NPC actions retain the original PP behavior.

The parent passes the frozen control and fails the other three groups,
with 30 incorrect result captions. All four groups pass with the repair.

- Full core: 3,780 passed, one existing ignored.
- App library: 393 passed, 38 existing ignored.
- Fresh desktop NEW GAME through m06: 39.3s; all 11 subsystem
  scenarios: 146.5s, unchanged source and binary identities.
- Seven GBA performance and 13 memory cases passed with unchanged fixtures,
  thresholds and timeouts. Minimum heap 4,928 B and stack
  8,704 B.
- Actual BattleVisualEffects/draw_battle, identical inputs at frame 155:
  the parent omits the Conversion result; the candidate names Enemy TANK.
  Both captures inspected; no full PokemonGame/audio-clock or original-renderer
  timing claim, and no new m49 validation claim.

Transform sprite publication, status HUD timing, and recovery captions remain
separate follow-up work.
