# 界面字体留白与提示框修正

基线为 `master` 的 `6a452f766edf9af7f9b133f693492a7338185df9`，
对应 PR 分支 `fix/ui-overlay-font-padding`。

修正背包操作菜单和学习器确认框、队伍操作菜单和多行提示的字高与边框间距。
名人堂仪式和联盟电脑共用信息卡，昵称在框内留出顶部空白，等级、属性采用
标签左对齐、数值右对齐的同一行布局。仪式标题位于信息卡下方，避免覆盖第二属性。玩家名字、时间、金钱和英文图鉴评价也调整排版。
老虎机提示与下注框、图鉴“区域未知”和大木博士命名提示框补足文字空间。
老虎机增量重绘的光标位置与完整绘制同步。

## 截图复现

[`capture.rs`](capture.rs) 是前后两次运行完全相同的固定状态捕获程序。
将其临时复制为 `crates/pokered-app/examples/capture_ui_overlay_padding.rs`，
分别在上述 master 提交和 PR 分支运行：

```bash
cargo run --locked -p pokered-app --example capture_ui_overlay_padding -- /absolute/output/path before
cargo run --locked -p pokered-app --example capture_ui_overlay_padding -- /absolute/output/path after
```

两次使用相同的 `gfx/` 资源、中英文设置和状态构造。队伍使用六只 20 级宝可梦，
学习器使用 TM01 且选中 NO；名人堂使用 100 级拉普拉斯或皮卡丘，在 `MonText`
入口帧截图，玩家统计在 `PlayerStats` 入口帧截图；老虎机使用 seed 42、100 枚币，
分别截图下注与结果状态。队伍提示使用固定两行和四行文本覆盖布局边界，
这些状态是渲染测试夹具，不代表完整游玩流程。

共 14 个场景 × 2 种语言，28 组前后截图，均为原始 160×144 帧。
图像 SHA256、捕获源码 SHA256 和变化像素数记录在
[`captures.json`](captures.json)，PNG 位于
[`docs/screenshots/ui-overlay-font-padding`](../../screenshots/ui-overlay-font-padding)。
前后截图已经人工检查，单属性和双属性信息卡、四行提示、最大时间与金额均有覆盖。

## 验证

回归检查覆盖全部 TM/HM 的确认框底部留白、背包操作菜单底部留白，
老虎机全部下注光标切换的增量重绘与完整绘制一致性，以及名人堂标题出现后
第二属性文字保持可见。

测试和编译结果见 [`validation.json`](validation.json)。


## 名人堂评审调整

评审前版本为 `6192f80618d9f12a6699455456aa2e736f3e9190`。
昵称从 y=24 调整到 y=30，在文本框内部上沿保留 6 像素空白，左右各留 4 像素。
首轮调整中，等级、第一属性和第二属性的标签与值分别放在同一行（y=50、72、94），
标签移除原来独占一行时使用的尾部斜杠，值按字宽统一右对齐。
英文最长属性 FIGHTING/ELECTRIC 也能留出标签和值之间的空白。

新增 6 张 `*-review-before.png` 保留评审前 PR 画面，对应现有 6 张 `*-after.png`，
覆盖中英文名人堂单/双属性和联盟电脑。master 对照截图保持原样，after 已更新。
评审追加验证记录在 `validation.json` 的 `hof_review` 字段。

## 名人堂标题留白修正

评审前版本为 `12f276c12e850890f8be0bffc100b8525c39061d`。
下方标题框原先仅有 8 像素内部高度，无法容纳 10 像素字体。
本轮将信息卡内部高度从 88 缩短到 80 像素，等级与属性行使用 y=50、70、90，
标题框从 y=120 上移到 y=112，内部高度从 8 增至 16 像素。
标题文本置于 y=122，考虑中文字形的基线偏移后，实际字形上下各至少保留
3 像素内部空白，边框完整位于屏幕内。
信息卡仍保留昵称顶部留白，双属性内容与标题互不覆盖。

新增 6 张 `*-title-review-before.png` 与更新后的 `*-after.png` 对照，
覆盖中英文仪式单/双属性以及共用信息卡的联盟电脑。
增加像素回归检查，要求中英文标题上下各 3 行内部像素全部为空白且标题仍可见。
本轮验证见 `validation.json` 的 `hof_title_review` 字段。
