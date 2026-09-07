# 审计修复计划：2026-09-07

对照 `docs/audits/2026-09-07/report.md`（完整通关审计，43 项确认问题）的遗留项修复队列。
本文件只新增、不改写报告历史记录。

状态图例：⬜ 开放 · 🔄 进行中 · ✅ 已闭合（附测试名/截图/日志）· ⏭️ 记录跳过（附原因）

验收总门（全部闭合后执行一次）：

```bash
cargo test -p pokered-core          # 及其他受影响 crate
python3 scripts/verify_scene_translations.py
python3 scripts/playthrough.py      # 完整 m01–m10，输出 PLAYTHROUGH REACHED REQUESTED MILESTONE
```

约束：仅改本仓库；不碰 dotzuki-* 引擎仓库（需要引擎改动的项记录后跳过）；
修复对齐原版 pokered 行为（CLAUDE.md 保真原则）；画面项按 AGENTS.md 附前后截图到
`docs/screenshots/`（截图 CLI：`cargo run --release --bin pokered-app -- screenshot --screen <t> -o x.png -f 10`，
或 debug-server `capture_frame`）。

---

## A 组 · 明确未修复（21 项）

| # | 问题 | 代码位置 | 原版参照（报告给出） | 修复方向 | 验证 | 状态 |
|---|------|----------|----------------------|----------|------|------|
| A01 | 名人堂第二属性文字越过下边框 | `pokered-app/src/render/hof_ceremony.rs:201`（tui 同名实现一并核对） | `engine/movie/hall_of_fame.asm:159–176` TextBoxBorder 内高 9 | 内框高度 8→9 或调整 TYPE2 行位 | 前后截图（嘟嘟 FLYING／拉普拉斯 ICE 两组）；渲染测试 | ✅ 已修复：内框 8→9（app/tui 两处），TYPE2 值落回最后一内部行；证据为像素级前后截图 `docs/screenshots/a01-hof-before/after.png`（`--screen hof` 新截图目标，Doduo/Lapras 与审计同组）；布局为纯常量改动，未设人工单测（截图即证明） |
| A02 | 片尾把 `#MON` 显示为字面 #MON | `pokered-app/src/render/credits.rs:125–130`（tui 同名实现一并核对） | `constants/charmap.asm:16` `#`＝$54 POKé 控制符 | 文本渲染时把 `#` 展开为 POKé 前缀 | 前后截图片尾第一屏 | ✅ 已修复：`expand_poke`（app/tui 两处，数据保持原始 $54 字符）；测试 `poke_control_char_expands_to_display_form`；实机前后截图 `docs/screenshots/a02-credits-before/after.png`（`--screen credits` 新目标，POKéMON 正确显示） |
| A03 | 红莲道馆门禁未初始化，可穿锁门 | `maps/CinnabarGym/script.scene` | `engine/events/hidden_events/cinnabar_gym_quiz.asm:143–200` 未解锁写 0x54 | @load 按六门旗标初始化门块；答题后替换 | 真实地图测试：无旗标不可穿、答题后开门 | ✅ 已修复：@load 按六门旗标写六门（横 $54=84 / 纵 $5f=95，坐标 (9,3)/(6,3)/(6,6)/(3,8)/(2,6)/(2,3) 取自上游 CinnabarGymGateCoords）；测试 `cinnabar_gym_gates_initialized_per_quiz_flags`（.blk 全开默认下先红）；答题开门由 `cinnabar_quiz_machine_correct_answer_opens_gate_only` 实测 |
| A04 | 红莲问答替换原版机制与题目 | `maps/CinnabarGym/script.scene` | 六个机器隐藏交互；`data/text/text_2.asm:340–374` 原题 | 恢复机器交互＋独立门旗标，训练家可挑战；恢复原题文案 | 场景 AST 测试；`verify_scene_translations.py`；实机截图 | ✅ 已修复：问答从七名训练家身上移回六台机器（原版坐标 (15,7)/(10,1)/(9,7)/(9,13)/(1,13)/(1,7)，朝上交互，OnInteract 绑定照雕像先例）；恢复原版六题与答案（N,N,N,N,Y,N，原文取自上游 text_2.asm）与开场/对错文案；答错→门关+对应训练家开战（`cinnabar_quiz_wrong_answer_sends_gate_trainer` 实测 OPP_BURGLAR4 入战且门不开）；训练家改回正式对话战斗（战胜才置 TRAINER 旗标、不再隐藏）；旗标/门块独立。机器与门的旗标映射（机器 n→GATE(n-1)）已在 scene 头注释说明 |
| A05 | 捕获闪电鸟后对象仍可见可交谈 | `maps/PowerPlant/script.scene` | `home/trainers.asm:185–212` EndTrainerBattle 对野生静态对象 HideObject | 完成分支隐藏对象并去掉鸣叫对白 | 省实场景测试：捕获后旗标 true 且对象隐藏 | ✅ 已修复：win/caught 分支补 `hideObjectByName("POWER_PLANT_OBJ_9")` 并删除已胜分支的再鸣叫；同修 config 缺失的 toggleId 绑定（npc_id_by_toggle 解析不到对象导致隐藏键写了但运行态不可见——与 A15 同根因）；测试 `zapdos_hidden_after_capture_and_stays_hidden_on_reentry`（真实交互→野生战→resume("caught")→隐藏，重入图仍隐藏） |
| A06 | 新战斗派出 0 HP 队首 | `pokered-core/src/battle/state.rs:512`、`battle/mod.rs:1260` | `engine/battle/core.asm:216` 找首个有 HP 队员 | active index 初始化改为首个存活队员 | core 单测：队首倒下时派出下一只 | ✅ 已修复：`battle_start_skips_fainted_lead`＋`from_parties_mirrors_first_alive_mon` 先红后绿；core 2478 全绿；m01–m06 里程碑链通过（`PLAYTHROUGH REACHED REQUESTED MILESTONE`）；exp 初始标记随出场者 |
| A07 | 坂木 B4F 门锁未初始化 | `maps/RocketHideoutB4F/script.scene` | `scripts/RocketHideoutB4F.asm:11–35` 未解锁写 0x2d | @load 按守卫旗标写关门块；解锁后恢复 | 隔离步行测试：未胜守卫不可穿 | ✅ 已修复：@load 补 `@else` 写关门块 $2d=45（.blk 默认开放，stash 验红）；两守卫全胜经 writeback 重跑 onLoad 立即解锁；测试 `rocket_hideout_b4f_door_closed_until_both_guards_beaten` |
| A08 | 正辉电脑事件绑在空地 (5,4) | `maps/BillsHouse/map.json` + `script_config.json` | `hidden_events.asm:488`、`bills_house_pc.asm` 由 (1,4) 按旗标三态分派 | 三态分派（分离/普通提示/收藏）绑到实际电脑 (1,4) | 真实地图测试：未帮助→提示；帮助后→分离 | ✅ 已修复：删除 (5,4) 假 sign（total_sign_count 守卫 340→339 并注明依据），分离/正辉 PC/监视器三态合并绑到 (1,4) 的 sign 2；测试 `bills_pc_and_cutscene_run_at_real_pc_tile` 第一段（fresh 仅监视器文本）＋既有 `pc_trigger` 契约保持（MET_BILL→BillsPc） |
| A09 | 正辉缺少进出机器演出 | `maps/BillsHouse/script.scene` | `scripts/BillsHouse.asm:19`（入机隐藏）、`:62`（出机路线） | 补怪物入机隐藏与人形出机移动路径 | 场景测试＋实机截图 | ✅ 已修复：同意后怪物 moveNpc 上行三格入机 (6,2) 后隐藏（WalkToMachine/EntersMachine）；分离后人形 setNpcPosition 到机内 (6,2) 再 moveNpc 走出到 (4,4)（BillExitsMachine）；测试 `bills_pc_and_cutscene_run_at_real_pc_tile` 全流程（stash 三份改动验红：旧版怪物不隐藏、(1,4) 无分离） |
| A10 | 长背包列表不滚动，光标移出屏幕 | `pokered-ui/src/menus/bag.rs:38` | 原版背包列表按窗口滚动 | 可见窗口＋scroll offset；光标按窗口内行计算 | 前后截图（HM03/HM04 长列表）；UI 测试 | ✅ 已修复：Auto 高度封顶布局 th（底边框不出屏）＋条目窗口化滚动（offset 保证光标可见，CANCEL 按窗口渲染）；测试 `long_bag_list_scrolls_and_stays_in_the_box`＋`cancel_row_visible_when_cursor_on_it` 绿；截图对 `docs/screenshots/a10-bag-list-before/after.png`（capture-bag-list.py 注入 25 种物品驱动，stash 隔离对比——修复前底边框出屏、末行截断） |
| A11 | 狩猎地带 START 球数绘制在框外 | `pokered-ui/src/menus/start.rs:12` | `player_state.asm:225` 内框 7×3，行 (1,1)/(1,3) | 修外框尺寸与第二行 y 坐标 | 前后截图（SafariWest/Centre） | ✅ 已修复：信息框改内部 7×3＋边框（总 9×5），两行标签落回屏格 (1,1)/(1,3)；几何单测 `safari_zone_start_info_box_matches_original_layout` 先红后绿；前后截图 `docs/screenshots/a11-safari-start-before/after.png`（新 `--screen safari-start` 截图目标） |
| A12 | 塔顶火箭队战败后不逃离不隐藏 | `maps/PokemonTower7F/script.scene` | `scripts/PokemonTower7F.asm:31` RocketLeaveMovement→HideObject | 胜利分支补离场移动＋隐藏 | 真实场景测试：旗标 true 且三名队员隐藏 | ✅ 已修复：@load 新增离场编排（writeback 战后 rerun 触发，LEFT 闩旗标防重放；1/2 号 moveNpcTo 走到下楼瓦片 (9,16) 后隐藏，3 号所在壁龛周边瓦片实为阻挡、仅能一步离位后隐藏——限制已记录于 scene 注释）；config 补 POKEMONTOWER_7_OBJ_1..3 toggleId 与 onLoad；测试 `tower_rockets_leave_and_hide_after_victory`（三旗标→离场→隐藏→重入仍隐藏） |
| A13 | CUT 后树仍显示（仅碰撞更新） | `pokered-core/src/overworld/field_moves.rs:132` vs `pokered-app/src/render/overworld.rs:340` | 原版砍树后块替换立即可见 | 渲染读取运行时 map_data.blocks 改动 | 前后截图 SHA1 必须不同；渲染测试 | ✅ 已修复：`draw_overworld` 优先读 `screen.map_data` 的运行时块（地图匹配时），静态 .blk 仅作回退——CUT/门块/所有脚本换块立即可见；视觉证据 `docs/screenshots/a13-cinnabar-gates-before/after.png`（红莲门块经 A03 写入运行时后，修复前渲染为空洞、修复后门杆显示；同一代码路径即 CUT 树的渲染读取）；捕获脚本 `capture-cinnabar-gates.py` |
| A14 | 遗忘招式菜单长名称越界 | `pokered-ui/src/menus/party.rs:178` draw_move_choice | `learn_move.asm:123` 第 4 列起内部宽 14 格 | 按原版加宽独立招式框 | 前后截图（LEECH SEED／POISONPOWDER） | ✅ 已修复：招式框改第 4 列起、内部宽 14（总 16）；测试 `forget_menu_box_wide_enough_for_long_move_names` 绿；截图 before＝审计 `cut-forget-menu.png`、after＝`docs/screenshots/a14-forget-menu-after.png`（capture-forget-menu.py 实抓） |
| A15 | 已离场劲敌重进图后复现 | `pokered-core/src/overworld/update.rs:2704`、`sync_flags_from_engine` merge、`screen.rs:1990` | 隐藏/显示互斥键冲突（hidden 被 shown 回写覆盖） | 统一互斥语义：hidden 优先或写时互斥清除 | 回归测试：胜利离场→出图回图不可见；CONTINUE 后一致 | ✅ 已修复：`apply_hidden_object_flags` 第三段跳过已置 `__OBJ_HIDDEN_*` 的对象——stale SHOWN 键不再复活隐藏对象；测试 `hidden_object_stays_hidden_when_stale_shown_key_exists` 先红（审计复活现场）后绿 |
| A16 | 捕获卡比兽后仍播"返回山里" | `maps/Route12/script.scene` | `scripts/Route12.asm:50` `wBattleResult==2` 跳过对白 | 分离 win／caught 分支（B04 Route16 同类一并处理） | 场景测试：捕获分支无对白且解封正常 | ✅ 已修复：win／caught 分支拆分——捕获只置旗标+隐藏（无任何叙述），击败才播"回到山里"；Route16 同步（B04）；测试 `route12_snorlax_caught_skips_mountain_dialogue`（真实现实交互→笛→战斗→两分支 outcomes 断言对白有无、旗标与隐藏） |
| A17 | 奖金文案硬编码 Player＋多余分页 | `pokered-core/src/battle/mod.rs:2470` | `data/text/text_2.asm:867` `<PLAYER> got ¥…/ for winning!` | 用玩家名渲染；去掉 tip/Total 分页 | 战后文本测试；实机截图 | ✅ 已修复：`trainer_winnings_messages`＋`winnings_text_uses_player_name_and_total`；m06 实机捕获单页 `RED got $175 for winning!`（`capture-prize-text.py`）；前后截图 `docs/screenshots/a17-prize-before/after.png` |
| A18 | 冠军之路机关被对话直接解开（含 2F/3F 踏坐标误判） | `maps/VictoryRoad1F/2F/3F/script.scene` | `scripts/VictoryRoad1F.asm:28–41` CheckBoulderCoords 检查真实石头位置 | 一层交谈不解锁；2F/3F 改查石头实际压开关坐标 | 真实地图测试：未推石不解锁；推石到位解锁（怪力推石本身可用，勿动） | ✅ 已修复：`field_moves::victory_road_switch_for` 照 seafoam_hole 先例——推石落点命中开关坐标才置旗标+开门块（1F (17,13)→(4,6,29)；2F (1,16)→(3,4,21)、(9,16)→(11,7,29)；3F (3,5)→(3,5,29)，上游原表）；三层 scene 删除近似实现；测试 `victory_road_switch_requires_boulder_pushed_onto_it` 端到端绿 |
| A19 | 希巴房间未战可通过出口 | `maps/BrunosRoom/script.scene` | `scripts/BrunosRoom.asm:2,11–26` 每帧按旗标写开/关块 | @load 补未胜写关门块 36 | 隔离 CONTINUE 步行测试：未战不可过 | ✅ 已修复：@load 补 `@else` 关门分支（.blk 默认开放）；测试 `brunos_room_exit_door_blocked_until_beaten`（stash 场景改动后先行验证红：无 @else 时门保持 5）；writeback 战后重跑 onLoad 立即开门 |
| A20 | 菊子战胜后出口未更新 | `maps/AgathasRoom/script.scene` | `scripts/AgathasRoom.asm:2,11–26` 每帧更新出口 | 战斗交接后按旗标立即换块（每帧或战后回调） | 测试：胜利后 block(2,0) 立即为开放 | ✅ 已修复：新增 `rerun_map_on_load_script`（EndTrainerBattle 后重跑图 onLoad，等价原版每帧脚本；挂起等待战斗结果的脚本跳过）；测试 `agathas_room_exit_opens_after_victory_without_retalk`＋writeback 级 `trainer_win_reruns_map_on_load_door_blocks`（先红后绿） |
| A21 | 战斗外 Ether 固定恢复第一招 | `pokered-core/src/items/bag_use.rs:254` | `item_effects.asm:1968–1988` MoveSelectionMenu（Elixir/Max Elixir 跳过） | 补招式选择流程；Elixir 类仍跳过；B07 同路径一并覆盖 | 子系统测试：Ether 弹招式菜单、选中招 PP+10；scenarios.py 相关项 | ✅ 已修复：UseItem 模式下 Ether/MaxEther/PPUp 选中队员后进入 ChooseMove 选招阶段，确认后经 `bag_use::finish_pp_restore` 恢复所选槽位（Elixir 类照旧整体恢复）；测试 `ether_item_enters_move_choice_instead_of_applying`＋`elixir_item_still_applies_without_move_choice`＋`ether_restores_the_chosen_move_slot`＋`elixer_restores_all_moves_without_menu` 全绿 |

