# 月见山楼梯导航工具修复

基线为 master `959b3e38`。本次只修改 Python 主线驱动和导航测试，没有修改游戏引擎、地图或剧情。

## 问题与原作依据

真实全新主线 retry2 的 m12 在 `nav_warp(5,5,"MtMoon1F","MtMoonB1F")` 失败：首次 `nav_to` 耗尽 30 次预算，后备 `nav_to_map(5,6)` 也耗尽预算。最终为月见山一层 `(16,10)`，frame 203544，玩家和 warp 均 Idle，没有对话或脚本。该位置到楼梯仍有可走路径；失败记录不能证明楼梯触发器卡死或重入。

原作 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c` 的 [OverworldLoop](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/overworld.asm) 连续调用两次 `DelayFrame`。普通步行的八次移动推进需要 16 个渲染帧；骑车加速的四次推进需要 8 帧。冻结的当前引擎中，单格真实按键录制也观察到第 16 帧完成步行。驱动仍使用步行 8 帧、骑车 4 帧的旧预算，计划三格通常只走到两格就必须重规划。洞口 `(14,34)` 到目标楼梯的初始 BFS 路径为 90 格，30 次预算不足以容纳这些额外重规划。

原作 [MtMoon1F_Object](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/data/maps/objects/MtMoon1F.asm) 中的登山男固定在 `(5,6)`，正好是后备路径按方向顺序选择的第一个内侧邻格。原驱动只看地形可通行，把固定对象也当成会离开的临时 NPC，因而空等 `30 × 200` 帧。

## 修复

- 普通导航的 `movement_frames` 使用步行 16 帧、骑车 8 帧；自行车道逆坡为 16 帧。脚本移动的既有预算保留。
- 用实际观察到的 NPC 索引和 canonical 地图的 `Stationary` 数据识别固定阻挡，正在行走或有脚本路径的对象仍可等待。游走 NPC 即使暂时静止，也保持等待机制；缺少元数据时同样保留等待。
- 楼梯后备路径跳过固定 NPC 占据的内侧邻格，继续使用真实跨地图路径和楼梯。登山男占据 `(5,6)` 时改从 `(5,4)` 接近。

重试次数、指定目标地图检查和到达检查没有放宽。

## 实际构造存档验证

冻结 binary 的 SHA256 为 `503ab0f626ef1839862b7ed8059ed51154530b1c21bb4b0af23905f3e9979fcd`。各运行使用独立目录、独立 TCP 端口、seed 42 和完整逐次捕获 PNG。运行后没有 warp、修改存档/flags/队伍或 debug 自动导航操作；由现有驱动执行按键、战斗和对话快进。

这是显式构造场景，不是原始 m12 的运行时续存档：从 canonical SaveBuilder 模板进入月见山一层 `(14,34)`，朝左，钱 2903，一枚徽章。常规场景加载了失败记录中的全部 18 个 true `EVENT_` flags，初始 `get_flags` 核对一致且没有额外 true `EVENT_`。原失败的其他辅助对象 flags 位于真新镇、研究所、常磐市和 22 号道路，没有复制为本场景的运行时对象状态。为隔离训练师战败对导航结果的影响，队伍显式改为满血妙蛙花 Lv50（150 HP），招式 Tackle/Growl/LeechSeed/VineWhip，PP 24/40/10/10。原始努力值、RNG 续状态、NPC 战后位移和 field-loop 阶段未恢复。

| 场景 | 普通步行预算 | 结果 | 总捕获数 |
| --- | --- | --- | --- |
| 常规 flags，只有 `nav_to(...,tries=30)` | 8 帧 | 走 53 格后停在一层 `(20,7)`，未收敛 | 1360 |
| 相同快照，只有 `nav_to` 和已有 warp handoff 等待 | 16 帧 | 完成 90 格，到 B1F `(5,5)` | 4207 |
| 额外显式预设一层七位训练师已击败，只有 `nav_to` | 8 帧 | 相同 30 次预算失败于一层 `(20,7)` | 1360 |
| 相同全击败快照，只有 `nav_to` 和已有 handoff 等待 | 16 帧 | 完成 90 格，到 B1F `(5,5)` | 2905 |
| 常规 flags，8 帧预算加新的固定 NPC 后备路径 | 8 帧 | 从未收敛中恢复，经 `(5,4)` 到 B1F `(5,5)` | 5319 |

每组 A/B 的输入快照逐字段相同。90 格按按时间排序的 overworld 位置记录计数，跨战斗 handoff 保留前后记录；每次位移都是同地图的一个相邻格。成功结果均为 overworld、指定 B1F `(5,5)`、移动和 warp Idle、没有运行中的脚本或对话。`nav_to` 可在源楼梯的淡出期间返回；验证随后只观察已有 handoff，没有追加路径预算或使用后备路径。

全部 PNG 已读回验证为 160×144，capture index、文件名和 manifest 一一连续对应。manifest 的 runtime `frame_count` 偶有跳号和重复，不把该字段作为原作硬件时钟或逐帧视觉一致性证据。

另一个保留妙蛙草 Lv17、HP33/50 的构造重跑在训练师战后回到真新镇，4918 帧，明确记为失败；本次没有证实其战斗差异原因，也没有把它解释成旧存档旁文件污染。新的全主线 m12 和后续流程仍需从头重跑确认。

## 自动回归与证据

全新开机验证 `audit-844-fresh-main-flow` 在 m01–m08 通过后，m09
首次森林训练师战败正常回城。恢复训练入口仍有一个 24 帧的常磐市南口
调用，停在常磐市 `(20,35)` 后报错，frame 35785。该运行退出 1，
没有走到 m12，证据和完整视频保留。这个遗漏调用也更新为与已合入
#229 相同的 40 帧按键 / 44 帧观察预算；未修改游戏引擎、目标地图
或到达检查。新的全主线需要从头重跑，不把本次失败算成月见山通过。

`python3 -m unittest discover -s scripts -p test_playthrough_navigation.py -v`：46 项通过。新增覆盖固定登山男与暂时行走/脚本/游走对象的区分、固定目标不耗尽等待预算、楼梯后备路径避开固定对象，以及独立 16 帧移动模型下的完整 90 格路线。现有 map mismatch、错误 warp 目标、黑屏后回城、方向地毯和游走 NPC 等断言保持。

本机证据根目录为 `.task-tmp/fidelity-39-plus`，主线原始证据在 `audit-838-full-main-flow-retry2/`；构造过程、初始 flags/NPC、最终完整 state、game log 和全部 PNG/manifest 在：

- `audit-844-budget-only-8/`、`audit-844-budget-only-16-settled/`
- `audit-844-budget-only-8-full90/`、`audit-844-budget-only-16-full90/`
- `audit-844-navigation-scope-8/`、`audit-844-navigation-scope-16/`
- `audit-844-mtmoon-fixed-entry/`（保留的 HP33 战败）

诊断录制脚本在同一证据根目录的 `audit-838-mtmoon-diagnosis/budget-only-probe.py` 和 `nav-scope-probe.py`。不因这些导航场景宣告原作画面、完整战斗或全主线一致。
