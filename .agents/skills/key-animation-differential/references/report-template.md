# 关键动效差异审计

日期：`YYYY-MM-DD`

## 结论

先用表格给出每个场景的 `PASS` / `PARTIAL` / `FAIL` / `BLOCKED`，并用一句话说明差异。把离场和到达等不同阶段拆开判定。没有完整 raw-time、actor、background 和 state 证据时不得写 `PASS`。

| 动效 | 录制有效性 | 结果 | 观察 |
| --- | --- | --- | --- |
|  |  |  |  |

## 录制方法

记录：

- reference ROM source commit、ROM hash、版本/DEBUG 变体；
- current commit、构建命令、binary 路径；
- emulator/renderer、分辨率、FPS、输入方式；
- snapshot/save、地图、坐标、面向、party 和触发输入；
- PNG → emulator frame 映射、触发帧、关键阶段首帧、首个稳定最终帧；
- 构建失败或工具限制，以及这些限制是否影响结论。

## Raw-time 对齐

先列语义窗口，不做时间重采样：

| 场景 | 实现 | `t0` | 中间 phase anchors | `t_end` | 原始帧数 |
| --- | --- | --- | --- | --- | --- |
|  | reference |  |  |  |  |
|  | current |  |  |  |  |

再列定量门槛：

| 指标 | reference | current | delta/阈值 | 结果 |
| --- | --- | --- | --- | --- |
| phase 顺序 |  |  | exact |  |
| 原始时长 |  |  | 默认 ≤5% 或场景 ±1 帧 |  |
| actor 轨迹 |  |  | 场景阈值 |  |
| background 轨迹 |  |  | 最大单帧差 ≤1px |  |
| 最终状态 |  |  | exact |  |

## 帧证据

每个场景至少提供一张使用相同 `t+N` 列的 raw-time 对比图。某侧提前结束时显示 `ENDED`，不得复制最后一帧或按阶段拉伸。需要连续节奏时，提供两段短 MP4：

```markdown
![场景对比](../../screenshots/visual-key-animations/<scene>-compare.png)

录制：[原版](../../screenshots/visual-key-animations/<scene>-reference.mp4) · [当前版](../../screenshots/visual-key-animations/<scene>-current.mp4)
```

说明对齐锚点；不要把两个运行的文件名帧号直接当成同一时刻。普通 contact sheet 只能做导航，不能作为 `PASS` 证据。

## 实现侧交叉检查

分别写明：

1. 录制中实际观察到的阶段和时间；
2. 当前源码中对应的状态机/渲染路径；
3. 原版汇编 routine 是否包含该阶段；
4. 哪些差异只是 ROM 版本、精灵、名字、语言或存档造成的 confounder。

源码常量一致不能推翻录像中的 timing、相机或合成差异。结论必须以运行时证据为准。

## 复现性

记录至少两次确定性录制的关键指标。两次均失败才确认 finding；两次结果不一致时先修复捕获稳定性，不下动画结论。

## 后续建议

只提出与证据直接相关的修复或补充测试。分析任务不应默认修改产品代码。
