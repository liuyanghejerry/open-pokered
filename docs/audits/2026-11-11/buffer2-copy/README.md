# 菜单返回：屏幕缓冲区 2 的复制

`LoadScreenTilesFromBuffer2` 过去立即替换整个逻辑画面，漏掉原作关闭
AutoBG、逐字节复制 360 字节、再开启 AutoBG 的过程。现在每条指令经
共用总线执行；中断或预算用尽后从原位置继续。最后一次写入完成、
AutoBG 仍关闭时绑定现代 UI 图块，再执行原作的开启指令。

这补齐一段真实执行过程，尚未解决菜单返回的全部动画差异。

## 原作依据

- `pret/pokered` 的 `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`
  中 `home/tilemap.asm`、`home/copy.asm`。
- 图鉴、训练家资料、背包、设置各正常退出两遍；只读 hook 观察原作。
  PyBoy 2.8.1 与 2.9.0 结果一致：被调用过程为 18,884 T-cycle，
  加外层 CALL 为 18,908；360 次写入期间 AutoBG 均关闭。
- `probe-original.py` 使用原作 ROM、符号表及既有原作存档；加载夹具后
  仅使用按键和只读 hook。示例：

```sh
python3 probe-original.py --rom /path/to/pokered.gbc \
  --symbols /path/to/pokered.sym \
  --state ../../2026-11-05/start-menu-close/fast-cancel/reference.state
```

## 验证

基础对照为 master `c58ee10c`，实际回放使用普通 SRAM、冷启动 CONTINUE
和按键；录制时不修改状态。录制、日志及运行报告只保存在临时目录。

```sh
cargo test -p pokered-core --locked --offline
cargo test -p pokered-app --features debug-server --locked --offline
# 在 crates/pokered-gba 下：
cargo +nightly-2025-12-07 check --release --locked --offline
```

- 核心测试 3,998 通过、1 忽略；应用测试（`debug-server`）1,116 通过、
  92 忽略。包含逐字节写入、拒绝执行、序列化恢复和所有中断边界检查。
- 新开局实际输入到 m06 通过；这不是本次完整通关验证。
- 桌面图鉴、资料、背包、设置的普通退出和恢复期短按各两遍；
  输入恢复仍为 tick 12、23、9、9，与 master 相同。
- SAVE 取消、短按、选 NO 按住 A、选 YES 按住 A，各两遍；仅 YES
  更改存档，独立进程 CONTINUE 通过，恢复时刻与 master 相同。
- 石英高原队伍返回及短按、真新镇 FLASH 各两遍通过。
- ARM 检查、默认 release GBA 的实际按键/SRAM 回放，以及独立
  `perf-benchmark` ROM 的七组既有性能门限通过；未修改门限。
  GBA SAVE 的四种分支各两遍，写入和冷启动 CONTINUE 逐像素及 SRAM
  对照一致；FLASH 回放可继续移动。

GBA 的完整 START/场景画面首次出现时刻如下（录制 tick 从 0 计）：

| 路径 | master | 本次 |
| --- | ---: | ---: |
| 图鉴返回，含短按对照 | 155 | 156 |
| 资料返回，含短按对照 | 194 | 194 |
| SAVE 取消，含短按对照 | 167 | 173 |
| SAVE 选 NO，tick 600 松 A | 609 | 609 |
| SAVE 选 YES，tick 2000 松 A | 2008 | 2015 |

石英高原有活动人物，不能沿用城市夹具“返回后整屏等于进入前”的断言：
master 两遍也不满足它。本次返回后的整屏像素与 master 的返回画面一致，
普通退出和短按各两遍一致；保留此范围限制。

## GBA 菜单组合的内存回归

#452 旧版在石英高原有两处新增分配失败：图鉴退出后再 SAVE（NO/YES），
以及训练家资料退出。相同普通 SRAM、相同按键在 master 正常完成。

两个独立 UI 图块空间不能共用编号，但不必各自预留 256 个图块的堆容量。
采样现在先在 4 KiB 的有界临时工作区完成去重，再为实际使用的图块分配
准确容量。避免先预留最大容量再缩小所需的新旧分配重叠；原有图块编号、
采样失败的原子性及 360 字节复制过程不变。新增测试覆盖全部 256 个编号，
并验证第 257 个新图块不能破坏已发布的画面或编号。

`2026-11-05/save-return/replay-gba.py` 新增 `--after-menu`，可选择直接保存、
图鉴、队伍、背包、训练家资料或设置。回放除了 SRAM，还检查前置菜单
确实返回 START、保存后返回地图、重新打开 START、独立冷启动 CONTINUE，
并拒绝模拟器 panic／分配失败日志。START 比较标签区域以允许记住的光标
不同；地图与 CONTINUE 比较完整画面。完整连续录制及日志只保存在本地。

```sh
python3 docs/audits/2026-11-05/save-return/replay-gba.py 1 no-held \
  --after-menu pokedex --rom /path/to/default.gba --sha256 ROM_SHA256 \
  --bridge /path/to/mgba-sram-bridge.so --output /path/to/local-output
```

验证使用默认 release ROM、实际按键和普通 SRAM，六种前置菜单 × 四种
保存输入 × 两遍，共 48 次、69,600 个连续物理帧；每组两遍的输入、像素、
SRAM 一致。FLASH 两遍及独立 CONTINUE 通过。应用测试 1,116 通过、
92 忽略；新开局至 m06（非完整通关）和七组性能门限通过。

另用仅增加栈水位观察的诊断 ROM 测量，六条路径的最小未使用栈均为
9,632 字节，高于 4,096 字节门限；诊断代码未保留。诊断 ROM 不代替
上述默认 ROM 的实际回放或性能结果。

与本 PR 修复内存前的有效录制相比，直接取消、短按和 NO 的全部连续
图像不变；YES 有四个物理帧不同，最终稳定场景从 tick 2114 到 2115。
FLASH 有四帧过渡图像不同，返回后的移动结果一致，仍不宣称动画完全等价。

内存回归截图的前版是 #452 修复前的默认 ROM，后版是本次默认 ROM：

- 图鉴 → SAVE → NO：按住 A 600 个物理帧后松开，取确认后的 tick 1399。
  前版停在 SAVE，后版已返回石英高原。
- 训练家资料退出：相同按键后的物理帧 3482，前版停在白屏，后版回到 START。

## 仍有差异

桌面图鉴/资料的输入恢复与原作的 14/24 仍不同；背包/设置的分批刷新、
START 按住方向和 SAVE 松 A 的既有差异仍待处理。GBA 物理恢复耗时仍
未与原作一致，SAVE 取消比 master 多 6 帧、YES 多 7 帧，不能宣称完整动画等价。

`docs/screenshots/2026-11-12-buffer2-copy/` 保留相同输入 tick 的前后图：
图鉴 tick 155，以及上述两处内存回归的相同按键时刻。
