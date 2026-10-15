# Unidentified Tower encounters register as seen (904)

Entering a Pokémon Tower battle now registers the underlying species as seen even when the enemy is displayed as an unidentified GHOST. The Silph Scope controls identification; it does not gate the original Pokédex write. This restores the saved and player-visible seen entry without granting ownership.

Original [LoadEnemyMonData](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/core.asm#L6129-L6137) converts `wEnemyMonSpecies2` through `IndexToPokedex`, then sets `wPokedexSeen`. Four actual source controls independently confirm that the unscoped Tower6F Marowak battle adds National105 before the normal SAVE endpoint. This is the original's behavior despite the unidentified GHOST presentation.

The production diff removes only the `!is_ghost` condition around the existing encounter `set_seen(species)`. A regression test fails on the baseline and passes after the change. It checks hidden Marowak/Gastly, identified Marowak and an ordinary Pidgey encounter, including the existing ghost-presentation flag and unchanged ownership. The actual input matrix covers the scripted Marowak encounter; other Tower species have shared-path/static and unit coverage, not a claimed complete runtime matrix.

## Actual controls and saved bytes

The preinput fixture is actual fresh-mainline `heal-003` SaveData with Bulbasaur Lv6/HP21, full PP, RED/ID25506 and cash1500. It constructs legal Tower6F `(11,16)`, no Scope, bag PokeDoll2, three beaten Channeler flags and an undefeated ghost. Initial dex has seen6/owned1 and Marowak unseen. Validation uses ordinary SAVE, LEFT into `(10,16)`, ghost battle BAG/Doll, A or B, the mother/calming dialogue and another ordinary SAVE. No validation-time battle/RAM setters or direct battle calls are used.

BEFORE and AFTER each run A/B twice. Every per-answer repeat has identical states, actions, initial SRAM and final SRAM. All four paired BEFORE/AFTER controls have the same fixture, actions and entire initial32KiB SRAM. Their final32KiB SRAM differs **only** at:

| Offset | BEFORE | AFTER | Meaning |
|---|---|---|---|
| `0x25c3` | 0 | 1 | seen105: `seen[13]` bit0 |
| `0x3523` | 34 | 33 | Updated save checksum |

Play time is identical across each pair. The full party404 bytes, box1122 bytes, name/OT/nickname/PP, bag/cash, owned19 bytes and all other seen bits are protected. The ordinary ghost-departure flag and one consumed Doll are common to both versions. The four source controls use the same initial38-byte dex and produce the same single seen-bit addition, with unchanged owned flags and checksum-valid ordinary SAVE. Their correct original camera construction is `event_displacement +7+width10`; the earlier failed camera attempt remains excluded and archived.

Each version then has two independent normal BOOT/Continue runs from its actual final SAVE. START→POKéDEX plus identical page/row inputs reaches frame1536. Both repeats match states/actions/PNGs exactly. BEFORE ends at species19 with seen6; AFTER lists species105 MAROWAK with seen7. The two committed screenshots are untouched captures of the same simulated frame and input sequence, not debug-selected dex entries.

## Identity and verification

Clean BEFORE: master `f44e3ef500ea92b24e625bb82687a449a2ee532d`, binary SHA `c1d22a1da550ae611bee78d831c188cec684abe15bc3649193da022577ece7d0`. Clean AFTER: `0463f568a1edb7874c7c4dae530b7e86129e640f`, SHA `9267a1a4cce1971a758000c44998fc69203df803ec7cdf463adb28a9446ec77c`. Both are frozen dev opt0/debug0 debug-server builds. Native BEFORE4, correct AFTER4 and both saved-dex2 groups all have authoritative exit0 reports.

The initial AFTER attempt supplied a handwritten incorrect source argument and was interrupted with exit130. **Its entire attempt is excluded**, including any partial records. Its unchanged wrong argument and real frozen-binary identity remain in `metadata/audit-904-rejected-wrong-source-argument.json`; no history is relabelled. The valid AFTER driver receives the complete correct source from the frozen identity JSON.

Baseline regression:1 FAIL. AFTER full app270 PASS/39 ignored, core2884 PASS and explicit binary build PASS; original logs and hashes are archived. Latest-master integration after #246 is separate root-owned work; captures retain their actual f44/0463 identities.

`verification.json` has the compact checked matrix; `screenshots.json` maps frame/source/SHA. `evidence.zip` retains source4, valid native8, saved-dex4, raw SRAM, scripts, identities, logs and rejected records with exact-content SHA deduplication. Every original path has been read back and verified; ROMs, executables and emulator states are excluded. Verify or extract with `python3 extract_evidence.py evidence.zip --verify-only` or an output directory instead of `--verify-only`.

This is a seen-registration/save/UI audit. It does not claim original/native Doll text or consumption timing equivalence, complete animation/audio/RNG parity, all campaign reachability or full mainline completion. Those presentation and timing concerns remain separate from this small registration change.

进一步整合 master `dfeefa34` 后，`5672f312` 完整 debug app 271 通过，39 个已有忽略项，0 失败。核心源码未再改变，沿用完整 core 2884 通过结果；日志见 `integration.json` / `integration-logs.zip`。截图仍标记真实的 `f44` / `046` 录制身份。

已整合中毒流程 master `7310d298`。生产整合提交 `9c1220f5` 完整 debug app 280 通过、39 个已有忽略项、0 失败；核心源码与该 master 完全相同。完整日志见 `master-7310-integration.json` / `master-7310-integration-logs.zip`，原截图与录制身份未重新标记。
