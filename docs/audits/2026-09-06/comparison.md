# 原版对照审计（2026-09-06）

当前版本：open-pokered-3 `a952ebf`；参考：`/Users/liuyanghe02/develop/pokered-worktree` 的 pret/pokered 汇编 `fbcf7d0e1`。参考目录中的未跟踪 `pokered-rust/` 不作为原作标准。

本轮确认 7 类残留差异。采用实际生产调用路径对照汇编，并用现有测试、调试端口补充验证；不是全剧情通关或逐帧 ROM 对拍，不代表只有这 7 类问题。未修改游戏实现。

## 1. 高：战斗升级自动覆盖第四招，可能覆盖 HM

- 当前：`crates/pokered-core/src/battle/experience/level_up.rs:70`，槽满直接写 `mon.moves[3] = move_id`、重置 PP，没有选择、取消、HM 守卫。`battle/mod.rs:4585` 的实际经验结算调用 `gain_experience`，随后仅显示 learned 消息。
- 原版：`engine/pokemon/learn_move.asm:98` 起选择遗忘招式，`:178` 拒绝删除 HM。
- 触发：携带四招的宝可梦通过战斗升级至学招等级；第四招会被无提示替换。第四招为 CUT 等 HM 时同样被覆盖。
- 证据级别：生产调用链静态确认。已有进化后/道具学招选择界面并不能保护这条战斗升级路径。

## 2. 高：新游戏初始金钱为 0，原版为 3000

- 当前：`crates/pokered-core/src/save/game_data.rs:374` 初始化 `player_money: 0`；完整经过标题、新游戏、大木开场后调试读取仍为 0。
- 原版：`engine/movie/oak_speech/init_player_data.asm:24` 的 `START_MONEY EQU $3000` 写入 BCD 金钱，十进制为 3000。
- 影响：早期商店购买能力、战败金钱惩罚及游戏经济与原版不符。
- 证据级别：运行复现；见 `runtime-evidence.json` 和 `scenarios.log` 的 m02。

## 3. 高：Haze 黑雾清除对象与效果范围错误

- 当前生产路径：`crates/pokered-core/src/battle/pokered_rules/mod.rs:2191`，清除双方主要异常状态；临时效果只移除 Confused、LeechSeed、Toxic、FocusEnergy，因此 Reflect、LightScreen、Mist、Disable、XAccuracy 等仍保留。
- 原版：`engine/battle/move_effects/haze.asm:16` 起仅治疗目标的主要异常状态；`CureVolatileStatuses` 清除反射壁、光墙、白雾、定身法等相应效果。替身等不应被一概删除。
- 影响：中毒/灼伤的使用者可错误自疗；对手的屏障或定身法不能按原版解除。
- 证据级别：生产代码与汇编确认。`pokered_rules/tests.rs:2844` 的 `haze_resets_stages_but_preserves_reflect` 反而断言错误行为（保留 Reflect），本轮测试通过。
- 注意：`p5_native.rs` 有另一个会清空所有效果的辅助实现，不能用它描述实际战斗行为。

## 4. 中：满血 Rest 睡觉仍成功

- 当前：`crates/pokered-core/src/battle/pokered_rules/rules.ron:392`，直接 RemoveStatus → Sleep(2) → HealFraction，无满血失败前置判断。
- 原版：`engine/battle/move_effects/heal.asm:16` 起先检查 HP，满血直接失败，再进入 REST 分支。
- 证据级别：实际 StackDriver 测试复现。`pokered_rules/tests.rs:675` 的 `rest_sleeps_self_via_decoupled_stack` 以满血宝可梦开场（`:247` 的构造器令 hp=max_hp），断言成功睡眠；本轮通过。
- 范围：本项只确认满血行为；未把原作恢复招式的 255/511 HP 差值 bug 一并视作已完成验证。

## 5. 中：双子岛巨石链式显隐不正确（也会画出不应出现的巨石）

