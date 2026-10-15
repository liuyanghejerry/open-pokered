# 捕球提示、音效与首次确认继续点

用球结果由真实球动画完成事件继续；捕获音效结束后才接受成功页确认。Safari 成功/失败投球补用球文字与实际捕获名；没有额外用球页A或新计时器。首次发布成功页不能把同一A同时当作确认，提前启动BattleOver倒数。快照只镜像既有 enabled/page，保留新/旧快照边界；没有新 live owner 或完整VFX/PCM快照。

## GBA 检查修复后的发布身份（2026-10-15）

受测源码 `9bce1d6382bd350721551f5e249e6b7cebd6cd2e` 在 `d8d4d2a9` 上只增加两个 renderer 文件的改动与地图缓存回归。将本帧已算的 live 地图指纹交给背景缓存，省去重复扫描；每帧仍验证实时方块，没有跨帧 revision、删帧或放宽基线。原 CI ROM 七场景已在本地逐项复现，其中移动最慢绘制 3114 超过 3113.1；新 ROM 为 **3107**，31 项原门禁全部通过，24 次绘制保持。新完整 app debug-server **330 通过／39 忽略**。旧核心、默认 app、determinism 和捕获录制仍保留各自身份。

从 literal master `44f22348` 构建基线，与 source-equivalent `0fb/d8d` 及修复 `9bce` 使用同一普通存档和完整按钮序列，均至实际第 **799** 帧。三张 PNG／RGBA 相同，优化前后完整公开状态序列相同，队伍、金额和输入存档未改；无新的 SAVE。Headless 图只验证 Game.draw 可见输出；RenderSession 实时地图修改和完整像素／调色板由 app 测试验证，GBA ROM 给出实际硬件计时。

精确身份、原 CI 失败与本地环境失败、新源码冻结、五阶段日志和三份实际输入见 `perf-1009.json`／`perf-1009-evidence.zip`。ZIP 是普通 `index.json` + `files/`，不使用旧 `extract_evidence.py`；逐文件 SHA 已解包回读验证。ROM、ELF、可执行文件仅留本地。新发布头的三个官方工作流仍须全部通过后合入。

## 此前 publication-993 的发布身份（保留历史）

#264 实际合入 `44f2234823eb5bd0d793ef966871f6028426aaa2` 后整合实时 master `44f2234823eb5bd0d793ef966871f6028426aaa2`，相对该master仅8个本PR源码/测试路径（6带生产逻辑、2仅测试）。原受测 runtime 为 `0fb1b57fd3096ba0332cd814319305fee1dc754b`，旧本地 docs head `534f06c52a892d5b1c03b6ced0236c99b1695fd7` 不是另一份runtime身份。原2172项编译映射不变；另从受测Git blob补证漏记的run-wasm/src/main.rs为2173项，docs外2691项NUL Git blob/模式与literal symlink也完全相等。原日志仍标2172项；没有伪称重新执行2173项。

v5的905组：core2930、debug-server app329/39忽略、default app318/37忽略、determinism10全部通过。`publication-993.json`/`publication-993-logs.zip` 仅整理本PR日志和身份，没有其他937组数据、构建或游戏重录。新head三个远程工作流还须全部通过后才能合入。

旧六图继续是dfee/fbed、帧2717/2233/2633的历史匹配前缀。新增ACK两图是a2fa/0fb在相同1177物理按键、实际帧1177：前返回野外，后停留捕获页，两者逻辑音效仍playing；该严格对照没有SAVE。另8次健康/HP1、前后各2普通NO教学/SAVE控制均保护原party/PC/bag/dex/cash，属于单独自适应控制。见 `docs/screenshots/capture-presentation/ack-publication-943/verification.json`；不得把自适应成功帧1146/1143当同帧对照。

下列矩阵、GBA结果、候选限制和旧失败均保留当时版本，不是本次新录制。普通HP1报警例外保留；原作/原生随机数不强匹配，不声称全部动画/原作时钟/PCM波形一致。CLI是逻辑SFX；真实三声道/PCM非静音由独立Game测试给出。snapshot DTO不恢复完整音频或VFX；legacy缺元数据仅保留已有当前页ACK兼容。

## 历史捕球矩阵、音频与集成（原身份）

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

此前整合 master `6d30ba5a`（#257，训练家图鉴修复）：完整核心 **2889 通过**、debug-server 应用 **292 通过/39 忽略/0 失败**。精确源码与日志见 `master-6d30-integration.json` 和日志包。原作与原生实际捕获继续保留各自冻结身份；本次整合不重标截图或存档。


合入训练家图鉴修复后的精确失败源 `50ece821b7e4fd2794c049d7de59ff50b0254efc`，在 CI 和本地均测得训练家更新 233 ticks，超过未改动的 232.3 门限。CI checkout merge `4526acc4` 与该源文件差异为零；下载的 CI artifact 只有 JSON/日志，没有 ROM，因此本地复现明确使用相同失败源重新构建的 ROM。单次球 caller 状态查询的候选仍为 233，已撤回，失败日志与首次新增测试构造器编译错误保留。最终仅在普通及 Link 战斗的逐帧玩家名称同步处用 `clone_from` 复用已有 String 容量，捕获续流程、VFX、字幕、音效和存档生产逻辑不变。

历史整合 master `8eb08f56945bf7db3650896295e8048cf97d14c8`，最终生产冻结源为 `f0f528666afbaf4458d1bc6e911e6ef43e7e8053`。完整 core 2889、app 293（39 个既有 ignored）、determinism 8 与 debug-server app 构建通过；最新独立 GBA 七场景两重复通过，训练家 224、野生战 206、野外静止 382 ticks。没有放宽门限，也不将本地通过称为远程 CI 全绿。

八个完整固定输入控制仍保留真实 `c3c38f71` 冻结身份：31884 帧、4812 张采样图逐字节一致，八份普通 SAVE 分别与已有对应成功控制的全部 32768 字节一致。最新 master 与最终整合版本另录普通返回 frame2717、Safari 投球 frame2233/成功字幕 frame2633，六张图与既有公开 PNG 完全相同，具体新身份见 `performance-919-screenshots.json`；旧 `screenshots.json` 未重标。Safari 保持自然捕获，所有失败尝试保留；短字幕重放不额外声称完成最终 SAVE，完整原作/Safari SAVE 范围仍为历史矩阵。

本轮 audit919 的可读汇总与限制见 `performance-919-verification.json`。`performance-919-evidence.zip` 包含新输入、逐帧截图、普通存档、完整哈希清单、失败候选及日志；可用既有 `extract_evidence.py` 独立验证。旧证据 ZIP 均逐字节保留，ROM 与可执行文件没有打包。

此前整合 master `f0787e19`（#268，巨石测试隔离；包含 #258 中毒与进门顺序）：core 2890 通过/0 忽略；app 300 通过/39 忽略；determinism 8 通过/0 忽略，全部 0 失败。当时受测源码 `3bd1b4ea17d59b406060a3caa3e79335fc033c44`、编译包装脚本与日志见 `master-f078-integration.json` 和独立日志包。历史原作/原生录制与 SAVE 保留各自冻结身份；本次新 head 仍需远程三个工作流全部通过。
