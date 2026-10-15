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

这里的前图是私有依赖组合阶段的对照，**不是 literal master**。发布阶段在 #267 合入后另补实际 master 的前图与控制；本档案保持原身份。

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

原作和原生的战斗 RNG、HP／经验以及全场时钟不强行匹配。原生实机控制使用 `--no-audio`；HealAilment 事件由单测检查，不声称 PCM／三声道波形一致。实际变身控制覆盖百变怪；共享原身份入口还服务 Mimic，没有新增 Mimic 实机结论。字体、中文、拼音输入法、对话框布局以及 IRQ 时钟不在本项审计范围。

## 证据

[verification.json](verification.json) 汇总真实身份和终态；[archive.json](archive.json) 给出 ZIP 哈希。`evidence.zip` 仅收显式列出的 935 控制及上述已接受的 930 原作控制，1378 条原始路径按 346 个 SHA256 内容去重。打包时全部字节与本地原始记录逐项相等，保留输入、PNG、原始 SRAM、状态、原作只读 hook、冻结身份、测试／GBA 日志，以及早期测试期望／驱动错误和真实 GBA 失败。没有 ROM、可执行文件或模拟器 state。

```bash
python3 docs/audits/battle-pp-restore-caller/extract_evidence.py --verify-only
python3 docs/audits/battle-pp-restore-caller/extract_evidence.py --output /your/local/evidence
```

fixture 与脚本保留原始路径和构造说明；运行原作仍需要合法本地 ROM 和所记录的外部初始地图 state，这些未放进档案。旧驱动通用 scope 字符串中的 `master32f` 是继承的祖先标签；AFTER 的实际冻结头是 `af3f`（含 `1dff`）。活跃成员为取得胜利会继续消耗 Tackle，保存检查因此验证恢复的 Growl，全部四槽 PP 另在敌方行动前的实际 FIGHT 菜单核对。原始脚本／记录未据此改标。
