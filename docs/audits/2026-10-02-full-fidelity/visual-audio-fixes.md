# 音画确定项修复状态

审计基线和原作引用见 [visual-audio.md](visual-audio.md)。源代码修复提交从 `cf977c7` 开始；后续提交补标题版权、TUI图鉴与credits专用图块。这里只汇总已证实的差异；完整 SGB 显示模式、原作非法物种越界，以及逐曲 PCM/全游戏逐帧 ROM 对拍仍是明确的验证边界。

| 项 | 实现 | 本分支验证与截图 |
|---|---|---|
| VA01 | 原版权三行、GAME FREAK 字标、标题版权/Red/Blue 版本字标使用专用原图块；Setup 阶段暂不显示未加载的 logo。按用户要求保留项目 Fusion Pixel 英文、中文和数字字体；撤回原作通用字库替换，仅保留专用边框、PK/MN和命名下划线图形。 | copyright、splash 181/244/284、Red/Blue title 同帧图；最终集成title after包含 `5b80516`，本轮after统一重拍自 `eebde2f`。 |
| VA02 | Logo signed y 逐 tile 裁剪，去除负 y 钳制。 | opening signed-blit 测试、全开场/title framebuffer cache 测试通过；title 121/125 同帧图。 |
| VA03 | 标题宝可梦 signed x 裁剪，沿用 7×7 留白。 | title 441/507/563 图；共同 padding 测试涵盖 5/6/7-tile 图片。 |
| VA04 | starter 离开后 10 帧球表，仅移动第10个 OBJ；基础 Y 修为 $74。缓存 key 包含球帧。 | 标题球表/状态转移测试通过；title 441 包含新球阶段。 |
| VA05 | Crash 在第20帧反弹开始，Whoosh 在36帧停顿结束进入版本滑动时播放。 | 原汇编事件点核验、标题状态测试；没有新 PCM 对拍。 |
| VA06 | 有音频时等实际 cry 结束，立即全白3帧；无音频保留有限等待。版本滑入28帧，再 Delay3 并等 Whoosh。 | 标题真实声音完成 signal/三帧白屏测试通过。 |
| VA07 | 统一原作 7×7 缓冲留白、底对齐及全缓冲翻转；Oak/evolution/trade/Hof/league-PC/dex/credits 接入。credits 负 x 不再推回屏内。 | 合成 5/6/7-tile 全缓冲测试通过；Oak65、evolution190、trade40、Gengar/Raichu、Hof220图。TUI 同步。 |
| VA08 | 英文命名坐标、框与键盘节距恢复；昵称携带物种名和17帧 GB party icon；满名保留抬起下划线。中文 IME 布局保留。英文框为保留的项目10px高字形预留空间，case标签在框下独立显示。 | 命名开闭闪白测试通过；player5、LAPRAS17 同概念输入图（基线丢弃物种字段）。下划线使用独立专用图形，不替换普通字体。 |
| VA09 | 6/17/33 GB HP节拍、SGB数学5/16/32；球/化石上下1px；修正所有图标真实帧偏移、16px对称 OAM右列镜像；局部损伤高度17px。 | HP阈值与有效帧偏移测试通过；全部10类图标、绿6/黄17/红33图；三个 party 局部重画测试通过。 |
| VA10 | 专用 trainer_info 边框/背景、圈、数字与徽章纵向摆放；Red按原 VRAM列重排裁剪；NAME/MONEY/TIME位置和补零金额。 | 原 start_sub_menus/draw_badges逐条核验；trainer-card5；最终集成图包含正确 TrainerCard asset category。 |
| VA11 | 地图背景、(4,2)标准框、三 cursor 行/第四预览行、末项CANCEL；首尾不wrap、窗口到边再滚动；过滤背包用独立构造保留既有政策。 | 独立 rustc 对同一 core 源码10/10测试通过；实际 CeladonMartElevator sign流程 seed42/frame70前后图与完整trace，双方玩家(3,1)/Up/cursor0一致。 |
| VA12 | GB Morph正常灰度；原 SGB PAL_BLACK 请求单独暴露，不将其错误套在普通 GB。 | evolution状态7项测试通过；evolution190图；visual_verify_evolution oracle同步普通GB和SGB请求。 |
| VA13 | 专用 diploma边框/圈、Player/名字/正文/签名坐标；右侧Red按$90 OBP和behind-BG priority合成。中文正文保留。 | diploma5、中文0图；最终集成图包含正确 TrainerCard asset category。 |
| VA14 | 三个8帧 FadePal6/7/8；MonFade保留Hall框、FinalFade保留stats；前导fade从实际地图开始；cry结束再计80帧。缓存必须区别首个fade帧。 | core Hof3项通过；Hof0/220/480/488图。app旧cache测试发现首阶key遗漏，已加1区分；根集成完整app测试已证明该项通过。 |
| VA15 | HoF后clear100、黑条128；音乐在黑条开始时触发。 | credits开场228帧/音乐事件测试通过；该状态由真实HoF完成路径进入。 |
| VA16 | 四级 WHITE/AA/55/BLACK每级5帧；fade20帧后再完整等待90/120，避免漏算20帧。 | credits4项状态测试、完整credits cache像素测试通过；credits5/10/15图。 |
| VA17 | 原THE END双行 interleave图块、原坐标与四阶palette等待；字形index3从出现起即黑。copyright页使用原专用三行图块。保留有意省略非法越界种类的现有政策。 | credits6600图、完整credits cache测试；未声称复制原越界bug。 |
| VA18 | 已交共享 dotzuki 音序器修复：新音符初始化、整数/小数 pitch-slide 步长及原借位行为；游戏升级其依赖由主集成提交完成。 | 当前bug trace和原汇编引用在审计文档；共享引擎测试由主代理提供。没有新ROM PCM对拍。 |

