# 战斗中恢复 PP：发布准备与证据索引

## 2026-10-15 本地发布准备（尚未发布）

本次唯一受测发布源码为 `cb8e6b7da2abd7e8b4442ce545c827136f3f71d5`：旧整合 `016a606c` 合入已验证的 `744ebbb9`，仅增加两个渲染文件中的单帧地图指纹复用；相邻 PP 缓存测试与地图像素测试发生追加冲突，保留两者，原冲突与解决记录一并归档。原 PP 最终确认、一次消费和捕获强确认保护不变。`publication-1013.json` 当前明确为 `PREPARED_PENDING_ACTUAL269_MERGE`；只有 #269 实际合入后，独占发布树整合实时 master 并证明源码一致才可发布。

新执行 1011：debug app **334 通过／39 忽略**、默认 app **323 通过／37 忽略**、determinism **10 通过**，独立 GBA build/fix/record/compare 的 **31 项原门禁全部通过**。movement peak3103、trainer224、idle349、Oak2994；七个场景的采样和渲染次数与1008相同。全部2175编译输入（含run-wasm两文件）、2693项docs外NUL路径的Git blob／模式／literal symlink及702GFX在七阶段前后和当前逐项一致。core **2944** 仅保留原 `016a` 运行，依据core/data/vendor/Cargo组件字节与模式同一性；没有重新运行core库，也不把旧整份源码冒称与新renderer相同。

`016a` 的旧整合证据独立保留：1584个实际单帧公开状态及952／1584两图与原`af3f`相同，另400帧真实确认续接只扣一次，原Transform PP10、工作PP[10,0]；**没有新增SAVE**。旧2173 compiler／2692 regular non-doc范围未覆盖两项run-wasm和目录symlink，1001与979提供回溯2175／2693完整范围补证，不修改原manifest或重跑测试。970独立审查、旧四组host日志、合并强ACK冲突解决均在新ZIP。

独立 [949 Mimic 实际验证](mimic-949-report.md) 完成原作／literal`a2fa`前／冻结`af3f`后各一次普通SAVE（共3次）：Ether恢复原Mimic至10，工作招式保持PoisonSting；第二次满PP Ether无效果；再使用复制招式、切换和保存后原Mimic为PP9。旧版本保存PP7。覆盖仅这一Ether路径，不扩为全部PP道具／MimicPPUp／link／嵌套变身或音频结论。[949原始验证](mimic-949-verification.json)、[独立归档](mimic-949-archive.json)与116路径／90blob的`mimic-949-evidence.zip`保持真实身份。

[当前发布准备](publication-1013.json)与`publication-1013-logs.zip`只收有限显式白名单：965／970／979的965投影／1001范围补证以及1011新七阶段日志与源码身份。无全audit目录glob、ROM、exe或模拟器state。旧1378路径主ZIP和69路径literal-master补充ZIP完全不变；历史42次SAVE与949新增3次分组计数，严格截图和965重播不重复计SAVE。

下方各章节是当时 `c6e`／`af3f`／`a2fa` 阶段的原始历史说明。旧“待267合入”、旧测试与GBA总数不表示当前发布状态。四对图片仍为实际`a2fa` BEFORE／`af3f` AFTER，相同fixture、seed42、完整输入前缀及实际帧688／1328／1288／952；没有改标为016a、cb8或未来发布head。正式PR使用绝对raw URL。

## 历史原始说明（身份保留）

# 战斗中恢复 PP：调用方、确认和变身身份

Ether／Max Ether 的招式选择现在显示原作的 “Restore PP of which technique?”，选择和结果页保留队伍与招式列表；Elixir／Max Elixir 的结果页保留队伍。PP 当即恢复并发出 HealAilment 事件，最后一页真实 A/B 确认才扣一次道具，再接原有敌方免费回合。没有增加 Delay50。取消或无效果不扣道具，也不让敌方行动。

