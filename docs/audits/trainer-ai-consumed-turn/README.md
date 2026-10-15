# 玩家用道具、主动换人后仍执行训练家 AI

原作在玩家成功使用道具或主动换人后，仍会咨询训练家自己的换人、道具 AI。复刻的 `EnemyFreeTurnAfterItem` 和主动换人均进入 `run_enemy_free_turn_stack`，该路径直接执行敌方招式，没有普通 FIGHT 回合已有的 TrainerAI 回调。因此 Juggler 不能在这些回合换人，Misty 也不能使用 X DEFEND。

父 PR #257 已合入。最终对照重新构建真正的主分支 `6d30ba5a`，修复版本在其上整合为 `4dcbffa8`；没有把历史 b7c81/ed6 的捕获重标为当前构建。

## 原作与实现

原作 `engine/battle/core.asm:326–345` 根据 `wActionResultOrTookBattleTurn` 跳过玩家选招，仍经 `SelectEnemyMove` 进入速度分支。两条分支在416、454行调用 `TrainerAI`，成功 special action 后跳过 `ExecuteEnemyMove`，随后仍处理该方中毒、烧伤、寄生种子伤害。

`engine/battle/trainer_ai.asm:290–319` 排除野生与通信战斗。`ExecuteEnemyMove` 内部才处理状态与蓄力：`CheckEnemyStatusConditions` 在5729–5731行消耗 Hyper Beam 的 recharge，`EnemyCanExecuteChargingMove` 才清除 CHARGING_UP。AI 道具替代本次攻击时不会提前消耗这些 continuation。

修复在同一生产 free-turn StackDriver 的 enemy `before_action` 中调用现有决定、应用 AI 的 helper。成功时使用已有 `TurnSuppressed` 跳过本次攻击，仍运行 residual；训练家换人经父分支实际加载通知登记对应种族、重置参战位。野生、通信与耗尽次数的情况提前返回，没有新增队伍副本或持久流程字段。普通 FIGHT 热路径不变。

原 free-turn 会预先解析 Metronome/Mirror Move；现在使用普通 FIGHT 已采用的 stack called-move handler，在 AI 和状态 gate 之后解析。这样被训练家道具替代的攻击不会预先取随机招式或错误显示调用招式的消息。AI 道具也不会错误消耗 recharge，或把换人清掉的 Substitute 当成被击破。

## 实际普通按键与存档

验证前构造合法 Lv100 妙蛙种子、队伍、地图、初始图鉴、未战胜训练家标志及 X ACCURACY×99。验证阶段只使用普通方向、A/B、战斗和 START → SAVE；原作 hook、复刻 TCP 查询只读。没有在验证中写 RAM、指定 AI 结果或调试开战。这是局部机制的构造场景，整份剧情模板没有自然通关来源的声明。

每场景各重复两次，每次先实际进行六个成功道具回合或六次主动换人，再正常结束战斗并普通 SAVE。

| 实际场景 | 原作每重复 special | 修复前 | 修复后每重复 special |
|---|---|---|---|
| FuchsiaGym Juggler #4：Drowzee34/Hypno34，六次 X ACCURACY | 自然换人2次 | 六回合均选定敌招 | 自然换人2次 |
| 同一训练家，六次主动在两名成员间换人 | 自然换人1次 | 六回合均选定敌招 | 自然换人2次 |
| Misty，六次 X ACCURACY | 第2回合使用 X DEFEND | 六回合均选定敌招 | 第2回合使用 X DEFEND |

原作6次、最终干净主分支6d30修复前6次、最终整合4dcb修复后6次均读回有效主校验和的32 KiB普通存档。历史b7c81/ed6的另外12次控制仍原样保留，不代替最终当前版本。同一构建同场景的两个重复 SRAM 相同。每次 X ACCURACY 正好消耗1，六次换人不消耗道具；每个恢复的 special 回合均没有再执行敌招。六个回合之后实际打开首次 FIGHT 菜单读取 `battle_moves`，玩家 PP 仍为40/35/10/30；没有把 mid-battle `party` 的开战快照当成实时 PP。前后 current box、PC storage、HoF 和归档箱 SRAM banks 也保持相同。

源 Juggler 控制继续用 Growl 并最终战败；源 Misty 使用正确的1-based第二格光标2选择 Tackle 后获胜。复刻在观察完六回合后使用 Tackle 获胜。随机流、HP、战斗长度、收益和具体 special 次数的跨提供者差异均保留，没有要求整体 SRAM、结果或逐帧等价。

## 冻结身份与画面

