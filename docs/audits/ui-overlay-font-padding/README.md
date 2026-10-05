# 界面字体留白与提示框修正

基线为 `master` 的 `6a452f766edf9af7f9b133f693492a7338185df9`，
对应 PR 分支 `fix/ui-overlay-font-padding`。

修正背包操作菜单和学习器确认框、队伍操作菜单和多行提示的字高与边框间距。
名人堂仪式和联盟电脑共用信息卡，等级、属性采用 12 像素行距，仪式标题位于
信息卡下方，避免覆盖第二属性。玩家名字、时间、金钱和英文图鉴评价也调整排版。
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
