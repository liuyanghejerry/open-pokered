# Prize full-bag dialogue

> 归档说明（2026-11-11）：原始录制包、逐帧日志和运行报告已从当前源码树移除。下文保留历史结论与验证范围；旧附件可在清理前的 Git 历史中查阅。复测请使用保留的脚本，将生成文件写到临时目录。

Original `engine/events/prize_menu.asm` .bagFull prints `_OopsYouDontHaveEnoughRoomText`, defined in `data/text/text_2.asm`. All three TM choices incorrectly used another sentence. This corrects English dialogue only; Chinese translation and mechanics are unchanged.

62 `fidelity_` core tests pass. The existing TM prize test now checks the source failure text for all three prizes and its absence after cancellation or success. Native debug-server build passes.

Actual input on base master and the fix uses identical constructed full-bag states, COIN_CASE, 9999 coins, counter3, first TM, confirmation YES. Both retain coins and all inventory entries after saving and separate CLI readback. Screenshots at the same recorded frame665 show complete failure text. `verification.json` retains the observed states and storage checks. This is seeded boundary validation, not a fresh playthrough or all-prize runtime coverage.