| 用途 | 编译提交 | 二进制 SHA256 |
|---|---|---|
| 最终修复前，实际合入父257的干净 master | `6d30ba5abfb40e0873b6b4bea962f0ae29989bfe` | `5e7c8caf53d7825330a828cf54d987ff6c0da0861401ec4850e9ffe0b776d70e` |
| 最终修复后，基于6d30 | `4dcbffa810e75dfb8d2bf0e82640973de054194c` | `57a277a04a442324cc76c5fa6dfb794004ca9e09b29b24c119429724b9011629` |
| 历史原始审计前版本，干净 master | `b7c81d82d7d94a06555f631c3c70d114f299d9d8` | `bb844b56204ef0c75c5a9e29282a54d19558798afb88f2f206597938a60bb2c9` |
| 历史私有父257后续修复版本 | `ed65979115173b9db9d95ca9c22ede6f42965c59` | `ec6deed1c5777a11a1d134245d566f2b0366974733ddbde4e26585a3436dd688` |
| 原作 Red | `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c` | ROM `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b` |

两个原生版本都使用各自运行目录、二进制副本、存档与端口；现代 fixture 没有共享 legacy sidecar。最终修复前使用新的独立 `fidelity-909-current-before-target` 完整构建，修复后使用自有909 target；两者都明确构建 debug-server 应用，并保留源码/配置逐文件哈希。

`docs/screenshots/trainer-ai-consumed-turn/misty-item-{before,after}.png` 是同一 fixture、相同完整普通输入前缀、同帧 **1080** 的实际画面，两边各重复两次：前为 “Enemy STARYU used TACKLE!”，后为 “MISTY used X DEFEND!”。这一短视觉重播停在1080帧，普通 SAVE 由上述独立完整控制证明。当前两张图是新运行的6d30/4dcb捕获，不是旧图重标；历史b7c81/ed6图和原始身份保留在原证据ZIP及 `manifest-historical.json`。

## 回归与界限

- 新增10项核心回归，经生产道具菜单/成功药品 continuation、主动队伍选择入口执行；覆盖失败药品不耗回合、AI 替代攻击且只执行一次、Juggler 实际 seeded 换人与加载通知/参战位、Hyper Potion 先治疗再承受 residual、AI 不消耗 charge/recharge、野生/通信/次数耗尽排除，以及 Metronome 抑制与 Mirror Move 正常执行。
- 完整核心 **2898通过**；完整应用 **289通过、39忽略**；快照确定性集成 **8通过**；显式 debug-server 应用构建通过。
- 最终使用新的独立909-current GBA source mirror/target 和 nightly-2025-12-07，原七场景性能门禁通过一次，训练家更新232 ticks，静止场景382 ticks，Oak绘制2994 ticks；原门限和基线不变。这不表示远程 CI 已完成，也不证明 GBA 上所有交互 special 动画或音频逐帧等价。
- 实际原作矩阵只含上述 Juggler 与 Misty 场景。Potion、训练家治疗 residual、charge/recharge、通信与 called moves 的新增覆盖为单元/静态检查，没有冒充自然 Hyper Beam/Dig 或实际双端通信采集。

`current-master-verification.json` 是最终当前版本的简明结果，`current-master-evidence.zip` 保留新6d30/4dcb的完整实际记录、同帧截图、构建/回归/GBA日志及身份清单。原 `verification.json` / `evidence.zip` 按原始字节保留历史b7c81/ed6/source控制，不作为新构建结果。两个ZIP都对相同原始字节按 SHA256 去重；`entry-manifest.json` 保留所有原路径、长度及哈希，运行 `python3 extract_evidence.py evidence.zip --verify-only` 可独立校验。ROM/可执行文件只保留精确哈希，不打包。

原始失败均保留并排除：原作过短按键、光标索引误用、强制换人及 Misty 颁奖对话的驱动问题；复刻 numeric ItemId fixture 拒载、PartySubMenu 调试字符串假设；缺失本地 gfx 链接、把真实药品 HP 与动画显示 HP 混淆的测试断言、wrapper 执行位及第一次 GBA cwd。旧派生 PP 草稿也保留，由真正 `battle_moves` 的读取结果更正。它们均没有改称玩家 bug 或通过结果。

已整合最新 master `8eb08f56`（#262，FLASH 触发修复）：完整核心 **2898 通过**、debug-server 应用 **289 通过/39 忽略/0 失败**。精确源码与日志见 `master-8eb-integration.json` 和日志包。原作与原生实际捕获继续保留各自冻结身份；本次整合不重标截图或存档。
