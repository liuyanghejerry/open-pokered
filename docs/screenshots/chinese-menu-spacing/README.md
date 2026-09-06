# 中文菜单排版修复

基准：`master` 提交 `dc65c6c`。先在该提交上运行同一截图工具，再修改渲染并重跑。原始 PNG 为 160×144；总览仅以最近邻方式放大 2 倍。

## 前后对比

| 界面 | 前 | 后 |
|---|---|---|
| 选项页 | ![前](01-options-before.png) | ![后](01-options-after.png) |
| PC 道具列表 | ![前](02-pc-list-before.png) | ![后](02-pc-list-after.png) |
| PC 多行提示 | ![前](03-pc-message-before.png) | ![后](03-pc-message-after.png) |
| 图鉴侧栏 | ![前](04-dex-menu-before.png) | ![后](04-dex-menu-after.png) |
| 背包数量 | ![前](05-bag-quantity-before.png) | ![后](05-bag-quantity-after.png) |
| 商店数量与金额 | ![前](06-mart-quantity-before.png) | ![后](06-mart-quantity-after.png) |
| 保存确认 | ![前](07-save-before.png) | ![后](07-save-after.png) |
| 战斗菜单 | ![前](08-battle-menu-before.png) | ![后](08-battle-menu-after.png) |
| 主菜单 | ![前](09-main-before.png) | ![后](09-main-after.png) |
| PC 盒子列表 | ![前](10-pc-boxes-before.png) | ![后](10-pc-boxes-after.png) |
| 商店确认 | ![前](11-mart-confirm-before.png) | ![后](11-mart-confirm-after.png) |
| PC 数量框 | ![前](12-pc-quantity-before.png) | ![后](12-pc-quantity-after.png) |
| PC 放生提示 | ![前](13-pc-release-before.png) | ![后](13-pc-release-after.png) |
| PC 图鉴评价 | ![前](14-pc-rating-before.png) | ![后](14-pc-rating-after.png) |
| 商店购买列表金额 | ![前](15-mart-buy-before.png) | ![后](15-mart-buy-after.png) |
| 商店首页金额 | ![前](16-mart-main-before.png) | ![后](16-mart-main-after.png) |

## 复现

```sh
cargo run --release -p pokered-app --example ui_audit_capture -- /tmp/menu-capture
cargo run --release -p pokered-app --example pc_tour -- /tmp/pc-capture zh
```

对比基准时将本 PR 的截图工具复制到基准 checkout：`ui_audit_capture.rs` 与 `pc_tour.rs`，再运行相同命令。主菜单基准另由 `screenshot --screen main-menu --lang zh -f 10` 截取（相同静态菜单）。

常用菜单使用六只 Lv100 宝可梦、99 个道具和 999999 金钱；战斗推进到 PlayerMenu，PC 沿用原 pc_tour 示例数据。脚本额外验证设置项全部位置、“否”选项、99 个道具的总价、第 12 个盒子、满盒末尾的取消项。

英语检查：`ui_audit_capture -- /tmp/menu-en en` 和 `pc_tour -- /tmp/pc-en en`。中文以外仍保留原来的文字间距；背包数量、商店数量框和 PC 数量框的越界修复适用于两种语言。