# 战斗机制逐项原作审计（2026-10-02）

基线：open-pokered `72ff719`，pret/pokered `fbcf7d0`。下面行号均指修复前基线。
生产入口是 `crates/pokered-core/src/battle/mod.rs:4341` 的 StackDriver，
`battle/pokered_rules/runtime.rs` 的适配与 `pokered_rules/mod.rs` / `rules.ron` 的规则。
legacy `turn::execute_turn` 只作测试 oracle；与 legacy 一致不能证明还原原作。
原作是稀疏检出，缺失文件通过 `git show HEAD:<path>` 读取，引用行号仍是该 Git 文件行号。

优先级：P1＝正常游戏中错误机制或持久化数据损坏；P2＝特定交互、边界或原作故意保留的 bug 差异。
静态确证意为双方完整相关控制流足够确定差异；动态确证另有生产代码探针。
以下均超出 FIDELITY_GAPS.md 最新两项遗留，而不是历史注释的重复登记。

## 1. 入场、状态检查及 PP

| ID | 优先级 / 触发 | open-pokered 实际 | 原作预期与证据 | Rust 证据 / 确证方式 |
|---|---|---|---|---|
| B01 | P1：睡眠、冰冻、混乱自伤、完全麻痹阻止行动 | 先消耗玩家 PP，再运行 BeforeMove 状态门；睡眠例 PP 20→19 | 状态门在 PlayerCanExecuteMove 之前；只有到 core.asm:3118-3122 才 DecrementPP；睡眠/冰冻门 core.asm:3320-3377，混乱/麻痹 core.asm:3427-3479 | battle/mod.rs:4248-4257、4341；生产 BattleScreen probe 睡眠确证，另三条静态同路径 |
| B02 | P1：任意混乱自伤 | 50 级、100 攻防自伤 28；忽略攻防等级并意外加 Normal STAB | 原作直接 CalculateDamage，跳过属性修正/随机伤害；该例 19；使用当前已修正攻防 core.asm:3672-3697、5751-5798 | p5_native.rs:821-847：stage=0，move/attacker 全 Normal；types.rs:106-109 加 STAB；生产 StackDriver probe |
| B03 | P2：混乱自伤或完全麻痹时处于 Bide / Thrash / Fly / Dig / Wrap | BeforeMove Fail 后仍保留这些锁定 volatile | 原作 core.asm:3430-3464 清除 STORING_ENERGY、THRASHING_ABOUT、CHARGING_UP、USING_TRAPPING_MOVE；混乱自伤同时清 INVULNERABLE，只有完全麻痹保留 INVULNERABLE，形成原版 Fly/Dig bug | p5_native.rs:792-795、809-812 只 Fail；runtime.rs:509-554 从仍存 arena 重建 flags；静态确证 |
| B27 | P2：burn/paralysis 与自身能力改变、对方能力招、Haze、Transform 的交互 | 总在伤害末端按 burn / 速度末端按 paralysis 减半/四分一，因此自身能力重算仍被罚，对手能力招不累积重复惩罚，Haze 源虽重置 stats 仍被罚，Transform 没复制真实已受罚工作数值 | core.asm:6271-6363 罚直接改工作 stats；effects.asm:414-449 自身能力重算忽略状态罚，499/503-505 自己升能力后误再罚对方，689-699 降能力后也重罚目标；haze.asm:8-14 抄 unmodified 后不重罚；transform.asm 抄当前工作 stats | damage.rs:124-128 每次物理伤害固定减半；mod.rs:2886-2898 每次速度固定四分之一；完整原作源码确证，已修复真实 working-stat 载体并增加 8 个独立生产回归 |


## 2. 命中、等级、伤害与暴击

