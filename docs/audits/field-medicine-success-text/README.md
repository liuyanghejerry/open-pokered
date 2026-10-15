# 场外用药提示：当前发布身份

#261 head `a9321f2111a073b295b04df65fb3e8d2a84d1f76` 实际合入 `f9ef53e21e053c95537c4d73a0389ab1d4a5e9d7` 后整合实时master `f9ef53e21e053c95537c4d73a0389ab1d4a5e9d7`。本轮受测源码为 `744ebbb9b891db442488f79f3d22f462e698956d`，2173完整compiler、2691项docs外NUL blob/模式/literal symlink、702GFX逐项等同发布源码。相对该master只有bag_use.rs与dialog_text.rs；261单帧地图指纹优化作为已合入依赖保留。

1008七阶段实际全部通过：debug app330/39忽略、default app319/37忽略、determinism10及GBA build/fix/record/compare的31项不变门槛。core2934保留原a07a/v5运行，只经1901core/data/vendor/Cargo组件证明；不是新core运行、不是旧整份2173与744相等。见publication-1010.json与publication-1010-logs.zip。仍需新发布head的三个远程工作流全绿。

下方原作6/原生前6/后6普通SAVE和三组952帧截图都保留各自历史身份；严格截图重放没有额外SAVE。本轮没有新actual，旧失败与局限保持。

## 历史原证据（身份保留）

# Field medicine success text

Field bag medicines now use the original selected member's English success text: HP recovery says `SPROUT / recovered by 20!`, paralysis cure says `SPROUT's / rid of paralysis!`, and revival says `SPROUT / is revitalized!`. The same original table supplies the Antidote, Burn Heal, Ice Heal, Awakening and Full Heal messages. Full Restore uses Full Heal's message when it only cures an ailment at full HP, otherwise the HP recovery message.

The change only constructs the returned successful text in `items/bag_use.rs` and adds narrow dialogue-localization templates preserving existing Chinese translations. The canonical party, item effect, one-item consumption, HP animation, 50-frame notice and A/B continuation remain the existing field menu owners. This is independent of the battle medicine change in PR267.

Original reference: revision `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, `engine/items/item_effects.asm` (`ItemUseMedicine`), `engine/menus/party_menu.asm` (`PartyMenuItemUseMessagePointers`) and `data/text/text_2.asm:1384–1432`. The controlled ROM hash is `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`.

The three actual field-menu cases are target row1, nickname SPROUT: Potion HP1→21/35; Parlyz Heal paralysis→none at35/35; Revive HP0→17/35. Each provider/version has two ordinary START→ITEM→USE→target→600 frames without ACK→A→SAVE controls. All fixtures are constructed legally before boot/input; there are no gameplay setters, runtime RAM writes, forced battles or reloads during validation. These are accelerated constructed scenarios, not a fresh campaign.

The original six controls keep their recorded script/version identities. The historical readonly native six controls use clean8eb and are retained separately; they are not relabeled as current master. The fix matrix independently rebuilds clean05f6 master BEFORE and the production fix AFTER into new targets. Every native before/after pair uses identical full action records, exact fixture bytes and complete ordinary 32768-byte SRAM. Each group's two repeats also has identical SRAM. All saved two-member88-byte records match the original expected HP/status-only changes; PP/moves/DVs/EXP/stats and OT/name tables remain unchanged. Native controls additionally protect full current box/PC/HOF, cash/coins/dex/events; original controls protect current-box WRAM, cash/dex/events and name tables.

For screenshot attribution, all three cases additionally replay the same complete seed42 physical input prefix, one frame per command from0 through952, on the independent BEFORE and AFTER executables. Every frame records and asserts `frame_count`. The final17 frames are captured contiguously; the committed comparison pairs are frame952. These bounded replays do not add SAVE controls or claim original/native timing parity.

Meaningful units cover all healing/revival items, nicknames including the full10-character name, every status template, both Full Restore message branches, no-effect unchanged state, PP restore unchanged behavior and preserved narrow Chinese translations. Full core/app and the unchanged seven-scenario GBA performance gate are recorded in `verification.json`.

`evidence.zip` is a content-addressed archive. Its manifest keeps original file paths and each file's SHA256/size; extraction validates every blob before writing. Its inputs are explicitly enumerated920 controls/scripts/logs/identities and the immutable earlier920 readonly custody archive, never a shared audit-directory glob. ROMs, executables and emulator states are excluded. The nested initial archive retains rejected old source symbol/decoder attempts; these are controller mistakes and do not count as accepted controls. A first GBA launch selected a nonexistent mirror workdir before creation, started no process and is retained as metadata.

Limits: new actual controls cover Potion, Parlyz Heal and Revive; the remaining item templates and Full Restore branches have source-backed unit coverage. No font, Chinese wording, dialogue-box layout, detailed HP-bar/icon animation, map restoration, original/native clock/RNG/audio/full-SRAM parity is claimed. Snapshot/load/battle flow is untouched; no new persistent state is introduced.

To inspect the portable fixtures/inputs/screens/SRAM without a game binary, run `python3 extract_evidence.py --verify-only` or `python3 extract_evidence.py --output /your/private/evidence-directory`. The extracted current controls contain exact SaveData JSON fixtures and actions; each strict replay contains the complete `buttons.json` and every per-frame observation. The immutable earlier920 archive has the same manifest/blob format and can be verified/extracted by passing `--archive` to the same tool.

此前整合 master `f0787e19`（#268，巨石测试隔离；包含 #258 中毒与进门顺序）：core 2893 通过/0 忽略；app 296 通过/39 忽略，全部 0 失败。当时受测源码 `fed824303a4f886b78c132a31086db3151d28960`、编译包装脚本与日志见 `master-f078-integration.json` 和独立日志包。历史原作/原生录制与 SAVE 保留各自冻结身份；本次新 head 仍需远程三个工作流全部通过。