| VA19 | Credits 27个8px剪影步进每 VBlank 一步，修正原来的二分之一速度。 | 同一production源码的5项状态测试通过，新增单步/27帧终点断言；最终credits定帧图。原VRAM staging和CPU扫描线采样仍未做ROM对拍。 |

| VA20 | native/TUI地图共享原OBP0 $D0调色板；index1是不透明白、index2是AA，index0保持透明；native镜像player也走palette-aware blit。 | 真实packed 2bpp图块覆盖地图背景的像素回归测试通过；down/left/right固定地图帧截图。原SGB模式仍独立，未声称验证GBA实际OBJ输出。 |

此前本分支首次 app lib 结果为109通过、3失败。两项电梯局部光标测试发现擦除旧箭头时误擦右侧楼层，已将该处损伤限制到一个8px tile；HoF缓存测试发现 MonText 与首个 MonFade 使用相同 key，已修正首阶区别。此前集成后完整app lib两组128/128、127/127通过。菜单/地图局部重画修复保留，普通字体已恢复为项目 Fusion Pixel，并按其真实宽度和10px字形高度复核菜单、地图和对话。对话与战斗文字的两行间距修为12px，真实像素回归覆盖英文下伸笔画、中文和底框；最终EXP截图的 `points!` 在136–137行保留白色空隙。命名、Town Map、MONEY和PC相关布局也已按保留字体复核。最终完整构建和测试结果见[字体保留验收](validation/font-preserved.md)。

最终固定帧证据为[59对截图及manifest](../../screenshots/2026-10-02-full-fidelity/README.md)，以及[6对Town Map/NPC/link movie截图](../../screenshots/2026-10-02-original-font/README.md)。全部65张after统一来自冻结的 `eebde2fc3a4f78c38b4c7b5d49e0db2b830cb235`，保留项目 Fusion Pixel 普通英文字、中文字和数字，仅使用专用UI图形还原边框等画面。CLI/debug app SHA为 `ea717f1c2d6332f4608fc13194296cdb166ea20662d8d0f7c379bd17ba005e52`；初始化fixture SHA为 `c0aa573eb408a0c3df6380df30909c29d0d8aa964c5e9b4208d297f5450f680f`，按该app精确crate metadata冻结完整依赖图，并启用 `fidelity_after` 与 `feature="debug-server"`。manifest和新写入ledger记录source、二进制/图像哈希、帧与相位，全部before校验和不变。五组纯字体实验图明确标为历史并从最终PR验收移除；此前6751981、cd9acde与c51209a的截图来源也未改称新source。

资源表核验只能证明数据：战斗动画、move SFX、cry表全量0diff，不能推出实际呈现帧/播放采样已全量对拍。

原调色板的纠正已用仓库RGBDS工具转换真实PNG核验：`HoFGBPalettes` 的dc宏是MSB→LSB（macros/data.asm:60），字节C0/D0/E0/F0；普通字库经ShiftFontColorIndex成为index2、按四级渐显。THE END图形不经过该转换，实际仅index0/3，因此原作只执行palette等待，文字本身从出现起就是黑色。copyright页也用LoadCopyrightTiles，不能按普通credits字符串重排。
