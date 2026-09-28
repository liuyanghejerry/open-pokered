# PR 验证（2026-09-29）

- PR 基线：`origin/master` `758f40e`；与此前截图基线 `403514b` 文件树一致。
- PR 分支：`fix/gba-audio-battle-sequencing`。
- `cargo test -p pokered-app --features debug-server --lib --test cable_club_flow --test evolution_audio_timing`：110 + 8 + 2 passed；1 项按需截图夹具 ignored。
- `python3 -m unittest scripts/test_gba_performance.py scripts/test_gba_memory.py scripts/test_gba_frame_timing.py`：27 passed。
- 最新 master 的独立工作区与 PR 分支运行同一 `visual_verify_battle_sequence.rs` 夹具，均通过；同帧截图与输入记录位于 `docs/screenshots/gba-battle-pr/`。
- `git diff --check` 通过。其余核心／ROM／全招式验证见 README。
