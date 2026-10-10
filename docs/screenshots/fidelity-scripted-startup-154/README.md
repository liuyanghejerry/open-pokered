# 模拟方向首步接续当前场景循环

153 的起步首帧仍多出两名门房 NPC，共 195 像素。原作 CloseTextDisplay 返回地图脚本后，SafariZoneEntranceAutoWalk 在当前 JoypadOverworld 调用中安装模拟方向，随后继续该循环的方向分派。复刻安装新路径时又消耗了旧的 field_loop_wait，第一步比这次调用晚一帧，NPC 已先通过 OAM 管线显示。

此次对绝对 MovePlayer 与 MovePlayerRelative 的首次启动清除旧循环等待，首步仍使用共有计数和视口等待；相对移动在路径尚未创建时也使用场景呈现状态。显式 FacePlayer 同步动画中的朝向，对应在随后的文本载入/reset phase 之前进行的转向更新，没有补某个画面的固定相位或强行隐藏 NPC。

第一次候选只处理绝对 MovePlayer，实际返程使用相对命令，因此第一轮的相对对齐断言失败。该候选、其原始录制及失败日志保留，**最终证据只使用 final-native 的第二轮源码及其哈希**。这不是仅改对齐帧号：两种命令的生产分派和相对移动呈现入口均有代码修复，并重新录制。

## 验证

原作固定 fbcf7d0，SRAM、输入和准备沿用 [148](../fidelity-safari-return-148/README.md)。一步额度及空球端点是受控设置，不证明正常付费进入或最后一球战斗。原作新增只读 OAM、VRAM、SCX/SCY 快照，每场景各两次，全部 2,808 PNG/原始帧状态与有效 148 录制逐字节一致。

原作起步第 500 帧画面没有两名 NPC，第 501 帧才显示。OAM 内存与 SCX/SCY 的帧末快照另存 video.json，不能把帧末内存直接等同于已绘制屏幕；该过程涉及 VBlank/PrepareOAMData/DMA。此前 Native 首步晚一帧，NPC 已显示；改正首步接续及转向后，该帧画面与原作一致。

| 场景 | 原作 / 最终原生起步 | 校验范围 | 坐标/计数 | 完整 RGB |
|---|---|---|---|---|
| 超时 | 500 / 373 | 起步相对 0–51 | 52 / 52 匹配 | 52 / 52 匹配 |
| 空球 | 350 / 283 | 起步相对 0–51 | 52 / 52 匹配 | 52 / 52 匹配 |

最终原生通过真实 CONTINUE、实际相同输入录制，各场景重复两次、每次 702 帧，共 2,808 PNG 字节重复一致；JSON 比较仅排除两个未使用的初始随机种子，原始 JSON 保留。RGB 比较覆盖整个 160×144，没有人物/背景掩码或像素容差。不是 PNG 文件字节跨引擎一致，也不是绝对帧同步。

核心 2,736 项、应用 201 项通过（24 项捕获 helper 默认忽略），GBA 发布构建及原阈值下 31 性能指标通过。12 源文件和 ELF 哈希见 provenance。归档含最终录制、原作视频/状态/事件、失败候选、工具、构建与测试日志，SHA-256 去重对象全部重新验哈希。

**仍未完成**：绝对公告、对话及图形恢复等待、通用 CPU/LCD 相位；FollowNpc 原作全场景、转盘动画和工作量、地图连接、冠军之路及菜单恢复等前置门禁；最终 head 的全新主线、独立 CONTINUE 和全部 22 项 CI。只证明这两种受控返程的起步相对 52 帧，不扩大为完整事件或全游戏无差异。

## 相同 SRAM、输入、硬件帧 373

master 前图为实际 31b1eda，后图为最终生产源码。父提交 57501b5 的相同帧另列；原作第 500 帧仅作为相同起步相对时刻的参考，后图与其完整 RGB 匹配，绝对时间仍不同。

![前 master](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-startup-154/timeout-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-startup-154/timeout-after.png)

![父提交参考，第373帧](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-startup-154/timeout-parent.png)

![原作参考，第500帧](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-startup-154/timeout-original.png)
