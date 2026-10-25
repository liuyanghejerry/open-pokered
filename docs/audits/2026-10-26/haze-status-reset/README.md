# 黑雾的行动取消与定身法清除

基线为 `5b3cd42342aa9aabc2546fb5f925c3b7d2a9a414`。

黑雾先手解除目标的睡眠或冰冻后，目标本回合仍不出招，但原版不会额外打印无法行动提示。复刻原先把这次静默取消翻译成 `can't move!`，多出一页需要确认的文字。黑雾后手时，之前已经发生的睡眠、冰冻或醒来提示则应保留。

另一个独立遗漏是定身法：原版黑雾清除双方的禁用招式及回合数，生产环境的效果栈却没有移除 Disable。本次同时修复，下一回合可从 FIGHT 菜单选择原先被禁用的招式。

## 原作依据

对照 pret/pokered 的固定提交 `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`：

- [HazeEffect_](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/move_effects/haze.asm) 清除目标状态；睡眠或冰冻时把其 selected move 写为 `$ff`，并清除双方的 Disable 字节。
- [ExecutePlayerMove / ExecuteEnemyMove](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/core.asm) 对 selected move `$ff` 直接结束本次行动。

[original-haze.json](original-haze.json) 记录 12 次准备状态后的 HazeEffect 调用，以及 2 次 `$ff` 行动入口调用。只读断点验证状态、所选招式、文本调用及 RNG；没有修改 ROM 或注入代码。它不是从普通输入运行完整回合的证据，也没有验证整段动画的时间。

## 回归与截图

`fidelity_haze_cured_action.rs` 的 4 个测试覆盖 60 次受控 FIGHT 回合：双方、先后手、启用与禁用动画展示、睡眠/冰冻/其他状态、下一回合恢复出招，以及双方定身法清除后的实际菜单选择。断言包含文字、HP、PP、RNG 消耗和对应动画是否发生。

[capture.rs](capture.rs) 使用公开 BattleScreen 输入、实际 BattleVisualEffects 和 draw_battle，使用 Fusion Pixel 字体。六个准备好的对局各运行 481 次更新；修复前后使用相同输入，在相同更新编号各保存 42 张截图。四个先手黑雾对局的后段提示改变；两个后手对照的全部截图逐像素一致。PR 所附前后图均取玩家睡眠对局的第 480 次更新。

这些截图用于验证可见提示和菜单流程；它们没有运行完整 PokemonGame 或音频，也不证明原作与复刻逐帧等时。

最终代码的验证结果：

- 核心：3,798 项通过，1 项文档测试沿用忽略；应用：407 项通过，40 项沿用忽略。
- 独立构建的桌面程序：从 NEW GAME 到 m06，通过全部 11 项系统场景，包括另一进程读取保存结果后 CONTINUE。
- GBA：未修改的性能基线检查通过；13 项内存压力场景通过，运行 323,040 帧，最低空闲堆 4,928 B，最低剩余栈 8,704 B。运行器为 mGBA 0.10.5。

代码、程序、截图及测试身份见 [evidence.json](evidence.json)。本次没有重跑 m01–m49 全主线，也不宣称其余待处理差异已消失。
