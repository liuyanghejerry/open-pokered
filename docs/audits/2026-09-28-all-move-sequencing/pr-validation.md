# PR 验证（2026-09-29）

- PR 基线：`origin/master` `758f40e`；与此前截图基线 `403514b` 文件树一致。
- PR 分支：`fix/gba-audio-battle-sequencing`。
- `cargo test -p pokered-app --features debug-server --lib --test cable_club_flow --test evolution_audio_timing`：110 + 8 + 2 passed；1 项按需截图夹具 ignored。
- `python3 -m unittest scripts/test_gba_performance.py scripts/test_gba_memory.py scripts/test_gba_frame_timing.py`：27 passed。
- 最新 master 的独立工作区与 PR 分支运行同一 `visual_verify_battle_sequence.rs` 夹具，均通过；同帧截图与输入记录位于 `docs/screenshots/gba-battle-pr/`。
- `git diff --check` 通过。其余核心／ROM／全招式验证见 README。

## CI 失败项修复

- 自动赶路遇到战斗时等待展示／HP 动画，不把无效按键等待计入 300 次操作上限；保留独立的仿真帧上限，防止停滞无限等待。原 `travel_to_pewter_city_end_to_end` 测试未放宽，复测通过。
- 根据狩猎区新脚本重新生成 `story/graph.json`；没有改变图一致性断言。
- 选项画面快照更新为已审核的空心标记布局；其余画面 hash 不变。
- `cargo test -p pokered-app --features debug-server`：完整运行通过，310 个测试（含 lib/bin 共享测试的重复运行）。
- `cargo test -p pokered-agent --lib`：51 passed。
- `cargo test -p pokered-ui-preview --lib`：58 passed。
