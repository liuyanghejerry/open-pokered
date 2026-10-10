# 脚本玩家路径接回场景等待

脚本路径以前每硬件帧减一次玩家计数，并在完成一格的同一帧启动下一格。此次使用已有 field_loop_wait：起步设置八后立即减一次，等待普通场景的首步视口重绘；后续每两硬件帧推进；提交坐标后返回场景循环，下一格在新循环启动。NPC 仅在实际逻辑推进时更新，并使用玩家推进前的 walking 状态。转盘在已进入场景循环时启动，不重复消耗等待。

这是共用路径的循环顺序修复，**仍未接入通用 CPU/LCD 工作相位模型**。首步视口等待沿用普通场景的现有近似；原作工作量和中断已有独立证据，但尚未全部进入生产调度。FollowNpc 的原作完整事件对照、转盘动画/工作量、地图连接及既有草稿门禁仍需完成。

真实原作 SRAM 经实际 CONTINUE 载入，超时与受控空球端点各重复两次，每次 702 个硬件帧。原作固定 fbcf7d0，准备/输入和限制与 [148 返程证据](../fidelity-safari-return-148/README.md) 相同：一步额度是受控设置，空球不是实际最后一球战斗，不证明正常付费进入。这里没有修改录制过程中的坐标、NPC 或地图瓦片。

| 场景 | 原作起步 | 原生起步 | 校验范围 | 坐标与计数 | RGB 不同帧 |
|---|---:|---:|---|---|---:|
| 超时返程 | 500 | 374 | 起步相对 0–51 帧 | 全部匹配 | 51 / 52 |
| 空球返程 | 350 | 284 | 起步相对 0–51 帧 | 全部匹配 | 51 / 52 |

这是**起步相对对齐**，不是同一硬件帧的完整时序一致。三次提交在相对第 15、32、49 帧，下一格起步在 17、34 帧，与实际原作逐帧计数独立匹配。公告、对话、地图载入的绝对延迟不同，RGB 差异仍涉及背景/摄像机、OAM、玩家呈现，详见逐帧 pixel-count，不能宣称画面一致。

两个原生重复录制的 PNG 字节完全一致；完整 JSON 仅排除两处未参与此场景的初始随机种子进行比较，原始 JSON 保留。2,808 张原生 PNG、逐帧状态、工具、来源和日志在 evidence.zip，按 SHA-256 去重并全部重新验哈希。原作与 master 的原始录制已在 148 归档，此处 verification.json 固定其 SHA-256；工作量原作在 150 归档。

核心 2,736 项、应用 201 项测试通过（24 个捕获 helper 默认忽略），GBA 发布构建、12 文件来源校验和原阈值下 31 指标性能比较通过。首次回归的三项失败日志保留：转盘多等待已修复；研究所跟随的锁步测试不再要求旧 8 帧硬件时钟，但不作为原作完整跟随证明；渡的剧情测试改为等脚本完成后检查坐标和关门，而非固定 600 帧提前断言。新增计数轨迹回归直接使用原作 500–549 帧证据。

## 相同设置、输入、硬件帧 390

主分支前图来自实际 master 31b1eda，后图为此次生产源码，输入和帧号不变。帧号 390 正处于修复后的路径中。

![前（master 31b1eda）](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-stride-151/timeout-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-stride-151/timeout-after.png)

以下另列 c62dc0b 的相同硬件帧 390，已逐文件确认录制来源源码与该提交一致；6ba6475 仅增加文档，生产源码相同。用于隔离本次时钟变化，不替代 master 前图。

![父提交参考](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-stride-151/timeout-parent.png)

原作另取第 516 帧，即同一起步相对第 16 帧，作为参考；不把它当作同一硬件帧或 RGB 匹配证据。

![原作参考](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-stride-151/timeout-original.png)
