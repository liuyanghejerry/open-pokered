# 背包中场景道具的使用与返回

本修复只处理背包场景道具的菜单归属、使用结果和返回路径。原作 `StartMenu_Item` 只有 `UsableItems_CloseMenu` 中的道具与成功自行车使用才退出菜单；普通道具和失败使用回到 ItemMenuLoop。城镇地图/图鉴查看结束也回到该列表。

- 驱虫喷雾显示玩家使用文字，最终确认之后扣除一件，继续留在背包。
- 代币盒显示实际数量；大木博士包裹、普通不可用道具打印原作拒绝文字，确认后保留背包。
- 自行车直接使用，跳过 USE/TOSS。室内、冲浪或强制骑车的拒绝留在背包；成功上下车仍退出背包并沿用独立 #156 的音乐切换。
- 背包中的城镇地图/图鉴关闭后回到原列表，保留选择项。
- 旧公开 use_field_item 返回 bool 的入口保留兼容；应用使用包含 consumed/closes_bag 的明确结果。

## 基线、源码和验证

实际 master：`38d8f140df16e2dc7f1353d2727bf65586654e32`。基线生产源码未经修改，仅追加与候选完全相同的 cfg(test) 场景驱动；game.rs 生产前缀 SHA256 为 `29c54dfdd33a9ef3b48eadc2d91326f48c4bf326b0a77e4838700dd3a334ccc0`。

候选录制版本：`b00ef803aec2112c7ad891ba707ba71ce3adc04b`，包含独立 #155 丢弃文字基础设施和已同步的上述 master。七个变更源码和冻结程序 SHA256 在 `audit-213-native-a4-immutable/manifest.json`。截图不声称对应将来同步产生的新提交；后续集成须通过其自己的 CI。

构建结束后立即冻结各自可执行文件，再运行测试与捕获；捕获期间未使用共享目标中的可变程序。候选核心 **2719**、应用 **195** 全部通过（21 个按需捕获辅助忽略）；实际 master 的同一所有权回归在 repel-a 第16帧失败，直接证明其提前退出背包。

每侧15场景，每场景301帧，各重复2次，共 **18,060 PNG** 和完整 JSON。两次完整记录逐字节一致；每帧保留缓存绘制与全量绘制的所有像素一致。场景包括 A/B 确认、拒绝文字翻页、三种自行车拒绝、上下车、地图/图鉴二号列表项返回。每个截图对来自相同受控场景、输入和帧。

受控场景以相同 seed42、English、Medium、纯 Bag 入口运行；世界先加载并等待120帧。普通道具/查看器使用真实常青市中心地图，自行车成功使用真实1号道路。冲浪状态和强制骑车锁显式注入，不能据此声称自然行走覆盖了这些入口。

## 原作证据

参考源版本 `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`：

- `engine/menus/start_sub_menus.asm` 的 `.choseItem`、`.useItem_closeMenu`、ItemMenuLoop。
- `data/items/use_overworld.asm` 的 UsableItems_CloseMenu。
- `engine/items/item_effects.asm` 的 ItemUseRepel / ItemUseCoinCase / ItemUseNotTime。

使用 PyBoy 的真实 ROM 指令钩子和画面录制，受控既有常青市中心场景，注入物品、Medium=3、菜单游标，并实际 START→ITEM→使用；未直接调用原作物品函数。各场景重复两次完整 PNG/JSON 字节一致：自行车3拒绝 **2484 PNG**；喷雾/代币盒/包裹/金珠 **3602 PNG**；地图/图鉴/420代币 **2628 PNG**；二号物品地图/图鉴返回 **1918 PNG**。拒绝和普通物品没有 CloseStartMenu，喷雾最后确认才 RemoveUsedItem；查看器返回 cursor=1、wBagSavedMenuItem=1。原始快照源、控制输入及限制见附带脚本，ROM、SRAM、状态文件和可执行程序没有归档。

## 截图

- repel-before/after：第100帧，库存与菜单归属。
- bicycle-before/after：第100帧，直接使用及拒绝。
- town-map-before/after、pokedex-before/after：第130帧，关闭后的列表和游标。

## 证据使用和限制

`evidence.zip` 保存完整 JSON、构建/测试/捕获日志、源哈希、原作/当前录制脚本。PNG 以 SHA256 无损去重；解压后运行 `python3 restore-pngs.py` 恢复每一帧原文件路径和字节，脚本同时验证哈希。重复检查比较的是原始录制文件，未用去重记录替代原始检查；完整原始捕获目录仍保留在本地。

早期 a1 的旧钓竿文字/双按自行车测试失败，以及 a2 错把原作 CONT 保留上一行作为现有分页布局的测试预期，都保留日志；a3/a4 已修正测试预期并完整通过。没有为此改变用户排除的对话布局。

本修复不宣称原作绝对 CPU/PPU 进入时间、字体图块或窗口布局完全相同。成功逃脱绳的30帧保护、所有窗口开关、普通背包重开游标、PC/商店流程和更广泛功能仍另行审计；成功逃脱绳的即时消耗不属于本次修复。有效钓竿沿用原有动画流程，不声称本组覆盖了其完整时间线。
