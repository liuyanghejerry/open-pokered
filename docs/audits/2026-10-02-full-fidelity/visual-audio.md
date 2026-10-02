# 音画保真审计（2026-10-02）

审计基线：open-pokered `72ff719`，原作 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。参考树 `/workspace/onboarding/pokered-reference-full`。以下行号均指该冻结基线，后续修复会移动行号。普通 GB 灰度为主要基准；SGB 独立列出。中文是扩展功能，以下英文字体和英文布局差异不要求取消中文。

这是本轮源码逐项核对、当前 native 截图及真实音序器状态探针。没有进行新的原版 ROM 音画对拍。9 月的招式逐帧对拍、选项截图和卧室对拍只标作历史证据。`visual-captures/manifest.json` 记录当前截图，Hof/Credits 使用 CLI 播种电影状态；title CLI 有自身 Copyright 120 帧前导，真正 LogoBounce 从截图帧 121 开始。不能将任意 CLI 截图帧直接视作原版开机的帧号。

## 按玩家流程确认的差异

P1：明显影响主要体验或系统性的错误；P2：正常可见的保真差异；P3：小范围像素差异。这里的优先级属于保真审计，不把每个改版 UI 都称作玩法 bug。

| ID / 优先级 | 触发、当前表现与原作预期 | 当前依据 | 原作依据 / 证据边界 |
|---|---|---|---|
| VA01 / P1 保真 | 所有英文文本：常规文字仍是 Fusion Pixel 10px（Latin 前进 5px），原作是 8px tile 字体；普通框线来自通用框线，专用菜单边框也未全部使用。版权、GAME FREAK 字标亦沿用文字绘制。标题版名字加载不存在的 red_version_tiles 导致始终退化为普通字体，Blue 也未选专用 blue_version 图块。 | `crates/pokered-ui/src/backends/framebuffer.rs:113`、`:179`；`crates/pokered-app/src/render/gamefreak_splash.rs:38`、`:60`；锁定 dotzuki-renderer `7efac8a` 的 `embedded_font.rs:1`、`:100`。 | `home/load_font.asm:1`、原 `gfx/font/font.png`；`engine/movie/splash.asm:218`。9 月 options-original.png 是历史原版截图；本轮 options-00005.png 确认现状。这是设计选择造成的保真差异。 |
| VA02 / P2 bug | 标题 Logo 最初下落、反弹时：负 y 被钳制成 0，整张 Logo 在顶端停住，多帧没有下落；应从屏幕上方裁剪进入，继续跟随 SCY。 | `render/title.rs:197` 的 `.max(0)`；当前 CLI f121 与 f125 像素完全相同。 | `engine/movie/title.asm:35`、`:69`、`:149`、`:178`，SCY 64 开始，Logo 位于 tile(2,1)。无新 ROM 同帧截图。 |
| VA03 / P2 bug | 标题宝可梦轮换滑出左侧：负 x 被钳制成 0，停在左边后突然消失；应被左边界逐渐裁掉。 | `render/title.rs:83`；当前 f439 与 f441 像素完全相同。 | `engine/movie/title2.asm:19`、`:50`，SCX 实际改变。 |
| VA04 / P2 未还原 | 标题展示 starter 后 Red 手里的球：目前完整静态 player PNG，没有独立第 10 个 OBJ 上下抛球的 10 帧动画。 | `render/title.rs:91`；`title_screen.rs:369` 至 `:415` 只有 mon 的轮换，没有 ball 状态。 | `engine/movie/title.asm:231` 至 `:236`；`engine/movie/title2.asm:85` 至 `:119` 的 TitleBallYTable。源码确定，没有新 ROM 对拍。 |
| VA05 / P2 bug | 标题配合动作的音效：IntroCrash 在 LogoBounce 刚开始播放，Whoosh 在 LogoPause 刚开始播放；原作 crash 在 -3 反弹段，whoosh 在落定后等待 36 帧结束才播放。 | `game.rs:3668`、`:3671`。 | `engine/movie/title.asm:157` 至 `:160`、`:189` 至 `:197`。声音事件时点确定，未录新 PCM。 |
| VA06 / P2 bug | 标题按键后的叫声等待：无论物种固定 60 帧，再做 16 帧渐白；原作等待真实叫声结束，再 GBPalWhiteOutWithDelay3，非同样固定渐变。 | `title_screen.rs:151`、`:417` 至 `:430`。 | `engine/movie/title.asm:239` 至 `:254`；`home/pokemon.asm:145`。不声称每种叫声都会被截断，确认的是等待和转场不同。 |
| VA07 / P2 bug（共同根因） | 大木的 Nidorino、进化、交换、名人堂与 PC 名人堂图片：部分路径直接画紧裁原 PNG，漏原作 7×7 的水平 tile 对齐和底部对齐；大木 Nidorino、进化、交换还漏原作水平镜像。进化 y=8 尤其不是原作 tile(7,2) 的基准 y=16。 | `render/oak.rs:139`、`render/evolution.rs:142`、`render/trade.rs:215`、`render/hof_ceremony.rs:282`、`render/pc.rs:452`。 | `home/pics.asm:65` 至 `:106` 的 x=((8-wTiles)/2)*8、y=(7-hTiles)*8；翻转 `home/pokemon.asm:96`；调用 `oak_speech.asm:79`、`evolution.asm:102`、`trade.asm:750`、`hall_of_fame.asm:115`、`league_pc.asm:97`。补充核验：Pokédex 仅镜像紧裁 PNG，6-tile 宽的图片应连同 7×7 留白整体翻转，因此水平留白仍错 8px；credits 的紧裁图片也漏留白，左裁剪 helper 还将负 x 的图块推回可视区。原作 `CopyUncompressedPicToHL` 翻转整个缓冲，不能把紧裁镜像当作相同。 |
| VA08 / P2 未还原 | 英文命名界面：姓名与下划线居中，键盘第一行 y=48，整个大框到屏底，昵称时也没有物种名与动态 party icon；原作姓名(10,2)、下划线(10,3)、键盘(2,5)、昵称物种名(4,1)与动态图标。 | `pokered-ui/src/menus/naming.rs:14` 至 `:35`、`:50` 至 `:97`；`render/oak.rs:233`。 | `engine/menus/naming_screen.asm:96`、`:337` 至 `:390`、`:453` 至 `:475`。中文 IME 是扩展；英文差异独立核定。 |
| VA09 / P2 bug | 队伍菜单选中图标统一 16 帧换相位，绿/黄/红 HP 无速度变化；Ball/Helix 的两帧加载同图且位置不动。其他图标也有实质根因：Mon/Fairy/Bird/Water 永远取同一帧；8px 图标 frame2 取不存在的第4个tile、回落frame1；16px对称图标未遵原OAM翻转右列。原作普通 GB 三档 6/17/33 VBlank，Ball/Helix 切换时整体向下 1px。 | `render/menu.rs:154`、`:244`、`:320`；`pokered-renderer/src/mon_icon.rs:69` 至 `:79`；`render/session.rs:952`。 | `engine/gfx/mon_icons.asm:12` 至 `:42`、`:64` 至 `:79`、`:88`。SGB 是 5/16/32，不混入普通 GB。 |
| VA10 / P2 未还原 | Trainer card：目前两只通用框、图标 x=13/49/85/121 和 y=92/118、Red 位于 x=96；原作专用 trainer-info tiles，标题 BADGES tile(6,9)，数字 tile(2+4i,11/14)，脸/徽章在下一行，两者并非同一横排。 | `render/trainer_card.rs:18`、`:39`、`:50`、`:75` 至 `:110`；当前 trainer-card-00005.png。 | `start_sub_menus.asm:477` 至 `:565`；`draw_badges.asm:46` 至 `:100`；图片 upper-right tile(15,1) 后原作有特定裁剪/重布。功能不坏，布局未忠实还原。 |
| VA11 / P2 未还原 | 电梯/过滤背包：目前清成整屏文字菜单、7 行、footer A SELECT / B BACK；电梯逻辑缺末项 CANCEL，Up/Down 全列表循环、窗口居中跟随。原作保留地图背景，PrintText WHICH FLOOR 后使用标准 SPECIALLISTMENU：3 个可选 cursor 行、第 4 项预览，端点不循环，cursor 越过窗口边缘才滚动，A 选 CANCEL 与 B 均取消。 | `render/elevator.rs:21` 至 `:50`、`:69` 至 `:97`；基线 `core/elevator_screen.rs:77` 至 `:97`、`:67` 至 `:74`；当前 elevator-00005.png。 | `engine/events/elevator.asm:1` 至 `:23`；`home/list_menu.asm:41` 至 `:48`、`:105` 至 `:110`、`:177` 至 `:195`、`:364` 至 `:372`、`:524` 至 `:528`。过滤背包受具体脚本形式影响，未对每处原作菜单完成单独 ROM 对拍。 |
| VA12 / P1 保真 bug | 进化 Morph：普通 GB 也黑底、白色反相精灵。原 PAL_BLACK 是 SGB whole-screen palette 命令；普通 GB 直接跳过，应该保持普通灰度画面。 | `render/evolution.rs:69` 至 `:93`；`evolution_screen.rs:260`。 | `engine/movie/evolution.asm:49`、`:96`；`home/palettes.asm:38` 至 `:42` 的 wOnSGB guard。历史 f190 截图已确认当前反色，非新 ROM 对拍。 |
| VA13 / P2 未还原 | 毕业证：当前普通细线矩形、没有 Player 标签、姓名移到 x62、无右侧 Red，正文坐标和特殊圈图块未还原。 | `render/diploma.rs:15` 至 `:51`，当前 diploma-00005.png。 | `engine/events/diploma.asm:12` 至 `:66`、`:92` 至 `:117` 的专用边框、CircleTile、Player 字样和 Red OBJ。功能存在，画面仍重排。 |
| VA14 / P2 bug | 名人堂每只宝可梦间的淡白阶段：MonFade 只重画无 Hall of Fame 框的 mon info，不做渐变，随后切白；initial/final fade 直接白屏。原作每次 GBFadeOutToWhite 3×8=24 帧，当前 FADE_FRAMES=16。当前 monfade f473/f487 完全同图。 | `render/hof_ceremony.rs:150` 至 `:176`、`hof_ceremony.rs:38`。 | `engine/movie/hall_of_fame.asm:69`、`:282`；`home/fade.asm:26` 至 `:40`。同时原作 HoFDisplayMonInfo 的 PlayCry 阻塞再等待80，当前仅异步播叫声并计80，叫声时长未计入。 |
| VA15 / P2 bug | HoF 结束到 credits 第一页：现立刻开始主题和第一页；原作先 clear 等100，黑条完成并开始 music 后再等128。 | `game.rs:3552` 至 `:3562`，`credits.rs:207` 的初始 Hold。 | `engine/movie/credits.asm:1` 至 `:34`。确定漏两段前导，非模糊节拍印象。 |
| VA16 / P2 bug | Credits fade：当前 5 个名义色阶 WHITE/C0/80/40/BLACK，indexed buffer 实际使 C0/80 都量化为 AA，导致相邻 5 帧重复且20帧后才黑；原作 C0/D0/E0/F0 对 ShiftFontColorIndex 后的 index2 应是白/AA/55/黑，各5帧，最后黑已从第15帧起。当前 f5/f10 同图。 | `render/credits.rs:22` 至 `:28`，`credits.rs:235` 的 fade_step。 | `engine/movie/credits.asm:36` 至 `:46`、`:133` 至 `:137`。 |
| VA17 / P2 未还原 | THE END：当前单行小字体文字，原作用 gfx/credits/the_end 的 2 行专用图块于(4,8)/(4,9)，再执行4×5帧 palette等待。注意PNG转2bpp只有index0/3，所以文字从出现起即黑，不能把FadeInCredits调用误认为其字形视觉渐显。 | `render/credits.rs:146` 至 `:149`，当前 credits-06600.png。 | `engine/movie/credits.asm:245` 至 `:265`。copyright 后第16只越界宝可梦当前有意省略（credits.rs:22），必须另列原作 bug 选择，不计作无意新 bug。 |
| VA18 / P1 音频 bug | Safari/进化、治疗音乐和标题 CH3 的 pitch_slide：用前一音符而不是紧随命令的新音符算方向与 freq_step，且用全部差值作为每帧步长，没有按 note_delay-length_modifier 求商与保留小数（含原作借位 bug）。Safari 第一音一帧冲到目标，下一音 step=0 完全不滑。 | 实际锁定 dotzuki-audio `f57af30` 的 `sequencer.rs:450` 至 `:474`、`:598` 至 `:639`、`effects.rs:89` 至 `:125`。本轮现行 rlib 状态探针：`audio-pitch-slide-current.csv`，SAFARI_ZONE f0 freq1548 step1899，f1直接1899，f6新音1714 step0直至f11。 | `audio/engine_1.asm:432` 至 `:461`、`:785` 至 `:799`、`:1140` 至 `:1236`；使用曲目 `audio/music/safarizone.asm:9`、`pkmnhealed.asm:8`、`titlescreen.asm:380`。这是实际音序器路径确认，不只是 approximate 注释。尚无新 ROM PCM 对拍。 |

