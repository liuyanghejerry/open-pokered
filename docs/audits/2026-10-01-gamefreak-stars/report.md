# Game Freak 开场大小星星修复

基线：`c380a0b`（PR #108 合入后的 master）。此修复覆盖 native 与 TUI 的星星绘制。

## 原版依据

考据使用 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`：

- [`LoadShootingStarGraphics`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/movie/splash.asm)：大流星的左上、左下分别来自 `MoveAnimationTiles1` 的图块 3、19；初始 OBP1 为 `$a4`。
- 同文件 `GameFreakShootingStarOAMData`：四个 OAM 都选择 PAL1，右上、右下使用 XFLIP。
- 同文件 `MoveDownSmallStars`：OBP1 每步异或 `$a0`，在 `$a4` / `$04` 间切换；图块中的上方星点保持颜色 1，下方星点变为白色。
- 同文件 `SmallStarsOAM`：PRIO | PAL1，底部黑边遮住下落的小星。大流星没有 PRIO，可跨过黑边。
- [`MoveAnimationTiles1`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/animations.asm)：实际资源为 `gfx/battle/move_anim_1.2bpp`，仓库现有 PNG 与加载接口已可使用，无需新增资产。

两个前端改用同一 `pokered_renderer::gamefreak_stars` 实现，移除重复的小星图块拼接与整块隐藏逻辑。版权、字标字体、Logo、核心阶段与坐标/闪烁节奏均未修改。

## 验证

- `cargo test --locked -p pokered-renderer`：389 passed。
- `cargo test --locked -p pokered-app --lib --features debug-server`：111 passed。
- `cargo test --locked -p pokered-tui`：23 passed，1 ignored（旧 FLY 截图输出测试）。
- `cargo test --locked -p pokered-core gamefreak_splash`：6 个单元测试及 1 个集成测试通过。
- 新增像素参考测试逐帧覆盖大流星的 40 帧、小星的 144 帧及 40 帧 PostDelay。参考路径独立解码图块颜色、镜像、裁剪、背景遮挡，不调用生产 blit 或调色板转换。
- 原生生产绘制函数截图测试在基线和修复后均通过，固定开场总帧 258 / 338 / 341：大流星 / 小星可见 / 小星闪烁。

截图复现（以 phase/frame 从 0 开始计数）：

```sh
PR_SCREENSHOTS=/tmp/gamefreak-stars cargo test --locked -p pokered-app \
  --test visual_verify_gamefreak_stars -- --ignored
```

截图保存于 `docs/screenshots/gamefreak-stars/`，每个状态均有 `before` / `after`。基线先加入同一截图夹具、捕获后再修改生产代码，状态与帧数一致。

这是原版源码与图块数据驱动的绘制验证，没有进行原版 ROM 的逐帧差分，也不代表整个开场电影的时序已完成审计。
