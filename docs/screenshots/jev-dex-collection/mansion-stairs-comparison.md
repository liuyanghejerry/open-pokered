# 玉虹大厦后门楼梯：自动迈步碰撞

- 前：master `0f94a6fa`，独立 target 目录构建。
- 后：本 PR 的门口自动迈步碰撞修复。
- 两边同一隔离场景、seed 42、原版锁定 gfx `1e96034092686d006e863cace09e87273051a3d8`。
- 起点 CeladonMansion1F `(3,1)`；8 帧 LEFT + 112 帧无输入，
  同步 `press_timeline`，第 120 帧 `capture_frame`，原生 160×144 PNG。
- 前：到达 2F 后被无条件 DOWN 推进墙格 `(2,2)`。
- 后：碰撞阻止自动 DOWN，停在可通行楼梯 `(2,1)`，可以向右继续。

这些截图使用 `--skip-intro --warp` 隔离种子，仅验证引擎，
不属于正式图鉴或录制 lineage。脚本、完整状态与日志保留在
`.artifacts/jev-navigation-repair-20261002/`。

原版依据：`PlayerStepOutFromDoor` 只模拟一次 DOWN；
`JoypadOverworld` 在读取该输入时把索引从 1 减到 0，因此后续
`CollisionCheckOnLand` 不跳过碰撞。没有修改原版门／楼梯 tile 表。

- [自动迈步](https://github.com/pret/pokered/blob/1e96034092686d006e863cace09e87273051a3d8/engine/overworld/auto_movement.asm)
- [输入与碰撞检查](https://github.com/pret/pokered/blob/1e96034092686d006e863cace09e87273051a3d8/home/overworld.asm)
