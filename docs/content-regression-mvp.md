# m01–m10 支线内容回归 MVP

这一路验证的是“原版明确存在的支线交互能否在运行中的重制版发生”，范围限定在 Route 1 和常青森林。沿用 `playthrough.Game` / TCP DebugClient，但独立于主线 milestones。所有预期来自固定 `pret/pokered` revision `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`；本次选用的 9 个原版文件与采样时 master `a1a22aaf84d1675bcdbaeb194592379d586d838e` 逐字节相同。

当前套件已扩展至 10 例；满包地面道具问题的修复与验证见 [修复记录](full-bag-pickup-fix.md)。下方初版实测数据保留为发现过程证据。

## 运行

```bash
cargo build --bin pokered-app --features debug-server
python3 scripts/content_regression.py --list
python3 scripts/content_regression.py --repeat 3 --output /tmp/content-new-run
python3 scripts/content_regression.py --only forest-visible-full-bag-keeps-object
python3 scripts/content_regression.py --calibrate
```

Python 标准库，无新增 Python 依赖。每例独立开新游戏、自动探测空闲端口；输出目录必须不存在或为空，以防旧存档混入。默认在 `/tmp/pokered-content-*` 保存 `report.json`、每例 `protocol.jsonl`、关键 `observations.json`、游戏日志和存档。任一断言失败退出码为 1，仍继续其余场景；用新目录复跑，不覆盖证据。校准命令故意把 Route 1 首份 Potion 奖励期望从 1 改为 2，复用同一真实交互与原有断言，预期退出 1。

## 覆盖与前置条件

每例从真正的开机流程进入卧室，随后 debug 注入一只 Lv5 Bulbasaur，必要时填充 20 个互不相同、且不含待领取物品的背包栈，然后 warp 到局部场景起点。行走至相邻交互位、转身和对话翻页全部使用真实按键；不使用 `skip_dialogue`、不写被测奖励/隐藏状态。保存通过 debug `save`，重新启动新进程并从 CONTINUE 恢复。

这是**带明确状态前置的局部 playtest**：没有从 Pallet Town 连续步行到所有目标、不证明整条路径可达，不替代 fresh milestone 通关。满包用例是系统边界合成状态，不声称这些 20 种道具在 m10 前自然可获得。重启测试验证存储与恢复，不覆盖 SAVE 菜单操作。没有读取重制版脚本或数据来生成运行时预期。

| 用例 | 原版依据与检查 |
|---|---|
| `route1-sample-once-reload` | NPC 初始 home 为 (5,24)，真实追随其游走位置交互；首次 Potion ×1，重复及重启后只说售卖精灵球，无重复奖励 |
| `route1-full-bag-consumes-offer` | 原版 `CheckAndSetEvent` 在 `GiveItem` 前：满包虽然失败，样品机会仍永久消耗；此处刻意保留 Gen I 行为 |
| `forest-npc-position-dialogue` | 静止 NPC1 在 (16,43)，显示与朋友来寻找宝可梦对战的文本，背包不变 |
| `forest-visible-antidote-once-reload` | (25,11) 可见 Antidote ×1，领取后消失；重复、地图重进、保存重启不再出现或多给 |
| `forest-visible-full-bag-keeps-object` | 满包拒绝领取；重复及重启后保留 Antidote；实际丢弃一栈后可领取，重启不重复 |
| `forest-potion-full-bag-retry` | (12,29) Potion 同样覆盖满包拒绝、丢弃后领取与重启 |
| `forest-pokeball-full-bag-retry` | (1,31) Poké Ball 同样覆盖满包拒绝、丢弃后领取与重启 |
| `forest-full-bag-existing-stack` | 20 格已满，但已有 Poké Ball ×1：正常合并为 ×2，隐藏对象并保存 |
| `forest-hidden-antidote-once-reload` | 面向 (16,42) 隐藏 Antidote ×1；重复和重启后不重复获得 |
| `forest-hidden-full-bag-repeatable` | 满包时先显示 found 再显示 no room；不置获得位，第二次仍可发现并拒绝领取 |

每例到源文件的映射、明确预期和源 SHA-256 位于 [oracle.json](../scripts/content_regression_fixtures/oracle.json)。离线保留原版源文件并校验哈希，运行时无需联网。

## 实测发现（初版历史记录）

确认一个内容 bug：**常青森林可见 Antidote 在背包满时永久丢失**。运行中背包仍为 20 栈、Antidote 数量 0，但 NPC `text_id=5` 变为 `visible=false`，`EVENT_GOT_VIRIDIAN_FOREST_ANTIDOTE=true`，`__OBJ_HIDDEN_VIRIDIAN_FOREST_OBJ_5=true`；对话错误显示 `RED found ANTIDOTE!`。原版 [PickUpItem](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/events/pick_up_item.asm) 在 GiveItem 失败时不会隐藏道具。这里只交付失败用例与证据，不修改游戏行为。

初轮还出现过一个检测器误报：隐藏道具满包对话由原生引擎提供，`script_effect=None`；采集器只读脚本 text 得到空字符串。协议证据显示三页真实文本正常。已改为同时采集 `dialogue_state.waiting_for_input=true` 的完整页文本，再完整重跑。它不计为产品 bug，说明统一观测接口的成本不能忽略。

## 运行记录

2026-09-08，重制版 `b647e4de7fb4970db04c983d3bed25a0f8ae0bb9`，debug profile + debug-server。
全套 7 例 × 3 轮，合计 **140.423s**，每轮约 **46.8s**；18 pass / 3 fail，3 fail 都是同一满包可见道具丢失。没有把首次隐藏道具采集器误报计入此结果。

| 用例 | 通过次数 | 平均耗时 |
|---|---:|---:|
| `route1-sample-once-reload` | 3/3 | 7.90s |
| `route1-full-bag-consumes-offer` | 3/3 | 8.29s |
| `forest-npc-position-dialogue` | 3/3 | 5.14s |
| `forest-visible-antidote-once-reload` | 3/3 | 7.23s |
| `forest-visible-full-bag-keeps-object` | 0/3 | 5.34s |
| `forest-hidden-antidote-once-reload` | 3/3 | 6.77s |
| `forest-hidden-full-bag-repeatable` | 3/3 | 6.12s |

随后为失败用例补充保存重启证据，单独又跑 3 次，**21.506s**，3/3 仍失败；每次重启后该对象仍隐藏、奖励仍为 0。强化后该用例约 7.2s，整套预期增加约 1.8s。校准错误期望测试 **5.451s**，报告 `Route1 first gift expected 2 Potion, observed 1` 并返回 1。

全协议与存档本地保存在 `/tmp/content-mvp-final`、`/tmp/content-mvp-loss-restart`、`/tmp/content-mvp-calibration`；可审阅摘要及重启后状态已保留到 [content-regression-evidence.json](content-regression-evidence.json)，不依赖临时目录长期存在。

## 限制与下一步

这 7 例是固定原版资料驱动的发现探针，还不是 AI 自主探索器，也未运行原版 ROM 验证内容。无需复杂代理即可在正常主线以外发现真实异常，是此阶段的投入产出优势。没有把上述 bug 的多个状态表现当成多个问题，也没有因为原版行为“不合理”而改写预期。

NPC 测试仅覆盖一名游走 NPC 的 home 与交互、一名静止 NPC 的位置与对话，不证明完整活动范围；隐藏道具满包测试验证重复可发现，尚未覆盖腾空背包后成功领取。后续可从已取得的真实 milestone 存档分叉，增加不 warp 的步行可达性、满包后丢弃一件再领取、条件对话与地图支路。最终应把探索失败轨迹缩减为同类稳定场景。
