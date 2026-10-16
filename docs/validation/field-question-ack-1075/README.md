# 问题文字结束后直接进入选项

自行车店的购买提问、狩猎地带的收费提问和提前离开提问，原来会在文字完整显示后再多等一次 A/B，菜单才出现。本修改将这四个现有脚本调用换成末尾不等待额外按钮的 `printFieldText(..., true, false)`：Bike 店员一次，Safari 收费一次，Safari 正常返回的 OnLoad 和提前离开的坐标入口各一次。欢迎词、中间翻页、后续答复、选项内容及收费/退场逻辑保持各自既有脚本。

## 原作依据与范围

固定原作是 `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。`text/BikeShop.asm` 的 `_BikeShopClerkDoYouLikeItText` 以 `done` 结束，脚本随后执行 `HandleMenuInput`。`text/SafariZoneGate.asm` 的 join 和 leaving-early 字符串以 `@` 返回，外层脚本再调用 `YesNoChoice`。`home/text.asm` 中 `@` 与 `DoneText` 返回，`PromptText` 才经过 `ProtectedDelay3`、`ManualTextScroll` 等待按钮。本修复仅去掉这四处问题末尾多余的外层 ACK，不将普通列表自动解释为 Yes/No。

Safari 收费原作通过第二次 `DisplayTextID` / `PrintText_NoCreatingTextBox` 进入文字；这里不声称其完整外层准备时序已经还原。现有 `PrintFieldText` 入口准备和最后字母延迟保留。Bike 的 `NO_TEXT_DELAY` / A-B 退出来源、完整两选项优先级和选择结束 15 VBlank 等待均不在本修改中。

## 实际证据与版本

下表是三个普通冷启动存档案例。每组前后使用同一物理 SRAM、一次启动种子 42、完全相同的逐帧输入；最后一个问题页完成后不按 A/B，继续 64 帧 neutral。截图取两侧相同的绝对帧。前侧仍在问题文字，后侧已显示原选项且选中第一项。

| 案例 | 前侧问题完成 F | 相同绝对截图帧 / 输入总数 | 后侧首次选项帧 |
| --- | ---: | ---: | ---: |
| 自行车店购买 | 955 | 1019 | 959 |
| Safari 收费 | 1135 | 1199 | 1137 |
| Safari 正常返回、提前离开 | 776 | 840 | 800 |

这些运行的真实源码身份是 **前 M9 `25a9d8c…`，后 H2 `2c9b876…`**，不是 M10。随后普通合并 M10 得到 `4007b3c…`：两份场景和 435 行测试文件仍逐字等 H2；共享模块只保留原测试注册并合入 M10 的内容。该源整合不重标旧运行。整合后的 `4007b3c…` 已另行执行八条定向命令，共 **69 个命名测试通过**（5 个新 core、3 个已有 core、2 个已有 App、1 个 Boa API、58 个 agent）；实际命令、测试名与程序哈希见 `component-checks-M10.json`。这组是组件检查，不新增冷启动 Game 或截图。

H2 已执行八条定向命令共 67 个命名测试通过：5 个新 native 嵌入式调用测试、3 个已有 core 控制、2 个已有 App 控制、1 个 Boa API 测试、56 个严格 agent 测试。第四个 EarlyRow 入口由组件测试覆盖，未额外声称冷 Continue 会跳过正常 OnLoad 去触发它。组件 fixture 的初始设置与普通 Game 实证分开。

三个候选案例合计实际重放 3058 帧输入、3061 个公开状态、24 张原始候选截图。只读独立终验逐项核对了原始 RPC、每帧零补帧回执、全部前后截图解码、物理存档、源码/程序哈希和进程终态。钱包均为 1000，队伍 HP/PP、地图位置、背包及剧情 flags 与各自基线末端一致；没有回答最终问题或实际收费。Game 由工具在诊断边界对自有进程组终止，不能把其 SIGTERM 写成自然游戏退出。命令使用 `--no-audio`，未验证 PCM 或设备可听性。

## 首次失败没有改写

最初的三案例候选 wrapper 在 Bike 1019 帧通过，随后 Safari 收费原 1103 帧输入失败，Return 按首败停止而未运行。旧收费基线的早页 A 在第 915 帧落到候选尚未结束的最后字母延迟，候选最终仍停在较早页；它没有走到最终问题的 DONE 交接。本次保留该真实失败、原向量和诊断，不删字母延迟或延长旧向量求通过。

之后另建了 **只由 M9 产生** 的收费验证协议：每次合法欢迎词/较早页等待输入时，先固定 32 帧 neutral 阅读停顿，逐帧断言同页、无选项，再按原 A1+N7；最后问题仍零 A+64 neutral。32 是预声明的测试输入策略，**不是原机指令时长**。它形成新的 1199 帧收费向量，候选原样重放。Return 沿用原 840 帧；Bike 沿用首次已有的 1019 帧通过案例，不重跑或把旧整体失败改成通过。

## 文件与复现限制

- `fixtures/`：三个实际使用的 32 KiB canonical SRAM。
- `tapes/`：三个实际重放的 JSON 字面序列；每项是 `null` 或单个按钮名。它们均排除基线在最终边界之后另做的 A1+N32 探针。
- `controllers/`：实际执行过的两版基线发现控制器和两版固定候选控制器，源码逐字归档。
- `plans/`：对应实际 READY 计划原件；`provenance.json` 列出每个来源、SHA、运行身份、截图帧和本地原始结果。
- 对照图存于 `../../screenshots/field-question-ack-1075/`。PR 应用绝对 raw URL，不能使用相对图片路径。

控制器通过正常 CLI `run --headless --speed 0 --seed 42 --debug-port … --lang en --no-audio --save …` 冷启动，按普通输入经过启动画面、Continue 和地图内路线。RPC 仅使用 `get_state`、`get_flags`、`get_bag`、`press_timeline` 和 `capture_frame`；候选不按观察状态挑按钮，不调用 setters、warp、restore、skip 或运行时 RNG 修改。截图前后核对公开状态/flags/背包没有改变。

调用接口如下（这些是归档接口示例，**不要直接覆盖现有输出目录**）：

```sh
python3 controllers/before-discovery-v2.py --plan <审查后的本地READY计划> --plan-sha256 <SHA> --case bike-sale --output <新空目录> --execute-reviewed-before-only
python3 controllers/admission-before-discovery-v3.py --plan <审查后的本地READY计划> --plan-sha256 <SHA> --case safari-admission --output <新空目录> --execute-reviewed-before-only
python3 controllers/candidate-fixed-replay-v2.py --plan <审查后的本地READY计划> --plan-sha256 <SHA> --execute-reviewed-candidate-fixed-replay
```

**这不是独立的一键复现包。** 原计划含本会话 E 目录、冻结 CLI、两个历史源码检出、引擎/GFX 清单和构建身份的绝对路径；这些大文件不随 Git 提交。当前 M10 工作目录也不满足历史 M9/H2 的 HEAD 守卫。另一环境需独立检出相同历史源码，正常构建 CLI、恢复原存档/字面输入，建立并审查新的 provider/源码守卫及空输出计划，不能改输入以迎合候选；新运行应保留新的真实身份。完整原始 RPC/状态日志留在本地归档，不重复提交。
