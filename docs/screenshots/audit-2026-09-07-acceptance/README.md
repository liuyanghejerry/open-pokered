# 审计验收截图

基线：远端 master `0c32b9d`，在独立 worktree 编译。前后均执行同一份 `capture-acceptance.rs`，固定状态和帧数，分别调用 app / TUI 的实际渲染器；没有使用改图或示意图。

复现命令（在对应 checkout 执行，base checkout 仅复制截图入口和共享 fixture）：

```sh
cargo run -p pokered-app --example capture_audit_acceptance -- /tmp/app-shots
cargo run -p pokered-tui --example capture_audit_acceptance -- /tmp/tui-shots
```

每个文件的 SHA-256 见 `manifest.json`。`empty-party-before/after.png` 是另附的真实新游戏按键复现，before 使用合入 master 后、空队伍修复前的二进制；正式 master 对比为下表的 `app-empty-party-*`。

## app

| 画面 | 前（master） | 后 |
|---|---|---|
| 红莲门禁初始化 | ![前](app-gates-before.png) | ![后](app-gates-after.png) |
| 红莲第一门原版旗标 | ![前](app-quiz-solved-before.png) | ![后](app-quiz-solved-after.png) |
| 闪电鸟旧档恢复 | ![前](app-zapdos-reload-before.png) | ![后](app-zapdos-reload-after.png) |
| 急冻鸟旧档恢复 | ![前](app-articuno-reload-before.png) | ![后](app-articuno-reload-after.png) |
| 火焰鸟旧档恢复 | ![前](app-moltres-reload-before.png) | ![后](app-moltres-reload-after.png) |
| 二层落石初始隐藏 | ![前](app-boulder-before-drop-before.png) | ![后](app-boulder-before-drop-after.png) |
| 三层落石后隐藏 | ![前](app-boulder-after-drop-before.png) | ![后](app-boulder-after-drop-after.png) |
| 希巴未战出口 | ![前](app-bruno-door-before.png) | ![后](app-bruno-door-after.png) |
| 饮料显示名 | ![前](app-drinks-before.png) | ![后](app-drinks-after.png) |
| 长背包滚动 | ![前](app-bag-before.png) | ![后](app-bag-after.png) |
| 遗忘招式框 | ![前](app-move-choice-before.png) | ![后](app-move-choice-after.png) |
| 狩猎地带信息框 | ![前](app-safari-before.png) | ![后](app-safari-after.png) |
| 名人堂嘟嘟双属性 | ![前](app-hof-Doduo-before.png) | ![后](app-hof-Doduo-after.png) |
| 名人堂拉普拉斯双属性 | ![前](app-hof-Lapras-before.png) | ![后](app-hof-Lapras-after.png) |
| 片尾 POKéMON | ![前](app-credits-before.png) | ![后](app-credits-after.png) |
| CUT 后树消失 | ![前](app-cut-tree-before.png) | ![后](app-cut-tree-after.png) |
| 正辉入机器 | ![前](app-bill-machine-before.png) | ![后](app-bill-machine-after.png) |
| 领取初始宝可梦前不中毒全灭 | ![前](app-empty-party-before.png) | ![后](app-empty-party-after.png) |

## tui

| 画面 | 前（master） | 后 |
|---|---|---|
| 红莲门禁初始化 | ![前](tui-gates-before.png) | ![后](tui-gates-after.png) |
| 红莲第一门原版旗标 | ![前](tui-quiz-solved-before.png) | ![后](tui-quiz-solved-after.png) |
| 闪电鸟旧档恢复 | ![前](tui-zapdos-reload-before.png) | ![后](tui-zapdos-reload-after.png) |
| 急冻鸟旧档恢复 | ![前](tui-articuno-reload-before.png) | ![后](tui-articuno-reload-after.png) |
| 火焰鸟旧档恢复 | ![前](tui-moltres-reload-before.png) | ![后](tui-moltres-reload-after.png) |
| 二层落石初始隐藏 | ![前](tui-boulder-before-drop-before.png) | ![后](tui-boulder-before-drop-after.png) |
| 三层落石后隐藏 | ![前](tui-boulder-after-drop-before.png) | ![后](tui-boulder-after-drop-after.png) |
| 希巴未战出口 | ![前](tui-bruno-door-before.png) | ![后](tui-bruno-door-after.png) |
| 饮料显示名 | ![前](tui-drinks-before.png) | ![后](tui-drinks-after.png) |
| 长背包滚动 | ![前](tui-bag-before.png) | ![后](tui-bag-after.png) |
| 遗忘招式框 | ![前](tui-move-choice-before.png) | ![后](tui-move-choice-after.png) |
| 狩猎地带信息框 | ![前](tui-safari-before.png) | ![后](tui-safari-after.png) |
| 名人堂嘟嘟双属性 | ![前](tui-hof-Doduo-before.png) | ![后](tui-hof-Doduo-after.png) |
| 名人堂拉普拉斯双属性 | ![前](tui-hof-Lapras-before.png) | ![后](tui-hof-Lapras-after.png) |
| 片尾 POKéMON | ![前](tui-credits-before.png) | ![后](tui-credits-after.png) |
| CUT 后树消失 | ![前](tui-cut-tree-before.png) | ![后](tui-cut-tree-after.png) |
| 正辉入机器 | ![前](tui-bill-machine-before.png) | ![后](tui-bill-machine-after.png) |
| 领取初始宝可梦前不中毒全灭 | ![前](tui-empty-party-before.png) | ![后](tui-empty-party-after.png) |
