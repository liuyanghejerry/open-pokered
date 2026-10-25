# Level-up HUD publication

The turn resolves before its presentation. Reading the final party state into
the HUD showed the new level and maximum HP while the enemy was still using
Splash. Current HP then remained at its old value throughout the growth notice.

Keep each experience award's level/current HP/maximum HP as typed presentation
metadata. Publish active growth together on the first growth-text page. Match
bench notices too, so identical party names and level captions keep their order.
The metadata survives battle snapshot serialization; older snapshots default to
having none. Consumers without presentation retain immediate state publication.

## Original evidence

Reference: pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c,
`engine/battle/experience.asm:145-246`. GainedText prints before the new level
and stats are calculated. Current HP increases by the maximum-HP gain. For the
active party member, DrawPlayerHUDAndHPBar runs before GrewLevelText; bench
members skip that HUD redraw. Each EXP.ALL pass executes this sequence.

This is source-level original evidence, not an original-ROM runtime comparison.

## Regression and validation

Four integration tests execute eight prepared public FIGHT turns, including
both action orders, EXP.ALL's two awards, bench growth, restoring the gained
page, and consumers without presentation. The parent fails three groups;
the immediate-publication control passes. The candidate passes all four.
Four additional unit tests cover identical bench/active captions, transformed
species, a multi-page growth notice with JSON snapshot round trips, and older
publication snapshots without the added metadata.

- Full core: 3,772 passed, one existing ignored.
- App library: 393 passed, 38 existing ignored.
- Fresh desktop NEW GAME through m06: 51.4s; all 11 subsystem
  scenarios: 141.8s, source/binary identities unchanged.
- Seven GBA performance and 13 memory cases passed with unchanged thresholds,
  fixtures and timeouts. Minimum heap 4,928 B; stack 8,704 B.
- Same input and frame 3 through actual BattleVisualEffects/draw_battle:
  the enemy is still announcing Splash. Before: L21 and 69/80 HP.
  After: L20 and 69/76 HP. Both images inspected. The turn's RNG draws remain 3.
  The integration tests verify L21 and 73/80 HP when growth is announced.

The capture uses prepared combatants, not a full PokemonGame/audio-clock or
original-renderer comparison. No new m49 claim. Sprite/status publication,
per-hit narration and the separate original growth stats box are outside this
repair. Battle calculations and experience amounts are unchanged.
