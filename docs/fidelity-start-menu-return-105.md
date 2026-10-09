# START 子菜单返回（105）

原作 home/start_menu.asm 的 RedisplayStartMenu 跳过 DisplayStartMenu 的 START 音效，执行 DrawStartMenu、UpdateSprites 后重新读取菜单输入。复刻返回队伍、背包等入口时会重新播放 START 音效，且没有重绘输入等待。

修复增加三硬件帧的重绘等待，取消返回时的 START 音效，并以关闭子菜单时的真实按键样本为基线。因此重绘期间松开的短按不重放，一直按着的返回 B 键也不会再次关闭 START 菜单。新按住的方向或 A 键在首次 Joypad 被读取。

原作固定 fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c。受控 3F 场景实际进入队伍 A120、返回 B170，Redisplay/DrawStartMenu205、首次 Joypad208。14 个边界案例（七种输入各重复两次）结果一致：Down206/207 单帧无效，Down208/209 当帧有效，Down/A206 持续按住到首次读取时有效，一直按着的 B 无新动作。返回无 START 音效。

主分支 31b1eda3112514d6bc803af559ec7f7c9e20374f 与修复使用相同只读捕获夹具、真实 START→POKEMON→B 输入及相同原始按键序列。返回前图像相同；返回第一帧的 Down 短按使旧实现选中 ITEM，修复保持 POKEMON。两侧九帧 PNG/JSON 均重复一致。截图、探针、日志及散列在 screenshots/fidelity-start-menu-return-105/。

核心 2723 项、共享应用 197 项通过。首次应用构建缺少测试中的 StartMenuItem 局部导入，补齐后完整测试通过，失败日志保留。

范围限制：本次只对齐 RedisplayStartMenu 重绘后的三帧输入及音效；原作从队伍 B170 到重绘205 的 35 帧恢复过程尚未与复刻对齐。当前完整新游戏回归使用上一项104的不可变二进制，不能作为105最新提交的完整通关证明。长按重复、组合按键优先级及空队伍入口仍需继续审计。
