# PC、背包与联机布局复审

原作拉丁字形恢复为8px后，PC菜单使用原作的两tile `PK/MN` 字形和菜单/BOX框尺寸，光标与标签分开绘制；名人堂等级、属性标签和值分行，保留右侧正面图区域。native与TUI采用相同布局。联机提示缩为屏内可显示的 `L/R A:OK B:BACK`。背包按像素分别放置名称与数量，数量右缘固定为152px。

原作依据包括 `engine/pokemon/bills_pc.asm:121–129` 的12tile菜单及标签坐标、`:149–168` 的9tile BOX框和两位数字，`:342–350` 的菜单字形/BOX标签，以及 `engine/menus/league_pc.asm:98–121` 的正面图位置和名人堂入口。对应回归位于 `crates/pokered-app/src/render/pc.rs` 的 `layout_tests` 和 `crates/pokered-ui/src/menus/bag.rs` 的 `mixed_glyph_item_names_do_not_push_quantities_through_the_border`。

中文列表回归保留12px行距及完整字形。旧断言误把第二行原作数字 `1` 在 `(73,21)` 的合法像素当成重叠；新断言检查两行字形完整、没有相交且具有完整空白扫描行，不移动生产行坐标。

## 可重跑截图

[capture-ui-world.py](capture-ui-world.py) 直接运行预构建 `pc_tour`；[ui-world-captures.json](ui-world-captures.json) 保存二进制哈希、源码版本和全部32张图路径。基线为 `72ff719` 生产渲染器、冻结的原依赖和 `31ad6e7` 同一套fixture。after为 `8837a2a32c2ba8c345db16e97b01f49016e88950`，二进制SHA256为 `3ee8e23fcc99b373bb4c76e24e63ac467e8380f211924cd319de9494713db3eb`。

```sh
python3 docs/audits/2026-10-02-full-fidelity/capture-ui-world.py \
  --after /workspace/onboarding/pokered-fidelity-final/pc_tour \
  --after-source 8837a2a32c2ba8c345db16e97b01f49016e88950
```

8个案例各运行EN/ZH，16组配对文件名和语言一致，全部32张图为160×144；逐一目检16张after。截图是固定存档状态的生产渲染结果，不是ROM逐帧对拍或连续玩家流程。

| 案例 | EN before / after | ZH before / after |
| --- | --- | --- |
| PC主菜单 | [before](../../screenshots/fidelity-ui-world/before-en/02_main_menu.png) / [after](../../screenshots/fidelity-ui-world/after-en/02_main_menu.png) | [before](../../screenshots/fidelity-ui-world/before-zh/02_main_menu.png) / [after](../../screenshots/fidelity-ui-world/after-zh/02_main_menu.png) |
| Bill菜单与字形 | [before](../../screenshots/fidelity-ui-world/before-en/04_bills_menu.png) / [after](../../screenshots/fidelity-ui-world/after-en/04_bills_menu.png) | [before](../../screenshots/fidelity-ui-world/before-zh/04_bills_menu.png) / [after](../../screenshots/fidelity-ui-world/after-zh/04_bills_menu.png) |
| 取出操作弹窗 | [before](../../screenshots/fidelity-ui-world/before-en/06_withdraw_popup.png) / [after](../../screenshots/fidelity-ui-world/after-en/06_withdraw_popup.png) | [before](../../screenshots/fidelity-ui-world/before-zh/06_withdraw_popup.png) / [after](../../screenshots/fidelity-ui-world/after-zh/06_withdraw_popup.png) |
| 道具PC菜单 | [before](../../screenshots/fidelity-ui-world/before-en/14_item_menu.png) / [after](../../screenshots/fidelity-ui-world/after-en/14_item_menu.png) | [before](../../screenshots/fidelity-ui-world/before-zh/14_item_menu.png) / [after](../../screenshots/fidelity-ui-world/after-zh/14_item_menu.png) |
| BOX No.12 | [before](../../screenshots/fidelity-ui-world/before-en/24_bills_box12.png) / [after](../../screenshots/fidelity-ui-world/after-en/24_bills_box12.png) | [before](../../screenshots/fidelity-ui-world/before-zh/24_bills_box12.png) / [after](../../screenshots/fidelity-ui-world/after-zh/24_bills_box12.png) |
| 实际取出THUNDERSTONE文本 | [before](../../screenshots/fidelity-ui-world/before-en/25_withdrew_thunderstone.png) / [after](../../screenshots/fidelity-ui-world/after-en/25_withdrew_thunderstone.png) | [before](../../screenshots/fidelity-ui-world/before-zh/25_withdrew_thunderstone.png) / [after](../../screenshots/fidelity-ui-world/after-zh/25_withdrew_thunderstone.png) |
| 名人堂等级与属性 | [before](../../screenshots/fidelity-ui-world/before-en/26_league_hof_type.png) / [after](../../screenshots/fidelity-ui-world/after-en/26_league_hof_type.png) | [before](../../screenshots/fidelity-ui-world/before-zh/26_league_hof_type.png) / [after](../../screenshots/fidelity-ui-world/after-zh/26_league_hof_type.png) |
| 实际背包名称/数量 | [before](../../screenshots/fidelity-ui-world/before-en/27_bag_font_bounds.png) / [after](../../screenshots/fidelity-ui-world/after-en/27_bag_font_bounds.png) | [before](../../screenshots/fidelity-ui-world/before-zh/27_bag_font_bounds.png) / [after](../../screenshots/fidelity-ui-world/after-zh/27_bag_font_bounds.png) |

## 证据边界

- 生产背包当前在中文模式也使用英文道具名；混合CJK名称测试是共享UI的边界回归，不列为生产中文背包已经发生的bug。
- 生产英文取出文本原本就是两行。22字符单行 `Withdrew THUNDERSTONE.` 是换行helper的合成边界输入，不列为基线真实取出流程溢出。
- 名人堂fixture没有加载精灵图资源；截图和断言检查左侧信息不进入原作正面图保留区，不声称截图展示了实际正面图。
- 此脚本运行native渲染器；TUI布局有源码复审和共享布局回归，未将native截图充作TUI截图。
