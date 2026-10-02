# 原作战斗机制修复与验收

基线：open-pokered `72ff719`；对照：pret/pokered `fbcf7d0`。修复前差异和双方行号见 `battle-mechanics.md`。所有期望来自原作控制流，legacy turn 仅是已有测试 oracle。

| 审计项 | 修复 | 生产回归 |
|---|---|---|
| B01 | PP 在状态门之后消耗；两回合招式蓄力不扣、出手才扣；失败 Mirror Move 仍受状态门 | `blocked_turns_and_charge_gather_do_not_spend_pp`、`failed_mirror_move_spends_pp_only_after_status_gates` |
| B02–B03 | 混乱自伤无属性/STAB，使用当前攻防与 burn/等级修正；自伤取消连续状态并清半无敌，完全麻痹保留原版半无敌 bug | `confusion_uses_typeless_modified_own_stats_and_cancels_charge`、`full_paralysis_cancels_fly_but_preserves_original_invulnerability_bug` |
| B04–B05 | X Accuracy 绕命中随机但保留 Dig/Fly 与 OHKO 速度门；自我效果不进行敌方命中测试 | `x_accuracy_bypasses_roll_but_not_dig`、`self_boosts_and_heals_skip_enemy_accuracy_and_invulnerability` |
| B06–B07 | 固定等级伤害使用真实 level；Psywave 按原作玩家/敌方的零值区别拒绝采样 | `fixed_level_damage_and_psywave_rejection_use_real_level` |
| B08–B09 | Focus Energy 高暴击取整；缩放取低字节；Explosion 缩放后减防 | `critical_rounding_and_screen_stat_low_byte_match_rom` |
| B10 | Disable 随机挑非空槽，对玩家/联机拒绝零 PP | `disable_selects_known_slot_and_skips_empty_pp_on_player` |
| B11 | 单机玩家 Mimic 选择敌方已知招式，敌方/联机随机；保留原槽剩余 PP | `mimic_choices_preserve_remaining_pp_and_original_party_moves` |
| B12 | Transform/Mimic 保留原 party 身份，换出/结算恢复；Transform 复制 DVs、catch-rate、当前类型与 unmodified stats；EXP 使用原种族 | `transform_restores_party_identity_and_copies_effective_types`、`transformed_exp_uses_original_species_growth_and_learnset` |
| B13 | Counter 使用跨回合共享 wDamage，正常命中与 Substitute 路径，保留原版旧伤害/自身伤害交互 | `fidelity_multiturn.rs` 中三个 Counter 回归及共享伤害免疫回归 |
| B14 | Wrap 首次设置原版条件抽样持续时间；续击复用原伤害，不重抽命中/暴击/伤害；被困状态门与最终回合释放 | `fidelity_multiturn.rs` 中 Wrap/被困敌我睡眠回归 |
| B15 | Thrash/Petal Dance 在继续行动之前计时，miss 也推进；疲劳混乱 2–5；自然到期 counter=0 | `fidelity_multiturn.rs` 中 Thrash 回归 |
| B16–B17 | 保留原版缺 HP 255/511 恢复失败；睡眠覆盖 recharge 目标既有状态并清 recharge | 恢复回归、`sleep_overwrites_existing_status_on_hyperbeam_recharge` |
| B18 | Conversion/Transform 当前类型进入 STAB、相性和状态免疫；保留 Conversion/Transform 自有半无敌检查 bug | `converted_poison_type_blocks_primary_poison`、`conversion_and_transform_preserve_their_own_invulnerability_checks` |
| B19 | Trainer AI Layer3 用 TypeEffects 首个匹配，保留双属性 AI bug | `trainer_ai_uses_first_matching_type_chart_row` |
| B20–B22 | 每敌经验后重置参与旗标；只学最终等级招式；每只显示实际分摊 EXP，gain/level/learn 按原作逐只顺序 | `exp_flags_reset_and_only_final_level_move_is_learned`、`exp_notices_keep_each_mon_gain_and_level_together` |
| B23 | 暴击读原 party stats，忽略徽章/Transform working stats；单机敌 Transform 暴击按自己等级与复制 DVs 重算，联机读原 enemy party stats | `critical_damage_uses_party_stats_instead_of_badge_or_transform_copies`、`transformed_enemy_crit_recalculates_own_level_except_in_link_battles` |
| B24 | Bide 继续回合在状态门后累加共享伤害、计时，初始回合不减；释放走正常 Damage/Substitute，清原计数/累积 word | `fidelity_multiturn.rs` 的初始计数、旧共享伤害、Substitute、零伤害、16-bit wrapping、Counter 反 Bide 回归 |
| B25 | 捕获任何已 Transform 野怪时按原版强制重建 Ditto，保留原 DVs 和当前 HP/status | `catching_a_transformed_non_ditto_preserves_original_ditto_assumption_bug` |

Mimic 使用原作选择框，不显示普通攻击菜单的 TYPE/PP；app 与 TUI 同步渲染，snapshot 保留等待选择的状态。Transform/Mimic 的临时复制不会永久写入存档，EXP 与学招均查原始 party 身份。

保留的原版行为：Focus Energy 降低暴击、Recover 等特定 HP 差失败、完全麻痹 Fly/Dig 半无敌、Trainer AI 首匹配、Counter 旧共享伤害，以及单机 Transform 捕获 DVs/catch-rate。伤害防御除零继续防冻结；不要求逐 byte 同步原版 RNG 旋转序列。

本分支首次验证：`cargo test -p pokered-core --test fidelity_battle --test fidelity_multiturn` 20 + 11 tests 全过；`cargo test -p pokered-core --lib` 2602 tests 全过；`cargo check -p pokered-app -p pokered-tui -p pokered-ui` 通过。之后追加 B24 完整机制回归与 B25 捕获回归，最终文件有 21 + 16 个 production tests；它们需要根分支集成后最后一次运行，当前不标为已验证。

另一个待复核边界：徽章重施与非整数 stat-stage 的取整顺序。完整伤害数值并未宣称穷举一致。
