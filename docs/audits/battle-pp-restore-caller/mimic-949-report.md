# 949：实际 Mimic→Ether→切换保存验证

原作、实际主分支 `a2fa94f1` BEFORE、935 冻结 `af3f5241` AFTER，各完成一次普通训练家战斗与普通 SAVE（共3次，全部校验有效）。AFTER 与原作都保存为原招式 Mimic、PP9；BEFORE 恢复了临时复制的 PoisonSting，切换后丢失恢复结果，保存原 Mimic PP7。它验证 935 原身份入口确实覆盖一个独立于 Ditto 的玩家 Mimic 路径，没有发现需要新增的生产改动。

## 普通输入与结果

首次输入前构造合法 Pikachu20：TM31 Mimic＋初始 Thundershock／Growl＋16级 QuickAttack，PP `[10,30,40,30]`，PP Ups0，DV9A78、零 StatExp、OT0／NINTEN；健康合法 Bulbasaur20 替补，Ether2。原作准备森林地图 state，原生使用完整 JSON；起点均 ViridianForest(28,32)，随后正常 Down 触发既有训练家，验证期间没有写 RAM、强制战斗结果、改变 RNG 或传送。原作直接导入 PyBoy、解析只读编译符号，没有执行旧驱动，也未读写 `stats-59-reference-{1,2}` 输出。

1. FIGHT 选 Mimic，再从敌方已知招式中实际选择 PoisonSting。Mimic PP10→9；下一回合使用复制招式，PP9→8。
2. BAG→Ether→活跃成员。原作／AFTER 招式菜单显示原 Mimic；BEFORE 显示临时 PoisonSting。原作／AFTER 立即恢复原槽到10，工作招式仍为 PoisonSting，其余三招与 PP 不变。真实120帧不确认期间无敌行动，原作／AFTER 仍保留 Ether2，最终一次 A 确认才扣到1并接敌方回合。BEFORE 已提前扣到1，且按 PoisonSting 最大35把工作 PP 恢复到18。
3. 同场再次使用 Ether。原作／AFTER 的原 Mimic 已满，显示无效果，保留1、不触发敌方回合；BEFORE 又把工作 PP18→28并用掉第二份。
4. 再使用一次复制招式，原作／AFTER 的工作与原 Mimic PP10→9；BEFORE 工作28→27、保留的 Mimic8→7。切换到替补，正常 Tackle 完成训练家战斗，然后 START→SAVE。三份 SRAM 都保留原四招 `[Mimic,Thundershock,Growl,QuickAttack]`；第一槽分别为原作9／BEFORE7／AFTER9，其余 PP `[30,40,30]`。

## 可审证据

`audit-949-verification.json` 汇总原始目录、冻结身份、普通输入、79张阶段 PNG 哈希、3份 SRAM 校验和、保护字段与限制。原作只读 hook 同时记录队伍／战斗 PP、DecrementPP、RemoveUsedItem 和敌方入口：成功只 RemoveUsedItem 一次，第二次无效果不扣、也没有敌方执行。原生记录完整按钮数组与每次同步推进的实际帧号；活跃 PP 在真实 FIGHT 菜单检查，原身份最终由普通 SAVE 检查。

原生前后 fixture 字节相同，四张表面有完整相同按钮前缀及相同实际帧：第一次 Ether 选择1424／结果1576、第二次选择1905／结果2057。原作与原生不做全场帧同步。已目视原作 Ether／无效果页及原生前后相同选择页，修复后保留队伍与原 Mimic，旧版本为战斗 HUD／PoisonSting。

三份保存的 OT11 原字节、默认昵称、原招式、DV、等级／属性等身份字段和非出招 PP 都核对；替补 Tackle 保存 PP 分别33／33／32，逐一与真实出招次数2／2／3吻合。原生 current_box、全部 pc_storage、HallOfFame 与对应初始 JSON 完全一致；原作当前箱、队员 OT／昵称及 owned dex 不变。训练家奖金60和原生 event_flags 的172字节变化保留；HP／状态、经验／StatExp、时间及整份跨 provider SRAM 不强行匹配。

`audit-949-evidence.zip` 仅收本轮三个独立目录、脚本／fixture、身份、日志和验证文件，原始路径按 SHA256 blob 去重，全部压缩内容与原文件逐字节读取校验；不含 ROM、可执行文件或模拟器 state。构造 state／私有 ROM 本地保留，哈希在记录里。两次脚本空格 SyntaxError 都在实际运行前／验证器解析时发生，失败脚本与日志保留；没有失败玩法控制或无诊断重试。

范围仅一次 Ether 成功＋满 PP Ether 无效果＋继续消耗＋切换 SAVE。没有新增 Max Ether／Elixir／Max Elixir、Mimic PP Up、嵌套 Transform、升级学招、link、快照、音频波形或全主线的实际结论。FIGHT PP-Up 分母候选由948负责，本轮没有修改或重复修复。W935、两个既有 ZIP、借用的源码／地图／二进制／target 均未修改；没有构建、push 或 PR。
