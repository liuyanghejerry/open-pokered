# 交换确认菜单（71）

原作版本：`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。

`engine/link/cable_club.asm:716–728` 通过 `DisplayTextBoxID` 显示 TRADE/CANCEL 两项菜单，并读取 `wCurrentMenuItem`。`engine/menus/text_box.asm:217–220,309–320` 设置监听 A/B、最大项 1，调用 `HandleMenuInput` 后优先检查 B：B 直接选第二项。`home/window.asm:48–110` 先处理 UP/DOWN（禁止绕回），再返回同一帧被监听的 A/B。

复刻此前把 UP 和 DOWN 都实现成翻转选项，并在方向键分支直接返回，因此顶部 UP 会变成 CANCEL，底部 DOWN 会变成 TRADE，DOWN+A 不提交取消，A+B 则提交交换。修复为 UP 选第一项、DOWN 选第二项（同时按上下时 UP 优先），然后处理同帧 A/B，B 优先取消。

真实两游戏实例通过连接、游戏机、双方队伍菜单到达确认页；修复前原生测试分别证明 UP 越界、DOWN+A 留在确认页和 A+B 执行交换。修复后保留相同断言，还验证下移后 UP+A 确认、对方确认后实际进入交换动画，双方取消后重新显示队伍列表。

截图夹具 `capture_trade_confirmation_keys` 捕获顶部 UP、DOWN+A、A+B 三个场景，使用同一 fixture、输入、帧数与 160×144 画布。只读测试代码没有注入确认菜单状态或直接触发 flow 方法。

验证：debug-server 全部应用单元回归 182 通过；未优化、带调试信息的完整应用测试目标共 460 通过。新增的四个原生测试使用 16 MiB 工作者栈，避免大型双游戏夹具耗尽默认测试栈，断言及生产流程不变。

截图实际 master 基线为 `a284a95ccf626e1a4b6573622fff76433f5ff0d9`。三组捕获均为第 385 帧，前后 fixture 模块哈希、队伍数据和帧数一致，详见 `docs/screenshots/fidelity-71/manifest.json`。

PR #133 合入后重新从 master 捕获基线，三张图与原基线逐字节一致；本次菜单差异与此前按键声和状态页修复无关。
