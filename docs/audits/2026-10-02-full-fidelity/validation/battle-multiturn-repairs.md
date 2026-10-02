# Counter / Wrap / Thrash 修复验证

修复对应基线审计 B13–B15；源码在 battle worktree 由 battle agent 集成，早期剧情代理协作实现。

原作依据：`engine/battle/core.asm:4554–4608` Counter；`core.asm:3538–3566` Thrash/Wrap 连续回合；`core.asm:683–697` 整回合结束后清零 Wrap；`effects.asm:791–808` Thrash 初始计时；`effects.asm:1075–1102` Wrap 初始计时及清 recharge；`core.asm:5439` 已被束缚敌人的 CANNOT_MOVE 早退。

独立生产测试 `crates/pokered-core/tests/fidelity_multiturn.rs` 直接使用正式 StackDriver、正式规则和 legacy adapter，不调用旧 execute_turn oracle。

验证命令：

```sh
source /workspace/onboarding/env.sh
CARGO_TARGET_DIR=/workspace/open-pokered/target cargo test --offline --locked -p pokered-core --test fidelity_multiturn -- --nocapture
```

2026-10-02 首次执行：10 passed / 0 failed；编译 21.81 秒，测试 0.02 秒。完整输出见 `battle-multiturn-tests.log`。

| 场景 | 断言 |
|---|---|
| Counter 对 Substitute | 真实 HP 不变，分身承担双倍伤害 |
| Counter 跨回合共享伤害 | 保留原作 wDamage bug；读取目标当前 selected move 类型/威力，拒绝 Ground |
| Counter 命中/半无敌 | 普通 1/256 miss 与 Dig/Fly 阻挡均生效；miss 清共享伤害 |
| 自我效果与免疫对共享伤害 | 失败的 Recover 不调用命中测试、不清 wDamage；属性免疫清零 |
| Wrap 连续攻击 | 后续伤害等于首击；不重新抽 crit/accuracy/formula RNG；目标闪避与半无敌不影响已开始连续攻击 |
| Wrap 最后一次与结束 | 最后 tick 对手仍束缚；整回合结束清旗、下一回合释放 |
| Wrap 首击失败与 Hyper Beam | 条件第二 duration byte 在 hit test 前；失败释放目标、仍清目标 recharge |
| Wrap duration | 两分支原作条件抽样分别保留 4 / 5 次总 RNG 消耗 |
| Thrash/Petal Dance miss | 首击 miss 仍启动锁定；后续 miss 仍递减；到期疲劳发生在本次攻击之前 |
| Thrash fatigue | 2–5 回合；本次攻击不立即运行新 confusion gate |
| 束缚时双方睡眠 | 首击之前双方照常检查睡眠；后续敌人 CANNOT_MOVE 跳过所有状态检查，玩家睡眠仍递减 |

Counter 仍遵循项目现有均匀 RNG 字节接口，并未实现 ROM 菜单游标内存残留或整条 Game Boy RNG 序列仿真；Counter 的 power=1 按原作先 CriticalHitTest，因此成功执行通常有 crit+accuracy 两次抽样、没有伤害公式抽样。

正式 app 的一次 turn 后进行 adapter writeback/rebuild；Wrap zero-counter volatile 和 continuation damage 是该 turn 内 scratch，writeback 清 trapping 旗。原作本来在双方执行后 CheckNumAttacksLeft 清旗，不能在最后 tick 的第一行动者执行完立即释放目标。

Battle agent 会统一 rustfmt、执行其余 production 回归及 full-core；这份记录保留最初 10 个专门边界测试的结果，不替代最后整合测试。
