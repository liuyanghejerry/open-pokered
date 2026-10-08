# 通信菜单按键与状态页鸣叫（59、69、70）

对照原作 `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。

- **59：通信菜单缺少按键声。** `HandleMenuInput` 在被监听的 A/B 返回时播放 `SFX_PRESS_AB`。修复双方队伍列表、本方 STATS/TRADE 菜单和交换确认的声音；未监听的列表 B 和直接轮询 Joypad 的底部 CANCEL 保持静音。真实游戏输入后检查音序器和非零 PCM，并检查两个静音分支。状态页入口按键声与鸣叫之间的原作画面加载时间仍需单独核对，本次不宣称解决该时间线。
- **69：组合按键执行了不同操作。** 原作 `engine/link/cable_club.asm:312–538` 先更新纵向光标，再处理列表 A，最后处理横向切换；STATS/TRADE 子菜单则先处理其监听的横向键，再 B、A。复刻此前顺序相反，RIGHT+A 会看对方而不是打开本方菜单，子菜单 RIGHT+A 会打开状态页，DOWN+RIGHT 会复制移动前的光标。修复这些顺序，保留不等长双方队伍的光标钳位。
- **70：鸣叫中能提前翻状态页。** `engine/pokemon/status_screen.asm:172–173` 调用 `PlayCry` 后才进入按钮等待；`home/pokemon.asm:145–149` 的 `PlayCry` 阻塞到声音结束。复刻此前立即接受 B 并切到招式页。普通队伍/PC 共用状态页和通信状态页现在等待逻辑音频结束。生产 `--no-audio` 仍有逻辑 PCM 音序器，因此静音不会缩短等待。真实通信输入和普通静音运行均验证早按 B 无效、鸣叫结束后 B 切页、A 返回。

## 视觉证据

基线 master `a77efa20f5118cc31693fb2a394b942104a5f6e5`；前后仅加载同一只读测试模块，运行 `capture_menu_input_and_cry_fidelity`，测试不注入菜单状态或修改运行时流程。配对游戏通过真实连接、游戏机交互和按钮输入打开菜单；普通队伍状态页通过 START → POKéMON → STATS 打开。每对使用相同 fixture、输入、帧数和 160×144 画布，记录见 `docs/screenshots/fidelity-59-70/manifest.json`。

| 场景 | 输入 | 修复后 |
| --- | --- | --- |
| party-right-a | 本方列表 RIGHT+A | 本方 STATS/TRADE 菜单 |
| action-right-a | 本方 STATS 菜单 RIGHT+A | 选择 TRADE，未打开状态页 |
| party-down-right | 三只对两只队伍 DOWN+RIGHT | 对方第 2 只 |
| link-stats-early-b | 对方状态页鸣叫时 B | 仍在能力页 |
| party-stats-early-b | 普通静音队伍状态页鸣叫时 B | 仍在能力页 |

既有三个连续按键测试增加了真实鸣叫等待，保留原有选择、取消和通信状态断言。

验证结果：debug-server 单元回归 178 通过；未优化、带调试信息的全部应用测试目标共 452 通过；通信集成 8 通过。master 基线两个早按 B 测试都在 Moves/Stats 断言处失败，修复后通过。
