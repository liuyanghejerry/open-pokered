# 双方游戏机启动流程（72、74）

原作版本：`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。

**72：另一方的请求不能冻结未操作游戏机的玩家。** 原作 `engine/pokemon/bills_pc.asm:503–539` 的左右游戏机事件分别设置本方 link state 并显示 Just a moment.，随后才进入 `CableClub_DoBattleOrTrade`。复刻此前收到 RequestTrade/RequestBattle 就无条件打开全局 yes/no 窗口。玩家站在桌子外也会冻结，随处按 A 就能接受请求。

修复将收到的请求保存在非模态 room 状态中；玩家必须通过本方游戏机的真实场景交互打开 Just a moment.，确认后才能回应。已移除旧的全局 peer prompt。request/accept 都在启动时读取当前队伍，双向同时启动继续使用现有 host/guest 冲突处理。

**74：确认后少了 80 帧启动等待，并显示错误的等待文字。** 原作 `engine/link/cable_club.asm:4–23` 首先 DelayFrames 80，随后显示 PLEASE WAIT! 并交换名字、随机数、队伍资料。复刻此前在触发游戏机时立即发送请求，再把 Just a moment. 的确认变成 Waiting...!。现在确认 A/B 后等待 80 帧，之前不发送/接受资料，随后显示 PLEASE WAIT!。游戏机场景命令到达游戏循环的首帧不接受同帧 A/B，保证启动文字不会被同一个按键跳过。

真实双游戏回归验证：远离桌子的 peer 确实已收到请求但仍在 room 状态；在原地按 A 不能接受；方向键能走回桌边；通过游戏机确认后的第 79 帧仍未接受，第 80 帧才接受，最终双方进入队伍列表。原有交换、取消、强制进化、部分存档和通信战斗回归保持全部断言，只更新双方实际启动入口。

本次未宣称解决状态页载入时的完整按键声/鸣叫时间线、通信房间入场及人物坐标，或交换动画后的 serial 同步和 40 帧等待；这些仍在审计。

验证：debug-server 单元回归 179 通过；通信集成 8 通过；未优化、带调试信息的所有应用测试目标共 454 通过。前后截图基线为 master `a284a95ccf626e1a4b6573622fff76433f5ff0d9`；同一只读捕获函数、夹具、输入、帧数、队伍和位置，记录于 `docs/screenshots/fidelity-72-74/manifest.json`。
