# 捕获结算结果与卡比兽剧情回调

真实捕球成功原先进入 `BattleOver { won: true }`，并被统一结算为 `Win`。地图脚本因此收到 `win`，让 Route12/Route16 卡比兽捕获后仍播放“返回山里”的击败对白。此前直接传入 `caught` 的场景测试没有覆盖这个上游入口。

修复只在 `BattleScreen::finalize_settlement` 分类真实捕获：传入胜利状态且已有 `captured_mon` 时使用 `BattleOutcome::Captured`。写回随后才取走这只宝可梦，返回 `caught`。没有新增状态副本，也没有改剧情脚本。老人演示没有 `captured_mon`，仍走原返回结果；普通获胜、逃跑和战败保持原分类。

## 原作依据

- `engine/battle/core.asm` 的 `.returnAfterCapturingMon`（2289–2294）将 `wBattleResult` 写为 2。
- `scripts/Route12.asm:45–63`、`scripts/Route16.asm:46–64` 在结果 2 时跳过回山对白，并完成各自 BEAT 事件。
- `home/trainers.asm:185–212` 对传说静态野生对象的非黑屏结局置事件并隐藏；本修复没有改动这些场景规则。

以上依据来自本地提取的原作反汇编；这里不声称本轮运行了原作。

## 回归范围

新增四项 core 公共调用回归：

- Route12 队伍空位与 Route16 非空 PC 箱：吹笛场景排队 → 实际 BAG/MASTER BALL 菜单输入 → 确认文本结束 → 真实结算/写回 → 用返回值恢复场景。断言 `Captured`、正确 party/box 槽位、旧记录与道具保护、没有回山对白、BEAT/隐藏及重入隐藏。没有直接设置 settlement 或传入预设 `caught`。
- Safari BALL 菜单成功后也得到 `Captured`。这是预设 HP1 的 core 状态，保证真实捕获公式对所有已接受随机值成功；不冒称普通 Safari 自然遭遇记录。
- 队伍和当前箱均满时，拒球/确认返回背包，不扣球、不修改双方状态、不发布捕获或完成静态事件。
- 实际 FIGHT 胜利/全队战败、RUN 及老人自动演示保持原结局；只有战败请求黑屏传送。

## 本地验证与身份

生产修复为 `ee9ada9da013fd60d43114b43c08e73a0743050a`，基于实际 master `3105f07761ed8b5a619b28cc2fa07f9d36e34846`。后继 `fdb33c2971bf028dbf2d2f3098ce348649ca3cee` 仅修正新增测试的 Route16 吹笛站位，生产、Cargo 与引擎输入未改。

| 执行身份 | 实际结果 |
|---|---|
| ee9，首次完整 `pokered-core` | unit 2963 PASS、1 FAIL；尚未到 integration/doc |
| fdb，仅原失败 exact unit | 1 PASS；其余 2963 unit filtered out |
| fdb，Cargo metadata 枚举的全部 39 integration targets | 412 PASS、0 FAIL |
| fdb，library doc tests | 1 ignored |

首次失败是测试把 Route16 站位设为 `(26,11)`；原作及当前实现的吹笛坐标仅为 `(27,10)/(25,10)`。仅将该测试改为 `(27,10)` 面向左侧，并同步重入坐标；所有捕获/确认/结算/剧情断言保留。原失败日志与结果未覆盖，不把分阶段验证重标为 fdb 的一次完整 core 运行。四个新增名称均有且仅有一次通过记录，分别来自上述实际身份。

命令均使用已验证的离线锁定环境、单任务 `-j1`，没有重复已通过的 2963 unit。开始/结束均核对 2180 项编译输入、21 项 Cargo 输入、e533 引擎 981 项源文件和 702 项 GFX；全部未变，所有拥有的测试进程已终结。新测试文件 rustfmt 与差异检查通过。

这是 core 菜单/战斗结算/脚本调用的测试证据；本轮没有运行普通 Game、原作模拟器、普通 SAVE、App/CLI 或 GBA，也不声称完整画面、声音或时钟一致。
