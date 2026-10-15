# 招式禁用信息与选择提示验证

修复两处原作差异：招式同时没有 PP 且被定身法封住时，先返回无 PP 提示；英文招式信息框在禁用行显示 `disabled!`，省略普通 TYPE/PP 信息。依据原作 `engine/battle/core.asm` 的 SelectMenuItem（2640 起）、PrintMenuItem（2841 起）和 DisabledText（2909–2910），对照版本为 `pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。

## 实际游戏前后对比

同一预启动 JSON 快照、seed 42 和 3364 帧正常输入，卡比兽对战金黄道馆的呆呆兽。对手实际施放定身法；在第 3316 帧确认禁用状态，第 3364 帧将光标放到「睡觉」（Rest）。没有通过调试命令强设定身法或战斗数据。

- 前：`battle-disabled-info-1035-before.png`，错误显示 `TYPE/PSYCHIC` 和 `PP 10/10`。
- 后：`battle-disabled-info-1035-after.png`，正确显示 `disabled!`。
- 两张实际 `Game.draw` 的 160×144 PNG，仅信息框内部 292 个像素变化，区域 `[8, 74, 67, 95)`。

每侧使用 53 批、不超过 64 帧的 `press_timeline`，逐批校验 start/queue/end/frame_count 及 padding=0；批量仅减少调试协议往返，输入顺序和每帧按钮完全相同。每侧共 110 组请求/响应，53 个完整公开状态检查点逐字相同。截图前、截图响应内和截图后的状态也相同，截图不推进游戏。

两侧普通 SAVE 和剧情标记 sidecar 的全部字节相同，SAVE 主校验和为 207：

```text
ordinary.sav               e51ac96a99fc89c9822ce2eb00798f5eb96fe13854d909c2d52cdf44f6693bd8
ordinary.script_flags.json f4efc4004cbd0303bf77935023aaab313698517f9cf86708430889c77d4c3211
```

公开状态证明 Rest 当前 PP 为 10、disabled=true；公开 API 没有 max_pp 字段，前图分母 10 另由实际 PNG 确认。两侧各自拥有的 Game 进程均已结束。

这是 `--snapshot`、`--skip-intro`、`--warp SaffronGym,16,14` 的控制场景，输入前缀包含一次真正的普通 SAVE；并非普通存档冷启动验证，也不代表战斗结束、胜利后存档、所有前端渲染路径或原作逐帧时序均已验证。

前图保持原构建身份 `15615c5558faa4ea8da0e2d891a49ff329dd3d49`（二进制 `fd5d132f…`）。它与实际基线 `master@cc1534a182b9021e685acfea84efb3b8086faf52` 仅差三个文档文件，2177 个编译输入及 702 个图像资源一致。后图构建为运行代码提交 `dbd4d23b3fe5179a4d14c3a85ded627dc83b4aa9`（二进制 `1391dfd5…`）；随后只添加文档和截图，不改动其 2178 个编译输入。

## 自动回归

| 检查 | 通过 | 失败 | 默认忽略 |
| --- | ---: | ---: | ---: |
| pokered-core 全套 | 3361 | 0 | 1 个文档示例 |
| pokered-ui 全套 | 122 | 0 | 0 |
| pokered-app debug-server 全部目标 | 782（含 lib/bin 重复模块） | 0 | 90 |

核心测试属于原提交 `26da1063abbea5ed2c2427c2b2e582981c3004b4`；后续合入 Stats 修复及本信息框精修后，1902 个 core/data/Cargo 输入未变，因此未重复执行核心矩阵。UI、app 和 CLI 构建在最终运行代码提交 dbd4d23b 执行。

真实 public menu caller 测试覆盖无 PP 优先、拒绝不修改 PP/Disable、移到另一招后可正常选择。UI 测试覆盖 tile/pixel 两种 Painter、PP 为 0 或非 0 的禁用行、移入及移出后的 TYPE/PP 恢复。完整 FrameBuffer 测试保留原有普通行转换，并加入禁用行：每个单元目标实际比较 48 组完整画面，lib/bin 各执行一次，共 96 组比较通过。

普通英文行保留原本的两块局部清除（1120 像素）；仅选中禁用行时清除整个信息框内部。绘制不修改招式或战斗数据。

```text
cargo test --offline --locked -j1 -p pokered-core
cargo test --offline --locked -j1 -p pokered-ui
cargo test --offline --locked -j1 -p pokered-app --features debug-server
cargo build --offline --locked -j1 --bin pokered-app --features debug-server
```

首次核心构建因本地新工作树未连接 gfx 而在测试前中止，补齐同一资源后通过。app 测试返回成功后，产出的 19 张历史截图触发工作树守护；这些产物已归档并恢复，没有重跑或跳过测试。本次 PR 只收录上述实际对比截图，初始日志及原测试结果均保留。官方 CI 以本 PR 的实际提交为准。
