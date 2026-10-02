# GBA 性能门禁复现与修复

2026-10-02：本地复现 c3847df 的性能回退，并通过不改变原作像素的绘制优化，使现有 **7 个场景、31 项门禁全部通过**。性能基线、15% / 25 ticks 阈值、场景窗口、自动输入和计时脚本均未修改。

## 来源与验证范围

GitHub Actions run `37008297481` / job `110841683101` 的 benchmark ROM 构建通过，第 10 步 `Record and compare GBA hardware-timer baseline` 退出 1；后续步骤跳过。远端日志和 artifact 获取受 HTTP 403 限制，因此这里保存的是本地复现证据，不是远端任务的日志。

本地使用 CI 指定的 `nightly-2025-12-07`、`agb-gbafix 0.25.0`、`perf-benchmark`（包含 `autopilot` 与 `profiling`），以及 mGBA 0.10.5 的 Ubuntu noble 原发布包。发布包 SHA256 与工作流逐字一致：

```text
0bbf1e7ca511cd4b443239b97546f699df72211241a1db9177e331866031d8e9
```

mGBA 版本输出为 `0.10.5 (26b7884bc25a5933960f3cdcd98bac1ae14d42e2)`。使用同样的 Qt 前端、Xvfb、`SDL_AUDIODRIVER=dummy`、stdout 行缓冲和配置参数；本地直接启动 Xvfb，CI 通过 `xvfb-run` 启动。运行库隔离解包到 `/tmp`，没有修改系统库。Timer 2 以 GBA CPU 时钟 / 64 计时，不采用主机墙钟作为性能指标。

修复前构建时本地 HEAD 为 b19f3a1，Cargo、游戏 crates 和性能脚本相对 c3847df 没有生产差异。修复源与主分支映射：

| 内容 | 独立分支提交 | 主分支提交 |
| --- | --- | --- |
| 原作文本框批量填充、1 倍字体直接写像素 | ad8dadf | 479c409 |
| 不透明原版字形批量填充纸色 | 29115dd | 33b06d3 |
| 测试使用公开 TileRect 构造函数 | 7a5a354 | 0229a8f |

最终性能 ROM SHA256：`e69bd3322f29d88cc24ea62b87b1e29d4556aa149753eebee3f85f07b817004f`，保留在 `/tmp/pokered-gba-performance-opaque.gba`。其绘制生产源对应 33b06d3；之后的驱动和文档更改未纳入这个 ROM 的来源声明。完整来源、基线 checksum 和原 ROM checksum 见 [manifest.json](../gba-performance/manifest.json)。

## 实测结果

每项上限为 `baseline + max(baseline × 15%, 25 ticks)`。最初 `record` 成功取得全部 7 个场景，`compare` 退出 1，共 11 项超标；因此本地确认的是性能回退，没有观察到采样超时、缺场景或崩溃。

| 场景 | 基线 update / draw均值 / draw最大 | 修复前 | 最终 | 门禁 |
| --- | --- | --- | --- | --- |
| intro-title-v1 | 142 / 3823 / 14926 | 109 / 1155 / 10259 | 109 / 1156 / 10259 | 5/5 PASS |
| oak-dialogue-v1 | 167 / 2613 / 9336 | 436 / 19233 / 61601 | 192 / 2874 / 8697 | 5/5 PASS |
| overworld-idle-v1 | 333 / — / — | 341 / — / — | 342 / — / — | 1/1 PASS |
| overworld-movement-v1 | 343 / 252 / 2707 | 351 / 282 / 2984 | 352 / 282 / 2985 | 5/5 PASS |
| battle-entry-v1 | 192 / 2761 / 8898 | 301 / 9618 / 41703 | 196 / 2700 / 8914 | 5/5 PASS |
| trainer-battle-entry-v1 | 202 / 2454 / 10197 | 243 / 4581 / 42543 | 211 / 2427 / 10216 | 5/5 PASS |
| pokedex-entry-v1 | 93 / 44 / 17856 | 100 / 142 / 57120 | 96 / 39 / 15961 | 5/5 PASS |

