# 畏缩生命周期与战斗叙述验证

较慢的对手后手使用头锤时，可能在玩家已经行动后留下畏缩标记。复刻原先会把这个标记带到下一回合，让玩家无法行动；后续受到的头锤伤害又被误判为混乱自伤，显示错误台词。

修复按照原作 `MainInBattleLoop`，在下次普通战斗菜单前清除双方过期的畏缩，保留原作 Rage／recharge（需要休息）的绕过分支。真正发生在同一回合、阻止后手行动的畏缩仍然生效。畏缩和混乱自伤分别记录实际阻止行动的原因，文字与 HP 演出使用同一原因；训练家 AI 在同回合重建战斗状态时也保留这条临时记录。记录不会写进存档或带入后续回合，PP 收费政策没有改动。

## 实际原生游戏对照

实际 before 为 master `9289961`，after 为修复 `c997646`。两次均从相同预启动 JSON、seed 42 和金黄道馆位置开始，经正常按键菜单 SAVE 后触发训练家战斗。运行中仅使用正常按键输入、状态读取和截图，没有修改状态、重设 RNG、强制战斗或跳过对话。

前 3499 帧按键相同，55 个完整公开状态采样点逐项相等，没有归一化。3499 帧双方都停在失忆术所在行，失忆术 26 PP，实际战斗 HP 为 204。随后两边都只按一次 A，再连续 96 帧无按键。

| 观察 | 修复前 | 修复后 |
| --- | --- | --- |
| 本回合首条战斗叙述 | 错误显示混乱自伤 | 正常使用失忆术 |
| 实际战斗中的失忆术 PP | 仍为 26，行动被错误阻止 | 26 → 25，正常行动 |
| 本回合已解析的玩家 HP | 204 → 191 | 204 → 191 |
| 第 3596 帧的可读台词 | `SNORLAX hurt itself in confusion!` | `SNORLAX used AMNESIA!` |

HP 取自 `get_state` 的实时 `battle_live`／`evaluation.party`，不是尚未回写的存档队伍。两边随后都记录了敌方头锤。首条叙述尚未确认时，后面的头锤演出尚未展示，HUD 仍显示 204；这与已解析的回合 HP 191 属于不同演出阶段。3508 帧截图也保留用于检查过程，主对照使用文字已可读的 3596 帧。

97 个后缀单帧状态、全部原始按键及 RPC 均已审查，时间轴 padding 为零。两次普通 SAVE 的 32768 字节、校验和、队伍字节和脚本 sidecar 均保持不变。两个新进程组都已终止并回收。

## 回归范围

`c997646` 的 5 个 core 回归实际通过：慢速头锤不应阻止已行动者的下一回合；快速头锤仍应阻止同回合后手并显示畏缩；真正混乱自伤在同时麻痹时仍显示正确原因；双方标记清理保留 Rage／recharge 绕过；训练家 AI 重建状态后原因记录不丢失。

这些 core 测试使用规范战斗驱动、明确的测试随机字节、生产回写与公开菜单调用，覆盖真实的头锤副效果及其生命周期。原生游戏对照另行验证上述金黄道馆案例，不能把全部 5 项 core 案例说成均经过原生完整游戏实测。

| 检查 | 实际版本与结果 |
| --- | --- |
| core 定向回归 | `c997646`，5 通过，0 失败 |
| 原生游戏固定输入对照 | `9289961` / `c997646`，1 组前后对照通过 |
| 官方 CI | 以本 PR 最终 head 的官方检查记录为准，本地验证不替代合入所需 CI |

`get_state` 没有暴露 RNG 或内部 FLINCHED 标记，因此公开状态相等不能作为它们直接实测相等的证据。标记生命周期来自原作源码、修复源码及 core 回归；正常行动恢复后也不要求继续复现先前错误版本的后续战斗轨迹。本次没有声称冷启动 SRAM CONTINUE、完整主线通关、真实 Link 联机、原 ROM 逐帧或硬件时序一致。

## 前后截图

| 场景 | 前 | 后 |
| --- | --- | --- |
| 3499 帧：相同选招起点 | ![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/battle-flinch-lifetime-1040/docs/screenshots/flinch-1042/entry-before.png) | ![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/battle-flinch-lifetime-1040/docs/screenshots/flinch-1042/entry-after.png) |
| 3596 帧：行动与叙述修正 | ![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/battle-flinch-lifetime-1040/docs/screenshots/flinch-1042/before.png) | ![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/battle-flinch-lifetime-1040/docs/screenshots/flinch-1042/after.png) |