## B 组 · 待复现 / 关联风险（8 项）

| # | 问题 | 位置线索 | 复现/评估方式 | 处理 | 状态 |
|---|------|----------|----------------|------|------|
| B01 | 送饮料菜单显示内部名 FRESH_WATER | `pokered-app/src/render/elevator.rs:61`（tui 同名核对） | 彩虹百货屋顶送饮料，过滤背包截图（已有 `drink-filter-first.png` 一次） | 确认后英文模式转显示名；附前后截图 | ✅ 已修复：抽 `filter_label` helper——item 常量经 `ItemId::from_const_name`→`lang_data::item_name` 转显示名（双语言），楼层名（1F 等）原样回退；测试 `filter_label_maps_item_constants_to_display_names` 绿。注意：过滤背包一次实测即见（审计 drink-filter-first.png 为红证据），行为变更微小故未重复实机截图 |
| B02 | EVENT_IN_SAFARI_ZONE 时间用尽后未清除 | SafariZone 退出流程 | 时间用尽传送回入口→再进入是否问"提前离开" | 确认后退出时清除旗标 | ✅ 已修复：`end_safari_game` 清除 `EVENT_IN_SAFARI_ZONE`（时间到弹出与离区两个路径共用）；测试 `safari_end_clears_in_safari_zone_flag` 绿 |
| B03 | 塔 6F 训练家与玩家坐标重叠 | `tower-trainer-player-overlap` 证据 | 宝可梦塔 6F 复现 NPC 移动重叠 | 确认后按 #47 先例加重叠保护 | ✅ 已修复（加固+守护）：定向复现未能重现重叠（走位在交战距离-1 处停止，玩家迎向走位会被停位错开）；按原版 TrainerEngage→MoveSprite 的强制走位语义，在遭遇走位期间冻结玩家 d-pad（update.rs movement_input 门控），守护测试 `trainer_approach_freezes_player_no_overlap` 常绿（断言无 NPC 与玩家同格且交接照常） |
| B04 | Route16 卡比兽同类对白写法 | `maps/Route16/script.scene` | 捕获/击倒卡比兽实测 | 确认后按 A16 同修 | ✅ 已修复：随 A16 同一提交拆分 win／caught 分支（scene diff 同形）；机制由 `route12_snorlax_caught_skips_mountain_dialogue` 双分支覆盖（Route16 handler 结构与 Route12 一致） |
| B05 | 其他传说鸟／电球伪装战后隐藏 | Articuno/Moltres/PowerPlant Voltorb scenes | 各自捕获/战胜后对象可见性实测 | 确认后按 A05 机制补齐 | ✅ 已修复：同 A05 补 hideObjectByName——PowerPlant OBJ_1..7（Voltorb×5/Electrode×2）、SeafoamIslandsB4F OBJ_3（Articuno）、VictoryRoad2F OBJ_6（Moltres），并补三图 config toggleId；共享机制由 Zapdos 测试覆盖（含重入持久性） |
| B06 | B4F onLoad 解锁复查未覆盖 EndTrainerBattle 回调位 | `maps/RocketHideoutB4F/script.scene`（静态发现，报告 §坂木门锁） | 关门后战胜守卫→战后是否解锁 | 确认后复查覆盖原版 `home/trainers.asm:187` 行为 | ✅ 已修复：A20 的 writeback→`rerun_map_on_load_script` 机制即该复查（`script_awaiting_battle` 挂起时跳过）；测试 `rocket_hideout_door_unlocks_on_second_guard_win_without_reentry` 实证第二守卫战胜后同 settle 内解锁，无需重入图 |
| B07 | Max Ether／PP Up 与 Ether 同路径 | `pokered-core/src/items/bag_use.rs` | 随 A21 修复一并补用例 | 随 A21 | ✅ 已修复：随 A21——`finish_pp_restore` 对 Ether/MaxEther/PPUp 同一槽位选择流程（game.rs 分支三件套）；party_screen 分支测试覆盖三件套进入 ChooseMove |
| B08 | 旧存档劲敌初始选择迁移缺口 | `rival_starter` 恢复逻辑 | 领队非初始线的旧档无法恢复历史选择 | 评估可迁移性：能修则修，否则记录限制原因后闭合 | ⏭️ 记录跳过：a807572 的恢复逻辑已覆盖可迁移类（旧存档从当前领队的初始进化线恢复并写入固定选择）；领队已换成非初始线的旧档在存档格式中不含历史选择信息，任何取值均为臆测——信息论上不可恢复，维持默认并对新档一律显式写入。限制已由审计报告与计划共同记录，不强行凑通过 |

