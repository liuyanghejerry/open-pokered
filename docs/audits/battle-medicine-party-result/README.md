# Battle medicine keeps the live party result

The complete PR also retains the full party view for no-effect medicine results and uses the original living-Revive caption. See [the negative-result audit](../battle-medicine-no-effect/README.md) for the current `f078` integration, literal-master before captures, full tests and root review. The positive-stage identities, captures and counts below remain historical evidence with their original labels.

The battle medicine target picker and successful result now use the full party view. The selected member's updated HP/status remains visible, including a bench member. Result text names that member and uses the original Potion, status-cure or Revive message. The existing HP/status effect and one-item consumption still occur once, and the result waits for the source's literal `DelayFrames(50)` followed by A/B before `EnemyFreeTurnAfterItem`.

`BattleState.player` remains the party owner, `BattleScreen.party_cursor` the selection owner and `current_message` the text owner. Rendering creates a temporary, read-only adapter from `persistent_party()`; it never sends input to the field party screen or writes `save_data`. Transform therefore displays the original party identity/nickname and original PP with live HP. These full party screens decline the small battle-overlay cursor cache, whose coordinates and visible fields differ.

Original reference: `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, `engine/items/item_effects.asm:805–1253` (`ItemUseMedicine`, `.showHealingItemMessage`) and `data/text/text_2.asm:1384–1432`. Healing, status cures and Revive all reach `ClearScreen → RedrawPartyMenu → DelayFrames(50) → WaitForTextScrollButtonPress`. Full Restore at full HP with an ailment uses the Full Heal message; restoring HP uses the Potion message.

## Actual input and ordinary SAVE controls

Final native BEFORE is a fresh, isolated build of clean master `8eb08f56945bf7db3650896295e8048cf97d14c8` (`12facc2aba0c57d01ef00d61e9dde1c78a1d6b6f5b2c4019bdd4860226dafd9d`). Final AFTER is `dd04edf2cc54466b05eb069954dc903826aab5f8` (`e91546d6c71613fe59b1b77cc510c92f68f15f39ba8de11f6c58f508788a27ca`). Complete build/source identities are included in the archive.

Each row has two original, two final-master BEFORE and two final AFTER successful controls. All 24 end with ordinary gameplay SAVE and a valid canonical SRAM checksum.

| Case | Target | Preinput target | Result before ACK | Item quantity |
| --- | --- | --- | --- | --- |
| Potion active | 0 | HP 1/52 | HP 21/52 | 2 → 1 |
| Potion bench | 1 | HP 1/35 | HP 21/35 | 2 → 1 |
| Parlyz Heal bench | 1 | HP 35/35, paralysis | HP 35/35, no ailment | 2 → 1 |
| Revive bench | 1 | HP 0/35 | HP 17/35 | 2 → 1 |

Fixtures explicitly construct legal Bulbasaur levels/stats/moves, target HP/status, item quantity, map and the unfought Forest trainer flag before input. Thereafter all providers use ordinary Down/trainer dialogue/battle ITEM/target selection, 600 no-input frames, an actual result acknowledgment, Tackle victory, and ordinary SAVE. No runtime HP/party/inventory writes or forced battle calls are used. Native uses seed 42; original/native RNG is not matched.

All eight final native BEFORE/AFTER pairs have byte-identical complete input action arrays **and byte-identical entire 32,768-byte ordinary SRAM**, including time and checksums. Party/PC storage, OT/nickname, PP, money and dex therefore have no native before/after save difference. Each provider also repeats exactly within each case. At the paused result both providers have restored the intended target, consumed one item and withheld the enemy turn. The bench's entire 44-byte record matches its expected post-medicine record in original and native SAVE. The active member's post-battle HP/PP differ between original and native because their battle RNG/Tackle counts differ; no cross-provider full-SRAM parity is claimed.

A separate bench-Potion replay expands the same complete 1,288-button prefix from frame 0 for both final native versions. Every physical frame is advanced and checked individually. PNG windows 620–720 and 1272–1288 are contiguous. The target pair is frame 648; the result pair is frame 1288. These bounded replays do not claim an additional SAVE.

## Tests and scope

- The new full-party target renderer regression fails on the original clean `6d30` production code, then passes with the fix.
- Final core library: **2,894 passed**; debug app library: **293 passed, 39 ignored**; default app medicine tests: **4 passed**; battle-text tests: **13 passed**.
- Tests cover active/bench effects and one consumption, unskippable Delay50, 600 idle frames, A/B handoff, status cures/Revive/Full Restore's two message branches, nickname and Transform identity/PP, no-effect/cancel and bag return, mid-battle JSON snapshot restoration, and actual reset-combo → Title/Continue → new-battle ownership. The ordinary load helper is not changed.
- GBA uses its own target and an exact 2,166-file source mirror outside the repository ancestry, pinned nightly `2025-12-07`. All seven configured performance scenarios pass against the unchanged checked-in baseline: trainer update 232 ticks, idle update 382, Oak draw 2994.

This change fixes the medicine caller/full party result and its named feedback. It does not rebuild HP-bar/icon animation, reload/audio/CPU timing, during this positive-only stage; the complete PR fixes the no-effect result backdrop as documented in the linked negative-result audit. PP restore effect/selection behavior remains covered by the existing core tests; successful PP-result presentation is not claimed fixed. Other status-cure message branches have source-backed unit controls, rather than separate actual recordings for every item. This is constructed scenario verification, not a fresh campaign.

Historical clean `216cb034`/`6d30ba5a` and the party-only intermediate `318b0819` retain their original identities and raw records. They are not labeled final AFTER. `metadata-errata.json` corrects shared human-readable scope templates that said HP1/Potion2 for the status/Revive cases; typed fixtures, initial state and actual item/HP/status records are preserved unchanged. The original failed debug-schema assertion, a misplaced localization draft's compile failure and the incorrect bare-load test assumption remain in the evidence. The final load regression follows the actual caller contract; no new load special case was introduced.

## Evidence

`verification.json` provides the final case assertions and identities; `screenshots.json` binds the committed pairs to their exact frames and raw SHA-256. `evidence.zip` stores source/native inputs, frames, states, ordinary SRAM, portable JSON fixtures, scripts, source/build identities and terminal logs as deduplicated SHA-256 blobs. It excludes ROMs, executables and emulator `.state` files. Original `.gbc.ram` files are ordinary SRAM, not ROMs.

```bash
python3 docs/audits/battle-medicine-party-result/extract_evidence.py --verify-only
python3 docs/audits/battle-medicine-party-result/extract_evidence.py --output /tmp/battle-medicine-evidence
```
