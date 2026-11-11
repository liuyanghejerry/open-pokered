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

- 核心测试 3,998 通过、1 忽略；应用测试（`debug-server`）1,114 通过、
  92 忽略。包含逐字节写入、拒绝执行、序列化恢复和所有中断边界检查。
- 新开局实际输入到 m06 通过；这不是本次完整通关验证。
- 桌面图鉴、资料、背包、设置的普通退出和恢复期短按各两遍；
  输入恢复仍为 tick 12、23、9、9，与 master 相同。
- SAVE 取消、短按、选 NO 按住 A、选 YES 按住 A，各两遍；仅 YES
  写入一次，独立进程 CONTINUE 通过，恢复时刻与 master 相同。
- 石英高原队伍返回及短按、真新镇 FLASH 各两遍通过。
- ARM 检查、默认 release GBA 的实际按键/SRAM 回放，以及独立
  `perf-benchmark` ROM 的七组既有性能门限通过；未修改门限。
  GBA SAVE 的四种分支各两遍，写入和冷启动 CONTINUE 逐像素及 SRAM
  对照一致；FLASH 回放可继续移动。

GBA 的完整 START/场景画面首次出现时刻如下（录制 tick 从 0 计）：

| 路径 | master | 本次 |
| --- | ---: | ---: |
| 图鉴返回，含短按对照 | 155 | 156 |
| 资料返回，含短按对照 | 194 | 193 |
| SAVE 取消，含短按对照 | 167 | 173 |
| SAVE 选 NO，tick 600 松 A | 609 | 609 |
| SAVE 选 YES，tick 2000 松 A | 2008 | 2014 |

石英高原有活动人物，不能沿用城市夹具“返回后整屏等于进入前”的断言：
master 两遍也不满足它。本次返回后的整屏像素与 master 的返回画面一致，
普通退出和短按各两遍一致；保留此范围限制。

## 仍有差异

桌面图鉴/资料的输入恢复与原作的 14/24 仍不同；背包/设置的分批刷新、
START 按住方向和 SAVE 松 A 的既有差异仍待处理。GBA 物理恢复耗时仍
未与原作一致，SAVE 的两个分支比 master 多 6 帧，不能宣称完整动画等价。

`docs/screenshots/2026-11-12-buffer2-copy/` 保留相同输入 tick 的前后图：
图鉴 tick 155、资料 tick 193；它们展示 GBA 恢复时刻的变化。
