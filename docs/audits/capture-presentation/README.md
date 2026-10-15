# 捕球提示与音效顺序

普通战和 Safari 用球后应先完成整段球动画，再显示捕获或失败结果。成功提示中的捕获音效结束后才接受确认；离开战斗前保留成功文字。复刻曾在动画中途播放捕获音效，随后被球的 Tink 覆盖，并在 BattleOver 短暂显示 `You won!`。

修复使用实际球动画完成事件恢复用球文字的调用方，不增加固定等待时间。用球文字没有额外确认键；成功、失败和昵称菜单的确认仍由玩家输入。事件只消费一次，在读档、快照恢复和重置时清理。Safari 成功和失败投球都补上玩家使用 SAFARI BALL 的文字，成功时使用实际收到的宝可梦名字。

低 HP 的普通战维持原作行为：警报启用时，原作会拒绝整段捕获音效。原作 Safari 不绘制玩家 HP HUD，HP1 也应正常播放捕获音效；本修复只调整 Safari 的报警条件，没有改变通用音频管理器。

冻结基线为 `dfeefa3411634e187119862288ad78d5a3e933fe`，二进制 SHA-256 `6e96cc08d37b86a9489f5c515c591b7c226ac650290ab9fc7a5e1fbd0664701c`。修复生产源为 `fbed8302e50735b77e658b4650dde4531e9acbe4`，二进制 SHA-256 `5dfffecc541dc26d7435492e4cdbc49173834f0887d7047339f0341a564dcbec`。后续文档或合并提交不会重标这些实际捕获身份。

测试包含八种普通战/Safari、健康/HP1、入队/送箱组合，真实 Game/VFX/三音轨执行与 PCM 非静音检查；投球后不给用球页额外 A，验证动画结束会自动进入结果。另检查失败动画、普通存档/快照/标题重置的过期事件清理。完整 app 273 通过、39 忽略，core 2885 通过。

原作是本地 fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c 构建的受控预输入场景。原作记录了实际走路遇敌、用球、只读 PrintText/PlaySound 钩子、逐帧通道与截图、普通 SAVE；没有验证期间写战斗 RAM。Forest 场景预先配置十个合法 WEEDLE4 草地槽，保留 rate8；Safari 保留原自然野生表和遭遇率。原作和复刻的 Safari 物种、随机 DV 与捕获随机数不要求相同，失败投球保留。

证据清单、各实际控制终点与同输入同模拟帧截图见 `verification.json`、`screenshots.json`。`evidence.zip` 使用 SHA 内容去重，解包后逐文件保持原始字节；不包含 ROM、可执行文件或模拟器 state。使用 `python3 extract_evidence.py evidence.zip --verify-only` 核对，或 `python3 extract_evidence.py evidence.zip DIRECTORY` 解包。

范围限制：不声称整段动画逐像素、音频波形或硬件时钟与原作一致；CLI 的 `audio_sfx` 是逻辑音效状态，完整原生三通道与 PCM 检查由独立 Game 测试提供。Safari 的出场、玩家背影与错误玩家 HP 栏属于另一个已记录候选，本次未修复。主线全流程与 Silph 34 帧差距不在本次验证范围。

后续整合 `b7c81d82d7d94a06555f631c3c70d114f299d9d8` 后，性能优化及重放冻结源为 `40e86bc15e7dd18ec8c2acee3566d4ac93546311`，独立冻结二进制 SHA-256 为 `49b8ae0d381f6afa47b34a63dde5e5be61a8c8f32bb73449dc062bdef35e9804`。球调用方检查移入有实际完成事件或动画内确认输入时才执行的冷路径，Safari 排除条件只在 HP 可能报警时读取。完整 app 283 通过、39 忽略；core 2888 通过，其全部 core/data 源字节与最终版本相同。

最终版本重新播放八个原生普通捕获控制的完整物理按键序列，逐一核对全部原记录投球后 PNG 和普通 SAVE 的 32768 字节，均与 `fbed` 完全相同。另重放健康/HP1 的真实自然 Safari 成功投球前缀，并重新捕获同输入、同模拟帧的三张修复后对比图，图片字节与本目录提交图一致；这两个前缀不计作新增最终 SAVE 控制。原来的 `dfee/fbed` 完整矩阵与归档身份保持不变。

独立 GBA 构建使用固定 nightly-2025-12-07、`perf-benchmark` 与 mGBA 0.10.5 的硬件定时器。干净 `b7` 和最终版本训练家更新均为 232 ticks；最终野生战为 214，干净 `b7` 为 213，七场景全部通过既有门禁，没有修改 baseline 或阈值。中间失败的 233/234 ticks 记录完整保留。身份、普通重放、最终同帧图和性能逐项结果见 `integration.json`；`integration-evidence.zip` 可用同一个 `extract_evidence.py` 验证与解包。该补充档不包含 ROM、可执行文件或模拟器 state。

随后整合主分支 `6085c98704a9b0cb25cbcd34afcc028184ce9217`，整合源为 `490146cac1046afd0a15a9c8c9468cb79b578ab5`。捕获 app/渲染/续接/测试文件与 `40e` 字节完全相同；新增 core 差异只有 POKé DOLL 的使用提示与确认后移除，完整 app 283 通过（39 忽略）、core 2889 通过，GBA 七场景再次通过（训练家 232、野生战 214 ticks）。本次仅更新整合测试与 GBA 构建身份，没有把旧调试二进制或普通输入重放重标为新源。逐项记录见 `master6085-integration.json`，日志见可用同一解包工具核验的 `master6085-integration-logs.zip`。

此前整合 master `216cb034`（#260，统一引擎 pin `76da597b`）：完整核心 **2889 通过**、debug-server 应用 **283 通过/39 忽略/0 失败**。精确源码 SHA 与日志见 `master-216cb-integration.json`、日志归档。实际重播和图片仍保留其原冻结身份。

已整合最新 master `6d30ba5a`（#257，训练家图鉴修复）：完整核心 **2889 通过**、debug-server 应用 **292 通过/39 忽略/0 失败**。精确源码与日志见 `master-6d30-integration.json` 和日志包。原作与原生实际捕获继续保留各自冻结身份；本次整合不重标截图或存档。
