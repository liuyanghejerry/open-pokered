# 推石与灰尘（93、95）

基线 master `31b1eda3112514d6bc803af559ec7f7c9e20374f`。原作 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`，零改动 retail Red，ROM SHA1 `ea9bcae617fdf159b045185467ae58b2e4a48b9a`，PyBoy 2.8.1，160×144、DMG RGB 255/170/85/0。

## 玩家能看到的修复

原实现把石块立即移动一格，立即播放灰尘，随后玩家提前走进石块刚离开的格子。现在先执行被阻挡的石块滑动，再播放八段灰尘，恢复图形后才让玩家重新操作。推石时移动、对话和菜单输入被阻挡；完成提示音只播放一次。调试接口的 control_ready 同样等待整段操作完成，避免自动导航提前继续。海沫岛石块落洞的隐藏与事件写入在灰尘结束后发生，保存事件后下层的对应石块仍出现。冠军之路开门和上下层石块事件保持可用。

灰尘使用原始 OAM 到 LCD 的 X−8、Y−16 转换；四块烟雾都移动。修正 OBP1 `$e4` / `$80` 闪烁调色板，OBP0 仍为 `$d0`。保留原作向下烟雾的特殊裁剪：`AdjustOAMBlockYPos` 在 Y≥112 时误写上一项属性为 `$a0`、将当前 Y 放到160；这会隐藏底部两块，并让右上块使用 OBP0、X 翻转和背景优先级。

DMG 精灵按较小的原始 X 优先，X 相同才按 OAM 顺序。横向烟雾不能一律画在石块后面。只为最多256个烟雾像素保存候选值，再按玩家/NPC 的不透明像素与 X 决定遮挡，不分配整屏优先级缓冲。

## 原作顺序和实测时间轴

依据 `engine/overworld/push_boulder.asm` 的 `TryPushingBoulder`、`MoveSprite`、`DoBoulderDustAnimation`；`engine/overworld/dust_smoke.asm`；`home/oam.asm WriteOAMBlock`；`engine/battle/animations.asm AdjustOAMBlockYPos`。

在四个受控场景中，t0 为真正向石块按下方向键：石块 OAM 从 t8 开始每两帧移动一个像素，t38 达到16像素；实际 LCD 从 t9 到 t39 显示这些位置。灰尘例程 t41 开始，t45 写 OAM 并闪烁；位置在下一帧 LCD 显示，八段每段三帧，最后显示到 t70。调色板寄存器立即生效，不能和 OAM 一起延迟一帧。t75 结束恢复和阻挡。

## 夹具和录制

[夹具](screenshots/fidelity-93-95/fixture.sav) 来自原作真实 SAVE。准备过程用原作已有的脚本 warp 进入海沫岛1F，再实际行走到 (18,9)、SAVE；未修改 ROM。原作准备时启用 NO_BATTLES，native 设置 encounter cooldown255 和 RNG0，均为排除随机遭遇的受控夹具，不能据此证明正常随机遭遇的概率。

原作和 native 都实际经过标题 Continue、队伍第一只宝可梦的 STRENGTH 菜单与对话。再实际绕行到向上 (18,11)、向左 (19,10)、向右 (17,10) 的起点。转身准备 native 按一帧、原作按两帧，以避免当前尚未修复的普通转向时序提前推石；之后两边均空走120帧。判定窗口的输入完全相同：对应方向按住 t0..15、t16 释放，一帧一图记录 t−1..199。

master 检出仅加入与修复版字节相同的只读录制函数，没有生产修改。每个版本、每个方向各录两遍；原作、master、修复版的连续 frame counter 均验证每次加1，两遍 PNG 和状态 JSON 相同。完整图像、状态和原作 OAM/调色板保存在三个 raw.zip；[manifest](screenshots/fidelity-93-95/manifest.json) 记录逐帧 SHA256、差异像素和夹具哈希。

| 场景 | 连续帧数 | 修复后与原作不同 RGB 像素 | 重复录制 | 判定 |
| --- | --- | --- | --- | --- |
| 向下推石 | 201 | 每帧0 | 两遍相同 | 此固定场景 PASS |
| 向上推石 | 201 | 每帧0 | 两遍相同 | 此固定场景 PASS |
| 向左推石 | 201 | 每帧0 | 两遍相同 | 此固定场景 PASS |
| 向右推石 | 201 | 每帧0 | 两遍相同 | 此固定场景 PASS |

窗口内玩家保持在起点，地图不变；石块最终在对应的一格之后。没有缩放时间轴、丢帧或重新取样。整段图像含所有地图、玩家、NPC 和烟雾像素，不只比较某个区域。原作八阶段、四方向的硬件 OAM 数字另作为独立回归 oracle，覆盖128个 OAM 项。

## 验证边界

上述 PASS 只覆盖四个不落洞的隔离推石场景。海沫岛落洞、冠军之路开门/落洞使用核心功能回归验证事件和最终状态，没有完成同场景原作连续录制，时间轴仍为 PARTIAL。音效只验证请求顺序与完成音一次，未比较原作音频波形。普通行走、骑车、转向、自由 NPC 移动时钟仍有独立差异；本 PR 不宣告整个地图系统或其他动画通过。

保留了先前失败结果：只修改 SRAM 地图/坐标而没有加载地图的夹具画面损坏，已拒绝；首版把 OAM 和调色板都延迟一帧，向下有5个不同帧；按 OAM 顺序直接绘制时，横向各有21个不同帧。这些结果没有计入 PASS。

回归：2709 项核心测试、195 项带 debug-server 的应用测试，以及495项未优化应用全目标测试（48套）通过。推石期间全部8键的核心阻挡、结束提示音唯一性、落洞后的下层事件、冠军之路开门/上下层保存重载均有检查。字体、中文、拼音输入和对话框布局按目标排除。

## native 复现

`cargo test -p pokered-app --features debug-server --lib --no-run`，对生成的测试执行文件运行 `capture_actual_boulder_dust_raw_93 --ignored`。设置 `FIDELITY_DUST_SRAM` 指向夹具，`FIDELITY_DUST_CAPTURE` 指向空输出目录，`FIDELITY_DUST_DIRECTION` 为 down/up/left/right。确认实际执行1项测试；仅看到0 tests不能作为录制证据。
