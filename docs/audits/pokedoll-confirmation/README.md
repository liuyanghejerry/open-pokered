# 皮皮玩偶确认流程

野生战中使用皮皮玩偶，原实现立刻扣道具并显示 `Got away safely!`。
原作 `engine/items/item_effects.asm` 的 `ItemUsePokeDoll` 调用
`PrintItemUseTextAndRemoveItem`，先显示玩家使用道具的文字、播放
HealAilment，等待 A/B 确认后才移除道具。它设置逃脱标记，但不改
`wBattleResult`，所以无透视镜的嘎啦嘎啦幽灵战仍进入母亲平静的事件。
原函数没有额外的音效结束等待，修复也不额外阻塞这次确认。

修复保留同一个背包菜单到文字确认点，确认时消费一次并释放菜单。
新增核心测试检查 A/B、确认前后数量、玩家名、战斗状态无额外回合和
重复回调不会重复消费；旧测试的提前扣道具断言同步改为确认后检查。

## 实际操作证据

- 原作 `fbcf7d0e`：A、B 各重复两次，全部通过普通输入和普通 SAVE。
- 复刻前：录制时 master `25443cf4`，可执行文件 SHA
  `50067e8d95b930650c94f8b0483e5e1fdd96a95c95c0031e8441af408fae1f95`。
- 复刻后：`cf65fc44`，可执行文件 SHA
  `d58fd38cb70d9a76128e201269cffb551981fc1b2c52db3454f873fa0cb3b622`。
- 前后 A、B 各重复两次，初始状态、完整输入及 fixture 相同；匹配截图
  frame 1795。确认前数量从错误的 1 修正为 2，确认后均为 1。
- 八次复刻普通最终 SAVE 前后逐字节相同，各重复状态、输入、PNG 和
  SAVE 相同；原作与复刻的全部 404 字节队伍区、1122 字节当前箱区、
  玩家名、ID、金钱逐字节相同。队伍 HP、PP、经验未改变。
- 原作四次与复刻八次均触发母亲平静事件并设置 `0x10f`。

**独立未修复项：** 原作这场无透视镜的幽灵战会登记嘎啦嘎啦“见过”，
复刻没有登记；owned 图鉴位保持相同。`verification.json` 明确保留
这个差异，未把全部图鉴区列入保护断言。本 PR 只修玩偶使用确认流程。

## 回归与范围

新增测试在原实现失败；首次修复时旧测试的提前扣除断言失败，原始日志
保留。更新该断言后完整 core 2883 通过；整合 master `f44e3ef5`
后的生产提交 `6fa1a257` 完整 core 2885、debug app 269 通过，39 个
已有忽略项，0 失败，详见 `capture-integration.json`。

这些是根据新主线实际存档构造的预启动诊断场景，包含指定地图坐标、
三个已击败训练家旗标和两个玩偶，不是全新主线通关证明。进入测试后
仅用正常走路、BAG、道具选择、A/B 与普通 SAVE，没有写入运行状态或
直接调用战斗。状态采样在多帧输入命令之间进行，未声称全帧画面、
音频波形或整个战斗系统与原作一致。截图保持真实历史构建身份，没有
重新标记为最后的整合版本。

## 原始证据

`evidence.zip` 按内容 SHA 去重，但每个原始路径的字节完整保留；
`entry-manifest.json` 对应 native 输入、状态、截图、SRAM、只读回读、
构建身份、测试日志、脚本与嵌套原作证据包。外层的 `archive.json`
记录 SHA 和逐项回读结果。嵌套原作包含 7354 个路径、262 个内容块，
不含 ROM、可执行文件或模拟器状态。

无效数字枚举 JSON 导致回退默认角色的旧 helper、错误地图摄像机与
墙内坐标的原作准备尝试均作为失败工具记录保留，未计为游戏错误。

```bash
python3 extract_evidence.py evidence.zip --verify-only
python3 extract_evidence.py evidence.zip /tmp/pokedoll-evidence
```

进一步整合只读诊断工具 master `dfeefa34` 后，完整 debug app 270 通过、39 个已有忽略项、0 失败；核心生产源码未改变。日志见 `diagnostic-integration.json` / `diagnostic-integration-logs.zip`。

已整合中毒流程 master `7310d298`。生产整合提交 `ef8b9576` 完整 debug app 279 通过、39 个已有忽略项、0 失败；完整 core 2888 通过。完整日志见 `master-7310-integration.json` / `master-7310-integration-logs.zip`，原截图与录制身份未重新标记。
