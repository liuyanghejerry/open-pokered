# 原作保真修复：审阅与验收索引

本 PR 保留游戏修复及正式回归测试，普通英文、中文和数字继续使用项目原有 **Fusion Pixel**。
完整审计报告、全量截图、原始日志和失败快照已独立归档，避免占据修复 diff。

基线：`72ff719b39634c153cb82d3f3ece200bd413c4e0`；原作：
`pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。
生产代码验收源码：`eebde2fc3a4f78c38b4c7b5d49e0db2b830cb235`。
这次精简只移出审计产物及专用截图工具，不改生产行为与正式测试。

## 按模块审阅

| 模块 | 修复内容 | 入口 |
| --- | --- | --- |
| 引擎依赖 | 所有消费者及 GBA lockfile 固定同一 `92ba95f`；螺旋转场终止、NPC 原作策略、滑音和行动前回调 | [Cargo.toml](../../../Cargo.toml)、[引擎 PR #78](https://github.com/liuyanghejerry/dotzuki/pull/78) |
| 战斗 | PP/状态门、固定伤害、Counter/Wrap/Thrash、Disable/Mimic/Transform、经验、徽章和状态能力值、AI 行动时点 | [生产规则](../../../crates/pokered-core/src/battle/pokered_rules/runtime.rs)、[战斗回归](../../../crates/pokered-core/tests/fidelity_battle.rs) |
| 剧情与地图 | 87 个场景脚本；其中44张地图包含104个地面球。成功才消费/隐藏、满包重试、奖励编号、赠物、位置和机关旗标；语义图同步 | [场景](../../../crates/pokered-data/maps/)、[解释器](../../../crates/pokered-core/src/overworld/native_script.rs)、[事件图](../../../crates/pokered-data/story/graph.json) |
| 存读与联机 | 原子库存、PC/育成元数据、原作SRAM对齐及旧存档迁移、NPC交换后普通SAVE/CONTINUE、真实TCP行动等待和战后返回/治疗 | [存档](../../../crates/pokered-core/src/save/)、[前端桥接](../../../crates/pokered-app/src/game.rs)、[系统回归](../../../crates/pokered-core/tests/fidelity_systems.rs) |
| 音画与菜单 | 负坐标裁剪、字标/OBJ调色板、边框、菜单和数量字段、命名、名人堂/结尾时序、实际音乐结束等待；保留原字库 | [native渲染](../../../crates/pokered-app/src/render/)、[UI](../../../crates/pokered-ui/src/)、[TUI](../../../crates/pokered-tui/src/)、[音频](../../../crates/pokered-audio/src/) |
| GBA内存 | 化石实验室完整17490B AST改为真实菜单返回后延迟加载小分支，最大2593B；保留取消原文和满箱重试 | [构建生成](../../../crates/pokered-data/build.rs)、[化石场景](../../../crates/pokered-data/maps/CinnabarLabFossilRoom/script.scene) |
| 导航工具 | 原楼层路径、CUT后的地图状态、训练家战斗交接等真实按键驱动修复及对应测试 | [驱动](../../../scripts/playthrough.py)、[导航测试](../../../scripts/test_playthrough_navigation.py) |

这些模块共享存档、解释器命令、前端状态和渲染接口，故保留为同一个修复 PR。
地图脚本、命令执行和事件图必须一起审阅；联机的新增状态必须配套前端处理。
原作依据和逐项基线复现见下方完整归档，不把共享根因的编号相加作为独立 bug 数。

## 验证

| 检查 | 已验证结果 |
| --- | --- |
| 全部16个native workspace包 | 123个目标、4596次执行通过，0失败、9跳过；含app lib/bin重复执行，不是4596个独立测试 |
| Web | `pokered-web`、`pokered-runner-web`、`pokered-ui-preview` 的 wasm32 检查通过 |
| Python导航 | 54项 `test_playthrough*.py` +9项 exploration，共63项通过 |
| 实际入口 | 11个定点场景、15阶段NPC交换/普通SAVE/正常启动CONTINUE、12阶段真实双TCP战斗通过 |
| 原字库与画面 | 原提供器逐像素/字宽回归、菜单边界和下伸笔画回归、14个预览golden通过 |
| GBA | release编译，本地原31性能门槛及40次Route22遇敌通过；确切源码CI通过13内存场景、逐帧渲染及慢ROM开场/门口/标题门槛 |
| 精简前的生产源码CI | 21项成功、1项按条件跳过；[GBA job](https://github.com/liuyanghejerry/open-pokered/actions/runs/37023355401/job/110891720787) |
| 引擎 | 488单测及3doctest通过；PR #78的工作流修复 `5f724eb` 9项CI全部通过，含真实APK构建 |

以上完整检查来自已冻结的生产源码，精简后的新提交保留它们的实际来源，不重新标记旧日志。
正式回归测试保留在 crates/scripts，原始命令、二进制哈希和检查证据见 [验收摘要](validation.json)
及 [完整验收记录](https://github.com/liuyanghejerry/open-pokered/blob/129cd8e79212fb5b1d2679d7512052ddd0c5faa8/docs/audits/2026-10-02-full-fidelity/validation/final-validation.md)。代表性图片与逐图来源见 [截图清单](screenshots.json)。

全量内容独立核对覆盖151种、165招式、391队994只训练家精灵、248地图及双版本野遇；
原始结构差异保留分类，不声称全部JSON逐字相同。

## 覆盖边界

连续新游戏第四次通过m01–m21，m22的驱动交接修复另有正常按键验证；本轮没有完成新的
m01–m49连续通关。定点存档及真实TCP起点为受控输入，不冒称连续培养自新游戏。
未穷举全部动态战斗/路线，未进行新的全ROM逐帧或全曲目PCM对拍、实体GBA验收、
完整SGB或物理串口互通，也未复制原作非法内存漏洞。
SRAM布局来自独立汇编原作RAM符号及字段转换测试，未做本轮真实原作ROM载入导出存档。
GBA远端gate成功有确切源码的CI证据，未取得的原始远端内存/时钟度量不伪造。
截图覆盖native呈现，不将其写成TUI逐帧验收；合成超长名字等边界另有正式像素测试。

## 完整归档

[完整审计与原始证据](https://github.com/liuyanghejerry/open-pokered/blob/129cd8e79212fb5b1d2679d7512052ddd0c5faa8/docs/audits/2026-10-02-full-fidelity/README.md) 保留全部历史结果及复现程序。
归档分支为 `archive/full-fidelity-audit-2026-10-02`，固定快照
`129cd8e79212fb5b1d2679d7512052ddd0c5faa8`。已撤回的字体实验仅属于历史资料，不计入最终画面验收。
