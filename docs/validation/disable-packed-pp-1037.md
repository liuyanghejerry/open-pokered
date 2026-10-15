# Disable 的 packed PP 与同回合扣费校验（1037）

当某槽当前 PP 为 0、但有 PP Up 高位，且另一个槽还有 PP 时，原作仍允许 Disable 抽中该槽。此前实现先按解包后的当前 PP 筛选候选，改变了可选槽及随机数消耗；玩家先手用完最后 1 PP 后，后手 Disable 还会读到未扣费的回合入口视图。

本次仅修改核心战斗规则、适配器与测试：保留真实 packed PP 视图，让 Disable 按原作顺序拒绝抽样，并在敌方读取之前通过现有扣 PP 函数消费玩家已执行动作。没有 renderer/UI/字体变更，按 AGENTS.md 的纯游戏逻辑例外不附截图。

## 原作依据

固定参考源：`pret/pokered`，版本 `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。

- `engine/battle/effects.asm:1286–1367`：先进行命中判定和“已有招式被 Disable”的检查；之后先 `BattleRandom & 3` 抽到非空招式，再对有限 PP 目标检查四个 packed PP 字节的 OR 与 `PP_MASK`。全部当前 PP 为 0 则失败；否则所选槽只检查**整个 packed 字节**是否为 0，失败才重抽，最后另抽持续时间 1–8。非 Link 敌方跳过 PP 检查，维持无限 PP 行为。
- `engine/battle/core.asm:3118–3123`：`PlayerCanExecuteMove` 在 `DisplayUsedMoveText` 后调用 `DecrementPP`，早于招式效果和后手行动。
- `engine/battle/decrement_pp.asm:1–43`：既有 Struggle、连击/连续动作等扣费豁免，以及 Transform 的战斗副本与原队伍 PP 分工。
- `engine/battle/move_effects/transform.asm:103–122`：复制招式的 packed PP 写为 5；遇到首个空槽后，剩余槽写 0。该值必须在同回合后手 Disable 前可见。

关键指令摘录（省略分支与地址准备；完整路径见上）：

```asm
; Disable: 先检查全体低六位，再检查所选槽的整个字节。
    or [hl]
    and PP_MASK
    jr z, .moveMissedPopHL
    add hl, bc
    ld a, [hl]
    pop hl
    and a
    jr z, .pickMoveToDisable
