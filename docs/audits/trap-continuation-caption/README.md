# Trapping continuation announcement

Bind, Wrap, Clamp and Fire Spin reuse their previous damage on later turns.
The original announces “<USER>'s attack continues!” before those attacks;
the port repeated “<USER> used <MOVE>!”.

Capture the executed continuation and use the same caption for narration
and the presentation entry. The animation handoff matches that entry's
caption, so changing dialogue alone would suppress the move animation.
Keep the receipt through later residual fainting, then clear it for the
next action. First uses and blocked users keep their existing paths.

Original reference: pret/pokered
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, `engine/battle/core.asm`
3554–3568 and 5928–5943; `data/text/text_2.asm` 1060–1063.

## Verification

- Eight original-ROM status-helper cases: both sides, active/inactive trap
  flags and counters 1/4. Active cases call PrintText with AttackContinuesText,
  decrement the counter and return the corresponding animation address.
- Four public-FIGHT tests cover 204 prepared turns with typed animation
  acknowledgements: all four moves, both sides, durations 2–5, fresh use after
  release, immune targets and Rage, frozen users, nicknames, and later poison
  fainting. Continuations preserve damage, RNG consumption and PP.
- On the parent, the original test revision fails the three continuation
  tests and passes the frozen control. The final test corrects the ordinary
  enemy's PP expectation to preserve its unlimited PP; all four pass.
- Full core: 3,751 passed, one existing ignored. Full app library: 393 passed,
  38 existing ignored.

- Fresh desktop NEW GAME through m06 and all 11 subsystem scenarios passed
  with unchanged source/binary identities.
- Seven GBA benchmark cases and 13 memory cases passed with unchanged
  fixtures, thresholds and timeouts; minimum heap 7,816 B, stack 8,704 B.

Actual renderer captures use identical inputs and frame 112 of a prepared
immune Wrap battle. Before: “MEWTWO used WRAP!”. After: “MEWTWO's attack
continues!”. Both have target HP 450, three RNG draws and Rage stage 1.
Both PNGs were inspected. The parent is the tested PR #409 head, whose tree
is identical to its merged master tree.

Reproduce the original helper observation with PyBoy and matching assets:

```sh
python original_oracle.py --rom original.gbc --symbols pokered.sym --state prepared.state --output observed.json
```

The ROM check enters isolated status helpers with prepared RAM, registers,
stack and short names. An existing RETI instruction enables VBlank;
read-only hooks observe text dispatch and return values. It does not verify
complete original turns, rendered dialogue or animation timing. Native
captures use BattleVisualEffects/draw_battle, not the full game/audio clock.
These regressions are not a new m49 playthrough.