| ID | 优先级 / 触发 | open-pokered 实际 | 原作预期与证据 | Rust 证据 / 确证方式 |
|---|---|---|---|---|
| B04 | P1：使用 X Accuracy 后攻击 | USING_X_ACCURACY 保存着，但生产命中不读；accuracy byte 255 仍 miss | core.asm:5290-5293 / 5309-5312 在命中 RNG 前直接返回成功；Dig/Fly/Mist 前置阻挡、OHKO 的当前速度比较仍适用 | items/battle_items.rs:20-23 设置 bit；runtime.rs:381-431 不传此 bit，pokered_rules/mod.rs:1556-1602 无 bypass；probe |
| B05 | P1：自我强化、Recover 等无目标命中测试的招式，对面正在 Dig/Fly 或高闪避 | 对敌方做统一命中测试，Swords Dance 对 Dig 失败；普通状态也有 1/256 miss | core.asm:3123-3146 按 ResidualEffects1 与 power=0 分派；StatModifierUpEffect effects.asm:351-383 无 MoveHitTest；heal.asm:1-108 同；具体会调用 MoveHitTest 的状态技仍须单独保留；Conversion 自身效果还检查目标 INVULNERABLE，Transform 保留玩家误查自己、敌方检查失效的原作 bug | pokered_rules/mod.rs:1577-1598、rules.ron:93-120；probe Swords Dance 对 Dig |
| B06 | P1：非 50 级 Seismic Toss / Night Shade | 10 级实际均造成 50，而不是 10 | core.asm:4643-4650 / 4761-4767 读攻击方实际等级 | RuleBindings::battler_level mod.rs:741-742 读测试用按 species LEVELS cache，1385-1386 默认 50；production 4272-4275 未 set_level；两招 probe |
| B07 | P2：Psywave | 对双方使用一个 byte 的缩放值，且同样错误读默认 50 级 | core.asm:4657-4669 玩家拒绝抽到 0 或 ≥floor(1.5×level)；4776-4789 敌方允许 0，二者均 rejection sample，非缩放 | rules.ron:507-510 RngScaledLevel；mod.rs:741-742；静态确证（等级问题由 B06 probe 验证） |
| B08 | P2：Focus Energy + 高暴击招式，base_speed/2 为奇数 | 先乘 8 后 /4，例如 Kingler speed 75 算 74/256 | core.asm:4491-4533 先 floor(base_speed/2)，Focus Energy 再 /2，最后高暴击 ×4；Kingler 得 72/256 | damage.rs:33-51；静态整数算式确证 |
| B09 | P2：Reflect/Light Screen 后防御值 ≥1024，或 Explosion 缩放边界 | Rust 缩放后仍保留 >255，Explosion 在缩放之前 halve defense | core.asm:4104-4130、4217-4253 缩放后只把低字节送 b/c，防御可能回绕；CalculateDamage:4317-4323 才对 c halve | damage.rs:85-89、144-150；静态确证；原版除零 freeze 在 Rust 以 max(1) 有意防护，不应为了保真让游戏冻结 |
| B23 | P1：暴击时玩家拥有徽章强化、stat-up glitch 或 Transform；敌方 Transform 后暴击 | 直接使用 battle working stats，徽章攻防仍影响暴击，Transform 暴击用复制的攻防 | core.asm:4060-4074 / 4090-4100 玩家暴击从 wPartyMonN 原始 stats 读取；敌方 core.asm:4155-4178 / 4192-4211 同。GetEnemyMonStat:4260-4299 联机取原始 enemy party stats，单机按当前 species/DVs 和自己的 level 重算（不是被复制目标等级） | mod.rs:1690-1702 直接 b.stats；runtime.rs:443-445 提供 boosted overlay，未另传 critical party stats；静态确证，修复增加真实生产暴击回归 |
| B26 | P2：徽章强化 + 非整比能力等级，例如原 attack73、Sharpen +1 | 重置 raw73，再徽章→82，之后伤害/速度按能力级修正→123；顺序错误并影响跨回合、Rage、X-item、RUN 与 Transform | effects.asm:414-449 先从 unmodified stat 乘能力等级→109，再499 ApplyBadgeStatBoosts→122；伤害读当前 battle stat，不二次乘级数 | badge_boosts.rs:158-181 raw carrier；mod.rs:1706-1731 下游伤害才乘能力等级；静态确证与新增 production Sharpen→BodySlam 70 对72 的回归 |


## 3. 特殊招式、连续招式及战斗身份