## 覆盖矩阵与本轮未发现新增差异的边界

| 流程 | 阅读/核对范围 | 结果及验证边界 |
|---|---|---|
| 版权→Game Freak→战斗电影 | `gamefreak_splash/opening/intro`、core phases、原 intro/splash；#109报告和独立图块测试复核 | 大小星已修，不旧报。版权/字标 VA01；战斗电影全部时序未新 ROM 对拍。 |
| 标题→主菜单→选项→继续 | title state、app audio events、menu painter、options layout | VA01–VA06。空心箭头/Continue 信息已有修复；不能把历史选项错标重复报为当前 bug。 |
| 大木→命名→入游戏 | oak、naming、原oak/naming汇编 | VA07/VA08；已有姓名菜单头像平移保留。 |
| 地图/NPC/移动/传送/草层/水花 | `render/overworld` 的实际底图、优先草层、NPC/玩家移动、跨图预览、CUT/FLY/SURF/FISH/治疗/SS Anne/warp palette 路径；历史卧室差分报告 | 未由本轮新的原版逐帧对拍证明所有248图与所有移动一致；没有按旧近似注释虚构新bug。#108 TUI FLY 鸟已修。 |
| START/保存/背包/商店/队伍/能力 | menu及pokered-ui menus（v1/v2）、icons、stats插图、原菜单/mon_icons | 通用字形 VA01，party VA09。v2引擎迁移本身不算bug；未逐种UI状态同ROM拍齐。 |
| 图鉴DATA/CRY/AREA/Town Map/FLY | pokedex/town_map，原pokedex/town_map | Entry 的 6-tile 图片整缓冲镜像留白也列入 VA07。AREA已有实际地图与玩家图；本轮未全151页字体/分页对拍。 |
| PC/联机/交换 | pc/link/trade与原league_pc/trade | VA07与列表字形；联机两队与stats预览已修，游戏机fallback仅无资源时可达，不能把fallback算普通桌面缺失。 |
| Game Corner/elevator/card/diploma | slots/elevator/trainer_card/diploma | slots使用原图块与tilemap；无新slot ROM节拍验证。VA10/VA11/VA13是确定版式缺口。 |
| 战斗出现/派出/招式/命中/HP/捕捉/濒死 | `render/battle`、Gen1 presentation queue/anim runtime；历史9月28日全165招式对拍 | 原表与move sfx/cry由根本轮验证0diff；历史isolated全招式和feedback不同门槛要分开。feedback 2/3/5/6 的少量LCD切行严格像素差异是历史FAIL，尚未新ROM重测。派出/撤回台词#108已修。 |
| 进化/HoF/credits | 三者core+renderer+audio glue，原movie asm，当前CLI截图 | VA07/VA12/VA14–VA18。整体流程存在，不称通关套件缺失。 |
| 音频 | pokered manager/低血警报/叫声modifiers/music SFX tables；锁定f57音序器/APU/output | VA05/VA06/VA14/VA15/VA18。数据0diff、音量非零≠听感/逐采样一致。本轮没有全曲目PCM对拍。 |
| SGB模式 | SGB数据/renderer测试、native真实render入口和固定灰度framebuffer | 数据/ColorPaletteState测试存在，但生产绘制使用固定GRAYSCALE且无mode路由/边框present，不能宣称已支持原作SGB。把SGB完整彩色/边框与HoF不同速度列作平台能力缺口，勿与普通GB错误混算。 |

本报告记录修复前基线。若修复后验收，请对照独立修复报告及固定同状态 before/after；本轮未做过的 ROM/音频对拍不能由测试通过补成已完成。
