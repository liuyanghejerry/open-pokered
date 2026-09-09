# 定量对齐与判定门槛

## 为什么代表帧会误导

从两段录像中分别挑“看起来处于同阶段”的帧，相当于人工做了时间重采样。它可以让速度错误、停顿缺失、相机瞬移和帧保持次数差异看起来一致。源码中存在相同坐标表，也只说明局部常量相同，不能证明最终合成画面相同。

因此对比必须保留两种视图：

1. **raw-time 视图**：从同一个语义触发点开始，列只允许使用相同的 `t+N`；一侧提前结束时显示 `ENDED`，不能重复最后一帧填满。
2. **phase-normalized 视图**：仅用于解释轨迹形状，可以按阶段归一化；不得用于 `PASS` 判定。

## Gate 0：录制有效性

以下任一条件不满足时，不得判 `PASS`：

- 两侧帧 ID 连续，且一张 PNG 对应一个 emulated frame；
- manifest 能把录制帧映射到 emulator/debug frame；
- trigger 前至少有一帧稳定状态，结束点是第一个稳定 post-animation frame；
- 输入边沿、按住/松开时机和停止条件相同；
- 场景、坐标、面向、存档条件和 ROM/build 版本已记录；
- 同一设置至少重复两次，时间/轨迹结果可复现。

如果当前 headless loop 在 debug 请求之间继续运行，不能用客户端请求时间推算帧号。应让同一捕获循环同时写图像与状态 manifest，或明确把 timing verdict 降为 `PARTIAL`。

## Gate 1：语义窗口

每个场景先声明不可变的窗口边界：

- `t0`：触发输入被游戏消费的帧；
- 中间 anchor：每个可见 phase 的首帧；
- `t_end`：动画结束后的第一个稳定帧。

分别记录两侧原始 `[t0, t_end]` 帧数。默认持续时间容差为 5%；需要逐帧还原的场景使用 ±1 帧。只有在先报告原始时长差异后，才允许为说明目的做归一化。

## Gate 2：分层测量

不要用全屏单一像素差替代行为分析。至少拆成：

| 通道 | 测量 | 常见漏判 |
| --- | --- | --- |
| phase/UI | phase 顺序、首帧、末帧、fade 亮度曲线 | 缺少停顿、文字过早出现 |
| actor/OAM | bbox/中心点、朝向、帧索引、可见性、阴影 | 轨迹形状近似但位置/节奏错误 |
| background/camera | 稳定 landmark ROI 的逐帧平移 | 背景静止后突然整屏跳动 |
| state | map、坐标、movement/transport/battle phase、input lock | 画面相似但状态提前提交 |

相机移动场景必须同时测 actor 和 background。ROI 应避开玩家、UI、水、花、闪烁 tile 与 NPC；若平移后的 residual 仍高，换 ROI 或把该段标为不可测，不要接受低置信结果。

## Gate 3：硬失败条件

下列任一项成立即为 `FAIL`，不能用“整体看起来像”覆盖：

- 可见 phase 缺失、增加或顺序改变；
- 原始持续时间超过阈值；
- actor 轨迹误差超过场景阈值；
- background 总位移相同，但分布方式不同（例如平滑滚动变成单帧跳变）；
- 当前最大单帧位移超过 reference 最大值 1px 以上；
- 最终状态/坐标正确，但到达方式包含 reference 没有的 snap、teleport 或停顿；
- 输入锁定窗口、地图切换帧或文字出现帧错误。

`PASS` 需要所有 mandatory channel 都有数据并通过。缺 actor 或 background 定量数据时使用 `PARTIAL`；无法复现时使用 `BLOCKED`。

## 工具

`scripts/compare_sequences.py` 对两个语义窗口执行：

- 连续帧检查；
- 原始时长比较；
- 指定背景 ROI 的逐帧整数平移搜索；
- 总路径、最大单帧跳变和运动分布比较；
- JSON 指标与禁止重采样的 raw-time 诊断图输出。

脚本依赖 Pillow。不要污染仓库环境；缺少依赖时在临时 venv 中安装并使用该 Python：

```bash
python3 -m venv /tmp/key-animation-venv
/tmp/key-animation-venv/bin/pip install Pillow
```

示例（Route 1 向下跳台阶）：

```bash
/tmp/key-animation-venv/bin/python \
  .agents/skills/key-animation-differential/scripts/compare_sequences.py \
  --reference-dir "$REF/frames" --reference-range 0:36 \
  --current-dir "$CUR" --current-range 775:792 \
  --roi 0,10,56,120 --max-dx 0 --max-dy 40 \
  --output "$OUT/ledge-metrics.json" \
  --diagnostic-image "$OUT/ledge-raw-time.png" --strict
```

本次已知录像在这个窗口下得到：reference 37 帧、current 18 帧；两侧背景总位移都是 32px，但 reference 分 16 次每次 2px，current 在落地时一次跳 32px。因此台阶动效必须判 `FAIL`。
