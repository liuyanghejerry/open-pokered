# 胜利之路推石：相位控制与启动耗时复核

本次没有修改生产游戏逻辑。3F 旧记录的 33 帧差异可以由输入前等待相位完整重现，不能据此增加固定延迟；2F 第二机关仍有启动耗时差距，不能因为其中一组相位画面相同而判定通过。

生产代码基线为 master `a2f1189ceb4074937222b9257267fd79348dff05`。原作是未改动的 retail Red，pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`，ROM SHA256 `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`。原作使用保留的 PyBoy 源码提供器，状态格式版本 29；安装的 2.7.0 wheel 仅供应依赖，不能载入这份状态，不是实际模拟器。身份信息见 `*-identity.json`。

## 3F：旧“晚一帧”结论应撤回

离线 SRAM 放置玩家于 3F (21,15)，原生程序正常启动、CONTINUE，通过队伍菜单实际使用 Strength。没有运行时设值、warp、skip_dialogue 或 restore。石块是地图原有的 (22,15)。窗口按 Right 16 帧、松开 184 帧，包含触发前一帧，共 201 帧。

原作在窗口前载入历史受控状态；原状态的地图、玩家、视图指针经过测试布置，不能算正常旅行证明。新增录制窗口只有按键与只读观察，没有运行时布置。

| 输入前额外等待 | 两侧落洞旗标首次置位 | 连续 201 帧 RGB | 玩家/石块坐标及旗标序列 |
| --- | --- | --- | --- |
| 0 帧 | t72 | 全部相同 | 全部相同 |
| 1 帧 | t73 | 全部相同 | 全部相同 |

各设置双方均重复两次，同一设置的完整记录一致。额外等待一帧的原生图像也逐帧重现了历史失败记录。这里撤销的是这条旧差异的证据，未宣告整个 3F、音频、按键锁定或正常旅行通过。

## 2F：匹配画面的相位可以掩盖另一处差距

受控测试把玩家置于 (9,14)、第 11 个对象石块置于 (9,15)，两侧都实际执行 Strength 菜单。原生复用现有 ignored 测试的地图加载和对象布置，属于单元夹具，不是普通 CLI 旅行证明。原作同样来自历史受控状态。

旧参考脚本的释放条件是 `t == player_y + 5`，本场景实际按住 Down **19** 帧，旧说明写成了 16 帧。本次两侧显式使用 Down 19 帧、松开 181 帧；每次 201 个连续帧。修复录制工具的菜单等待后，原生两个相位各重复两次；原作额外等待 0 至 5 帧，各重复两次。

完整比较矩阵如下。所有行使用原始时间轴，没有移帧、丢帧、缩放或相位重采样。

| 原生额外等待 | 原作额外等待 | 不同 RGB 帧数 | 原生/原作机关旗标时刻 |
| --- | --- | --- | --- |
| 0 | 0 | 27 / 201 | t7 / t8 |
| 0 | 1 | 0 / 201 | t7 / t7 |
| 1 | 0 | 50 / 201 | t6 / t8 |
| 1 | 1 | 27 / 201 | t6 / t7 |

原作等待 2、4 帧重现等待 0 的时序；等待 3、5 帧重现等待 1 的时序。原生同一设置的完整 JSON 和 PNG 各两遍一致。不能仅挑中间那一组吻合画面，便撤销 2F 差距。

原作首次 `hJoyHeld` 读到 Down 分别是 t1 / t0，原生记录的 `moving_direction` 首次变为 Down 同样是 t1 / t0。对应两组中，原生石块逻辑坐标提交为 t5 / t4，原作为 t6 / t5；机关旗标也是原生提前一帧。

新增只读例程入口轨迹，各相位重复两次：

| 原作额外等待 | MoveSprite | UpdateSprites 开始 | VBlank 进入 | UpdateSprites 返回 | TryWalking 提交石块坐标 |
| --- | --- | --- | --- | --- | --- |
| 0 | t3, LY58 | t3, LY108 | t3, LY144 | t4, LY68 | t6, LY127 |
| 1 | t2, LY58 | t2, LY108 | t2, LY144 | t3, LY68 | t5, LY127 |

这证明该原作场景的精灵更新跨过 VBlank。当前 `advance_boulder_push` 在建立推石状态后固定第 2 帧提交坐标，没有表示这段额外启动耗时。因果修复仍待实现；不能按地图名称硬加一帧，也不能据此宣称其他精灵数量、位置和 IRQ 负载已被覆盖。

## 证据与复现

- `3f-native-*.json.gz`：普通原生启动、菜单按键、逐帧完整状态/旗标/NPC、图像 SHA256，以及退出结果。
- `2f-native-*.json.gz`：受控单元夹具的完整连续状态与输入位。
- `*-original-*.json.gz`：原作连续状态、原始 RGB SHA256、输入、石块坐标及事件旗标。
- `2f-comparison.json`、`3f-comparison.json`：完整比较结果；2F 包括全部逐帧差异像素数。
- `2f-source-startup-trace.json`：独立只读例程入口轨迹，包含模拟器周期数。
- `raw-frames.tar.gz`：双方两个主要相位首遍的全部 1608 个 PNG 映射；按文件 SHA256 去重为 118 个 PNG。`index.json` 把每一个原始帧文件名映射到 `png/<sha256>.png`。只去重存储，不删减或复用时间轴上的帧。
- `SHA256SUMS`：压缩证据文件校验值。

2F 参考状态和准备脚本仍保存在旧草稿的 [victory-gates-raw.zip](https://github.com/liuyanghejerry/open-pokered/blob/fb013e5e68fa846a4936cb054cfdb9c69daf4bbd/docs/screenshots/fidelity-93-95/victory-gates-raw.zip)，以提交固定，避免删除分支后失效。使用原作文件必须核对上述 ROM 和状态哈希。

原生录制使用该提交的 `capture_boulder_routes_731`：

```sh
FIDELITY_DUST_SRAM=/absolute/path/to/seafoam-valid.sav \
FIDELITY_DUST_CAPTURE=/absolute/path/to/empty-output \
FIDELITY_DUST_SCENARIO=victory-switch2f2 \
FIDELITY_DUST_HOLD_FRAMES=19 \
FIDELITY_DUST_IDLE_EXTRA=0 \
cargo test -p pokered-app --features debug-server --lib capture_boulder_routes_731 -- --ignored --test-threads=1
```

再以 `FIDELITY_DUST_IDLE_EXTRA=1` 重复，每种至少两次。确认实际运行并通过 1 项测试，输出连续 201 帧；不能把普通测试中 ignored 或 0 tests 算作录制成功。默认按住时长仍为 16 帧，保留其他历史场景的输入约定。

本次本地应用库回归：397 通过、0 失败、40 ignored；另有 4 次显式受控录制通过。早期普通 CLI 准备未到目标位置、错误的观测字段访问，以及旧录制工具在菜单输入等待处失败的尝试，均未计入结果。最新主分支的全流程通关和范围外场景仍待独立验收。
