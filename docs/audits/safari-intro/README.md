# Safari encounter intro

Safari 野生出现提示确认后，按原作 `StartBattle` 的显式40帧等待进入菜单；保持 RED 背影、不派出队伍宝可梦、不显示玩家HP栏。临时队伍球仍保留。原作另有实测8帧前置工作，不把总48帧写成常量，也不声称整段时钟/轨迹/音频一致。

## 当前集成

#259 实际合入后整合 master `b50783c53980529e66561d525178512def859251`。全部2170项编译输入及 docs 外 2689 项 Git blob/模式等于受测 v2 `5a2cefe2a49dd3acdc9df7cbf46b813b23e55485`。v2 debug-server app316通过/39忽略、determinism10通过。core2914通过来自原始 v1 `270f52955da2b84d2fcc0ce7059dae5e62a7d386`，未伪称重跑：两版295项核心文件和其他编译输入完全相同，仅 app cfg(test) 的 Safari文本构造补了 backdrop 字段；首次 v1 app 编译失败保留。

`publication-967.json` 与 `publication-967-logs.zip` 提供身份、逐项比较及仅本PR的日志。本次没有重建/重录；新远程head仍须三个工作流全绿。

## 历史实际证据及范围

截图保持 BEFORE6085c987 / AFTER6abff9bf 身份，同完整fixture/seed3/2703帧按键前缀与模拟帧2703。原作、修复前后各两次 RUN→普通SAVE；保护数据与完整原始证据见 verification.json / screenshots.json / evidence.zip。构造活动 SafariCenter 存档，不覆盖首次入场缴费；原作与原生自然物种不同，不注入随机结果。

## 旧集成记录（历史，不代表当前重跑）

# Safari encounter intro

Safari encounters now keep RED’s back picture, omit the party Pokémon HP HUD,
and enter the BALL/BAIT/ROCK/RUN menu after the original explicit, unskippable
40-frame `StartBattle` delay. The extra “Go!” prompt, Pokémon send-out animation,
and persistent party-ball row are skipped. The temporary party balls in the
wild-appearance text remain.

The original selects `RedPicBack` and branches from `StartBattle` directly to
`DisplaySafariMenu` for battle type 2. Ordinary battles and the Old Man tutorial
have separate paths. Existing `gfx/player/redb.png` matches the original asset;
no new graphics are included. Capture continuation/audio and the last Safari
ball message are separate changes.

`verification.json` records frozen builds, actual input/SAVE controls, exact
40-frame boundaries, regression baselines, full test and GBA checks.
`screenshots.json` records a same-fixture/seed/input-prefix/simulation-frame pair.
`fixture.json` is the complete preboot modern snapshot used by the native trials.
Both builds share the complete input prefix through frame 2703. BEFORE then
needs one more A plus 300 neutral observation frames before RUN; AFTER goes
directly to RUN, so those later input sequences are not claimed identical.
The resulting SRAM clock/checksum differences are recorded without normalization.
It is a constructed active Safari Center visit, with 30 balls and 100 steps;
these trials do not test admission/payment or a fresh full playthrough.

The original trials use an explicitly documented preinput bootstrap plus party,
box, player, dex and event facts from a normal native SAVE. After construction,
validation uses only normal walking/menu/RUN/SAVE inputs and read-only hooks.
Source and native natural encounters differ (NIDORAN♂22 / VENONAT22); enemy pixels,
DVs, encounter entropy and whole-intro timing/trajectory/audio-waveform parity
are excluded. The source hooks reach `StartBattle` 8 frames after the text ACK,
then reach the menu after its explicit 40-frame delay. Only that explicit delay
is implemented here; the measured total of 48 is not a runtime constant or a
whole-clock parity claim.

The compact `evidence.zip` retains the original input logs, one PNG per physical
frame, frame manifests, read-only observations, normal SRAM saves, source/build
identity manifests and excluded failed controls. ROMs, executables and emulator
states are excluded. Verify it without extraction:

```sh
python3 extract_evidence.py evidence.zip --verify-only
```

Replace `--verify-only` with an output directory to restore the original paths.
Both content hashes and byte lengths are checked. The detailed source bootstrap
state remains private because emulator states/retail ROM data are excluded.

Latest tested integration: master `216cb034` merged at `3bd9ed30`; full core
2,889 and app 281 tests passed (39 ignored), with all seven GBA performance
scenarios inside the existing budget (trainer update 232 ticks). The two Rust
files changed by this fix remain byte-identical to the frozen `6abff9bf` capture
build. Historical captures retain their 6085/6abff identities.

此前整合 master `6d30ba5a`（#257，训练家图鉴修复）：完整核心 **2889 通过**、debug-server 应用 **290 通过/39 忽略/0 失败**。精确源码与日志见 `master-6d30-integration.json` 和日志包。原作与原生实际捕获继续保留各自冻结身份；本次整合不重标截图或存档。

此前整合 master `8eb08f56`（#262，FLASH 触发修复）：完整核心 **2889 通过**、debug-server 应用 **290 通过/39 忽略/0 失败**。精确源码与日志见 `master-8eb-integration.json` 和日志包。原作与原生实际捕获继续保留各自冻结身份；本次整合不重标截图或存档。

已整合最新 master `f0787e19`（#268，巨石测试隔离；包含 #258 中毒与进门顺序）：core 2890 通过/0 忽略；app 297 通过/39 忽略，全部 0 失败。当前受测源码 `aac4a78fe58febd3d2f7f97980c94bc420e2d6b0`、编译包装脚本与日志见 `master-f078-integration.json` 和独立日志包。历史原作/原生录制与 SAVE 保留各自冻结身份；本次新 head 仍需远程三个工作流全部通过。