绘制均值按整个采样窗口计，不是单次 render 均值。其余两个 gated 指标为 present 均值和最大值，全部通过。原始记录及逐项 31 项比较见 [修复前数据](../gba-performance/before-metrics.json)、[最终数据](../gba-performance/after-metrics.json)、[最终比较](../gba-performance/after-compare.log)；中间一次优化只剩 Oak 两项超标，亦保留其数据。

Oak 的 update 上限精确为 192.05 ticks，最终为 192；这项余量很小，应继续观察真实 CI，不把本地 PASS 声称为远端重跑 PASS。Oak render 次数为 307，既有基线为 272；标题时序、实际页面、字体和绘制失效状态可能改变固定窗口工作量，未将这一差别直接归因于新的核心算法。无需放宽窗口或阈值即可通过。

## 原因与修复

1. 原版不透明文本框原先逐格调用 `draw_glyph`，一个 20×6 框执行 7680 次像素写入，并反复量化空白纸色。修复对白底填充一次，再批量绘制原 $7A 的第 2、4、5 行和原 $7C 的第 2、4 列；四角继续使用原版位图。两侧相同 $7C、上下相同 $7A、角落和内部不透明白底均保留。
2. 原版英文字体在 1 倍大小时，每个墨点调用 1×1 `fill_rect`。修复直接使用 `set_pixel`，放大字体保留矩形路径。
3. 不透明字形逐像素绘制纸色仍增加角落和符号成本。修复先执行一次裁剪后的 8×8 纸色填充，再绘制墨点；纸色、任意墨色、透明色和出屏裁剪均保持同样的像素结果。

update 指标也随绘制优化下降，有明确的 GBA 路径：`pokered-audio/src/output_gba.rs:34–37` 的 VBlank IRQ 增加时钟；`99–106` 的音频更新按经过的硬件帧数循环执行 sequencer。`pokered-app/src/game.rs:3432–3433` 在游戏 update 内调用它，而 `pokered-gba/src/main.rs:917–935` 每轮最多执行一个逻辑 tick。慢 draw 跨越多个硬件 VBlank，会使下一次 update 补跑多次音频帧。因此最初的 update 超标不能直接证明战斗逻辑变慢；本次没有修改音频、逻辑 tick 或战斗机制。

## 像素等价与复现

主工作树默认功能测试实际通过（session 29087，exit 0）：renderer **397/397**、UI **12/12**，包括以下直接比较旧逐像素算法的矩阵：

- 真正的 FrameBufferPainter：440 个尺寸、位置、裁剪和颜色组合，包括最小框、无效小框、非 8 像素对齐视口和全出屏框。
- 原版字体：11520 个原 tile、裁剪位置、1/2/3 倍大小和颜色组合。
- 不透明原版字形：14400 个原 tile、裁剪位置、墨色（含透明色）及纸色组合。

证据见 [pixel-equivalence.log](../gba-performance/pixel-equivalence.log)。文本框 oracle 直接调用既有 Fusion 逐像素字形函数，避免新 helper 自证。

复现既有性能门禁：

```bash
cd crates/pokered-gba
CARGO_INCREMENTAL=0 cargo +nightly-2025-12-07 build --release --features perf-benchmark
agb-gbafix target/thumbv4t-none-eabi/release/pokered-gba -o /tmp/performance.gba
cd ../..
SDL_AUDIODRIVER=dummy MGBA_COMMAND='xvfb-run -a stdbuf -oL -eL mgba-qt' \
  python3 scripts/gba_performance.py record --rom /tmp/performance.gba \
  --output /tmp/performance.json --log /tmp/performance.log
python3 scripts/gba_performance.py compare \
  --baseline crates/pokered-gba/perf-baseline.json --candidate /tmp/performance.json
```

此次没有运行后续 Route 22 内存、广泛内存、帧节奏或慢 ROM 场景，也没有测试物理 GBA。结果仅覆盖这里列出的性能与像素等价门禁，后续工作流仍须单独完成。
