# 剧情节点间探索模式

`scripts/playthrough.py` 的目标是从开机走到指定通关 milestone；新的
`scripts/exploration_playthrough.py` 把同一条真实输入主线变成“节点间探索”
模式。它在每个 milestone 完成后保存一个临时 checkpoint，再从该存档的副本启动
独立探针。探针的奖励、事件旗标和失败不会污染下一节点的主线存档。

```bash
cargo build --bin pokered-app --features debug-server
python3 scripts/exploration_playthrough.py --list
python3 scripts/exploration_playthrough.py --until m10 --seed 73 --samples 2 \
  --artifacts /tmp/pokered-exploration
```

## 两类检查

- `blocked`：先使用 `nav_to` / `nav_to_map` / `nav_warp` 真实走到门禁内侧，
  再注入一次方向键。manifest 断言地图、位置、拒绝对话和相关 flag，验证新区域
  仍被正确阻挡。绝不使用 debug `warp` 作为穿门方式。
- `destination`：从该节点状态中随机抽取若干目的地，真实步行到达并可选择一次
  NPC/物件交互。预期奖励、对话、金钱和 flag 是显式 oracle，不从当前实现输出
  自动生成。

`--seed` 保证抽样可复现；`--samples` 控制每个节点抽取的目的地数量，blocked
探针始终执行。`--only` 可用逗号选择具体探针。每个探针目录都保存
`checkpoint.sav`、`protocol.jsonl`、`observations.json`、`result.json` 和游戏日志；
总报告在 `report.json`。明确违反 oracle 是 `fail`，路径或随机探索预算耗尽是
`inconclusive`，命令仍成功退出；后者不能直接当作内容缺失。明确违反 oracle 的
`fail` 会让命令以非零状态退出。

当前 manifest 覆盖：Daisy 在图鉴前后的对话、城镇地图奖励、常青市隐藏药水、
Route 22 Boulder Badge 门禁、Pewter 博物馆拒票分支、Vermilion Pokémon Fan Club
自行车兑换券、Fuchsia 好钓竿，以及 Route 5/6/8 的 Saffron 门禁。所有 milestone
都有显式 checkpoint 条目，后续新增支线只需在
`scripts/exploration_probes.json` 增加 manifest 场景和必要的通用动作。

报告中的 `checkpoint_coverage` 会同时列出每个节点可用和实际抽中的 probe；
`uncovered_checkpoints` 明确显示本次运行没有探索的节点，不会把空池或
`--samples 0` 误报成已覆盖。

模式只负责发现和保留证据，不自动修改游戏实现，也不把失败结果写成新的正确
基线。可以先用 `--until` 验证早期节点，再逐步增加后期 probe，避免把完整主线
和所有支线的耗时混为一个门禁。
