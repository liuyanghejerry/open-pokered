# FIGHT 拒绝提示与自动 STRUGGLE 验证

选择没有 PP 或被定身法禁用的招式时，提示原先会被招式菜单遮住，B 还可能直接退出到 FIGHT 主菜单。全部招式没有 PP 时，提示又会等待额外确认，阻断原作的自动 STRUGGLE 流程。

本次修复将这三个入口接到共用的战斗文字阶段：先同步文本框，按实际文字速度和按住的 A/B 打印，保留最后一个字的等待，再进入各自的继续方式。无 PP、禁用提示等待新的 A/B 确认；确认后更新菜单信息和光标，回到原来的招式行。全部无 PP 使用 DONE 结束文字，不出现手动确认阶段，随后等待调用方的 60 帧并执行 STRUGGLE。

按键状态取自前端实际按住的按键，保留程序真正读取 Joypad 时的历史。打印加速按键不会自动成为确认，菜单恢复期间的按键也不会被提前消费。选招与实际手动确认请求原有的 PressAB 音效；打印、同步和自动等待不会因此重复请求音效。

## 实际游戏验证

通过正常原生 CLI 启动共享的 `PokemonGame`，预启动载入 JSON 存档并定位到金黄道馆，固定 seed 42。随后只用逐帧正常按键输入、状态读取和截图：先通过游戏菜单 SAVE，再正常触发训练家战斗。前后版本使用完全相同的按键，不在运行中改状态、设 RNG、强制开始战斗或跳过对话。

| 场景 | 操作和观察 | 截图帧 |
| --- | --- | --- |
| 无 PP | 选中 0 PP 的头锤，显示完整拒绝文字；B 确认后回到头锤所在行，未消耗回合或 PP | 1814；返回 1823 |
| 定身法禁用 | 正常战斗中让对方使用定身法，选中仍有 21 PP 的失忆术；B 确认后保持原行及禁用状态，未消耗回合或 PP | 5596；返回 5605 |
| 全部无 PP | 只按一次 FIGHT A，之后连续 90 帧无按键；提示打印结束后等待，再实际执行 STRUGGLE，无额外确认 | 1863 |

本次正式截图的 before 为 master `9289961`，after 为 integration `27647c9`，两者的 debug CLI 均已实际构建成功。六次游戏运行全部通过。三组前缀分别有 28、87、28 个完整状态观察点，前后逐字相同；拒绝提示的后续各 50 帧未改变战斗、PP、物品或 RNG 观察状态。NoMoves 逐帧验证 91 帧：第 1801 帧进入 60 帧等待，第 1861 帧开始真实 STRUGGLE。每次游戏内 SAVE 的 32768 字节及校验正确，运行后文件保持不变。12 张新截图与此前相同输入的开发截图字节一致。

历史开发验证使用 master `3105f077` 与修复 `b3620dc3`，三组实际原生前后对照已通过，原始结果保持该身份。后续 `3105→928` 唯一生产代码变更是捕获后的结算分类；这些中途训练家战斗未进入该路径。本次完成后的重新截图将提供 integration 的直接证据。

## 回归范围

| 检查 | 实际版本及结果 |
| --- | --- |
| core 全套 | 历史开发版本 `28dcaa67`：3390 通过，0 失败，1 个 doc test 忽略 |
| App library | 历史开发版本 `28dcaa67`：340 通过，0 失败，39 忽略 |
| 本次 integration core FIGHT | `27647c9`：`cargo test -p pokered-core --lib battle::fight_`，18 通过，0 失败 |
| 本次 integration 捕获结算 | `27647c9`：`cargo test -p pokered-core --lib battle::capture_outcome_tests::`，4 通过，0 失败 |
| 本次 integration App FIGHT | `27647c9`：`cargo test -p pokered-app --lib --features debug-server fight`，5 通过，0 失败；覆盖声音入口、拒绝提示绘制和局部重绘 |
| 官方 CI | 以对应 PR 最终 head 的官方检查记录为准；本地验证不替代合入所需 CI |

本次 integration 定向回归合计 27 通过。上述命令本地还使用 `--offline --locked -j1` 和共享构建目录。FIGHT 用例覆盖最后一字等待、按住 A/B 加速、真正采样后的确认边沿、两次菜单同步、禁用与 PP 检查顺序、无可用招式的自动继续、阶段序列化，以及 Link 的本地等待规则。App 用例检查共享 `PokemonGame` 的音效请求到达实际 PCM 所有者；这不等同于与原 ROM 的音频波形比较。

原 ROM 的普通野生战斗无 PP 提示还做过独立按键输入观察，确认了逐字打印、提示与 Joypad 采样顺序。这里复现原作的调用阶段和输入语义，没有声称整个原生渲染与硬件逐帧或逐 T-cycle 一致。Link 只有本地逻辑回归，未验证真实双机联机；这些存档是预启动 JSON，不代表冷启动 SRAM CONTINUE 或完整主线通关。字体、中文、拼音输入法及对话框布局不在本次修改范围。

## 同帧前后截图

| 场景 | 前 | 后 |
| --- | --- | --- |
| 无 PP 提示，1814 | ![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fight-refusal-flow-1036/docs/screenshots/fight-refusal-1041/no-pp-before.png) | ![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fight-refusal-flow-1036/docs/screenshots/fight-refusal-1041/no-pp-after.png) |
| 无 PP 确认返回，1823 | ![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fight-refusal-flow-1036/docs/screenshots/fight-refusal-1041/no-pp-return-before.png) | ![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fight-refusal-flow-1036/docs/screenshots/fight-refusal-1041/no-pp-return-after.png) |
| 禁用提示，5596 | ![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fight-refusal-flow-1036/docs/screenshots/fight-refusal-1041/disabled-before.png) | ![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fight-refusal-flow-1036/docs/screenshots/fight-refusal-1041/disabled-after.png) |
| 禁用确认返回，5605 | ![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fight-refusal-flow-1036/docs/screenshots/fight-refusal-1041/disabled-return-before.png) | ![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fight-refusal-flow-1036/docs/screenshots/fight-refusal-1041/disabled-return-after.png) |
| 全部无 PP 自动继续，1863 | ![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fight-refusal-flow-1036/docs/screenshots/fight-refusal-1041/no-moves-before.png) | ![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fight-refusal-flow-1036/docs/screenshots/fight-refusal-1041/no-moves-after.png) |

本次本地记录：构建与定向测试摘要 SHA256 `d5af198c02591792987a94277aa52e65b2809a006fd00812433bbf1a9fbba561`；六次原生重放摘要 `63e69725fde1d36eb35618de0fac4084fb9b5476f6b864e296ffd880849e5c1b`。原始输入、RPC、状态、存档和进程退出记录保留在本次审计目录。