| ID | 优先级 / 触发 | open-pokered 实际 | 原作预期与证据 | Rust 证据 / 确证方式 |
|---|---|---|---|---|
| B10 | P1：Disable，尤其首回合目标还未行动 | 禁用目标上回合 last_move，未行动则完全无效 | effects.asm:1303-1348 随机抽非空槽；对玩家/联机另拒绝 PP=0；普通非联机敌人 PP 无限 | mod.rs:4288-4293、2153-2185；首回合 probe |
| B11 | P1：Mimic | 自动复制敌方上一回合 last_move，PP 改 5；没有玩家选择菜单 | effects.asm:1203-1270 普通单机玩家通过 MoveSelectionMenu 选对手任一已知招；敌方/联机随机抽非空槽；只改 move ID，不重置 PP | battle/mod.rs:4359-4385；BattleScreen probe |
| B12 | P1：Transform / Mimic 后换出或结束战斗 | Transform 的 species/stats/moves/PP 写进 party；Mimic 的替换招式同；后续保存永久写入；Transform 还未复制目标 DVs/catch-rate 的 battle bytes | 原作 Transform 只改 wBattleMon / wEnemyMon，transform.asm:1-129；ReadPlayerMonCurHPAndStatus core.asm:1800-1809 只把 HP、status 写回 party；decrement_pp.asm:19-30 transformed 不回写 party PP | runtime.rs:496-505、state.rs:378-412、settlement/writeback.rs:180-183；probe Ditto 变 Snorlax 后 reset volatile 和 settle 后 save 都仍 Snorlax；Mimic 同路径静态确证 |
| B13 | P1：Counter 面对 Substitute、Dig/Fly、低命中或前回合伤害 | 直接 pair_mut 扣真实 HP，不走 Substitute，完全跳过命中；仅可反当前回合收到的伤害 | core.asm:4554-4608 使用共享 wDamage（允许延续旧伤害/自身伤害的原版 bug），确认 N/F power>0 后 call MoveHitTest；正常 ApplyDamage 路径检查 Substitute | mod.rs:1564-1567、1829-1850，DamageTaken 每回合重建；静态确证，未穷举原版 Counter 菜单内存残留行为 |
| B14 | P1：Wrap/Bind/Fire Spin/Clamp 连续攻击 | 每次重跑 crit/accuracy/damage，后续能重新 miss、换伤害；原始命中数被单 byte 近似 | core.asm:3554-3566 明确跳过 damage calculation、DecrementPP、MoveHitTest，沿用上次伤害；effects.asm:1094-1102 有条件第二次抽 RNG | mod.rs:1987-2026、1519-1686；静态确证；近似抽样分布相同本身不是功能 bug，重新命中/随机伤害才是 |
| B15 | P2：Thrash/Petal Dance 到期，或连续招式 miss | 疲劳混乱用 (rng &7).max(1) 得 1..7，且锁定计数只在 DamagingHit 递减 | core.asm:3538-3550 在继续攻击之前递减，混乱 (rng&3)+2＝2..5；不以本次命中为计时条件 | mod.rs:1907-1919；静态确证 |
| B16 | P2：Recover/Softboiled/Rest 缺 HP 恰为 255 / 511，或满 HP Rest | 正常恢复，未保留原版失败判断；probe 400/145 Recover→345 | heal.asm:13-20 的有借位低字节比较 bug：差 255/511 失败；满 HP Rest 也失败，不能借它只治状态 | rules.ron:115-116、p5_native.rs:389-412；Recover probe |
| B17 | P2：给需 Hyper Beam recharge 的目标用催眠招 | 仍正常命中检验、不能覆盖现有状态，不清 recharge | effects.asm:35-41 先清目标 recharge；原先 recharge 则跳过所有命中与状态测试、直接设 sleep | rules.ron:264-266、RuleBindings set_status guard mod.rs:590-623；静态确证 |
| B18 | P2：Conversion 改成 Poison/Grass 后受 Toxic/Leech Seed，或 Transform 复制有 Conversion 的目标 | status HasType 仅读 species；Transform 的类型也回落 species 基础类型 | 原作对应效果直接读当前 battle-mon type；transform.asm:61-71 连当前类型一起拷 | mod.rs:654-657、2912-2917 明确承认；transform_install:2085-2095、runtime.rs:489-503；静态确证 |
| B29 | P1：较慢一方本回合 Mirror Move、目标睡眠/冰冻、Mirror Move 复制 Mirror Move；Metronome 选择 Quick Attack 或被状态门阻挡 | 读前一回合历史，首回合复制失败；睡眠后仍复制旧 Tackle；复制 Mirror Move 被当成功；玩家调用技预解析使结果改排序、阻挡前抽选择 RNG | used_move_text.asm:11-19 立即写 used-move byte；core.asm:3351/3361、5698/5707 睡眠/冰冻清0；4962-4984 读当前字节并拒绝0与Mirror Move；348-399 selected move 排序早于实际招式解析，3091/3123与5481/5515 状态门早于效果 dispatch | 初始基线 frontend 预解析与整回合后 last_move_used writeback；B28后 cd9acde battle/mod.rs:4174-4190 玩家仍预解析、4342 敌方读旧历史。独立 raw-rustc production probe 3/3原期望失败，见 `battle-called-history-before.log`；已增 native UsedMove carrier 与门后调用解析；根集成运行6个独立回归全过 |
| B24 | P1：Bide 蓄力/释放，尤其速度先手或目标有 Substitute | 初始回合结束即减计数，每回合末累加当前收到伤害并直接扣目标 HP；不读跨回合 wDamage，不走 Substitute | core.asm:3491-3534 在继续行动的状态门后累加共享 wDamage、DEC，释放在自己的行动时经普通 ApplyDamage；初始安装计数 effects.asm:774-786 不 DEC | mod.rs:2489-2541 Residual hook 与 pair_mut 直扣；原生产路径静态确证，新增初始计数/时序/共享伤害/Substitute 回归 |
| B25 | P2：野生敌方 Transform 后被捕获（含非 Ditto 用 Mirror Move 复制 Transform） | 保存复制后的 battle species/stats/moves/DVs/PP | item_effects.asm:469-480 原作将任何已 Transform 野怪视为 Ditto，再 LoadEnemyMonData；使用原 DVs、新 Ditto stats/learnset/PP，保留当前 HP/status | battle/mod.rs:3351 直接 clone enemy battle mon；静态确证，新增真实 screen Master Ball 捕获回归，刻意保留非 Ditto 变成 Ditto 原版 bug |