- 当前：`crates/pokered-data/maps/SeafoamIslandsB1F/script.scene:14` 和 `SeafoamIslandsB2F/script.scene:9` 只处理本层落洞后隐藏，未根据上层落洞建立初始显隐。
- 原版：`data/maps/toggleable_objects.asm:402` 起，B1F、B2F 巨石初始 OFF；上层落洞时通过 ShowObject 显示下层对应巨石。
- 运行：全新游戏直接进入 B1F/B2F，两个巨石均 `visible=true`。B3F 的六个巨石也全部可见，而原版 BOULDER5/6 初始 OFF。
- 影响：玩家未从上层推下巨石，下层已经出现可操作巨石，改变谜题顺序；仅修改默认隐藏还不够，需要同时接好跨层显示。
- 证据级别：运行数据 + 汇编；见 `runtime-evidence.json`。未运行原版 ROM 抓同帧画面。

## 6. 中：双子岛 B3F 强制水流未移植

- 当前：`crates/pokered-data/maps/SeafoamIslandsB3F/script.scene:10` 明确未实现强制水流，`script_config.json` 只有两个落洞坐标事件。仅强制切成 Surfing 不等于执行水流移动。
- 原版：`scripts/SeafoamIslandsB3F.asm:67` 的 DefaultScript 和 `:104` 的 MoveObjectScript 按巨石事件检测并解码强制移动 RLE；覆盖 (15,8) 和掉落至 (18,7)/(19,7) 的情况。
- 运行：未置巨石事件，调试进入 B3F (18,7)，120 帧后仍停在 (18,7)，脚本未运行。
- 影响：没有被冲走的强制移动及相应解谜限制。
- 证据级别：生产脚本缺失 + 调试落点验证。这里使用 debug warp；未从 B2F 徒步落洞重演全链。

## 7. 低：FLY 飞翔进出场使用错误动画

- 当前：`crates/pokered-core/src/overworld/field_moves.rs:393` 的 `fly_warp_to` 直接淡出，设置 `arrival_spin: true`，并发出 GoOutside 音效事件。
- 原版：`engine/overworld/player_animations.asm:46` 的 `.flyAnimation` 加载 BirdSprite、播放 SFX_FLY 和飞鸟轨迹；离场也有 DoFlyAnimation 编排。
- 影响：传送功能可用，但飞翔缺少原版鸟携带玩家的进出场画面；到达被旋转动画替代。
- 证据级别：双方动画分支静态对照，未进行原版 ROM 逐帧比图。

## 验证与边界

- 从参考目录复制 gitignored `gfx/` 到当前工作区以完成构建，未改资源内容。
- `cargo build --offline --bin pokered-app --features debug-server`：成功。
- `cargo test --offline -p pokered-core --lib`：2448 passed，0 failed。
- `python3 scripts/scenarios.py`：10/10 passed（背包、队伍、逃跑、胜利经验、捕捉、黑屏重生、存档往返、菜单、选项、NPC）。初次沙箱端口绑定失败不是游戏失败；授权后重跑通过。
- `verify_move_sfx_data.py`：166 行，0 diffs。
- `verify_cry_data.py`：151 种，0 diffs。
- `verify_battle_anim_data.py`：177 坐标、122 帧块、86 子动画、203 动画记录，0 diffs；使用 Cargo 锁定的 dotzuki `88f1fcc` 源码。数据一致不证明动画运行时接线与时序完全一致。
- `screenshot-all` 生成 12 张画面用于冒烟检查；实际查看了战斗、选项、能力页和大木画面，并将战斗改为 400 帧重新抓取。默认 5 帧战斗黑场属于入场过渡，空队伍能力页也是测试初态，均未作为游戏画面故障报告。临时截图位于 `/tmp/pokered-audit-shots/`。
- 未启动原版 ROM，没有同场景同帧的双侧截图，不能声称完成像素级视觉保真审计。未进行音频听感对比、完整通关、跨设备联机或所有前端检查。
- 旧 `FIDELITY_GAPS.md` / 8 月报告中的“联机缺失、图鉴 AREA 缺失、进化后满招无法选择”等结论已有后续实现，未重复列为当前缺口。

建议先处理 1–3，再修 4–6，最后补 7。修复时将原版预期加入测试；尤其需纠正当前对 Haze 和满血 Rest 的错误断言，不能仅以现有测试全绿验收。