原作 `ItemUsePPRestore` 的菜单、GetMaxPP 和写入均读取原队伍身份。百变怪变身后，Ether 因而选择并恢复原来的 Transform，再把整组四槽 PP（含 PP Up 位）复制到战斗数据，保留变身后的种类、招式、属性、HP 与状态。实现复用 `original_identity`、`party_mon` 与新配对的可变入口，没有第二份持久队伍。Max Ether／Max Elixir 在满 PP 且使用过 PP Up 时仍成功消耗的原作缺陷保留。[原作代码摘录](original-pp-transform.md)

结果上下文放在现有 `ShowingText.backdrop`，只保存目标、可选招式行与最终确认要消费的道具。普通分页、默认 Battle 背景和原 EnemyFreeTurnAfterItem 继续使用原流程；完整队伍仅通过既有只读绘制适配器显示。PP 画面避开普通战斗 HUD 的局部重绘缓存。

## 冻结身份

| 角色 | 实际版本 |
| --- | --- |
| 原作 | `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`；ROM SHA256 `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b` |
| 前 | `c6e5762f55d449cbba1c4fb896e63b3634585e25`：待合入 #267（含其负向用药修复）与 master `32f74146` 的组合；二进制 `d6cd45bb7983accf0fdad84db35d574fbd4ce842ea61b1ceee326d876594477b` |
| 后 | `af3f5241fd85b2623c8f9d1e6334cdb11aa07d53`：同一依赖、PP 修复与 master `1dff02a2`；二进制 `0d505cc0c1e078e92ab2ea06949c6dc228d88c79828430ca734b0687f6aac174` |

这里的历史前图是私有依赖组合阶段的对照，**不是 literal master**；上述记录与原 ZIP 保持原身份。#267 合入后的实际主分支前图已另补，见下方主分支对照补充。

## 普通输入和存档

14 次原作、14 次原生前、14 次原生后均通过真实森林训练家触发、菜单选道具、600 帧不确认、真实确认、普通战斗胜利和普通 SAVE。每组两次，覆盖：

- Ether 恢复替补的第二招；Elixir 恢复替补全部招式。
- Max Ether 满 PP 无效果；Ether 招式选择取消并返回背包。
- Max Ether 恢复活跃成员；Max Elixir 恢复替补。
- 合法 Ditto20 先实际使用 Transform，再用 Ether 选择原招式、重新打开 FIGHT 检查临时 PP、换到替补并胜利。

原作前八次复用已独立审核的 930 v2 控制；新增原作 Max 道具四次与 Transform 两次。所有构造均在首次输入前，验证期间没有改 RAM、强制战斗或传送。原作在确认前保留道具 2，且敌方没有行动；成功后才调用 RemoveUsedItem。

原生前后每对 fixture、seed42、完整动作和初始截图相同。12 对普通道具 SRAM 的 32768 字节完全一致；两对 Transform 只改变 `0x2f51` 的原招式 PP 9→10 和 `0x3523` 校验字节。重新打开 FIGHT 时仍是 PoisonSting／StringShot，临时 PP 从错误的 `[15,5]` 变为原作的 `[10,0]`。另保留早期未重开 FIGHT 的两次前版本控制，不混入这 42 次主控制计数。

四组严格逐帧重播逐条验证全部 11056 次单帧输入、实际 frame_count 和状态。配对共用 fixture 与完整按钮前缀，在相同模拟帧拍摄：[截图身份与哈希](screenshots.json)。严格重播本身不另计 SAVE。

## 回归与界限

- 完整 core：2909 通过。
- 完整 app debug-server：315 通过、39 忽略；完整默认 app：304 通过、37 忽略。
- 单测涵盖四道具／活跃和替补／A/B 确认、无效果／取消／濒死替补、最终页一次消费、变身原身份和失败不回抄、PP Up 原作判定、快照和旧快照默认。
- 实际 soft_reset→Title→Continue→新战斗验证旧消费不再继续。Title 可以保留不再更新的旧 BattleScreen；没有声称 bare `load_from_path` 会立即清空战斗或新增加载特判。
- GBA 使用未修改的提交门槛，全部通过：trainer224、idle349、Oak2993。旧 ce29 在旧基线上 trainer233 超门槛的真实失败保留；后续整合已经合入的主干 FNV／同名比较优化后重测，没有调整门槛。

