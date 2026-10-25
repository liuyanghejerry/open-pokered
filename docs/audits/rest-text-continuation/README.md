# Rest sleep-text continuation

The restored Rest captions still required an extra A/B press after the sleep
animation and HP refill. The original sleep caption ends in DONE, so its
PrintText returns to HealEffect and the regained-health result follows.

Mark only the final sleep-text page with typed continuation metadata. Let that
page advance after the frontend finishes the move and the HP bar is stable.
Earlier wrapped pages, RegainedHealthText, failures and frozen users keep their
existing acknowledgements. Headless callers need no visual acknowledgement.

Save the typed Rest pages and pending action/HP target with text-publication
snapshots. Restore a pending visual handoff when necessary; snapshots taken
after its completion continue the saved HP refill without replaying the move.
Older snapshots without these optional fields retain their current-page ACK.
No move mechanics, RNG, engine dependency or animation script is changed.

## Original reference

pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c:
`engine/battle/move_effects/heal.asm`, `data/text/text_3.asm:274-289`, and
`home/text.asm:209-228` (PromptText calls ManualTextScroll; DoneText returns).
Two prepared original healthy Rest calls, one per side, reach animation entry
after the sleep text with no button input and no ManualTextScroll call. The
probe executes original instructions and observes read-only hooks; no ROM patch.
This is an effect/text-call observation, not a complete original battle or a
cross-runtime frame-timing comparison.

## Validation

- Three integration tests run 20 prepared public FIGHT turns: eight successful
  Rest cases across both sides/orders and healthy/poisoned states, eight
  failure/frozen controls, four headless cases. They hold the visual handoff,
  then verify automatic continuation only after animation/HP completion and
  that the final health prompt still waits for input.
- Parent: controls pass; eight visual and four headless sleep pages remain
  stuck after 600 no-input frames. Candidate: all three groups pass.
- Three snapshot tests use 18 prepared Rest setups, checking pending animation,
  refill progress, final-result waiting and legacy optional-field compatibility.
  JSON roundtrips and frame-by-frame HP/message comparisons pass.
- Full core: 3,794 passed, one existing ignored. App library: 396 passed,
  38 existing ignored.
- Fresh desktop NEW GAME through m06: 41.7s; all 11 subsystem
  scenarios: 141.1s, unchanged source and binary identities.
- Seven GBA performance and 13 memory cases passed with unchanged fixtures,
  thresholds and timeouts. Minimum heap 4,928 B and
  stack 8,704 B.
- Actual BattleVisualEffects/draw_battle: four prepared FIGHT sequences,
  identical four button presses and six capture frames per case. Recover
  success/failure are unchanged controls; inspected Rest pairs show the sleep
  result continuing without another press. Input and image hashes are recorded.

The renderer fixture and snapshot tests do not verify the full PokemonGame
audio/visual clock. Other DONE/PROMPT boundaries and status HUD publication
remain separate audits. No new m49 validation is claimed.
