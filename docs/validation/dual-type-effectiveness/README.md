# 双属性效果提示与命中音效

原作 AdjustDamageForMoveType 会累计两个属性的伤害，但 wDamageMultipliers 的低位保留 TypeEffects 最后匹配行。因此冰冻光束打水／飞行的暴鲤龙是普通伤害，仍提示效果拔群。当前修复保留这一原作行为，并在实际攻击计算时记录结果，避免后一次属性转换（Conversion）改变前一次攻击的提示或命中动画。

命中声音来自 PlayApplyingAttackSound，位于震动／闪烁之前，不能由 HP 条或效果台词再次触发。Audio2 的战斗音效支持速度修饰：普通命中 $B0、拔群 $17F、效果不好 $81。这三种命中声都是 CHAN8 噪声；原作 Audio2_note_length 对 CHAN8 固定使用 $0100，因此这些参数不改变噪声音符时长，不能据参数断言听感速度变化。请求等待前一命令音效结束；普通游戏与 DirectBattle 共用同一请求。

验证范围和原作例程执行记录见 verification.json；原作例程使用预置操作数，不能冒充完整原作战斗或逐周期验证。原作 ROM 未包含在仓库中。

当前分支可运行核心和应用回归。受控截图：设置 FIDELITY_DUAL_TYPE_CAPTURE 为输出目录、FIDELITY_DUAL_TYPE_TICK=219，运行 cargo test -p pokered-app --features desktop,debug-server --lib capture_dual_type_effectiveness -- --ignored --test-threads=1。前后使用同一按键时序，初始画面和战斗状态相同。
