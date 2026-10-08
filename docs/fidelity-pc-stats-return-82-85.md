# PC 状态页返回与 Continue 朝向（82 / 85）

基线 `0c5d8c9adf32c9054ef48e4cb0d3c002bea93b18`（master / PR #139）。实现提交见截图 manifest。原作使用 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c` 的零改动 retail Red，ROM SHA1 `ea9bcae617fdf159b045185467ae58b2e4a48b9a`；PyBoy 2.8.1，160×144，显式 DMG RGB 调色板 255/170/85/0。

## 实际入口与修复

夹具来自原作实际操作：飞翔到常青市，走进中心，在电脑前 `(13,4)` 将 90 级椰蛋树寄存，然后通过游戏内 SAVE 保存。队伍剩五只，第一只为 5 级梦幻、只有 Pound；箱 1 为椰蛋树、四个秘传招式。原作与 native 都从同一 [SRAM](screenshots/fidelity-82-85/fixture.sav) 正常经过标题菜单选择 Continue；native 没有跳过介绍或直接设置画面。然后实际面朝上打开电脑，依次进入 Bill PC、DEPOSIT/ WITHDRAW、精灵操作菜单、STATS。

- **82（电脑返回部分）**：`StatusScreen2` 等到 A/B 后将音量恢复 `$77`、调用 `GBPalWhiteOut` / `ClearScreen`。电脑调用 `LoadScreenTilesFromBuffer1` / `ReloadTilesetTilePatterns` / `RunDefaultPaletteCommand` / `LoadGBPal` 才返回菜单。复刻原来立即显示、接受操作；现在恢复清屏期间的按键锁定和背景复原。
- **85**：原作 `main_menu.asm .pressedA` 设置 `PLAYER_DIR_DOWN`，`SpecialEnterMap` 通过 `ResetPlayerSpriteData` 重置玩家朝向。复刻误把保存的移动方向字节按精灵朝向值解读，原作朝上存档的 `$08` 被读取成朝左。正常 Continue 现在朝下；地图、位置、保存的队伍及原始数据保持不变。

## 连续录制与量化

STATS、翻页和退出均按住相应按键两个原始帧，第三帧释放，每次只推进一帧；退出记录 `t=-1..99`，没有缩放时间。队伍 / 箱子各录两次 reference、master、修复版；所有退出 PNG / JSON 及 native Continue 图与 metadata 的重复 SHA256 一致。master 检出仅增加与修复版字节相同的只读录制函数，没有生产修改。程序 / SRAM / 录制函数哈希及每个触发的绝对游戏帧见 [manifest](screenshots/fidelity-82-85/manifest.json)。

| 通道 | 原作 | master | 修复后 | 结论范围 |
| --- | --- | --- | --- | --- |
| 退出清屏 | 完整白屏 `t=1..6` | 立即显示操作菜单 | `t=1..6` 每个 RGB 像素一致 | 此窗口 PASS |
| 菜单输入 | `HandleMenuInput` 首次进入 `t=6` | 立即接受 | 实际输入回归确认早期 A/B 丢弃，`t=6` B 可返回列表 | 此输入边界 PASS |
| 音量 NR50 | `t=-1` 为 `$33`，退出开始变 `$77` | 同原作 | `t=-1..99` 数值全部一致 | 此寄存器 PASS |
| Continue | `(13,4)` 朝下，方向输入之前捕获 | `(13,4)` 朝左 | 同一 native 游戏帧 `570` 朝下 | 此状态 PASS |

第 7 帧恢复 PC 上、下两段背景，中间第三段在第 8 帧完成；保留画面渲染和每帧完整重绘回归一致。新增回归真实经过标题/Continue 菜单，覆盖原作方向字节 `1/2/4/8` 及旧格式 `0/12`；电脑队伍、箱子两个入口都验证清屏期按键不误触、帧 6 恢复输入、存储不被改写、重开 STATS 不保留旧清屏状态。193 项调试应用测试、482 项未优化应用全目标测试（48 套）通过。

## 尚未宣告对齐的部分

整段状态页动画仍为 **PARTIAL**。退出第 0 帧原作会因扫描线位置保留上部旧画面的片段，本次采用整帧清白，尚未复现该片段。普通队伍列表返回的载入时长、电脑队伍与箱子首次进入的时序、招式页按不同招式/等级构建及接受输入的时序仍在后续审计范围。字体与对话框布局按用户要求排除；没有把完整画面、PCM 或全部入口判成 PASS。

排除了三种无效推断：跳过介绍的构造入口不是实际 Continue；原先名为 `pc-saved-continued.state` 的状态实际上已按过 Up，不能证明 Continue 朝向；只改变 loadedMoves 的早期矩阵未同步 GetMaxPP 实际读取的队伍/箱子招式，不能推算任意招式的等待时间。

## 复现

在本提交构建 `cargo test -p pokered-app --features debug-server --lib --no-run`，保留生成的测试执行文件，然后设置 `FIDELITY_PC_REFERENCE_SRAM` 为夹具路径、`FIDELITY_STATS_TRANSITIONS` 为输出目录，执行测试文件的 `capture_stats_transitions_raw_80_82 --ignored`。这是读取自有测试夹具、真实推进菜单输入的录制入口；不会改写夹具，临时存档副本写在输出目录。

六个 `*-raw.zip` 保存两入口的连续原始退出图与状态记录；reference 包还包含原作 routine hook 时间。只有第一轮打包，第二轮以全文件哈希重复检查记录在 manifest。Continue 原作图是独立经过同一 SRAM 的正常菜单后、任何方向输入前的稳定画面；native 前后图均为相同输入和帧 570，不将原作与 native 启动时长视为相同。
