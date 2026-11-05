# 图鉴／训练家卡片退出恢复

**功能回归验证通过；完整动画对齐仍为 PARTIAL。** 退出后不再立即接受 START 方向键。图鉴执行白屏和地图重载，训练家卡片先重载字体，再重建地图。一次发生在恢复期间的 Down 不会把 POKéDEX 移到 POKéMON，或把训练家资料移到 SAVE。

| B 触发后的连续帧 | 原作 | 修复前 | 修复后 |
| --- | --- | --- | --- |
| 图鉴：首次 START 输入读取／就绪 | 14 | 0 | 12 |
| 训练家卡片：首次 START 输入读取／就绪 | 24 | 0 | 23 |
| 图鉴：整帧全白 | 1–9 | 无 | 1–8 |
| 训练家卡片：整帧全白 | 1–21 | 无 | 1–19 |

图鉴仍早 2 帧恢复输入，训练家卡片仍早 1 帧；白屏窗口也有差异。没有用固定总帧数补齐这些差距。GBA 的物理耗时未与原作对齐。本证据不关闭 #144 的剩余时序项。

实现对应 `ShowPokedexMenu.exitPokedex`、`StartMenu_Pokedex`、`StartMenu_TrainerInfo` 和 `ReloadMapData`。地图重建实际执行 `LoadCurrentMapView/DrawTileBlock` 的读写和分支，原作这一窗口的 45,236 T-cycle、360 个可见图块及 ROM bank 恢复均有独立测试。字节传输仍受公共 LCD／IRQ／音频时钟控制；标题菜单、捕获后的图鉴和背包查看器沿用各自返回路径。

`LoadTilesetTilePatternData` 固定读取 1,536 字节。原作构建会裁掉 PNG 末尾空白，因此读取还包含后续 blockset；真新镇住宅的尾部跨到 House 图块，地下通道在第 672 字节进入活跃 VRAM。24 套资源范围均以原作 ROM 的 FNV-1a 校验，地下通道的可变部分通过实时显存总线读取，并测试了中途暂停／序列化后恢复。原作字体只作为传输数据，实际文本仍用 Fusion Pixel。

## 版本与输入

- 实现提交：`003abe25`，父提交 `72719a77`。父 master 与采集修复前画面的 `8655c144` 代码树相同。
- 原作：普通 Red，原作源码 `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`；ROM SHA-256 `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`。PyBoy 有声、逐帧实际按键，无录制期间 RAM／ROM 修改。
- 原作准备状态复用 [reference.state](../start-menu-close/fast-cancel/reference.state)，SHA-256 `b12777818a990843546b623615caa67a05a811941dfcf4d078b46a564d6e09a4`。
- 当前存档复用 [fixture.sav](../start-menu-close/fixture.sav)，SHA-256 `8e69cf22e2ed9c1a940512a135b0dae52bad662f43206978d13b85bfad777ced`。桌面与 GBA 均冷启动 CONTINUE，再实际打开 START 和目标菜单。
- 都在常青市 `ViridianCity (20,30)`。原作从准备好的队伍菜单返回 START；当前从 SRAM 开始。名字、地图和位置一致，字体／排版、存档金钱／游戏时间不同，不据此判断像素一致。
- 桌面候选 SHA-256 `81fc7c747eadc47a749800205fedb38c34723c271275c081f161b41df6e0e5fd`；默认 release GBA ROM `3db158dd58f00a391f547468d508e0e8acc4745601688d2dbfcae2d0ecee90f7`，无 autopilot 或跳帧。

图鉴／训练家卡片各有中立、短按两个场景，每个版本各跑两遍。桌面与原作每遍保留 160 个连续实际更新帧；B 在 t+0、1，短 Down 在 t+2、3。候选最后按 A 重新打开同一菜单。GBA 每遍保留 800 个连续物理帧，B 为 t+0…7，Down 为 t+20、21；每组完整记录重复一致，直接读回 cartridge SRAM 未变。

## 验证与复跑

- 应用完整测试：1,114 通过、92 已有忽略；核心：3,995 通过、1 已有忽略。
- 专项涵盖室外、真新镇住宅、地下通道、石英高原大厅，确认实际白屏、LCD 关闭、图块传输、最终菜单、短按保护及缓存／全量渲染相同。
- ARM 检查和默认 GBA 构建通过；7 组 GBA 性能门限通过，未改变门限或计时方式。
- fresh NEW GAME → m06：41.05 秒通过，没有存档跳关、完成标记或缩减通关断言。这是菜单改动的早期回归，不当作本提交的完整 m49 通关证明。

原始逐帧录制、压缩日志、运行报告与校验清单属于临时验证产物，不提交到仓库。仓库仅保留本说明、必要截图和复现脚本；复跑时将输出目录指定在仓库之外或 `.task-tmp/` 内，验证完成后清理。

`replay-native.py` 接收 `--binary --sha256 --output --candidate`；省略 `--candidate` 可录旧版。`replay-original.py` 接收 `--rom --symbols --output`，需要 PyBoy。`generate-map-view-oracle.py` 使用同样参数生成地图重建测试输入（`--output` 为 JSON 文件）。

`replay-gba.py` 接收 `--rom --sha256 --bridge --case --output`；case 为 `dex-neutral`、`dex-short`、`trainer-neutral`、`trainer-short`。bridge 复用 [mgba-sram-bridge.c](../submenu-return/mgba-sram-bridge.c)，编译、链接 mGBA 0.10.5 后使用。每次运行使用新的输出目录，重复两遍后比较全部帧及状态。fixture 和 ROM 保持只读，程序只写输出目录内的副本。
