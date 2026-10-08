# NPC 交换台词组与结果顺序（76）

原作版本：`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。

真实 Route2TradeHouse 输入先完成选精灵和交换电影，随后观察到 Hey thanks! 在 RED traded ABRA for MR.MIME! 之前。原作 `engine/events/in_game_trades.asm` 的 DoInGameTradeDialogue 在 InGameTrade_DoTrade 成功返回后先 PrintText TradedForText，再进入 TRADETEXT_THANKS。五处场景的顺序相反：Route2TradeHouse、CinnabarLabFossilRoom、Route18Gate2F、VermilionTradeHouse、UndergroundPathRoute5。现在均先显示交换结果，再显示 NPC 感谢。

`data/events/trades.asm` 为九个可访问交换指定 CASUAL / EVOLUTION / HAPPY 台词组；`data/text/text_7.asm` 是五个分支原文。华蓝市 POLIWHIRL→JYNX 应用 EVOLUTION（组 2），此前用了组 3 的首次询问、拒绝、感谢和再次交谈，选错精灵也沿用组 3。现在恢复组 2。原版组 2 的再次交谈确实说宝可梦 evolved，这属于英文原作沿用日版 Blue 文本的行为，未改变交换种类或加入实际进化。

另外，RAICHU→ELECTRODE 的选错精灵回复应为组 2 的 Hmmm?，SPEAROW→FARFETCH'D、VENONAT→TANGELA、NIDORAN♂→NIDORAN♀ 应为组 3 的 ...This is no。这些回复此前互换。地下通道的英文询问、结果和再次交谈补回原文 species name 的性别标注。

回归通过真实场景交互、A/B 对话、选择队伍和电影，覆盖九种交换各自的成功、选择错误宝可梦、拒绝以及已完成状态再次交谈，共 36 个入口。成功交换验证收到的种类与总结/感谢顺序；其余分支验证队伍未被交换。固定 NPC 移动用于隔离对话，未调用 trade flow 或伪造交换结果。

验证：debug-server 单元 188 通过；未优化带调试信息的所有应用目标 472 通过；核心单元 2707 通过。三组截图从 master `7a526981bb3699f838f058c6683f509d2ee1c1ed` 捕获，同一只读模块、初始队伍、输入和帧数，地图、位置及种类/等级/招式完全相同。原有 NPC 交换 DVs/OT 随机字段未参与画面比较，这些画面不显示它们。详情见 `docs/screenshots/fidelity-76/manifest.json`。
