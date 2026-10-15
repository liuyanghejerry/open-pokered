# Stats 页 PP Up 上限修复验证

基础 PP 为 40 的 Growl（叫声）使用一次 PP Up 后，能力页的招式页此前显示 `47/48`，修复后显示 `47/47`。原作每次 PP Up 的增量为 `min(base_pp / 5, 7)`（整数除法），因此三次的上限依次为 47、54、61。

依据：原作 `engine/menus/status_screen.asm` 的 `StatusScreen2` / `GetMaxPP`，以及 `engine/items/item_effects.asm` 的 `AddBonusPP`；对照版本 `pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。修复继续使用 `RenderData::move_pp` 提供的基础 PP，并保留已有的 PP Up 次数限制。

## 前后截图

同一 Pikachu（20 级，Growl 当前 PP 47，使用一次 PP Up），相同预启动 JSON 快照、seed 42 和 616 帧按键。通过正常开始菜单 → 队伍 → STATS → 招式页进入；在第 616 帧使用不推进游戏的 `capture_frame` 取图。

- 前：`stats-pp-up-1034-before.png`，Growl `47/48`。
- 后：`stats-pp-up-1034-after.png`，Growl `47/47`。
- 两张原始 160×144 PNG 只有分母末位数字的 12 个像素不同，区域为 `[132, 99, 136, 106)`。

前图保留构建 `2623c94543149ad4083b96746d28333726a6a6e5` 的原始身份；其全部 2176 个编译输入和 702 个图像资源与实际基线 `master@25b61c0599957f042cd64906a8e61f71053cc73b` 一致。后图构建为运行代码提交 `15615c5558faa4ea8da0e2d891a49ff329dd3d49`。文档和截图在随后独立提交，未改动编译输入。

两侧各实际执行 616 个单帧输入，padding 为 0；每侧保留 1248 组调试协议请求/响应。六处公开状态检查点完全一致，当前 PP、HP、招式、PP Up、物品、金钱和剧情标记保持不变。私有 SAVE 路径始终不存在，两侧各自的 Game 进程已结束。

这是 `--snapshot`、`--skip-intro`、`--warp ViridianForest,28,32` 控制场景下的实际 `Game.draw` 画面，不代表普通存档冷启动、原作逐帧时序或所有前端渲染路径均已验证。

## 自动验证

执行并通过：

```text
cargo test --offline --locked -j1 -p pokered-ui
cargo build --offline --locked -j1 --bin pokered-app --features debug-server
```

UI 测试共 120 项通过，其中新增三个实际调用 `stats::draw` 的测试覆盖：

1. Growl 的 0/1/2/3 次 PP Up 上限为 40/47/54/61。
2. 当前 PP 为 0 和多个招式槽各自的 PP Up 不互相影响，绘制不修改宝可梦数据。
3. 使用渲染数据提供的基础 PP，先按次取整、再相乘，并保留次数上限。

未重新执行其他历史测试矩阵。官方 CI 结果以本 PR 的实际提交为准。
