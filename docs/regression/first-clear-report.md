# 一周目回归结果（2026-09-09）

**从 NEW GAME 到通关的 m01–m49 全链通过。** 第七轮 fresh 运行退出码0，
实际经历冠军战、名人堂与片尾；在任何调试保存前，以独立进程读取游戏自动存档，
CONTINUE 回到真新镇 `(5,6)`，确认八徽章 `255`、名人堂队数 `1`。

```bash
cargo build --bin pokered-app --features debug-server
PT_DEBUG=1 python3 -u scripts/playthrough.py --until m49 --artifacts /tmp/first-clear-fresh-a7
```

验证采用默认 Bulbasaur 开局。主线由实际按键、对话、战斗、治疗、购买与步行推进，
没有使用 warp、give_*、set_flag、构造快照或编辑存档代替游玩。正常败退后重新步行，
不读档重掷战斗。最终队伍：Zapdos Lv56 / Lapras Lv15 / Venusaur Lv55。

| 验证 | 结果 |
|---|---|
| fresh 主线 | 49/49 |
| 最终 scenarios（包括新增倒下后换人场景） | 11/11 |
| 最终 BDD | 15/15 |
| 导航与战斗策略回归单元测试 | 8/8 |
| 调试二进制构建、skill 校验、diff 检查 | 通过 |

## 证据与可复用流程

- [逐里程碑与历次尝试](first-clear-progress.json)：fresh 与开发续跑分别标注。
- [终点原始观察](first-clear-final.json)：包含名人堂/片尾阶段及独立进程 CONTINUE 状态。
- [更新后的 skill](../../.agents/skills/playthrough-regression/SKILL.md)。
- [完整路线、问题与处理记录](../../.agents/skills/playthrough-regression/references/first-clear.md)。
- 本地通关存档副本：`docs/regression/first-clear.sav`（32768字节，按仓库规则忽略）。
- 原始49份观察与游戏日志：`/tmp/first-clear-fresh-a7/`；驱动日志：`/tmp/first-clear-fresh-a7.log`。

前六轮 fresh 分别在 m09、m10、m15、m15、m15、m16 停止，全部保留证据。
修复集中于驱动：组合碰撞、入口上下文、断崖/箭头地板、动态地图缓存、学习与换人菜单、
剧情门禁、部分步行、短地图数组，以及早期治疗和船上挑战前正常训练。
后期 m11–m49 路线实现在 `scripts/playthrough_late.py`；调试端新增的是只读观察字段。

## 尚未确认的游戏行为

- 阿桔最后一只自爆同时击倒唯一队员时，观察到先发徽章、随后走动触发败退。
- 绿毛虫“吐丝”出现“效果拔群”提示，尚未单独复现核对。

以上两项已记录，未作为已修复的引擎缺陷。此次结论不包含其他初始宝可梦、可选通关后内容，
或画面/音频正确性。
