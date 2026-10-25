# Recovery move results

Recover, Softboiled and Rest omitted RegainedHealthText after healing, and
reported an attack miss when the original HealEffect failed. Rest used the
generic sleep caption and played its animation before the sleep result.

Print the original success/failure lines. Capture whether Rest's user had a
status when its action resolved, before RemoveStatus and before a later Haze.
Use the shared sleep caption as the animation anchor; the existing HP refill
then completes before the regained-health page. Failed Rest retains the used
move anchor and its missed-animation flag. No healing, status, RNG or stat
mechanics are changed, and the engine dependency is unchanged.

## Original reference

pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c:
`engine/battle/move_effects/heal.asm` and `data/text/text_3.asm:274-289`.
HealEffect's low-byte comparison fails at full HP and deficits of 255 or 511;
Rest selects StartedSleepingEffect or FellAsleepBecameHealthyText before its
animation, then prints RegainedHealthText after the HP bar/HUD update.

The included PyBoy probe executes 30 prepared original HealEffect calls for
both sides. Read-only hooks observe failure, Recover/Softboiled healing before
animation, or Rest's selected sleep text before HP healing. It also confirms
that Rest leaves the prepared burn/paralysis working stats unchanged. Entry
returns through the original VBlank RETI so interrupts run; no ROM patch or
injected instruction is used. This is not full-turn, rendered-text or animation
timing verification. ROM, state, symbols and probe hashes are recorded.

## Validation

- Four tests cover 96 prepared public FIGHT turns: 72 success/failure/frozen
  cases across both sides/orders, including all three failure HP values and
  poison/burn/paralysis; 12 Rest/Haze ordering cases; eight turns checking
  working-stat persistence through the next sleep turn; four presentation
  cases checking sleep text before animation and health text after HP refill.
- Parent: one group passed, three failed, with 60 wrong ordinary results,
  12 wrong Haze-sensitive results and an early Rest animation. Candidate:
  all groups passed. HP, PP, status and RNG checks remain intact.
- Full core: 3,788 passed, one existing ignored. App library: 396 passed,
  38 existing ignored.
- Fresh desktop NEW GAME through m06: 41.1s; all 11 subsystem
  scenarios: 141.3s, unchanged source and binary identities.
- Seven GBA performance and 13 memory cases passed with unchanged fixtures,
  thresholds and timeouts. Minimum heap 4,928 B and
  stack 8,720 B.
- Actual BattleVisualEffects/draw_battle: four prepared FIGHT sequences,
  identical four button presses and six capture frames per case. The selected
  before/after pairs were inspected; all captures and input hashes are recorded.

This PR aligns result wording and animation/HP ordering within the existing
text controller. Original DONE versus PROMPT acknowledgement behavior, status
HUD publication timing and complete audio-clock timing remain separate audits.
The renderer fixture does not run the full PokemonGame/audio clock. No new m49
validation is claimed.
