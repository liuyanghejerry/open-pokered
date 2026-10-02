# 后半流程修复对照

前图：`72ff719b39634c153cb82d3f3ece200bd413c4e0` 的独立 baseline binary。后图：`fix/fidelity-late-events` 本次修复构建（尚未集成其他代理的改动）。每例从同一 SaveData 模板开始，binary/save/sidecar 均隔离；这是定点种子验证，不是从新游戏连续通关。

| 场景 | 同一截图帧 | 修复前 | 修复后 |
|---|---:|---|---|
| Lance 入场 | 600 | 玩家停在 `(22,15)` 的走廊 | 按原作反向 RLE 走37步到 `(6,11)` 并锁门 |
| 塔5F净化区 | 122 | 治疗文字开始，地图正常显示 | 原作白色 fade 的第二调色板阶段；音乐保留 |
| 奖品选择 | 223 | 满队/满箱的 Abra 兑换已扣180币 | 尚停在新增的 YES/NO 确认，未收费 |

随后完成 YES 的容量失败分支：修复前币数 `9999→9819`，修复后仍 `9999`。Lapras 满队/满箱后 `EVENT_GOT_LAPRAS` 修复前为 true、修复后为 false；Kabuto 满容量交付后，修复前清除了待领取状态，修复后保留 `EVENT_GAVE_FOSSIL_TO_LAB` 和 `EVENT_REVIVING_KABUTO`。同名 JSON 保留所有 debug 命令/响应和截图时完整状态，`-input.json` 为输入模板。

此独立 late binary 未集成系统代理的 `GivePokemon` 满容量底层改动；满容量不写 Pokédex、不改变 blackout 目标等最终行为须在合并版本验证，不能从此日志宣称通过。

验证：`cargo test -p pokered-core --lib fidelity` 27项通过（其中11项本次新增矩阵）；`completed_static_encounters_hidden_from_persistent_events` 的12种静态对象持久事件恢复通过，覆盖新增 Mewtwo 接线。

复现命令（先加载 `/workspace/onboarding/env.sh` 并构建 debug-server）：

```sh
python scripts/verify_late_fidelity.py \
  --binary /path/to/audited/pokered-app \
  --template docs/audits/2026-10-02-full-fidelity/evidence/late-base.json \
  --label before \
  --output docs/screenshots/2026-10-02-late-fidelity
```

对应后图改为修复 binary 和 `--label after`。PR 应按 `AGENTS.md` 用本分支绝对 raw URL 嵌入 PNG 前/后图。
