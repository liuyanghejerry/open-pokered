# 空队伍 START 菜单（106）

拿到初始宝可梦之前，原作仍显示 POKEMON，选择后返回 START 菜单。旧实现隐藏这一项，因此整个菜单数量、项目位置及第一项行为均不同。

DrawStartMenu 无条件打印 POKEMON；StartMenu_Pokemon 检查 wPartyCount，空队伍直接跳到 RedisplayStartMenu。修复保留项目，空队伍 A 键触发三帧重绘，并以当前 A 按住状态为基线，避免持续 A 反复重绘。队伍有成员时仍进入队伍菜单。

原作固定 fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c。受控队伍数量为0，有/无图鉴分别各重复两次：菜单项目数量7/6，保存光标选中 POKEMON；A60 进入 StartMenu_Pokemon 后同帧 Redisplay/DrawStartMenu，未进入 DisplayPartyMenu，首次重新读取 Joypad63；确认音效144，仅确认音效，无重复 START 音效。

主分支31b1eda3112514d6bc803af559ec7f7c9e20374f与修复使用相同空队伍、无图鉴夹具。菜单前场景RGB和原始按键序列一致；主分支第一项ITEM，修复第一项POKEMON，选A后旧实现进入背包，修复保留START菜单。每侧七帧PNG/JSON重复一致。截图、来源探针、日志及散列归档在screenshots/fidelity-empty-party-menu-106/。

核心2724项、应用198项通过（19项忽略，包含只读采集入口）。首次核心测试2721通过、三项失败；这些旧测试断言空队伍隐藏POKEMON，已按原作证据改为完整项目列表与A重绘/第二项背包行为，失败日志保留。最新完整通关仍未证明：正在运行的a3使用104的不可变二进制，不能代替105/106最新提交的完整回归。
