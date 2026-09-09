# 关键动效差异审计

日期：`YYYY-MM-DD`

## 结论

先用表格给出每个场景的 `PASS` / `PARTIAL` / `FAIL` / `BLOCKED`，并用一句话说明差异。把离场和到达等不同阶段拆开判定。

| 动效 | 结果 | 观察 |
| --- | --- | --- |
|  |  |  |

## 录制方法

记录：

- reference ROM source commit、ROM hash、版本/DEBUG 变体；
- current commit、构建命令、binary 路径；
- emulator/renderer、分辨率、FPS、输入方式；
- snapshot/save、地图、坐标、面向、party 和触发输入；
- 触发帧、关键阶段帧、最终状态；
- 构建失败或工具限制，以及这些限制是否影响结论。

## 帧证据

每个场景至少提供一张标注了实现和内部帧号的对比图。需要连续节奏时，提供两段短 MP4：

```markdown
![场景对比](../../screenshots/visual-key-animations/<scene>-compare.png)

录制：[原版](../../screenshots/visual-key-animations/<scene>-reference.mp4) · [当前版](../../screenshots/visual-key-animations/<scene>-current.mp4)
```

说明对齐锚点；不要把两个运行的文件名帧号直接当成同一时刻。

## 实现侧交叉检查

分别写明：

1. 录制中实际观察到的阶段和时间；
2. 当前源码中对应的状态机/渲染路径；
3. 原版汇编 routine 是否包含该阶段；
4. 哪些差异只是 ROM 版本、精灵、名字、语言或存档造成的 confounder。

## 后续建议

只提出与证据直接相关的修复或补充测试。分析任务不应默认修改产品代码。