## 4. 训练家 AI 与经验/升级

| ID | 优先级 / 触发 | open-pokered 实际 | 原作预期与证据 | Rust 证据 / 确证方式 |
|---|---|---|---|---|
| B19 | P2：具有 Layer3 的训练家针对双属性 | 使用双属性相乘后的完整相性，例如 Poison 对 Grass/Poison 1×不鼓励 | core.asm:5192-5223 在 TypeEffects 第一个匹配就返回；Poison->Grass 的20先出现（type_matchups.asm:45），故原作鼓励毒系，忽略随后 Poison 抗性 | trainer_ai/move_choice.rs:121-127；静态确证；原版已知 AI bug，恢复它才是与原作一致 |
| B28 | P1：训练家敌方正在蓄力、Thrash、Bide、Wrap、Rage 或 Hyper Beam 充能；玩家先手造成异常/低 HP | 明确 guard 禁止 AI 使用物品/换人；解除 guard 后强制招式又覆盖 Nothing，充能也会被错误消耗；AI 在玩家攻击前决策、先手玩家时把物品延迟到敌方持续伤害之后，错误治疗/换人顺序且 KO 仍抽 AI 随机数 | core.asm:416-426 / 441-465 在实际敌方时点调用 TrainerAI，然后才执行敌方招式或持续伤害；trainer_ai.asm:291-320 无强制招式/充能 gate，619-635 治疗同时清 BADLY_POISONED | 基线 battle/mod.rs:3810-3815 guard、3973-4006 提前决策、4325 Nothing、4564-4574 延迟 apply；pokered_rules forced_action 覆盖 Nothing。新增 `fidelity_ai_locks.rs` 9 个真实 Screen 回归含 12 种锁定/先后手组合、即时异常/低 HP、KO RNG、治疗上限、换入怪持续伤害、Transform/初始 Bide PP；源码已修；根集成运行 9 tests 全过（矩阵含 12 种锁定/先后手组合） |
| B20 | P1：训练家首只敌人多人参战，后续敌人不再换人 | 已退出怪继续取得并稀释后续 EXP | experience.asm:275-289 每次 GainExperience.done 清参与旗标，只保留当前 active；每只敌人重新累计参战 | experience/gain.rs:58-120 从未重置；mod.rs:1367/4841/4900 只置 true；probe gain 后双 flag 仍 true |
| B21 | P2：一次经验奖励跨越多级并越过学招等级 | 逐级补学所有中间等级招式 | experience.asm:171-179 直接写最终等级，256 调 LearnMoveFromLevelUp；evos_moves.asm:341-349 只检查当前最终等级相等，未补中间等级 | experience/level_up.rs:77-85；静态确证 |
| B22 | P2：多人分 EXP 或 EXP ALL | 消息只给 active mon 一个未分摊全额经验数字，真实加的却是分摊值 | experience.asm:97-111 保存实际 wExpAmountGained；153-154 每个非濒死参与怪逐个 GainedText | battle/mod.rs:4965-4973 不用 gain 实际分享结果；静态确证 |

