# 脚本步行使用延迟呈现

151 已把脚本路径接回场景计数，但 ordinary_player_sprite_frame / ordinary_npc_sprite_pose 的判断仍排除活动脚本、路径和过场，导致脚本行走读取即时坐标/动画，并绕过摄像机和 OAM 延迟。此次让共用场景循环的 MovePlayer、FollowNpc 和已排队玩家路径使用相同呈现状态。特殊姿态、跳跃、淡入淡出等原有排除条件保留，NPC 自定义 scripted_frame 继续由自己的动画呈现。

原作、SRAM、输入和受控准备沿用 [148](../fidelity-safari-return-148/README.md)，逐帧范围沿用 [151](../fidelity-scripted-stride-151/README.md)。真实 CONTINUE，两个场景各重复两次，每次 702 帧；没有在录制中修改坐标、NPC 或地图。一步额度和空球端点均为受控设置，不证明正常付费流程或实际最后一球战斗。

| 场景 | 原作/原生起步帧 | 坐标/计数匹配范围 | 上次 RGB 错帧 | 本次 RGB 错帧 | 本次完整 RGB 匹配 |
|---|---|---|---:|---:|---:|
| 超时返程 | 500 / 374 | 起步相对 0–51 | 51 / 52 | 14 / 52 | 38 / 52 |
| 空球返程 | 350 / 284 | 起步相对 0–51 | 51 / 52 | 13 / 52 | 39 / 52 |

这是起步相对比较，绝对公告/对话/载入时间仍不同。pixel-count 保留所有错帧，没有忽略人物或背景区域，未把 RGB 匹配扩大为完整时序、整个事件或全游戏一致。

剩余换脚错帧集中在相对 7/8、15/16、24/25、32/33、41/42、49/50，主要为 97–98 像素；起步还有 195–247 像素差异。原作实际 AdvancePlayerSprite 入口 RAM 显示第一次步进时 intra-counter 已是 2，原生该帧后为 1；后续也相差一次 UpdateSprites。数据保存在归档中的 animation JSON，说明还须对照对话恢复到模拟方向起步之间的更新顺序；不能在当前场景直接补一个动画相位来声称通用修复。

核心 2,736 项、应用 201 项通过，24 个捕获 helper 默认忽略；另执行本场景四次真实 CONTINUE 录制，PNG 字节重复一致。JSON 比较仅排除两个未使用的初始随机种子，原始文件保留。GBA 发布构建和原阈值下 31 项性能比较通过，12 文件及 ELF 哈希见 provenance。2,808 张原生 PNG、完整状态、工具、日志及分析按 SHA-256 去重，manifest 全部对象重新验哈希；原作/master 完整录制在固定 SHA 的 148 归档。

PR 仍为草稿。通用 CPU/LCD 工作相位、上述起步/动画更新、FollowNpc 原作完整场景、转盘动画与工作量，以及之前的冠军之路/菜单恢复/最终 head 通关等门禁均未完成。

## 相同 SRAM、输入、硬件帧 398

前图来自真实 master 31b1eda，后图来自本次生产源码。

![前 master](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-presentation-152/timeout-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-presentation-152/timeout-after.png)

父提交 9132129 的相同帧 398 另列，用于隔离本次呈现变化，不替代 master 前图。

![父提交参考](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-presentation-152/timeout-parent.png)

原作另取第 524 帧，同一起步相对第 24 帧，仅作参考。该帧仍有 98 个像素差异。

![原作参考](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-presentation-152/timeout-original.png)
