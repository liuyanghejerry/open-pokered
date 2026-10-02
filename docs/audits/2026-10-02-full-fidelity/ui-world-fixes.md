# PC、背包与联机布局复审

按用户要求保留项目原有Fusion Pixel字体，普通英文、中文及混合文本继续使用项目字形和度量。PC保留有独立原作依据的菜单/BOX框和名人堂布局，专用 `PK/MN` 菜单图形与普通字体分开。名人堂等级、属性标签和值分行，保留右侧正面图区域；native与TUI采用相同布局。联机提示恢复原项目的 `L/R:SIDE  A:OK  B:BACK`。背包名称与数量按实际像素宽度布局。

原作依据包括 `engine/pokemon/bills_pc.asm:13–27` 和 `engine/menus/players_pc.asm:25–32` 的14tile主/道具PC框，`engine/pokemon/bills_pc.asm:121–129` 的12tile菜单及标签坐标、`:149–168` 的9tile BOX框和两位数字，`:342–350` 的菜单字形/BOX标签，以及 `engine/menus/league_pc.asm:98–121` 的正面图位置和名人堂入口；名人堂左侧框、昵称、等级与属性分行另直接对应 `engine/movie/hall_of_fame.asm:159–183`。对应回归位于 `crates/pokered-app/src/render/pc.rs` 的 `layout_tests` 和 `crates/pokered-ui/src/menus/bag.rs` 的 `mixed_glyph_item_names_do_not_push_quantities_through_the_border`。

中文列表保留项目既有12px行距。文字边界按Fusion Pixel实际字宽计算；英文换行仅在超过144px时发生。此前8px阶段的22字符固定两行和 `(73,21)` 固定墨点断言不作为最终字体证据。

## 可重跑截图

[capture-ui-world.py](capture-ui-world.py) 直接运行预构建 `pc_tour`；[ui-world-captures.json](ui-world-captures.json) 保存二进制哈希、源码版本、全部32张图路径及16张after逐图哈希。基线为 `72ff719` 生产渲染器、冻结的原依赖和 `31ad6e7` 同一套fixture。after为 `eebde2fc3a4f78c38b4c7b5d49e0db2b830cb235`，二进制SHA256为 `32f88cbe1460667664092a137f40a7a4ba3b694b377bf096d73cd41c6a46466b`。

```sh
python3 docs/audits/2026-10-02-full-fidelity/capture-ui-world.py \
  --after /workspace/onboarding/pokered-font-preserved-final-checked/pc_tour \
  --after-source eebde2fc3a4f78c38b4c7b5d49e0db2b830cb235
```

8个案例各运行EN/ZH，16组配对文件名和语言一致，全部32张图为160×144；逐一目检16张after，原有16张before的SHA256保持不变。截图是固定存档状态的生产渲染结果，不是ROM逐帧对拍或连续玩家流程。

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