## 保真边界与已核查项目

- 命中 1/256 miss、Swift 穿过 Fly/Dig、Gen1 不存在后世 Earthquake/Gust 等命中半无敌例外，当前代码正确。
- 实际伤害有 STAB、分属性物理/特殊、上限 997+2、Reflect/Light Screen 暴击忽略、爆炸减防、类型免疫；B09/B23 以外未把源码相似当作全覆盖证明。
- 原版 Focus Energy 降暴击 bug、Hyper Beam 击倒跳过 recharge、Toxic+LeechSeed 连续累计 bug、Haze 仅清目标状态与同回合睡眠/冰冻 forfeiture，已有实现；B08/B17 为另外的交互。
- Capture Rand1 拒绝采样窗、状态立即捕获、HP 与球公式、Safari 饵/石子 catch rate 与 upkeep/flee 读低 speed byte的完整主要分支已人工对照，除 B25 的 Transform 身份重建以外无新增确定捕获公式 bug；未宣称所有种类×HP×球的穷举一致。
- Trainer per-class item/switch routines人工抽查核心 gate、CooltrainerF 缺 ret nc 原版 bug、AI budget；B19 是具体新差异。非联机敌人无限 PP 符合原作。
- RNG 暴击 rotate_left(3)、伤害 rotate_right(1) 未在 Rust做，但采用均匀byte时概率分布一致；此项目并非逐 byte ROM RNG 模拟，因此只记随机序列差异，不计玩家机制 bug。
- 伤害防御除零保护是已写在 damage.rs:84 的有意防冻结。是否保留这种保护属于产品选择，不把保护称成意外回归。
- 学招 full-four prompt 已还原；本次 B21 是跨多级终点语义，B22 是 EXP文本。PP Ups、PC/Daycare 的审计由 systems 文件覆盖。

## 可复现证据

`battle-probe.rs` 使用真实生产 StackDriver、adapter 与 BattleScreen.update_frame，没有调用 legacy execute_turn。
它断言基线实际异常（因此基线 test 全绿不表示问题不存在）；原作期望从上表 asm 得出。

复现方式：将该文件复制至 `crates/pokered-core/tests/audit_battle_probe.rs`，
`source /workspace/onboarding/env.sh` 后运行
`cargo test -p pokered-core --test audit_battle_probe -- --nocapture`；结束删除临时 test。
结果：1 test / 11 条观察（其中 fixed-level 两招分开），全部成功，0.05 秒。
上述命中、伤害和状态 probe 直接驱动生产规则；Sleep/Mimic/Transform 另驱动真实 screen，Transform 还跑到真实 save writeback。

本文件是修复前审计证据；后续修复状态与测试结果应在根报告另列，勿删掉基线实际行为。