## C 组 · 审计续跑补丁已收编（a807572 等），核对即闭合

核对方式：确认所述回归测试存在，跑受影响 crate 测试通过。不重做实现。

| # | 项 | 核对点 | 状态 |
|---|----|--------|------|
| C01 | 一击必杀按当前速度判定 | ohko 速度/等级相反、同速、麻痹用例（`ohko-current-speed-tests.log`）  ✅ 核对通过：ohko 速度/等级相反、同速、麻痹回归（tests: pokered_rules ohko 系列，5 文件）；受影响 crate 测试全绿 |
| C02 | 地下通道渲染越界边界回退 | 南端视窗渲染测试（13 项）  ✅ 核对通过：`underground_exit_uses_its_own_route_after_crossing_tunnel` 等渲染/步行测试；受影响 crate 测试全绿 |
| C03 | 战斗用药按 ItemId 扣除 | 野生/训练家过滤扣除回归（`item-consumption-fixed-*`）  ✅ 核对通过：道具扣除回归（consume_selected_item / remove_item_at 用例，item_fidelity_tests）；受影响 crate 测试全绿 |
| C04 | 宝可梦屋雕像交互＋CONTINUE 绑定 | 雕开关真实地图测试（含 run_on_load 与 warp 两条路径）  ✅ 核对通过：`mansion_statues_offer_and_apply_switch_from_adjacent_floor`（含 CONTINUE 与 warp 两路径）；受影响 crate 测试全绿 |
| C05 | 金黄市解放（坂木胜后守卫隐藏） | SaffronCity @load 恢复对象测试  ✅ 核对通过：`saffron_liberation_clears_gym_guard_and_restores_citizens`；受影响 crate 测试全绿 |
| C06 | 董事长室 CardKey 门锁 | 有/无钥匙、门左右两侧测试  ✅ 核对通过：`silph_boardroom_door_opens_from_corridor_only_with_card_key`；受影响 crate 测试全绿 |
| C07 | 西尔佛坂木 OPP_GIOVANNI2 | giovanni party 测试  ✅ 核对通过：`giovanni_has_three_encounters` / `giovanni_gym_team`（trainer 数据校验）；受影响 crate 测试全绿 |
| C08 | 西尔佛 3F 门禁未解锁写 0x5f | 无钥匙/有钥匙/解锁重入测试  ✅ 核对通过：`silph_third_floor_door_requires_key_and_stays_open_after_reentry`；受影响 crate 测试全绿 |
| C09 | 西尔佛劲敌 startBattleSet 6 | 场景胜/负返回测试  ✅ 核对通过：场景胜/负测试（native_script SILPH_CO_RIVAL result==win 断言）+ `rival_battle_then_pokedex_scenario`；受影响 crate 测试全绿 |
| C10 | 塔楼梯 warp tile 0x13 | tileset is_warp_tile 测试  ✅ 核对通过：tileset warp 表测试（warp_tile 相关 6 文件，含 0x13）；受影响 crate 测试全绿 |
| C11 | 交谈训练家开战（含希巴漏修路径） | 守卫对白后排战测试；希巴/菊子真实地图交接测试  ✅ 核对通过：`rocket_hideout_talk_only_guard_battles_after_scene_dialogue`＋希巴/菊子交接测试；受影响 crate 测试全绿 |
| C12 | 地下通道四入口 last_map | 大地图测试（721 项）  ✅ 核对通过：`underground_exit_uses_its_own_route_after_crossing_tunnel`＋`scripted_underground_exit_overrides_stale_saved_last_map`；受影响 crate 测试全绿 |
| C13 | 离船 borderBlock 采样 | ship-exit 步行测试（724 项）  ✅ 核对通过：`ss_anne_departure_plays_once_and_erases_the_ship`＋离船步行测试；受影响 crate 测试全绿 |
| C14 | 船票 S_S_TICKET 名称统一 | 有票放行/无票推回分支测试  ✅ 核对通过：船票名称统一分支测试（S_S_TICKET 用例）；受影响 crate 测试全绿 |
| C15 | startBattleSet 接口＋result 同作用域 | 华蓝/塔 2F/Route22/冠军场景 AST 测试  ✅ 核对通过：华蓝/塔/Route22/冠军场景 result 作用域 AST 测试（native_script 1475-1664）；受影响 crate 测试全绿 |
| C16 | RIVAL2/RIVAL3 类名解析 | make/parse 往返测试  ✅ 核对通过：RIVAL2/RIVAL3 make/parse 往返测试（trainer_data）；受影响 crate 测试全绿 |
| C17 | 劲敌初始选择按存档 starter | 劲敌队伍选择测试（旧档限制归 B08）  ✅ 核对通过：`rival_gets_type_advantage_starter`＋rival_starter 恢复测试（4 文件）；受影响 crate 测试全绿 |
| C18 | 寄生种子不再复活倒下者 | `residuals_stop_after_either_battler_faints`＋StackDriver 回合测试  ✅ 核对通过：`residuals_stop_after_either_battler_faints`＋StackDriver 回合测试；受影响 crate 测试全绿 |
| C19 | 联盟入口四门 last_map | 两列×南北×两种来源测试  ✅ 核对通过：联盟入口四门 last_map 测试（Route22Gate）；受影响 crate 测试全绿 |
| C20 | 冠军之路三层洞口下落 | (23,15)→VR2F (22,16) 步行测试  ✅ 核对通过：冠军之路洞口下落测试（dungeon_warp 6 文件）；受影响 crate 测试全绿 |
| C21 | Route22 阶段清理（小刚胜后） | PewterGym 清理＋Route22 @load 修正测试  ✅ 核对通过：Route22 阶段清理测试（PewterGym 清理＋@load 修正，Route22 3 文件）；受影响 crate 测试全绿 |

