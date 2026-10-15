# FIGHT refusal and automatic Struggle validation

The FIGHT menu now keeps the original blocking text flow for an empty-PP or disabled move. A/B acknowledges the completed prompt and returns to the same move row. When no move can be selected, the DONE text and its delay lead directly to Struggle without a second confirmation. The phase owns printing, waits, sampled held input and the one-shot button-sound receipt.

## Source and executable identities

- Current base: `e6150de4cf06bfd8a8bbbaf3e65ceb8ecb6e0ce5` (merged flinch fix).
- Before executable was actually built at `c997646537fd2d5a0955c7192faa2f6233d640d9`, SHA-256 `a251317d620d558118a2c66e53862a1f2e29e5a6cbc62b55c561f5fbecd0b24f`. Its 2181 compiler inputs and all 2699 non-documentation files are identical to the current base; only five documentation paths differ. The executable retains its original identity.
- After executable was built at `a1efdac02d41946002e35051e4f8412264221afc`, SHA-256 `a8747aac9cd73ea64e855782e40353699b41070a69e7ca8f300e235c4e7154a2`. All 2187 compiler inputs, 2705 non-documentation files and 702 external GFX inputs were frozen before and after validation.
- Engine revision remains `e533a3d23a0cdbed1524a1ac91184733da4a6f7e`.

## Actual native comparisons

Each provider launched the real headless CLI with the same preboot JSON, initial Saffron Gym warp, seed 42 and English configuration. The game created an ordinary SRAM save through physical button input before meeting the trainer. Subsequent controls used only physical input timelines and read-only observations/captures. These are save-assisted battle scenarios, not a cold SRAM boot or a complete story playthrough.

| Case | Fixed physical input and observations | Same-frame published PNGs |
| --- | --- | --- |
| Empty PP | 1773-frame prefix, then 50 fixed frames. Headbutt has 0 PP; completed prompt waits for A; returning move cursor stays on row 0. | 1814 prompt, 1823 return |
| Disabled move | A fresh base discovery stopped at frame 5209 with Amnesia disabled at 18 PP. Replay that exact prefix plus 50 fixed frames; B acknowledges and returns to row 1. | 5250 prompt, 5259 return |
| No selectable moves | 1772-frame prefix, then A and 90 neutral frames. No later A/B input is sent. By frame 1863 the after build has used Struggle, while the before build still waits on its no-moves text. | 1863 automatic action |

All six runs passed. Their initial states and 28/82/28 complete public prefix endpoints respectively match without normalization. The two refusal cases preserve live PP, HP, inventory and party throughout the 50-frame suffix. Struggle legitimately resolves a turn: at frame 1863 its live player/enemy HP is 196/39 while the current narration frame still displays the earlier HUD values. Capture responses and the reads before/after each PNG agree exactly.

The new disabled prefix comes from one ordinary-input discovery after the flinch fix, including eight legal Amnesia uses from 26 to 18 PP. It replaces the old 5555-frame/21-PP trajectory for this base. No seed changes, live state setters, battle injection, dialogue skipping, restore, timing fitting or retries were used. Across the paired runs, all 1346 raw RPC pairs and 78 evidence artifacts were checked; valid 32768-byte SRAM, checksum, exact 44-byte party record and flag sidecar stayed unchanged. Each owned game was intentionally terminated and reaped, with its process group subsequently absent.

## Regression and GBA measurement

On the exact after source, the full core package passed: 2989 library tests plus 412 integration tests, with one documentation example ignored. The held-input adapter test and five Game binary FIGHT tests also passed. This includes receipt reset during waits/presentation and inactive Link exits, original text options, PP/Disable handling, captured settlement and the inherited flinch regressions. The full App test suite is left to official CI.

The actual GBA ELF identified an unconditional 132-byte action copy caused by returning `(ScreenAction, FightTextReceipt)`. Returning `ScreenAction` directly and passing the receipt by mutable output parameter removes that copy while preserving canonical/mirror sound aggregation. In the new ELF the ordinary core-call tail contains no such copy, and Game local stack reservation falls from 2204 to 2068 bytes.

One new ROM was measured using local SDL mGBA 0.10.5 hardware timers. All 31 original baseline comparisons passed; battle-entry update is 211 ticks (limit 220.8), trainer-battle entry is 229 (limit 232.3). The baseline, thresholds, fixtures and seeds were unchanged. Earlier failed measurements remain recorded separately; this local result does not replace official CI's complete ROM, memory, pacing and opening checks.

## Evidence retained in the workspace

- `audit-1044-pr279-root-native-six-review.json`: all six real runs, source/identity hashes, raw input, state, SAVE and lifecycle checks.
- `audit-1044-disabled-master-discovery-review-v1.json` and `audit-1044-disabled-fixed-pair-review-v1.json`: new discovery and fixed paired replay.
- `audit-1044-pr279-a1ef-root-host-perf-review.json`: raw host checks, actual ELF and independent 31-metric baseline calculation.
- `audit-1043-pr279-integrated-validation-v1/`: original build/test logs, source freezes, ELF/ROM identities and raw emulator measurements.

Earlier validation at base `9289961` and candidate `27647c9` remains historical in `fight-refusal-1041.md`; the source identities, inputs, screenshots and counts above are the current-base evidence.
