# 自动化通关尝试记录

版本：`a952ebf`。通过本机 debug-server 的按键、步帧、状态查询及存档协议执行；没有修改游戏实现。

## 实际进度与辅助边界

1. 新游戏 → 大木开场 → 领取妙蛙种子 → 劲敌战 → 商店包裹 → 图鉴 → 常青森林 → 小刚：完整新开局按键链通过。练级阶段打了 45 场野生战斗到 13 级，击败小刚后 15 级。见 `fresh-chain.log`。
2. 读档后发现室内出口错误，从真新镇步行返回深灰市，继续经过 3 号道路、月见山入口治疗、洞穴、化石守卫，取得贝壳化石并从东侧出口离开。期间正常发生战败、黑屏恢复和重走；没有修改等级、队伍、徽章或剧情旗标。
3. 为推进新路线，探索驱动补充了地砖对碰撞和单向跳台阶约束；修正了一次月见山 B1F 楼梯路线选择。原始驱动的失败不当作游戏地图错误。
4. 后续重试受存档问题及洞内战败影响。使用真实的化石检查点，通过 **一次 debug warp** 恢复到已经实际抵达过的 Route4 东侧出口 `(24,6)`。从那里正常步行至华蓝市、治疗、击败两名道馆训练家及小霞、取得 TM11、出馆治疗并存档。见 `assisted-resume.log`、`after-misty-state.json`。
5. 小霞战后妙蛙草 22 级，两枚徽章。第二枚徽章属于上述辅助续跑，不能称作无辅助全程结果；本轮未继续挑战第三道馆至四天王。

检查点和以下发现均来自本轮执行，不是仅引用旧审计文档。

## 新发现 A：室内存档读回后，出口传送到错误城镇（高）

**复现：** 小刚战后在 PewterGym `(4,2)` 保存 → 重新启动并 CONTINUE → 向南走出道馆。预期 PewterCity，实际 PalletTown 大木研究所入口附近 `(12,11)`；继续走完退出动作后为 `(12,12)`。两次独立重启重现，隔离目录的最终复现脚本亦重现。

**原因：** `crates/pokered-app/src/game.rs:1303` 的 `build_save_data` 没有将 `overworld.last_map` 写入已有的 `game_data.last_map`；`:1629` 附近 CONTINUE 重建 OverworldScreen 时也没有恢复它。`crates/pokered-core/src/overworld/screen.rs:962` 默认上一地图为 PalletTown。室内无显式目标的出口按错误的上一地图解析。

SRAM 序列化/反序列化已有 last_map 字段（`ser_game_data.rs:113`、`sram_deser_game_data.rs:40`），缺的是运行时同步。影响范围可能包含其他使用动态出口的建筑/洞穴；本轮直接验证的是深灰道馆。

**证据：** `after-brock.sav`、`pewter-exit-failure.json`、`pewter-exit-protocol.jsonl`、`save-repro.log`。

## 新发现 B：不同存档/检查点串用剧情显隐状态（高）

**复现：** 读取大木带入研究所、尚未选御三家的 `before-starter.sav`。队伍为 0，但若可执行文件目录保留后期游玩的附属状态，小火龙球（text_id=2）和妙蛙种子球（text_id=4）都不可见，只剩杰尼龟球。将同一存档交给没有附属文件的相同二进制，三球全部可见。

最终隔离对照复现结果：

| 同一个早期 SRAM | 小火龙球 | 杰尼龟球 | 妙蛙种子球 |
|---|---|---|---|
| 无附属文件 | 可见 | 可见 | 可见 |
| 后期存档附属文件 | 隐藏 | 可见 | 隐藏 |

**原因：** `crates/pokered-app/src/game.rs:153` 将 `pokered.script_flags.json` 固定放在可执行文件旁；`:1200` 和 `:1235` 的读写不随 `--save` 路径变化，`:1642` 读档时又将其合并到刚读入的 SRAM。`__OBJ_HIDDEN_*` 之类状态因此跨存档覆盖。

本轮月见山重试也观察到，取得化石后的隐藏键仍保留在较早检查点的加载环境中。探索后半段改用隔离二进制目录，并恢复检查点当时已经查询记录的显隐键；没有新建游戏进度旗标。

