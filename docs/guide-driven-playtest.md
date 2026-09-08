# 攻略驱动的支线与收集测试

这一轮先检索初代 Red/Blue 攻略，再从攻略选取目标并实际操作游戏。范围仍为 m10 内。
目标和奖励不从重制版当前输出反推；pret 只补充地图坐标和一次性奖励的精确语义。

## 收集的资料

- [Bulbapedia Part 2](https://bulbapedia.bulbagarden.net/wiki/Walkthrough:Pok%C3%A9mon_Red_and_Blue/Part_2)：返乡领取城镇地图、常青市隐藏药水、可选 Route22 对手战等。
- [Bulbapedia Part 3](https://bulbapedia.bulbagarden.net/wiki/Walkthrough:Pok%C3%A9mon_Red_and_Blue/Part_3)：常青森林物品清单与路线、博物馆参观；明确古老琥珀需之后取得 Cut。
- [GameFAQs Viridian City](https://gamefaqs.gamespot.com/gameboy/367023-pokemon-red-version/faqs/64175/viridian-city)：交叉核对常青市隐藏药水的绕行路线与返乡地图奖励。

采集日期为 2026-09-08。保存两篇 wiki 的 revision 链接、简短事实摘要、版本限定和待测清单，
没有镜像整篇攻略。检索到的 FireRed/LeafGreen 攻略排除；Psypokes 页面两次返回 502，
未用作测试依据。来源及每个场景的映射见 [guided_playtest_sources.json](../scripts/guided_playtest_sources.json)。

## 首批可执行目标

| 场景 | 执行与断言 | 状态准备 |
|---|---|---|
| Daisy 城镇地图 | 送包裹前拜访无奖励；实际完成包裹后回访获地图；桌上地图消失；重复及存档重启不复制 | 从开机跑 m01–m08，无 warp、无宝可梦或剧情标志注入 |
| 常青市隐藏药水 | 从南入口绕路走到树旁，检查获得药水；走回入口保存重启，再走回来不重复获得 | 只在开头 warp 到城市入口，提供 Lv5 Bulbasaur；无 Cut/Surf |
| 森林五件物品 | 从南入口步行收集精灵球×1、解毒药×2、药水×2，含两件隐藏物；从北门离开并保存重启 | 只在开头 warp 到森林入口，提供 Lv20 Bulbasaur 控制战斗成本；不注入物品或已拾取标志 |

重复领取和存档持久化是额外回归性质，并非声称攻略逐条描述这些边界。
两个入口构造场景只证明从入口往后的可达性。导航复用既有 BFS 和战斗处理，
不是逐字照抄攻略方向，也不是纯视觉导航；目标交互使用实际按键。主线公共驱动可能使用
已有对话同步命令，调试 save 验证存储恢复但不测试 SAVE 菜单。

## 运行和证据

```bash
cargo build --bin pokered-app --features debug-server
python3 scripts/guided_playtest.py --list
python3 scripts/guided_playtest.py --output /tmp/guided-new-run
python3 scripts/guided_playtest.py --only forest-five-item-tour --repeat 3
```

仅 Python 标准库，复用 `content_regression.Session` 和 `playthrough.Game`。
输出目录必须为空或不存在。每个场景保留全部协议请求响应、关键状态、游戏日志与存档；
报告每完成一例即落盘，含来源快照、脚本/来源/二进制哈希、每例耗时和调试命令计数。
失败会继续其他场景，最终退出 1；导航失败先查目标解析和驱动，不能直接算产品 bug。

第一轮森林测试把精灵球的交互位放在不可走的下侧格子，导航明确拒绝。
已依据可通行邻格改从右侧交互；可见药水也选用上侧邻格。这是攻略地标到坐标的解析成本，
不计为游戏缺陷，不通过直接 warp 到道具来规避。
第二次探测还发现既有 `face()` 在可走格子前可能前进一步，导致面对错误格子检查隐藏药水。
现通过从目标交互位后方步行到达、保留到达朝向解决，并在每次拾取前断言位置和朝向。
修正后完整走通五件物品并从北门离开；这些调整局限于新测试，未改公共主线驱动或游戏。

## 最终实测

2026-09-08，游戏 `b647e4de7fb4970db04c983d3bed25a0f8ae0bb9`，三场景各独立复跑三次，
**9/9 通过，合计 195.980 秒，平均约 65.3 秒/套**。Daisy 35.3–38.1 秒，
常青市 9.8–9.9 秒，森林 15.6–22.7 秒。森林战斗次数随运行变化，不断言固定耗时。
本批没有确认新的产品缺陷；上述两次早期失败均为驱动目标/朝向问题，保留说明以评估编写成本。

[机器报告](audits/guided-playtest/report.json)、[当次来源快照](audits/guided-playtest/sources.json)、
[关键状态摘要](audits/guided-playtest/observations-summary.json) 已归档。
完整协议及存档在本机 `/tmp/guided-playtest-final/`；临时目录不作为长期归档保证。
当前来源清单另补充了坐标源哈希与解析目标，运行快照忠实保留当次清单，不回写旧证据。
协议计数已核实：Daisy 三次均无 warp/give_pokemon/give_item/set_flag；
两个收集场景每次仅一次入口 warp、一次队伍准备，均无物品或剧情标志注入。

## 已收集但未执行

Route22 对手战及满足条件后回 Oak 领取精灵球、博物馆门票与参观、森林区域宝可梦捕获。
捕获任务需独立的随机探索预算；预算耗尽不能证明某宝可梦不存在。
古老琥珀和 TM42 的后期前置条件超出本轮范围。
目前是攻略指导 AI 编写并调试的固定任务集，不是运行时自主读取网页并无限探索的代理。
