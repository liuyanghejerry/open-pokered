# Old Man catch tutorial (899)

The coffee-NO demonstration now keeps the Old Man's back portrait and enemy HUD throughout, skips sending out the player's Pokémon, and preserves the successful catch caption through its return. The ball animation finishes before the caught jingle starts; the demonstration waits for the jingle instead of replacing its first channel with the last ball `Tink`. The player's low-HP alarm is disabled during this demonstration, including when the real party lead has 1 HP.

The relevant original paths are [ViridianCity's tutorial caller](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/ViridianCity.asm), [ItemUseBallText05 and its caught sound](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/items/item_effects.asm), and [LoadPlayerBackPic / battle setup](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/core.asm). Rendering reuses the existing `gfx/battle/oldmanb.png` and its existing scale/cache loader.

## Actual input and saved data

The native fixture is the actual fresh-mainline `heal-003` SaveData: RED, ID25506, Bulbasaur Lv6/21 HP, Tackle35/Growl40, cash1500. Before boot, the scenario changes only the map/position to Viridian `(17,6)`, facing and fast text; the HP1 variant additionally changes the legal current HP to 1. Normal movement approaches the visible wandering Old Man, A opens the coffee dialogue, DOWN/A selects NO, and normal text confirmations run the demonstration. START/SAVE writes an ordinary checksum-valid SRAM afterward. No battle/RAM setters or direct tutorial calls run during validation.

Final `64f15615` has two healthy and two HP1 controls. Each mode repeats with byte-identical states, actions, PNGs and SRAM. The complete 404-byte party area (including PP, OT/name and nickname buffers), box1122 bytes, bag42 bytes, cash3 bytes and player-name11 bytes match the matching legal initial fixture; HP1 remains 1. The examined fixture's dex38 bytes also remain intact. **Weedle was already seen in this fixture**: this does not establish protection for an initially unseen tutorial species, which remains a separate candidate.

The three original controls are healthy twice plus HP1 once, each using ordinary SAVE before and after the real coffee-NO tutorial. Read-only hooks record PrintText/PlaySound and each source frame records channels, APU registers and PCM hashes. Original `SFX_Caught_Mon` is physical ID154; native `CaughtMon` is logical ID101. These identify the same semantic sound in different namespaces. Source HP1 remains 1, `wLowHealthAlarm` stays 0, and all three caught voices run with nonzero PCM. The original healthy controls retain their historical preinput camera mistake (`+7+4` instead of Viridian's `+7+20`); their actual tutorial hooks, battle sound and saved-data observations are retained, but their field geometry is not used as parity evidence. The HP1 control corrects that camera formula and approaches from the side with normal input.

The historical full BEFORE controls were compiled at `fadd7c0b`; their preserved source identity is not relabelled as current master. Against those SRAMs, final healthy differs only at play-time seconds `0x2cf0`, frames `0x2cf1` and checksum `0x3523`; the elapsed saved clock is 194 frames later. HP1 additionally has the intended HP-byte change at `0x2f36`. This is **not** a claim that entire BEFORE/AFTER SRAM files are equal. The discarded initial equality assumption and the incomplete `8ff5fe18` two-guard fix remain historical evidence, excluded from final controls.

The final healthy outer harness reported exit143 for an unknown reason. Both case verification files, PASS lines and the final output path were written, and all assertions/raw records were independently checked. The HP1 harness exited0. This report counts four completed cases, not a healthy whole-harness exit0.

## Matched current-master screenshots

The fresh clean master BEFORE is `f44e3ef500ea92b24e625bb82687a449a2ee532d`, binary SHA `c1d22a1da550ae611bee78d831c188cec684abe15bc3649193da022577ece7d0`. Final AFTER is `64f15615de388244500a7f7e21c12425df9bbf50`, SHA `499a71ba2cdd91f02a5cdb7f0f89c362a8e7198d81e22264ce8b95eeb08e66d5`. They are separate frozen dev opt0/debug0 `debug-server` builds. Build identities and logs are archived without executables.

Two isolated strict replays use the exact same full fixture, seed42, 120 neutral boot frames and the first1380 buttons from final healthy repeat1. `actions[].buttons` are concatenated in order; stale `actions[].frame` annotations are not a scheduler. Each replay sends and observes one physical frame at a time and keeps all1500 PNGs plus `frame-manifest.jsonl`, whose `frame_count` is exactly1–1500 with no gaps. Both replays exited0. These are matched-prefix evidence, not additional full SAVE controls.

| Matched frame | BEFORE | Final AFTER |
|---|---|---|
| 930 | PlayerSendOut and “Go! BULBASAUR!” | Old Man already uses the ball |
| 1050 | Bulbasaur back / OLD MAN HP HUD | Old Man back / no player HUD; same used-ball caption |
| 1220 | Generic “You won!” | Actual caught caption with CaughtMon playing |

The same prefix returns BEFORE to the field at1232 and final at1426. Final's last `Tink` sample is1148; CaughtMon101 runs1212–1356, then the caught caption stays through BattleOver until the field return. Original controls independently observe145 active caught-sound frames too. Equal observed sound-frame counts do not establish waveform or whole-scene clock parity. The three pairs under `docs/screenshots/old-man-tutorial/` are untouched recorder PNGs; `screenshots.json` maps their physical frame, source file and SHA.

## Tests and archive

Final app `old_man` tests:2 PASS. The audio test executes both healthy and HP1, renders PCM, checks all three sequencer channels remain CaughtMon until completion, verifies the ball animation is finished at jingle entry, and requires all voices to finish before the field return. The renderer test checks the portrait across intro/menu/text/BattleOver, absent player HUD, retained enemy HUD and an ordinary-battle control. Final core2885 PASS; the earlier complete-candidate core `old_man`4 PASS and parent post-capture8 PASS remain identified separately. Latest-master integration is performed separately after this documentation handoff.

`evidence.zip` stores each exact file once by SHA and maps every original path back to its unchanged blob. It retains original/native frames, actions, states, valid ordinary SRAM, full fixtures, frozen identities, test/build logs, intermediate captures and failed drivers. ROMs, executables and emulator states are excluded. `archive.json` records its SHA and every archived path has been read back and verified. Run:

```bash
python3 extract_evidence.py evidence.zip --verify-only
python3 extract_evidence.py evidence.zip /tmp/old-man-evidence
```

The animation differential verdict is **PARTIAL**: this verifies the tutorial's ordering, modal return, portrait/HUD, native full-channel completion and declared save boundaries. It does not establish original/native animation geometry, input-clock or waveform parity, all starting dex configurations, whole-campaign reachability, full mainline completion or the separate Silph timing prototype. Every source setup modification precedes validation; failed hook/approach/setup attempts are retained and excluded from passing controls.

最新整合 master `dfeefa34`：`3c7b43e4` 的完整 core 2885、debug app 272 通过，39 个已有忽略项，0 失败。证据与完整日志见 `integration.json` / `integration-logs.zip`。原始录制及截图仍固定到实际 `f44` / `64f` 构建，不重贴为整合构建。

已整合中毒流程 master `7310d298`。生产整合提交 `ad48d4d7` 完整 debug app 281 通过、39 个已有忽略项、0 失败；完整 core 2888 通过。完整日志见 `master-7310-integration.json` / `master-7310-integration-logs.zip`，原截图与录制身份未重新标记。