## 进度日志

- 2026-09-07：计划建立。基线：分支 `audit/2026-09-07-playthrough-fixes`（a807572，工作区干净）。
- 2026-09-07：A06 闭合（`state.rs::new_battler_state` 与 `mod.rs::from_parties` 选首个存活队员，exp 初始标记跟随出场者；红→绿→core 全绿→m06 链通过）。A17 闭合（`settlement/money.rs::trainer_winnings_messages` 按原版单页文案，删除 tip/Total 分页；m06 实机截图验证）。已知无关噪音：cargo 扫描上游 dotzuki checkout 的 `dotzuki-template` 报包名错误，退出码 0，不影响构建与测试，按边界不动引擎仓库。
- 2026-09-07：A01 闭合（名人堂信息框内高 8→9，app/tui 两处；前后截图 a01-hof-before/after）。A02 闭合（credits 渲染展开 $54 `#`→POKé，裁剪改按字符计，app/tui 两处；前后截图 a02-credits-before/after）。为此给 `screenshot` CLI 新增 `hof`/`credits` 两个 movie-takeover 截图目标（tools.rs 种子分支，先例照 Pc/Naming）。app 45、tui 16 项测试通过。
- 2026-09-07：A19/A20 闭合。共用机制：writeback 战后（EndTrainerBattle）重跑当前图 `rerun_map_on_load_script`——等价原版每帧地图脚本重写门块；`@load` 按 warp 语义本就幂等，`script_awaiting_battle` 时跳过以防打断挂起剧情。A19 scene 补 `@else` 关门（stash 验红）；A20 靠 rerun。测试 3 项先红后绿；core 2482 全绿；m01–m06 链复验通过（capture-prize-text.py，rival 战胜利结算路径无回归）。备注：`verify_scene_translations.py` 指向旧 `examples/pokered` 布局（本仓库 0 场景恒过）；本次 scene 改动未触对白文本，手动 diff 确认英/中文本无变化。
- 2026-09-07：A07/B06 闭合（B4F `@else` 关门 $2d=45 + rerun 战后解锁；两项测试先红后绿，core 2484 全绿）。为 A03/A04 取得上游原始数据：pret/pokered `cinnabar_gym_quiz.asm`——六门坐标 (9,3)/(6,3)/(6,6)/(3,8)/(2,6)/(2,3)（横 $54、纵 $5f）、`EVENT_CINNABAR_GYM_GATE0..5_UNLOCKED` 独立旗标、答错映射 trainer(index+2)（已胜则不战）；A03+A04 将合并为红莲问答整体重做。
- 2026-09-07：A03/A04 闭合（红莲问答整体重做）。scene 重写：六台问答机（原版坐标/朝上交互/@choice，OnInteract 硬编码表照雕像先例加进 setup_triggers_for_map）＋七名训练家回归正式对话战斗（战胜置旗标、不再隐藏）＋@load 六门按旗标初始化。原版题目/答案/文案取自上游 text_2.asm；机器 n→GATE(n-1)、答错→训练家 npc(n+2) 的映射与上游 bit 解码差异已记录于 scene 头注释。测试 3 项新增全绿（gates 初始化、答对只开门不置训练家旗标、答错开战门不开）；core 2487 全绿；app 构建通过。
- 2026-09-07：A05/B05 闭合。三图 scene 在 win/caught 分支补 hideObjectByName 并删除已胜再鸣叫；根因排查发现 config 缺 toggleId 绑定导致 npc_id_by_toggle 解析失败（隐藏键写入但运行态 visible 不变——A15 报告的同款互斥键问题面），补齐 PowerPlant OBJ_1..9、Seafoam OBJ_3、VictoryRoad2F OBJ_6。测试 `zapdos_hidden_after_capture_and_stays_hidden_on_reentry` 全绿（交互→野生战→resume("caught")→隐藏→重入仍隐藏）；core 2488 全绿。
- 2026-09-07：A12 闭合。塔 7F @load 新增离场编排（writeback rerun 触发一次：moveNpcTo 下楼 (9,16)+hideObject，LEFT 闩旗标防重放；原版路径表 RocketLeaveMovement 取自上游）。3 号壁龛周边瓦片实为阻挡（实测 moveNpcTo 多目标均卡 (9,8)），仅一步离位后隐藏，记录限制。测试 `tower_rockets_leave_and_hide_after_victory` 全绿；core 2489 全绿；git diff --check 通过。
- 2026-09-07：A16/B04 闭合。Route12/Route16 卡比兽 handler 拆分 win／caught：捕获（wBattleResult==2）只置旗标+隐藏、不播"回到山里"；击败才叙述。测试 `route12_snorlax_caught_skips_mountain_dialogue` 双分支断言（对白有无、旗标、隐藏）全绿；core 2490 全绿；diff-check 通过。
- 2026-09-07：A15/A13 闭合。A15：`apply_hidden_object_flags` 第三段跳过已置 `__OBJ_HIDDEN_*` 的对象——stale SHOWN 键（sync 合并回灌）不再复活隐藏对象；测试 `hidden_object_stays_hidden_when_stale_shown_key_exists` 先红（审计复活现场）后绿。A13：`draw_overworld` 改读运行时块（CUT/门块等脚本换块立即可见），红莲门口前后截图为证（capture-cinnabar-gates.py，stash 隔离渲染改动）；app 45 项测试通过。备注：无 feature 的 `cargo build` 会覆盖带 debug-server 的二进制，截图/驱动前需 `--features debug-server` 重建。
- 2026-09-07：A08/A09 闭合。BillsHouse：删除 (5,4) 假 sign（total_sign_count 340→339 注明审计依据），分离/正辉 PC/监视器三态合并绑到实际电脑 (1,4)；同意后怪物上行入机隐藏、分离后人形从机内走出到 (4,4)（原版 WalkToMachine/EntersMachine/BillExitsMachine 路径，moveNpc 实现）；既有 pc_trigger 契约（MET_BILL→BillsPc）保持并通过。测试 `bills_pc_and_cutscene_run_at_real_pc_tile`（stash 三份改动验红）全绿；core 2493 全绿。
- 2026-09-07：C 组 21 项全部核对通过（逐项定位回归测试名，core 全套 2500 绿）；A10 截图对补齐（capture-bag-list.py：25 种物品 + START/ITEM 驱动 + stash 对比）。清单状态规范化（9 行状态格残留 ⬜ 已清除，0 开放项）。
- 2026-09-08：整体验收通过。① cargo test -p pokered-core：2500 通过 0 失败（新增 26 项回归）；pokered-app 46、pokered-ui 全套、pokered-tui 16、pokered-data 254、pokered-debug-server 4 全绿。② verify_scene_translations.py 通过（注意：脚本指向旧 examples 布局，本仓库 0 场景；本轮 scene 改动均未触碰对白文本，已逐一 diff 核对）。③ 完整 python3 scripts/playthrough.py（m01–m10）输出 PLAYTHROUGH REACHED REQUESTED MILESTONE（230s；m09 森林 PINCH 为驱动层已知抖动，自动恢复）。④ git diff --check 通过。⑤ 截图归档：a01-hof / a02-credits / a10-bag-list / a11-safari-start / a14-forget-menu / a17-prize 共 6 组前后对比于 docs/screenshots/。B08 为唯一 ⏭️ 记录跳过项（依据停止规则，理由与限制已记录）。汇总：A 组 21/21、B 组 8/8（1 项记录跳过）、C 组 21/21 核对通过。
- 2026-09-07：B03/B08 闭合。B03：定向复现不可重现（distance-1 停走），按原版语义加固——遭遇走位期间冻结玩家 d-pad，守护测试常绿；B08：可迁移类已由 a807572 恢复逻辑覆盖，非初始线领队的旧档信息论上不可恢复，记录限制后闭合（⏭️，不强行凑通过）。
- 2026-09-07：B01/B02/B07 闭合。B01：过滤背包行经 `filter_label` 转物品显示名（双语言，楼层回退）；B02：`end_safari_game` 清 `EVENT_IN_SAFARI_ZONE`（超时弹出/离区共用）；B07 随 A21（三件套同流程）。测试 3 项新增全绿；core 2499 全绿。
- 2026-09-07：A21 闭合（A 组全部完成）。UseItem 流程：Ether/MaxEther/PPUp 选中队员→复用 ChooseMove 选招→`bag_use::finish_pp_restore` 恢复所选槽位（Elixir 类跳过菜单照旧）；game.rs MoveForgetChosen 分支 Ether 族。测试 4 项全绿（菜单进入/跳过、槽位恢复、整体恢复）；core 2498 全绿。
- 2026-09-07：A14 闭合。遗忘菜单独立框：列 4、内部宽 14（上游 learn_move.asm:123）；单测断言矩形与文本不越界全绿；before/after 截图归档（capture-forget-menu.py 驱动 give_pokemon(4招 Venusaur)+HM01 菜单链）。
- 2026-09-07：A10 闭合。bag.rs：Auto 高度封顶布局 th＋条目窗口化滚动（offset 保证光标可见，CANCEL 行按窗口渲染）；测试 2 项绿（20 项列表五档光标位置全部框内；CANCEL 可达）。审计 hm03/hm04 截图为红证据；前后截图对下一轮经 give_item+START/ITEM 菜单驱动补齐。
- 2026-09-07：A11 闭合。信息框内 7×3＋边框（总 9×5），标签 (1,1)/(1,3)；几何单测先红后绿；`--screen safari-start` 新截图目标（debug warp 不经过门流程、无法激活游猎运行态，故种子注入 safari_info 走真实 draw）；前后截图 a11-safari-start-before/after.png；pokered-ui 全绿。
- 2026-09-07：A18 闭合。`field_moves::victory_road_switch_for` 挂进推石 Pushed 分支（照 seafoam_hole 先例）：落点命中三层开关坐标才置 ON_SWITCH 旗标并直接开门块；1F talk 解锁、2F/3F 踏坐标解锁三处近似全部删除（scene+config）。测试 `victory_road_switch_requires_boulder_pushed_onto_it` 端到端绿（摆石→按住怪力推→落点压开关→旗标+块 29；未推石旗标 unset）；core 2492 全绿；diff-check 通过。
