# 存档方向字段（89）

原作 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c` 的 `constants/sprite_data_constants.asm` 定义 PLAYER_DIR_RIGHT/LEFT/DOWN/UP 为位 0/1/2/3，即 1/2/4/8。人物 sprite facing 是另一套 0/4/8/12，不能互用。原作 OverworldLoop 的 noDirectionButtonsPressed 分支会保存停止方向并将 wPlayerMovingDirection 清零。

原作实际 SAVE 夹具（fidelity-82-85/fixture.sav）朝上停车的 SRAM 三字节是 `[0,8,8]`，依次为 moving/last-stop/current；修复前 native 写入 `[4,4,4]`。朝下 native 写入 `[0,0,0]`，其中 current/last-stop 错误。修复共享的 hosted/GBA 保存状态同步函数，输出原作位掩码；Idle 移动方向为零。同步补充字段文档并纠正两个存档技能的错误值表。不做反序列化字节归一化，原作 SRAM 原始值和旧文件均可导入；Continue 仍按原作固定朝下（PR #140）。

回归使用受控合法队伍和真新镇起点，通过实际方向输入完成移动、松开按键站稳，再经过 START→SAVE→确认→保存完成。读取生产保存的 SRAM 文件：独立原作符号 sMainData bank1:$a5a3，wMainDataStart=$d2f7，三个方向字段 $d528/29/2a，对应文件偏移 $27d4/5/6。直接检查这三个原始字节，不以序列化/反序列化自洽替代原作依据；另验证保存位置、队伍不变。朝下基线测试实际失败（0 != 4），修复后四方向通过。

| 停止朝向 | 当前/停止方向 | 移动方向 |
| --- | --- | --- |
| 下 | 4 | 0 |
| 上 | 8 | 0 |
| 左 | 2 | 0 |
| 右 | 1 | 0 |

194 项 debug 应用测试通过。此检查覆盖普通移动后停车的真实保存；没有宣告全部 native SRAM 可由原作 ROM 完整游玩，也未覆盖转向中或脚本移动时的最后停止方向历史。转向暂停及完整移动时序另作审计。

纯存档逻辑、schema 注释和技能说明修改，不改变屏幕输出，按 AGENTS.md 无需前后截图。复现：`cargo test -p pokered-app --features debug-server --lib actual_save_menu_writes_original_direction_masks_89`。

未优化应用全目标回归：484 项、48 套通过（库与二进制各执行保存回归）。
