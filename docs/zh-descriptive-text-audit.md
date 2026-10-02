# 中文开场与描述文案核对

基线：`72ff719`。这是 PR #110 的扩展范围；地图 NPC 语料见
[对白分页审查](zh-npc-dialogue-pagination-audit.md)。

## 覆盖与修正

| 入口 | 核对范围与结果 |
| --- | --- |
| 大木博士开场 | 六个文字阶段，中文与英文、四组名字；逐字推进、跳过显示、翻页及阶段转移 |
| 图鉴 | 全部 151 种宝可梦的名称类别和完整介绍；原有 302 个中文源片段，重新排成 156 个显示页 |
| 大木博士 PC 评价 | 全部 16 档捕获数量；真实菜单、确认、完成度、评价及关闭连线流程 |
| 名人堂结算评价 | 复用完整 PC 评价译文，全部 16 档均不超过三行；学习装置提示的逐像素绘制检查 |
| TM/HM 学习提示 | 全部 55 个道具；三行提示的字形宽度、行距及边框位置 |
| 进化与交换 | 全部 151 种宝可梦，合法上限的英文、中文及混合昵称/训练家名字；两行提示宽度 |
| 战斗与联机提示 | 中文战斗文字使用与地图对白相同的分页规则；绘制保持定稿行界，联机短句去除生硬换行 |
| 原生/Web 与 TUI | 共用核心的中文图鉴/PC 页表；TUI 开场与图鉴介绍按中文选择，不再显示英文文案 |

### 开场

原生绘制选择中文页表，但核心状态机仍按英文页数及字符数推进。
Greeting、Explanation、IntroducePlayer、FinalSpeech 的中文页表比英文短，
会出现多余空白页；短中文页的翻页等待也按英文长度计算。
现在状态机与绘制采用同一语言，中文页数为 2 / 1 / 4 / 1 / 3 / 3。
保留中文原文字符序列和英文原有页表、显示时序。

中文开场文本框两行移到绝对 tile `(1,13)`、`(1,15)`，避免 12px 字形
碰到底部边框；命名提示也放回短框内部。英文仍保留原有布局。
TUI 的命名输入、阶段文字及滑动期间的提示与语言配置一致。

### 图鉴

旧代码估算汉字宽度为 16px，逐字切行。中文源页界还沿用英文片段，
例如把“能不知疲倦地”与“爬上陡坡和墙壁。”分在两页。
现在先合并完整中文介绍，再按真实 144px 宽度排版，每页三行。
保护词语及中文标点，短末行保留必要上下文；完整句子放不下剩余行时
整体移到下一页。空行只用于补足三行高度，不单独生成显示页。
列表 DATA、捕获后的独立介绍及场景脚本的图鉴预览采用真实中文页数，
不再为了英文页数显示空页。类别标签使用真实字形宽度，避免挤到精灵图上。

独立 Jieba 整句分词审查覆盖 151 条完整介绍、156 页：空白页与断词候选均为 0。
核心测试核对全部字符不丢失、顺序不变、行宽、标点禁则及逐页退出行为。
英文页数仍读取原有英文页表。

### PC 与其他提示

PC 原本先按四个英文源行分页，再在绘制时逐行翻译。
中文句子因此被切成碎片，50 种宝可梦的评价甚至把“学习装置！”留到下一页。
现在完整消息先翻译，再决定中文行界及页数。空行作为段落边界，不产生空页。
重复的英文 `from my AIDE!` 按前文选择闪光或探宝器，修正误用译文。
16 档中文评价均可在一个 PC 消息页内完整显示；英文消息的行数及菜单流程保留。

名人堂结算原本只绘制评价的前两个英文源行，后半段直接丢失。
现在中文评价使用同一份完整译文，以 12px 行距显示三行。
已见/拥有数量合成一行，为完整评价留出空间；中文名字、时间及金额也放回各自框内。
时间与金额的标签、数值使用同一行基线，两组数值统一从 x=52px 开始；
“游戏时间”标签后留 4px 空隙。核对正常值、零值及游戏上限 `255:59` / `$999999`，
均完整处于统计框内。原生/Web 与 TUI 对应帧逐像素一致，框外评价和人物图不变。

中文 TM/HM 学习提示改为 16px 行距，避免末两行重叠。
战斗提示保持核心准备好的中文两行；进化、交换保留各阶段的明确行界。
联机取消、对战/交换确认等短句在一行内完整显示。

当前 GB 版数据没有道具/招式的散文说明字段：道具描述接口返回名称，
招式数据保存效果、威力、属性、命中率、PP。本次核对现有使用、学习和效果提示，
没有新增不存在的说明功能。自动排版及独立分词不能代替对所有未来文案的人工审校。

## 前后截图

`visual_verify_zh_descriptions` 和 TUI 同名 fixture 使用实际生产绘制路径。
前图恢复基线的核心及绘制代码，后图运行修复代码，资源与种子状态相同。
开场固定入口淡入已结束，A 在帧 0/2/4/6/8/10 按下，截图为帧 43；
其他比较保持相同条目、阶段及页面索引 0。PC 对齐到完成度提示后的首个评价页。

截图在 `docs/screenshots/zh-descriptions/`；PR 描述嵌入全部 19 组前/后图。
涵盖开场真实输入、静态开场、命名、图鉴、学习提示、战斗、进化、交换、联机、
PC 评价、名人堂结算、场景图鉴预览及 TUI 对应界面。
另在 `docs/screenshots/zh-hof-stats-alignment/` 保存四组名人堂对齐对照：
原生/TUI 的正常值与上限值。此处前图来自修正对齐前的 PR 提交 `9b07393`，
用于直接展示数值错位；上面的主对照仍以 master `72ff719` 为前图。

## 复查

```sh
ZH_DESCRIPTION_AUDIT=/tmp/zh-descriptions.json \
  cargo test --locked -p pokered-core --test zh_descriptive_text
python3 scripts/audit_zh_dialogue_pages.py /tmp/zh-descriptions.json
cargo test --locked -p pokered-ui --test zh_description_geometry
cargo test --locked -p pokered-app --test visual_verify_zh_descriptions
PR_SCREENSHOTS=/tmp/zh-description-shots \
  cargo test --locked -p pokered-app --test visual_verify_zh_descriptions -- --ignored
PR_SCREENSHOTS=/tmp/zh-description-shots \
  cargo test --locked -p pokered-tui capture_tui_zh_descriptions -- --ignored
PR_SCREENSHOTS=/tmp/zh-description-shots \
  cargo test --locked -p pokered-app --test visual_verify_zh_descriptions capture_zh_hof_stats_limits -- --ignored
PR_SCREENSHOTS=/tmp/zh-description-shots \
  cargo test --locked -p pokered-tui capture_tui_zh_hof_stats_limits -- --ignored
```

全量回归按 crate 分别运行，避免 workspace feature 合并影响平台测试。
core 2,947、data 425、UI 93、app（debug-server，lib）111、TUI 23 项通过，
另有真实 app 开场与 PC 流程、名人堂评价绘制三个集成检查，
以及现有中文图鉴截图 fixture 的退出流程检查。
