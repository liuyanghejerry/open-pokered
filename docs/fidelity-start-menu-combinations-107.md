# START 菜单组合按键与关闭音效（107）

原作 HandleMenuInput 先移动光标，DisplayStartMenu 检查到 Up/Down 时继续菜单循环，随后才可能处理确认或关闭。旧复刻优先用 B/START 关闭菜单，并允许方向+A 同帧选择新项。修复令方向优先，Up 优先于同时按的 Down；组合 A/B 仍播放确认音效。单独 START 关闭不应播放 A/B 确认音效，本次也修正此前新增音效条件过宽的问题。

原作固定 fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c。七种组合各重复两次，初始 POKEMON：Up+A、Up+B、Up+START、Up+Down 都只移到 POKEDEX；Down+A/B/START 都只移到 ITEM，未进入任何子菜单或 CloseStartMenu。A/B组合播放144，START组合无确认音效。另外三个持续60硬件帧输入各重复两次，Up/Down只有一次移动，hJoy7/6为0；START关闭无确认音效。因此当前START入口的方向长按不自动滚动，与既有边沿输入一致，不能推论其他菜单也一致。

主分支31b1eda3112514d6bc803af559ec7f7c9e20374f与修复使用相同夹具、菜单前图像、Down+B两帧原始输入。旧实现关闭到场景，修复保持START菜单并移动到ITEM。每侧九帧PNG/JSON均重复一致。截图、探针、日志及散列在screenshots/fidelity-start-menu-combinations-107/。

核心2725项、应用199项通过（20项忽略）。回归驱动open_start会等待只读field_menu.input_ready后才选择项目：打开菜单的两帧START+12空帧短于原作23帧等待，保存光标已经位于目标项目时必须等待首次Joypad。七项交互驱动测试通过，含该情形；功能、奖励、存档断言均未改动。正在运行的104a3目录保持原始副本，不能用它代替最新107的完整通关证明。

剩余边界：每次方向输入后，原作下一次HandleMenuInput拥有Delay3；复刻后续UI帧目前仍立即轮询。这需要独立的短按/持续按键探针再修复，当前证据不宣称整个菜单时序全部对齐。
