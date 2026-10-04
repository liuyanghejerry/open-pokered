# 图鉴大盘来源审计展示

`master` 没有 `dex-run/jev-dex-player.html`，因此无法从基线截取这个尚不存在的新页面。
这里的前图使用本 PR 修复前版本（d4b02eb2），后图使用修复后版本。
两图均以同一原生 `final.png` 为静态视频封面，Chrome headless，1440×1600。

右侧数据是专用于 UI 回归的两物种测试夹具，不是正式运行结果：
原生登记 Cubone/Marowak，但 Marowak 尚待合法来源补证，故有效计数应为 1/124。
夹具随后模拟一条没有新增登记的续跑快照，最近登记仍应显示 Cubone。

前：原生 2/124 被当成有效计数、待补证物种标绿、最近登记被空快照清空，未知来源未显示。

后：有效 1/124、原生 2 与待补证 Marowak 单独列出、待补证物种标黄、保留最近登记并显示未知来源。

另由实际播放器 JavaScript 回归覆盖合法来源补证事件、登记跳转、末尾不回跳，
测试命令：`python3 -m unittest scripts.test_jev_dex_player -q`。
