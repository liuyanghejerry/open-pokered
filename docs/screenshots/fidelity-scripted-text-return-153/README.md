# 对话恢复后的玩家动画更新

原作 `CloseTextDisplay` 在重载图形、清除 BIT_FONT_LOADED、重建视口后跳到 UpdateSprites，再返回地图脚本。复刻的脚本对话关闭未执行玩家这一更新；已有菜单恢复也只更新 NPC。此次在对话从打开到关闭时、以及既有字段恢复完成时，执行玩家 UpdateSprites 对应的计数/图像更新，让原有 OAM 管线继续呈现。FacePlayer 同步移动方向，避免它仍保留上一次步行方向或 0。没有直接设置某场景的动画相位。

原作固定 fbcf7d0，新增断点仅在可执行 CloseTextDisplay / UpdatePlayerSprite 例程。超时与空球各重复两次，共 2,808 张原作 PNG 与此前有效 148 录制逐字节一致，帧状态也一致。原作事件记录及实际入口 RAM 证明：

| 超时原作帧 | 操作 | 进入时 intra-counter | moving direction |
|---:|---|---:|---:|
| 492 | CloseTextDisplay | 0 | 4（下） |
| 499 | 恢复末尾 UpdateSprites / UpdatePlayerSprite | 0 | 4 |
| 499 | SafariZoneEntranceAutoWalk | 1 | 4 |
| 499 | 首步前 UpdateSprites / UpdatePlayerSprite | 1 | 4 |
| 500 | AdvancePlayerSprite | 2 | 4 |

空球的序列整体提前 150 帧。恢复 UpdateSprites 的返回地址为 0x526d（门房脚本中的文本调用之后），首步前的返回地址为 0x587（OverworldLoop），并非通过动画像素反推。原生修复后首步的 intra-counter 为 2、moving direction 为 4；之前是 1，且超时对话前后还保留上次向上的 8。

## 实际 CONTINUE 与逐帧对照

准备、SRAM 和输入沿用 [148](../fidelity-safari-return-148/README.md)，对齐范围沿用 [151](../fidelity-scripted-stride-151/README.md)。一步额度是受控设置；空球为战斗结束后的受控端点，不证明付费进入或真实最后一球战斗。原生四次录制各 702 帧，共 2,808 PNG，重复字节完全一致；JSON 仅排除两个未使用的初始随机种子，原始文件保留。

| 场景 | 原作 / 原生起步 | 相对 0–51 帧坐标与计数 | 完整 RGB 匹配 | 剩余 RGB 差异 |
|---|---|---|---:|---|
| 超时 | 500 / 374 | 全部一致 | 51 / 52 | 相对第 0 帧，195 像素 |
| 空球 | 350 / 284 | 全部一致 | 51 / 52 | 相对第 0 帧，195 像素 |

换脚边界的 97–98 像素差异已消除。这是起步相对比较，原作公告/对话/图形恢复的绝对等待仍不同。起步第 0 帧和通用 CPU/LCD 相位仍待修复，不宣称完整事件、音频或所有地图一致。

核心 2,736 项、应用 201 项测试通过（24 项录制 helper 默认忽略）；GBA 发布构建和原阈值下 31 指标性能比较通过。12 源码文件与 ELF 哈希见 provenance。evidence.zip 含 5,616 PNG、完整状态/事件、源码来源、工具、验证及测试日志；SHA-256 对象去重后全部重新验哈希。FollowNpc 原作完整事件、转盘动画/工作量、冠军之路、菜单恢复和最终 head 全主线/独立 CONTINUE/全部 CI 等既有草稿门禁保留。

## 相同 SRAM、输入和硬件帧 398

前图为实际 master 31b1eda，后图为本次生产源码。另列父提交 a44266d 的相同帧及原作起步相对第 24 帧（绝对第 524 帧）；后图与该原作参考的完整 RGB 相同，但不是绝对时刻一致。

![前 master](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-text-return-153/timeout-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-text-return-153/timeout-after.png)

![父提交参考](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-text-return-153/timeout-parent.png)

![原作参考，第524帧](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-scripted-text-return-153/timeout-original.png)