原作和原生的战斗 RNG、HP／经验以及全场时钟不强行匹配。原生实机控制使用 `--no-audio`；HealAilment 事件由单测检查，不声称 PCM／三声道波形一致。实际变身控制覆盖百变怪；共享原身份入口还服务 Mimic，没有新增 Mimic 实机结论。（这是 af3f 阶段的历史范围；2026-10-15 独立 949 补证见上方，不改原始记录。）字体、中文、拼音输入法、对话框布局以及 IRQ 时钟不在本项审计范围。

## 证据

[verification.json](verification.json) 汇总真实身份和终态；[archive.json](archive.json) 给出 ZIP 哈希。`evidence.zip` 仅收显式列出的 935 控制及上述已接受的 930 原作控制，1378 条原始路径按 346 个 SHA256 内容去重。打包时全部字节与本地原始记录逐项相等，保留输入、PNG、原始 SRAM、状态、原作只读 hook、冻结身份、测试／GBA 日志，以及早期测试期望／驱动错误和真实 GBA 失败。没有 ROM、可执行文件或模拟器 state。

```bash
python3 docs/audits/battle-pp-restore-caller/extract_evidence.py --verify-only
python3 docs/audits/battle-pp-restore-caller/extract_evidence.py --output /your/local/evidence
```

fixture 与脚本保留原始路径和构造说明；运行原作仍需要合法本地 ROM 和所记录的外部初始地图 state，这些未放进档案。旧驱动通用 scope 字符串中的 `master32f` 是继承的祖先标签；AFTER 的实际冻结头是 `af3f`（含 `1dff`）。活跃成员为取得胜利会继续消耗 Tackle，保存检查因此验证恢复的 Growl，全部四槽 PP 另在敌方行动前的实际 FIGHT 菜单核对。原始脚本／记录未据此改标。

## 主分支对照补充

#267 合入后，从实际 master `a2fa94f185c08dfb568929a022b2ca66da0e0aa4` 新建独立工作树与构建目标，冻结 BEFORE 二进制 SHA256 `5a4831ccf960d1c00794ec8cd80b43dbc64a181748552b5b6dce6ebb6145f558`。四组重播逐帧复用原 AFTER 的 fixture、seed42 和完整按钮前缀，核验全部 5528 个实际单帧状态与七张同帧画面。AFTER 保持原 `af3f` 冻结身份，没有重跑或改标；本补充不另计普通 SAVE。四组展示图的实际帧分别是 Ether 选择 688、Ether 结果 1328、Elixir 结果 1288、Transform 选择 952。

实际 master 已合并到本修复的 `279a11b96c71871ffc18033b2d9260df5f961863`。唯一冲突是老人教学测试的 ShowingText 构造字段顺序，保留已测试的 `af3f` 顺序及完整教学／用药测试模块。全部 2170 个已编译源文件的路径集合与 SHA256 均与 `af3f` 完全相同，因此沿用上述完整 core、app、GBA 检查；合并未新增生产改动，也未重复构建。

[literal-master-before-verification.json](literal-master-before-verification.json) 记录真实构建身份、四组单帧核验和源文件同一性；[literal-master-before-screenshots.json](literal-master-before-screenshots.json) 映射新主分支前图与原 AFTER。独立的 [literal-master-before-archive.json](literal-master-before-archive.json) 描述 `literal-master-before-evidence.zip`，仅收本次补录、对应的原 AFTER、构建／合并／校验记录。原 `evidence.zip` 的 SHA256 仍为 `00ea89aa328c1c556b55356b2626c3690916b19764e2dcdc7c9b07b67fe73966`，内容未覆盖。新 ZIP 也不含 ROM、可执行文件或模拟器 state。

```bash
python3 docs/audits/battle-pp-restore-caller/extract_evidence.py --archive docs/audits/battle-pp-restore-caller/literal-master-before-evidence.zip --verify-only
```
