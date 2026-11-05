# NPC 草地遮挡

修复 NPC 下半身盖在草叶上的问题。静止、当前一步走出草地、玩家从旁经过三段固定窗口与普通 Red 原作的 NPC 区域逐帧一致，各重复两遍。修复基于 master `6a6fcfae`，生产提交 `1d6cccd4`，独立拆自 #144。

| 场景 | 连续帧数 | 修复前不同的帧 | 单帧最多错误像素 | 修复后不同的帧 |
| --- | ---: | ---: | ---: | ---: |
| NPC 静止 | 21 | 21 | 53 | 0 |
| NPC 当前一步及停步 | 33 | 8 | 38 | 0 |
| 玩家向下走过 NPC | 31 | 23 | 38 | 0 |

NPC 的草地优先级随原有显示状态保留，通过两个既有发布阶段显示；玩家行走时保留前值。渲染仅让下方两块精灵图块的不透明像素让位给非零背景色，透明像素不抹掉其他人物。画面缓存也识别这个标记的变化。

原作依据为 `engine/overworld/movement.asm` 的 `CheckSpriteAvailability` / `GetTileSpriteStandsOn`，以及下方两块 OAM 图块的优先级。场景使用 Route 1，玩家 `(12,22)`，NPC 静止于 `(14,22)`，或从 `(15,22)` 向右走、剩余 12 个移动计数。两侧均在构造角色状态后推进四帧，再开始录像；录像期间只输入按键，不修改角色或背景。

原作普通 Red ROM SHA-256 为 `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`，源码 `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`，PyBoy 输出 160×144。ROM 不包含在本目录。原作三段分别录制 21、81、31 帧；桌面前后版本每段各录制 121 帧。原作和桌面均重复两遍，完整帧序列可重复；桌面每帧完整重绘与缓存绘制一致。除新增草地标记外，前后版本 121 帧的全部记录状态相同。

比较窗口从 `t=-1` 开始，分别到 `t=19/31/29`；按下方向键的时刻为 `t=0`，玩家场景在 `t=16` 松开。移动计数、图像相位、草地标记和玩家坐标逐帧一致。玩家场景的背景位移也逐帧一致，共 16 像素，八次各 2 像素，见 `background.json`。原作 NPC 走动录像后半段开始下一次随机移动；两侧没有对齐随机数来源，后续方向存在差异，不宣称随机巡逻完全一致。原生 `oam_pixels` 是保留的扫描状态，不能直接当作原作更新结束后的 WRAM 像素寄存器比较；人物位置以实际渲染区域、移动计数和图像相位验证。

应用测试 1,080 次、核心测试 3,985 次执行通过，保留既有 92/1 项忽略。ARM 检查、默认 GBA release 构建及新开局 m01–m06 均通过，后者耗时 41.169 秒。构建、二进制和源文件哈希见 `verification.json`。

GBA 使用默认 release ROM、普通 SRAM 冷启动 CONTINUE、真实物理按键、音频及完整渲染。前后版本各两遍，每遍连续记录 1,000 个物理帧；各版本的重复画面完全相同，实际 cartridge SRAM 未变。NPC 初始区域错误由 53 个像素降至 0；行走终点及重开 START 的图片在前后版本完全相同。未宣称 GBA 物理速度与原作一致，也未覆盖地图连接预览人物或所有脚本人物运动。

最初构造的 SRAM 含训练家 ID 0，被现有 GBA 启动检查当作新游戏；这两次录像已判为无效设置，未计入上述结果。提供的 `fixture.sav` 仅将该 ID 改为 12345 并更新校验和，原生导入器确认其他所有字段不变。这个启动检查不在本次渲染修复中改动。

## 重放

原作需要同一 ROM、对应 `.sym`、PyBoy 和 Pillow：

```sh
python replay-original.py --rom /path/to/pokered.gbc --symbols /path/to/pokered.sym --output /path/to/new-output
```

`reference.state` 是角色设置前的原作状态。可移植脚本已经重跑全部六次，生成的 PNG 和状态与首次录制逐字节一致。

桌面在 base 和 candidate 分别使用同一 `capture.rs`：

```sh
cp docs/audits/2026-11-05/npc-grass/capture.rs crates/pokered-app/tests/npc_grass_capture_1390.rs
NPC_GRASS_CAPTURE=/path/to/new-output cargo test -p pokered-app --features debug-server --test npc_grass_capture_1390 -- --ignored --nocapture
```

这是受控测试夹具内的实际 `PokemonGame::update` / `InputState` 连续录像；不是新开局流程的替代。目录必须未存在。临时捕获测试用完后移除。

GBA 使用 libmGBA 0.10.5，编译只读观察桥并指定普通构建的 ROM：

```sh
cc -shared -fPIC -O2 mgba-sram-bridge.c -I/path/to/mgba/include -L/path/to/mgba/lib -lmgba -o /path/to/bridge.so
python replay-gba.py after 1 --rom /path/to/pokered.gba --sha256 ROM_SHA256 --fixture fixture.sav --bridge /path/to/bridge.so --output /path/to/new-output
```

`savedataClone` 只读回实际 cartridge SRAM；不以临时宿主 `.sav` 文件是否变化判断保存行为。

`.json.gz` 保留两遍原作及桌面的连续像素和状态，以及 GBA 的全部逐帧哈希／输入。截图在 `docs/screenshots/npc-grass-1390/`。证据不足 1 MB，不包含 ROM、构建缓存或重复 worktree。