; duration 的 BattleRandom 在上述分支之后。
```

`PP=0, PP Ups=1` 的 packed 字节是 `$40`，所以只有“所有槽低六位均为 0”才使该目标整体失败。这保留的是原作规则，包括其 PP Up 边界行为。

## 实现边界

- 适配器给 Disable 提供四个实际 working PP 字节：低六位是当前 PP，高两位是该 working 槽的 PP Ups。它是现有引擎资源中的回合视图，没有把全局 PP 数量改成 packed 值。
- Transform 在同一资源视图写入 5/0 前缀；最终适配器消费这份真实结果。沿用已有 `original_identity` 保存原身份、working PP Ups 清零与切回恢复语义。
- 玩家通过实际 pre-move gates 后生成临时 `PlayerPpChargePending` 回执。敌方动作前的现有 callback 先写回 canonical 状态、消费回执，再运行 TrainerAI；玩家后手或先手已结束战斗时由 driver 返回后的收尾消费。
- 两个消费点调用同一个既有 `decrement_player_pp`，移除原先独立的回合末扣费。扣除后从真实 active Pokémon 同步 packed 视图；回执消费后即移除，`build_volatiles` 不重建它，TrainerAI 状态重建也不会复活它。
- 保留已有 Struggle、蓄力和连续动作扣费政策；不新增 Link 敌方扣 PP 政策，不改变非 Link 敌方无限 PP，不新增存档/快照字段或第二份队伍 owner。

## 测试与真实范围

新增 [11 个核心测试](../../crates/pokered-core/src/battle/pokered_rules/disable_pp_tests.rs)，使用真实 Pokémon constructors、生产 adapter 与 canonical StackDriver，预期槽位/PP/随机数抽取数直接来自上述原作，不使用 legacy oracle 作为 golden。

| 覆盖 | 实际断言 |
| --- | --- |
| Disable 有限 PP 与 Link | 0 当前 PP + PP Up 槽可选；全当前 PP 为 0 仍先抽 known 槽再失败；空招式/整字节 0 重抽；持续时间最后抽；已有 Disable 不抽槽或时长 |
| 敌方政策 | 非 Link 敌方不按 PP 拒绝，Link 目标按有限 PP 规则检查；PP/PP Ups 与其他 Pokémon 字段不被 Disable 改写 |
| Transform | 同回合 Transform 后 Disable 读取新 working PP；原身份保留；首个 None 后余槽为 0、不挪动招式 |
| 实际 BattleScreen FIGHT caller | 同为 seed42，快玩家 Growl 的 1 PP 输入扣至 0，慢敌 Disable 失败；2 PP 对照扣至 1 且确实成功 Disable，排除单纯未命中造成的假阴性 |
| 一次扣费生命周期 | 冻结/睡眠/需要 recharge 不扣费；玩家后手扣一次；Swift 先手击倒取消敌方 callback 后仍扣一次；重复消费与状态重建不重复扣费 |

FIGHT caller 测试通过 `BattleScreen::update_frame` 的 PlayerMenu → MoveSelect → 选招路径，并检查状态/PP 与等待期间不重复扣费。这是核心公开 caller 测试，**不是**一次完整 native Game 普通输入、普通 SAVE 或原作实机对照。本提交没有启动 Game/CLI/GBA。None 前缀诊断是明确构造的原始 engine 输入，不声称修复仍独立排队的 physical-slot/SELECT 功能。

旧 `runtime::tests::transform_persists_identity_once` 也保留。它现在通过真实 Ditto/Transform → canonical Transform（敌方 Nothing）取得完整 poststate，继续断言复制后 5 PP、TRANSFORMED 标志、后续手动耗至 3 PP 后不重置；不再手造不完整的 Transformed marker。

## 本地验证身份与首次失败保留

- 实际基线：`6bdbaed76ae1fbc69ec771ed0223b570c8d97f0a`（PR276 已合入）。
- 最终受测源码：`4f9b537f44a663c4b999e5447c97049b238a1b29`。相对 `3ba3a442c45eca608dccb29a625ab8a24b573daf` **只有上述旧测试 fixture 改动**，生产字节与新增 11 项测试不变。
- 运行前后实际核对：2179 个 compiler 输入的 Git mode/blob/文件字节、21 个 Cargo 输入、外部 ignored gfx symlink 与全部 702 个 GFX 文件。引擎仍固定 `e533a3d23a0cdbed1524a1ac91184733da4a6f7e`，依赖与 map fingerprint 优化未变。
- 最终仅运行一次 `cargo test --offline --locked --target-dir /home/jerry/develop/open-pokered/.task-tmp/fidelity-1030-tile-candidate-host-target -j1 -p pokered-core`。复用隔离 CARGO_HOME，`CARGO_INCREMENTAL=0`、dev opt/debug 均为 0。
- Cargo/runner 均 exit 0：单元组 **2960 PASS**；全部 41 个 summary 合计 **3372 PASS、0 FAIL、1 个 doc-test 忽略**，不是去重后的独立场景数。新增 11 项及修复的旧测试均各有一次实际 PASS；集成测试也已运行。源码校验通过，工作树受测源码未变。

保留的第一轮不是绿灯：在 `3ba3a442`，先 scoped11 为 11 PASS，再 fullcore 的单元组为 2959 PASS / 1 FAIL，后续集成组尚未运行。失败的旧测试只改 species/stats/moves 并手造 Transformed marker，漏掉真实 Transform 新写入的 packed PP，于是旧 Tackle 的 35 PP 被正确读回，而期望仍为 5。`4f9b537f` 仅修正测试输入生成，没有降低 5 PP 或后续 3 PP 断言，也未以新结果转标旧源码。

维护者本地保存的审计记录（本 MD 未声称附带这些原始日志）：

| 文件 | SHA-256 |
| --- | --- |
| `audit-1036-disable-original-primary.txt` | `83b3b8160a497dffb8119f8d08377f649b1699578d91d77f2686726692af8116` |
| `audit-1037-pp-scoped11.log`（旧 3ba） | `35a9915963757a97fa17828726fd1e680ba9b2de43b9588336357804367120c7` |
| `audit-1037-pp-full-core.log`（旧 3ba 首败） | `ea24a729fdb7515893993a4c82aadd78ad01849cf90c7230029011cc37cc7a01` |
| `audit-1037-test-only-4f9b.patch` | `9bee2295acb871296ac73aae55a85f7625e04f5c58e55f285778b14b7c2b4241` |
| `audit-1037-v2-pp-source-before.json` | `abbc6c6b778d4559726b33075837423d9e68ba4326eb2a7c244bbb9ad364d232` |
| `audit-1037-v2-pp-full-core.log`（最终） | `c12419ee15e3ffda7f4545868d287689904ff038160c8844e3a202f092069bbe` |
| `audit-1037-v2-pp-host-results.json` | `aee6f285536ba509159311231ceafaa7a5ca78e4f7a2eae4fac9ca6ae5950ea5` |
| `audit-1037-final-host-handoff.json` | `7367d08ca2109136e23b9f9d9b455c441311350a7779f24193e0df16bc56bb8f` |

本地核心验证已通过；正式 PR CI 尚待发布后执行。本记录不声明动画、音频、IRQ/时钟或完整主线等价。
