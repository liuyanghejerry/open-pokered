# 一击必杀的原作流程

原作 `PlayerCalcMoveDamage` / `EnemyCalcMoveDamage` 不把 OHKO 列入 SetDamageEffects。CriticalHitTest 消耗一次随机数后，OneHitKOEffect_ 按双方当前速度比较：较慢时将伤害清零、miss 置位、critical/OHKO 字节置为 $ff，并在 CalculateDamage 的零返回处分支退出，不再做属性或命中检查。失败提示为 `<TARGET>’s unaffected!`。

速度不低于目标时，先置伤害 $ffff、OHKO 字节 2，再经过 AdjustDamageForMoveType、RandomizeDamage、MoveHitTest。属性免疫不能被 X Accuracy 清除；最后匹配的属性表行决定无效／未命中台词。STAB 加法和每行属性调整均截为 16 位；RandomizeDamage 右旋随机数一次，拒绝小于 217 的结果，再按 /255 缩放。命中后通过普通扣血流程，替身吸收完整伤害，反击读取保留的共享 wDamage；成功提示 `One-hit KO!`。

参考版本：pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c，`engine/battle/core.asm` 的 PlayerCalcMoveDamage、JumpToOHKOMoveEffect、AdjustDamageForMoveType、RandomizeDamage、MoveHitTest、ApplyDamageToEnemyPokemon、AttackSubstitute、PrintMoveFailureText、PrintCriticalOHKOText，以及 `engine/battle/move_effects/one_hit_ko.asm`。

`original-cpu-controls.json` 记录 8 个准备好的原作 CPU 例程控制：两种免疫、成功、速度失败，以及 STAB／克制／抵抗的 16 位值。它们在例程之间显式传递实际观测值，使用有效返回栈和只读 hook；不代表完整原作战斗、随机数时序或画面一致性验证。

当前本地验证：核心库 3152 项、核心全部测试合计 3565 项通过（1 项已有文档测试忽略）；应用库 380 项通过（38 项原有忽略），应用完整测试合计 874 项通过（88 项原有忽略，含库／二进制重复覆盖）。新用例覆盖免疫双方命名、0／5／20 提示、速度判定、X Accuracy、伤害随机数先于命中检查、原作溢出值、替身后的反击。完整 CLI、GBA 和 master 前后截图仍待完成，尚未发布。
