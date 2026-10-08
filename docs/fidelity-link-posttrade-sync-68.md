# 交换动画、进化结束同步（68）

原作版本：`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。

真实双游戏输入确认差距：Bulbasaur/Kadabra 成功交换后，接收 Kadabra 的一方仍在 IsEvolving，另一方已 TradeCompleted，并可过早返回下一轮选择。原作 `engine/link/cable_club.asm:850–870` 在 TryEvolvingMon 后 ClearScreen、Serial_PrintWaitingTextAndSyncAndExchangeNybble，随后 DelayFrames40，才显示 Trade completed!、SavePartyAndDexData、DelayFrames50。`home/serial.asm:226–283` 的同步函数等待远端，并包含两个十帧的稳定交换循环。

修复在双方电影、强制进化和学习招式结束后发送 TradePresentationReady；即使先收到远端完成消息，也等待本方结束。双方就绪后保留二十帧同步等待和四十帧空白画面，再显示完成文字并部分保存队伍和图鉴。原有五十帧结果延时保留。每轮重新选择会清除完成状态，避免下一次交换复用旧确认。通信协议升为 v5，旧协议通过既有版本检查拒绝连接。

ClearScreen 后不再重绘旧队伍列表：同步显示 PLEASE WAIT!，四十帧延时为空白，完成后显示结果框。这里未调整对话框布局。

实际输入回归覆盖进化较慢的一方、不许 A/B 缩短四十帧、延时前保存仍是原队伍、延时结束保存强制进化后的 Alakazam。实际画面检查覆盖电影后顶部区域不能残留旧队伍。双驱动两轮交换回归验证双方完成确认与每轮状态清理。

验证：debug-server 单元 186 通过，未优化带调试信息的应用所有测试目标 468 通过，核心单元 2707 通过，通信集成 8 通过。基线为 master `383c6e532169b0cabfc8def5606f6e27c06c8e86`。三组前后截图使用同一捕获函数、真实操作、夹具、帧数和实时队伍；已提交队伍的差异是存档时序修复的预期。详情见 `docs/screenshots/fidelity-68/manifest.json`。

尚未把原作串行线的字节传输速度视作网络实现的逐周期等价，也未宣称完成全部状态页载入声画或全场景动画审计。
