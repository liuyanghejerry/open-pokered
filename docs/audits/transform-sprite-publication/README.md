# Transform picture publication

The resolved turn replaced the user's displayed species before Transform acted.
The renderer also looked up the current opponent whenever TRANSFORMED was set,
so an enemy that copied Snorlax later appeared as Tangela when the player switched.

Keep the previous display identity until Transform's result page. A typed record
carries the species this action copied; AnimationTransformMon uses it for the
in-animation preview. The stable display identity is published on the first
result page, including wrapped captions, and survives text-publication snapshots.
Draw that retained species on later turns. With animations disabled, execute
ChangeMonPic directly using the existing transfer duration and raster handling.
Original failure guards, nickname/species narration and battle mechanics remain.

## Original reference

pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c,
`engine/battle/move_effects/transform.asm` and
`engine/battle/animations.asm:2022-2062`.
Transform runs the animation (or directly AnimationTransformMon when disabled),
then copies the species and prints the result. ChangeMonPic loads the opponent's
front/back picture once; a later opponent switch does not load it again.
This reference is source-level evidence, not a new original-ROM timing capture.

## Validation

- Core integration: 20 public FIGHT turns, both sides/orders, original
  invulnerability guards, frozen controls and presentation-disabled controls.
  The parent passes the headless group and fails the display group with 10 early
  identities. Both groups pass with the repair; HP and RNG remain unchanged.
- Two publication tests cover wrapped first-result pages, JSON snapshot
  roundtrips and snapshots predating the new optional metadata.
- Three app tests run 22 prepared FIGHT/menu-switch turns: animation on/off,
  failed/frozen controls and a public PKMN switch. The switched enemy's actual
  rendered picture is compared pixel-for-pixel with a normal Snorlax picture.
- Full core: 3,784 passed, one existing ignored. App library: 396 passed,
  38 existing ignored.
- Fresh desktop NEW GAME through m06: 41.3s; all 11 subsystem
  scenarios: 142.6s, unchanged source and binary identities.
- Seven GBA performance and 13 memory cases passed with unchanged fixtures,
  thresholds and timeouts. Minimum heap 4,928 B and
  stack 8,704 B.
- Actual BattleVisualEffects/draw_battle compares four FIGHT turns and one
  public party-switch/free turn. Inputs and all 17 capture frames match between
  versions. Inspected pairs show both early-picture defects fixed at frame 3,
  and the copied Snorlax retained after switching to Tangela at frame 544.

The renderer fixture uses prepared combatants and does not exercise the full
PokemonGame/audio clock. The disabled-animation path reuses existing transfer
timing; no new original VBlank-equivalence claim is made. No new m49 claim.
Status-HUD publication and recovery narration remain separate follow-up work.
