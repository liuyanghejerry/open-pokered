# Game Freak 开场大小星星修复

基线：`c380a0b`（PR #108 合入后的 master）。此修复覆盖 native 与 TUI 的星星绘制，以及共享核心的阶段起始位置和闪烁相位。

## 原版依据

考据使用 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`：

- [`LoadShootingStarGraphics`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/movie/splash.asm)：大流星的左上、左下分别来自 `MoveAnimationTiles1` 的图块 3、19；初始 OBP1 为 `$a4`。
- 同文件 `GameFreakShootingStarOAMData`：四个 OAM 都选择 PAL1，右上、右下使用 XFLIP。
- 同文件 `MoveDownSmallStars`：OBP1 每步异或 `$a0`，在 `$a4` / `$04` 间切换；图块中的上方星点保持颜色 1，下方星点变为白色。
- 同文件 `SmallStarsOAM`：PRIO | PAL1，底部黑边遮住下落的小星。大流星没有 PRIO，可跨过黑边。
- [`MoveAnimationTiles1`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/animations.asm)：实际资源为 `gfx/battle/move_anim_1.2bpp`，仓库现有 PNG 与加载接口已可使用，无需新增资产。

两个前端改用同一 `pokered_renderer::gamefreak_stars` 实现，移除重复的小星图块拼接与整块隐藏逻辑。

## 等待帧顺序校正

原版在每次 `CheckForUserInterruption` 的等待前，先执行坐标或调色板更新；阶段入口对应第一次等待期间的画面。原来的核心先暴露未更新的初始状态，导致位置/相位滞后：

| 阶段 | 原版第一次等待 | 后续与末尾 |
| --- | --- | --- |
| 大流星 | 从 OAM `(160,0)` 先移动到 `(156,4)` | 每帧再移动 4px，最后一次等待为 `(0,160)`，共 40 帧 |
| Logo | `$f9` 先右旋两位为 `$7e` | `$7e` / `$9f` / `$e7` 各等 10 帧，末值 `$e7` 保留到小星、PostDelay；源码没有恢复指令 |
| 小星 | 从 OAM Y `$68` 先递增到 `$69`，OBP1 先切到 `$04` | 每 3 帧再递增 1px、切换一次；每波 8 次，共 24 帧，6 波合计 144 帧 |

核心现在保存当前 Logo 调色板，跳过动画时保留实际值，reset 时恢复 `$f9`。核心阶段总时长仍为 180+64+40+30+144+40 = 498 帧，音效一次性请求和跳过行为保持原有接口。版权和字标字体不在此修复范围。

## 验证

- `cargo test --locked -p pokered-core`：2925 passed，1 ignored（既有文档示例）。

- `cargo test --locked -p pokered-renderer`：389 passed。
- `cargo test --locked -p pokered-app --lib --features debug-server`：111 passed。
- `cargo test --locked -p pokered-tui`：23 passed，1 ignored（旧 FLY 截图输出测试）。
- 新增完整等待帧参考：按汇编执行顺序逐次改变坐标/调色板，再重复对应等待帧；覆盖全部 498 帧，核对阶段、Logo、大小星位置和闪烁。另测跳过后调色板保留与 reset。
- 新增像素参考测试逐帧覆盖大流星的 40 帧、小星的 144 帧及 40 帧 PostDelay。参考路径独立解码图块颜色、镜像、裁剪、背景遮挡，不调用生产 blit 或调色板转换。
- 原生生产绘制函数截图测试在基线和修复后均通过，固定开场总帧 258 / 284 / 314 / 317 / 338 / 341：大流星 / Logo 首次闪烁 / 小星入口 / 下一步 / 第二波第一步 / 第二波第二步。

截图复现（以 phase/frame 从 0 开始计数）：

```sh
PR_SCREENSHOTS=/tmp/gamefreak-stars cargo test --locked -p pokered-app \
  --test visual_verify_gamefreak_stars -- --ignored
```

截图保存于 `docs/screenshots/gamefreak-stars/`，每个状态均有 `before` / `after`。基线源码（core + native splash）与最终候选源码使用同一扩展截图夹具捕获；状态与帧数一致。本次相位修正前的首次 PR 提交为 `258b26d`；PR 展示的全部 before 都来自 master 基线。

这是原版源码与图块数据驱动的绘制验证，没有进行原版 ROM 的逐帧差分，也不代表整个开场电影的时序已完成审计。
