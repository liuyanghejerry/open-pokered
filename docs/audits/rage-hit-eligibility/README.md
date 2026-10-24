# Rage hit eligibility and single-hit results

A raging defender no longer gains Attack after an immune/missed attack, a knockout, or a status/charge action. Successful single-hit and fixed-damage attacks now print both the Rage-building caption and the result. The existing multi-hit handling uses the same action receipt.

Damage hooks record whether an actual strike landed and whether the target survived, including Substitute absorption. Enemy Psywave is the original exception: its successful zero-damage roll still builds Rage. Super Fang applies its HP loss before these reactions. Attack already at 999 preserves its stage and prints `Nothing happened!`; +6 returns without an attempt. Only successful increases reapply badge boosts and the attacker's burn/paralysis penalties.

A new Rage lock requires a successful strike, including a knockout, and is cancelled when that strike breaks a Substitute. A previously active lock survives later failed attacks. Empty Rage receipts cannot suppress Growl's real Attack-drop caption.

Original reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, `engine/battle/core.asm:3218–3243,4750–4805,4913–4957,5615–5643` and `engine/battle/effects.asm:351–514,1193–1203`.

## Verification

- Eight tests cover 168 prepared public-FIGHT turns on both sides, including ordinary/fixed damage, KO, misses/immunity, status/charge moves, stage/999 caps, burn penalties, Substitute and existing/new Rage locks. The identical final tests fail all eight against the parent libraries.
- Full core: 3,736 passed, one existing ignored.
- Fresh native NEW GAME through m06 and all 11 subsystem scenarios passed, with unchanged source/binary identities.
- Seven GBA benchmark cases and 13 memory cases passed with unchanged fixtures, limits and timeouts. Minimum free heap: 7,816 B; stack margin: 8,704 B.

Two prepared public-FIGHT captures use the actual `BattleVisualEffects` and `draw_battle`, identical input and identical frames per parent/candidate pair. At frame 139, Snorlax now shows the missing Rage-building caption before its Attack rise, with HP 446 in both builds. At frame 155, Dragonite remains at stage +5/Attack 999 and shows the failed attempt; the parent reached +6 and had already returned to the menu. Both have HP 309. All four PNGs were visually inspected; `verification.json` records hashes and traces.

These are prepared-state checks plus the stated native regressions, not a new full m49 playthrough. The captures do not execute the full game/audio clock or original ROM. Per-hit animation/result interleaving, repeated multi-hit critical/effectiveness captions and Substitute-break text ordering remain separate work. Overall original RNG ordering also remains outside this repair.
