# 开场性能与战斗出招顺序

## 开场

比较对象是上一轮已修复地图切换、已写入 SD 卡的版本。mGBA 使用与上一轮相同的慢速 ROM 总线配置（4/2 wait states、无 prefetch），PSG 音频开启。此次完整播放开场，未跳过耿鬼动画或标题侧滑。

| 场景 | 修改前有效 FPS | 修改后有效 FPS |
| --- | ---: | ---: |
| 耿鬼 / 尼多力诺 | 29.47 | 58.62 |
| 标题 Logo 入场 | 26.72 | 47.23 |
| Red Version 侧滑 | 27.40 | 59.73 |

FPS 由逻辑帧之间实际经过的 VBlank 数计算（GBA 刷新率约 59.73 Hz），不是桌面执行速度。794 个匹配逻辑帧的 RGB 哈希全部相同。`comparison.json` 保存统计，`opening-{before,after}.log` 保存逐帧哈希和 CPU 计时。

耿鬼热路径原本内联在 ROM 中；现在每个姿态组合成连续索引图，再通过 IWRAM 中的绘制函数复制。缓存离开开场即释放。标题只重绘正在移动的 Logo / 版本文字区域。IWRAM 保留原来的 8 KiB 启动 / IRQ 栈预算；计时构建的代码末端为 `0x03005c4c`。

这不是实机复测结论：首次载入仍有较长帧（耿鬼段最大 5 个 VBlank），Logo 入场也尚未全程达到 60 FPS。SuperFW / 存储卡的额外开销仍需插卡后确认。

同帧截图位于 `docs/screenshots/gba-opening-battle-order/`，分别为 gengar、logo、version 的 before / after。它们刻意保持像素一致，性能差异需查看逐帧时序证据。

复测方式：

```sh
cd crates/pokered-gba
cargo +nightly-2025-12-07 build --release --features opening-timing
agb-gbafix target/thumbv4t-none-eabi/release/pokered-gba -o /tmp/opening.gba
cd ../..
python3 scripts/gba_frame_timing.py --suite opening --slow-cart \
  --rom /tmp/opening.gba --log /tmp/opening.log --output /tmp/opening.json
```

## 战斗

旧实现提前把整回合最终 HP 同步到 HUD，视觉动画则从当前文本页猜测招式。长敌方名字或本地化分页会破坏识别；连续推进文本也能覆盖还没结束的动画。

新增展示队列保留回合事件中的招式、攻击方与分步 HP，在本地化、分页前绑定提示。前端等待本次招式及受击反馈结束，才启动对应 HP 动画；扣血结束后才允许推进下一页。中毒、灼伤、寄生种子及无法行动的提示也作为独立边界。计算引擎仍然一次性确定回合结果，改变的是玩家看到的展示顺序。

另修复了动画解释器已结束时，受击反馈等待计时不再递减的问题。否则新的顺序等待会永远卡住。

验证包括：

- 实际 `PokemonGame.update` 路径：双方先手顺序、中英文、动画开 / 关、连续按 A；两次伤害不提前显示，双方动作均能完成。
- 核心测试：道具后的敌方免费回合、中毒独立扣血、一击击倒不播放已倒下对手的招式。
- 十种招式 × 双方的动画及受击反馈最终可完成。
- 开机到首次劲敌战的真实输入流程通过（`playthrough.log`）。
- GBA 内存压力测试 13 项通过，包括所有招式、地图、满 PC / SRAM；最大 EWRAM 栈使用 15,504 字节（`memory-results.log`）。

当前展示握手由本地 GUI / GBA 前端启用；纯核心调用仍保持原有驱动方式，联机战斗驱动未改为这一展示队列。