**证据：** `before-starter-polluted.json`、`before-starter-isolated.json`、`later-script-flags.json`、`save-repro.log`。

## 新发现 C：满背包击败小霞，TM11 被永久标记已领但没有入包（高）

**测试边界：** 从本轮真实的华蓝市检查点单独开一条诊断分支，用 `give_item` 添加 18 种物品，将原有 TM34、贝壳化石在内的背包填至 20 格，然后正常走入道馆并战胜小霞。这是明确的背包状态构造，不属于普通通关那条存档。

**实际：** `EVENT_BEAT_MISTY=true`、`EVENT_GOT_TM11=true`，背包仍为原来 20 格，完全没有 Tm11。普通未满背包的同一挑战则正常获得 Tm11。领取旗标已置位后，再交谈会走说明文本分支，无法通过正常重领取得这份奖励。

**原因：** `crates/pokered-data/maps/CeruleanGym/script.scene:41` 附近，`giveItem("TM11", 1)` 返回值未检查，紧接着无条件设置 `EVENT_GOT_TM11`；已胜利但未领 TM 的补领分支也有同样问题。原版 `scripts/CeruleanGym.asm:52` 在 GiveItem 后先用 `jr nc, .BagFull` 检查失败，仅成功后才设置旗标，允许腾出空间再领。

**证据：** 两次独立诊断挑战均复现，见 `misty-fullbag-result.json`、`misty-fullbag-retry-result.json` 及同名日志；普通对照为 `after-misty-state.json`。未将其他馆主的类似代码直接列为已运行验证的问题。

## 已知问题的实战复现：藤鞭被升级自动覆盖

原有静态发现得到本轮真实战斗验证：妙蛙草 21 级时招式为 Tackle / Growl / LeechSeed / VineWhip；小霞战后 22 级变为 Tackle / Growl / LeechSeed / Poisonpowder，没有遗忘选择。见 `level-up-move-overwrite.json`。这会直接破坏后续游玩的草系攻击手段。

## 自动化工具缺口（与游戏缺陷区分）

- **遗漏地砖对碰撞：** `scripts/playthrough.py:113` 的 BFS 只判断目的地单块可通行。月见山在 `(10,22)` 反复试图向左跨越 `$20→$05`；这恰是原版 `data/tilesets/pair_collision_tile_ids.asm` 明确禁止的 CAVERN 高低差。原驱动与逐格但仍缺少此规则的驱动均失败，加入生产数据中的地砖对约束后通过。
- **遗漏单向跳台阶的路径边：** Route4 东侧出口至华蓝市的路径需要向下跳台阶；普通 BFS 报无路。加入与生产数据一致的 LEDGE_TILES 后，实际按键成功走到华蓝市。两个约束只补在本轮探索助手，未修改现有 `scripts/playthrough.py`。
- **训练家接战延迟：** 到达小霞前方时，普通训练家的接战仍可能待处理；第一次“未设置 EVENT_BEAT_MISTY”实际只打完普通训练家。等待接战完成，再与小霞对话后正常获胜，未将这一驱动判断失误列为游戏缺陷。
- 正常战败、野生遭遇随机性、选择抗性招式导致的 PP 浪费，均未作为游戏 bug。

## 重现与继续

构建后执行 `python3 docs/audits/2026-09-06/playthrough/repro.py`，可独立重查 A/B。脚本复制二进制到临时目录，隔离附属文件，输出预期与实际结果，不覆盖提供的存档。

`after-misty-assisted.sav` 是普通挑战小霞后、已治疗、位于华蓝市的检查点。相应显隐附属状态为 `after-misty-script-flags.json`；继续时应使用独立二进制目录，将此文件复制为该目录的 `pokered.script_flags.json`，并复制 SRAM 后再用 `--save` 加载，避免覆盖证据和污染其他存档。

本轮实际使用的局部寻路助手保留为 `navigation.py`。原始探索脚本和更完整的协议轨迹保留在 `/tmp/pokered-playthrough-audit/`。录像用于定位，不能算与原版 ROM 的同帧视觉对拍。本轮没有执行完整联盟通关，也没有验证所有其他地图/前端。
