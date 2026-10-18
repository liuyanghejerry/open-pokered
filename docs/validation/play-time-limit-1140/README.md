# 255 小时计时封顶

修复前到达 255 小时后会显示并保存 `255:59`；原作 `engine/play_time.asm` 的 `TrackPlayTime` 先将帧、秒、分钟逐级归零，在小时达到 `$ff` 时设置封顶标记，因此应停在 `255:00:00:00`。修复同时保留原作 `cp 60` 的相等进位语义。已有封顶标记的时间保持存储值，遵循原作的提前返回分支。

原作依据：pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`，`engine/play_time.asm`；本地原作 ROM SHA-256 和 12 组控制见 [证据](evidence.json) 与核心的 `play_time_rom_cases.json`。原作验证器直接解释这一小段 ROM 指令；输入忽略倒计时不属于 `PlayTime::tick` 的验证范围。异常字节控制用于确认原作指令语义，不声称自然可达。

## 实际游戏验证

同一份普通 SRAM 的计时离线设为 `254:59:59:59`，从正常开机选择 Continue，经训练师卡、正常菜单 Save，再由独立进程 Continue。前后各两次，截图分别逐字节相同，存档也分别相同；第 794 帧截图仅 `(92,43)-(100,50)` 计时数字区域不同。前后存档仅分钟/秒/帧和校验和不同。未调用运行时状态设置、debug save 或 skip-intro。

```bash
# 仓库根目录，先按 CLAUDE.md 准备 gfx 并构建含 debug-server 的正常程序
python3 docs/validation/play-time-limit-1140/replay.py /path/to/pokered-app /path/to/new-output 0
# 在修复前的 master 构建上，最后一个参数改为59；输出目录必须尚不存在
```

`fixture.sav` 是受控边界存档，不是全新通关证明。修复前截图在 master 检出下捕获，使用源码与该 master 等价的冻结普通构建；修复后使用提交 `30d57606771a0a36821c021a8214b4acaa913e8b` 的实际冻结构建，身份见证据。已提交的可移植复现脚本另跑一次，图像和存档与修复后两次完全相同。

## 原作控制与回归

```bash
python3 docs/validation/play-time-limit-1140/verify_original.py \
  --rom /path/to/pokered.gbc --symbols /path/to/pokered.sym --output /path/to/original-controls.json
cargo test -p pokered-core --lib
cargo test -p pokered-app --lib --features debug-server
```

可移植原作验证器的实际输出与留存控制完全一致。修改前，原作边界回归失败为 `255:59:59:59` 对 `255:00:00:00`；修复后 core 3,134 项通过，应用 381 项通过、39 项既有 ignored。此为专项逻辑、界面与存档验证，未替代最终整合 master 的完整通关回归。
