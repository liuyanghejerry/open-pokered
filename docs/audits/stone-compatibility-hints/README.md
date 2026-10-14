# 进化石队伍兼容性提示

从背包使用任意一种进化石时，队伍列表现在显示各成员的 `ABLE` / `NOT ABLE`，并隐藏该行的血量、血条和异常状态。提示按种族的对应进化石条目判断，倒下或中毒不改变兼容性。实际使用和进化仍由既有道具流程处理。

原作依据为 `engine/menus/party_menu.asm` 的 `.evolutionStoneMenu`：遍历 `EvosMovesPointerTable` 的 `EVOLVE_ITEM` 条目，匹配 `wEvoStoneItemID`，在名字右九列、下一行打印提示。此菜单不检查等级或当前 HP。复刻复用 TM/HM 的队伍提示渲染入口，改名为 `item_compatibility`，五种进化石使用只读 `get_evos_moves` 数据。

实际输入验证覆盖雷之石与叶之石，各重复两次。雷之石病例为倒下且中毒的 Lv6 皮卡丘，叶之石病例为中毒且受伤的 Lv22 臭臭花；第二位均为不兼容的 Lv12 妙蛙种子。原作与复刻均通过 START → ITEM → USE 打开列表，真实按 DOWN 选第二位、B 取消，再由普通 SAVE 写入存档。数据只在输入前构造，验证期间没有状态写入指令，也没有直接调用道具或进化例程。

- 原作四次菜单显示 `ABLE` / `NOT ABLE`，四次普通 SAVE 校验通过。
- 修复前四次与修复后四次的全部输入和截图帧相同；只有两个队伍列表截图变化，取消后的背包和普通存档结果截图完全相同。
- 每对修复前后完整 32768 字节 SRAM 一致，队伍、数量、金钱、实际 flags 均不变；保存的全部 88 字节队伍数据也与对应原作一致。
- 两次重复的输入、截图和存档 hash 相同。
- 59 项菜单测试通过，包括五种进化石、倒下且中毒的兼容成员、不兼容成员、已进化雷丘、普通队伍/Potion 和既有 TM/HM 对照。带 `debug-server` 的应用构建通过。

截图和实际验证记录在 [verification.json](verification.json)。原作 revision 为 `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`，ROM SHA1 为 `ea9bcae617fdf159b045185467ae58b2e4a48b9a`。修复前冻结程序来自 clean master `906ae8f8`，修复后来自 `f58e6d28`，版本与 SHA256 均保留；后者包含尚未合入的 #239，发布本修复前需先合入该共同提示入口。后续合入 master 的能力页状态修复不会改变本次进化石列表；不能将已有冻结程序重新标为后续提交。

本次实际菜单验证未选择使用进化石，也不证明进化动画或原作/复刻逐帧视觉一致。五种石头由菜单测试覆盖，实际原作输入仅覆盖雷之石和叶之石。第一轮 native 驱动按固定两次 DOWN 误入训练家卡，属于拒绝的验证尝试；[日志](rejected-first-native.log) 保留。修正后按实际 START 菜单中 ITEM 的索引走位。

可用当前仓库的 `debug-server` 应用复播 native 验证：

```bash
python3 docs/audits/stone-compatibility-hints/native_capture.py after /abs/path/pokered-app /abs/path/new-evidence-dir
```

`fixture-template.json` 保留完整输入前快照模板；脚本在私有目录复制程序，校验普通 SAVE 和 SRAM，所有操作记录与截图写入新目录。
