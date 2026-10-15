# Safari 最后一球失败的广播

最后一颗 SAFARI BALL 未捕获时，恢复原作 `PA: Ding-dong!` 和随后 `You are out of / SAFARI BALLs!` 两段广播。生产改动只有英文及其翻译键各一行。原作依据为 `engine/battle/core.asm:172–180,209–211` 与 `data/text/text_2.asm:824–829`。

## 当前集成验证

#256 实际合入后整合 master `2429092a8c487d366bdf8aec44e3b631d70bef2f`，父合入为 `2429092a8c487d366bdf8aec44e3b631d70bef2f`。相对于该实际 master，生产差异仅 battle/mod.rs 与 data/battle_text.rs 各一行。全部 2170 项编译输入及 docs 外 2689 项跟踪文件内容、类型与模式均与受测源码 `f42408ebf4a28c313c05a60e6c0a75aaf762bf50` 一致。沿用该相同源码的 core 2913 通过、debug-server app 315 通过/39 忽略、determinism 10 通过。没有重新构建或重录；本地证据不代表新远程 CI 已通过。

精确身份、比较范围、测试总数和日志白名单见 `publication-953.json` 与 `publication-953-logs.zip`。旧测试/CI/录制均保留历史身份，旧 `master-*-integration` 文件不是本次新执行结果。新 head 需 CI、Build GBA ROM、GBA performance 三个新工作流全部通过后才可合入。

## 历史实际证据

截图前版本为 clean master `b7c81d82d7d94a06555f631c3c70d114f299d9d8`，二进制 SHA256 `bb844b56204ef0c75c5a9e29282a54d19558798afb88f2f206597938a60bb2c9`；后版本生产源码 `aff8dcf1589babfabdcd8d049b5776cd0a0962f7`，二进制 SHA256 `f62161db6167811888026a743fab78f1b4cdeda0416eb0fb26e4f21a1581f78e`。同 fixture、初始 SRAM、相同完整物理按键前缀至帧2359，前缀 SHA256 `9ad4788c57d9cdc49d95e2d53c6140c9bfc2a52a5d6fa1474adb08ce8626e66f`。图片没有改称当前集成 head 的录制。

原作两次自然最后一球失败；原生前版本两次失败、一次成功，后版本两次失败，全部普通 SAVE 有效。验证前构造 Safari 活动/剩一球及合法队伍，之后只正常走路、自然遇敌、投球、确认和保存。原作/原生物种与 RNG 不同，捕获 entropy 没有全部被 CLI seed 固定；不宣称完整随机重放、跨提供者全部 SRAM 相等或整个 Safari/主线一致。

失败控制保护 party404、box1122、背包/现金、owned、玩家姓名/ID；自然遇敌改变的 seen 位不纳入初末不变断言。首个原生失败对照最终 SRAM 只在游戏时间0x2CF0/0x2CF1及校验和0x3523不同。原始版本/输入/SAVE/帧保护详见不变的 `verification.json`、`archive.json` 和 `evidence.zip`。

原始 archive 4128路径/366blob，排除 ROM、可执行文件和模拟器state；原始本地资料继续保留。可运行 `python3 extract_evidence.py evidence.zip --verify-only` 检查旧归档；本次发布脚本没有执行游戏、构建或旧脚本。
