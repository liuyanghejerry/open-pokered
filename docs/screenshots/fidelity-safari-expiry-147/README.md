# Safari 结束条件和公告

原作依据：`pret/pokered` 的 `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`，`SafariZoneCheckSteps`、`SafariZoneCheck`、`SafariGameOverText` 和 `_TimesUpText` / `_GameOverText`。

- 步数从 1 减到 0 的这一步仍允许完成；下一步检查到原有的 0 才结束。复刻此前在减到 0 时立即结束。
- 最后一颗球耗尽后，在返回原地的普通场景循环中结束，不再要求走一步。
- 超时显示 `PA: Ding-dong!`、`Time's up!`，然后 `PA: Your SAFARI / GAME is over!`。球耗尽只显示最后一段。
- 结束步骤直接返回，避免继续处理该步的中毒、寄养经验和遭遇；请求停止音乐并播放 `SFX_SAFARI_ZONE_PA`。

使用原作真实 SAVE 产生的 SRAM，复刻通过实际 CONTINUE 载入。准备阶段通过原作正常地图载入进入 Safari Center，并明确修改为一次受控的一步额度；这不证明正常付费进入流程。没有改写原作 ROM、玩家坐标、NPC 或地图瓦片。

输入相同：UP 在第 0–69 帧保持；A 在第 100–101、130–131、160–161、190–191、220–221 帧保持。主分支前图来自实际 `31b1eda`，前/后均取第 129 帧。原作图另取第 180 帧，作为台词参考，不是同步像素比较。

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-safari-expiry-147/timeout-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-safari-expiry-147/timeout-after.png)

![原作台词参考](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-safari-expiry-147/timeout-original.png)

原作与修复分支都在第 17 帧完成 1→0 的一步，第 34 帧完成下一步并触发结束。原作、主分支、修复分支各录制两次，共 1,452 张 PNG。各组 PNG 字节重复一致；原生完整 JSON 只排除两个未参与本场景的初始随机种子进行重复比较，原始 JSON 保留。9 项 Safari 回归、核心 2,735 / 应用 201 项单元测试、GBA 发布构建和性能比较通过。

`evidence.zip` 用 SHA-256 对象去重；`MANIFEST.json` 将原始路径映射到 `objects/<hash>`。全部条目重新读取并验哈希，见 `archive-verification.json`。包含原始状态、截图、准备阶段变更说明、SRAM、录制工具、构建来源和测试日志。最初误在文本数据标签设置调试断点的原作录制已拒绝，归档不包含那批画面；修正后只钩住可执行例程，重录 SRAM 与原生录制所用 SRAM 哈希相同。

这次证据证明结束条件和台词修复。原作在第 54 帧开始公告文本，原生在第 34 帧开始；广播调度的等待、完整对话恢复及 CPU/LCD 相位尚未对齐。冠军之路 2F 第二机关及其他既有草稿门禁也仍未完成。不能将本次测试解释为完整画面、音频或全游戏通关一致。
