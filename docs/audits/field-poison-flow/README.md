# Field poison continuation (896)

Walking poison now suspends at each newly fainted Pokémon's individual message. The next party slot remains untouched until that message returns. After all slots have been processed, any remaining Poison status causes four rendered frames with only background color 0 mapped to dark gray; the normal background returns before `SFX_POISONED`. If nobody remains alive, the separate player blackout message must return before healing, halving money and warping.

The suspended caller owns only a slot cursor and presentation phase. `SaveData.party` remains the sole party owner. Runtime snapshots preserve the cursor and remaining flash frames; ordinary SRAM loading and soft reset clear the old continuation. Flash input and actor work are blocked without treating a palette effect as FontLoaded text. Debug `control_ready` also waits for the caller to return.

The primary reference is [ApplyOutOfBattlePoisonDamage](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/events/poison.asm), including the blocking `DisplayTextID` calls and separate status-counting loop, plus [ChangeBGPalColor0_4Frames](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/gfx/screen_effects.asm). The recorded ROM SHA is in the verification and raw records.

## Actual controls

Each provider uses pre-input constructed Rattata Lv12, DVs `9A78`, max HP31, Tackle/Tail Whip/Quick Attack, full PP, cash3000 and last blackout map Pallet Town. Rattata's Normal type can legally receive Poison. This is a legal stats/moves/status construction, not a natural capture or whole campaign reachability claim. Source setup uses an explicit pre-validation EnterMap bootstrap into the genuine RedsHouse2F `(3,6)`; validation uses only normal walking, A/release acknowledgements and START/SAVE. No validation-time RAM edits or direct poison calls.

| Case | Initial HP / status | Source and native saved endpoint | Previously visible error |
|---|---|---|---|
| single | 1PSN,31healthy | 0FNT,31healthy | Baseline for one faint |
| double | 1PSN,1PSN,31healthy | 0FNT,0FNT,31healthy | Second slot already damaged before first ACK; merged text |
| blink | 3PSN,31healthy | 2PSN,31healthy | Background flash absent; poison sound too early |
| wipe | 1PSN,1PSN | 31healthy,31healthy; cash1500/Pallet | Healing/warp before ACK; player blackout message absent |
| mixed | 1PSN,3PSN | 0FNT,2PSN | Surviving slot damaged and sound played while first faint still open |

There are ten source ordinary SAVE controls, twenty historical native controls and twenty final current-baseline native controls. Each case repeats twice. Current native before/after use identical inputs, fixtures and 424 consecutive recorded frames. Initial/pre-trigger PNGs match, repeat frame states/PNG hashes match, and **the entire 32KiB saved SRAM is byte-identical before/after for each final pair**. All saved native party structs match the original's 44 bytes per Pokémon. Initial/final ordinary SAVE protects the existing box1122 bytes, bag42 bytes, dex38 bytes and party fields other than HP/status; checksums pass.

Four actual native flash frames change only background white to `#555555`; OBJ white and the other three shades remain intact. The first restored frame starts native logical sound66 (`Poisoned`). Original `PlaySound` receives physical ID`0x97`; these are different ID namespaces for `SFX_POISONED`.

## Frozen identities and screenshots

Final comparison BEFORE is clean master `25443cf4bd209ec77961450fb6b986ed5c2308c5`, binary SHA `82149bde69f216b11122770660f585b919f40c273182f9d0f7a980f23433d080`. AFTER is clean production head `a666f6094a59e126f3875a83da4d5c34bc708363`, SHA `313c16024d041969399502068e9ed2e5d294ba3e1f456f0fc19c5b333cbe2ab3`. These are separate dev opt0/debug0 `debug-server` builds in exclusive target896 using the same private worktree path, frozen before recording; each actual run has a private binary copy, directory, port and save with no initial flags sidecar.

The later merge of master `46cc1228d061bcdce77278593ec61906d2d79bfb` changes only gift tests under `cfg(test)` and documentation. It changes no production runtime. These captures retain their actual `25443/a666` source identities. Historical `24ab/9e9` captures and the unused `13c` intermediate binary retain their original identities too.

The six PNGs in `docs/screenshots/field-poison-flow/` are copied from genuine matched frames: double1488, blink1387, wipe1620. `screenshots.json` records each source path, frame, commit and SHA. The wipe pair shows the old already-warped map versus the newly waiting full blackout message; no final frame has been repeated as padding.

## Verification and limits

- Core2885 PASS; all380 core/data file hashes remain identical to the compiled test source at `13c4c897`.
- Final poison tests: debug-server9 PASS; default8 PASS. They exercise individual ACKs, real four-frame composition/cache equivalence, remaining-frame snapshots, separate blackout settlement, reset, ordinary SRAM loading and control readiness.
- After the test-only master merge, full debug app270 PASS /38 ignored /0 failed at `23dd8c03`. Earlier GiftQuestion failures and two rejected validation helpers remain in the evidence; they are not counted as passing results.

`verification.json` is the compact summary. `evidence.zip` retains all original input/state/frame/PNG/SRAM records, full verification, scripts, build identities and logs. Exact file contents are stored once per SHA256; the internal entry manifest maps every original path to its unmodified blob, preserving every captured frame and timestamp. `archive.json` identifies the archive and manifest hashes; all blobs were read back and checked against every original path. ROMs and native executables are excluded from the public archive and identified by their exact hashes.

Use `python3 extract_evidence.py evidence.zip --verify-only` to verify all original paths without writing them, or supply an output directory instead of `--verify-only` to reconstruct the raw tree.

This does not claim full playthrough completion or full framebuffer/audio parity. Source/native input cadence and initial campaign/box/dex contents are independently constructed. Original BGP=`E6` lasts four frame samples and is restored before sound. The mixed static source backdrop has three completely mapped rendered frames plus one partial LCD scanline frame; the blink boundary also overlaps ongoing walking BG transfers. The native whole-frame renderer does not reproduce that LCD edge. Font/dialogue layout, every-map/dark-cave behavior and wave-level audio are not compared here.

Two existing blackout presentation differences remain separate follow-ups: the original fades out to black while the existing native path fades to white, and the existing `heal_requested` consumer plays HealingMachine despite the original blackout's stop/music sequence. Their original hooks and native logical73 frames are preserved in the archive's `metadata/audit-896-blackout-followup-readonly.json`; this change preserves those existing paths.

最新整合：合入捕获流程 PR #251 对应 master `f44e3ef5`，保留读档时两种未完成流程的清理。生产整合提交 `d078c234` 的 core 2887、完整 debug app 278 通过，39 个已有忽略项，0 失败；完整日志和 SHA 见 `capture-integration.json` / `capture-integration-logs.zip`。原始截图及录制仍固定到真正的 `25443cf4` / `a666f609`，没有重贴为新的构建。
