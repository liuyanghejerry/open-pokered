# pokered 保真度审计与修正计划(2026-08)

跟踪文档:对照原作反编译(`/Users/liuyanghe02/develop/pokered-worktree`,pret/pokered)的
全面审计结果 + 修正执行状态。分支:`feat/fidelity-audit-fixes-2026-08`。

审计日期:2026-08-02。方法:5 路并行审计(招式动画数据 / 过渡与进出场动画 / 战斗机制 /
FIDELITY_GAPS.md 时效性 / 非战斗系统),基线 master @ 6c773ebfe。

验收原则:每项修正附带测试;`cargo test -p <crate>` 按 crate 验证(工作区级 cargo test
会因 feature 统一误报,见 AGENTS.md);涉及 DSL/场景的改动跑 roundtrip。

路径缩写:**CORE** = `crates/pokered-core/src` ·
**DATA** = `.../pokered-data/src` · **APP** = `.../pokered-app/src` ·
**RENDER** = `workspace/crates/jrpg-renderer/src`。

---

## 0. 审计总结论

- **招式动画数据 100% 对齐**:`scripts/verify_battle_anim_data.py` 0 diffs(203 动画条目 /
  86 子动画 / 122 帧块 / 177 基准坐标逐字节一致)。无招式间动画错配。
- **战斗机制覆盖率接近全量**:~80 个招式效果常量均有生产路径实现;标志性 Gen-1 bug
  (Focus Energy 降暴击、1/256 miss、Toxic 无上限、替身 0HP、OHKO 等级门、Counter 属性
  限定、Bide ×2、Hyper Beam KO 不蓄力)全部验证在位。
- **残余缺口集中于三类**:(a) 动画/视觉的运行时接线(数据齐了但前端不触发);
  (b) 5+6 处数值/规则分歧;(c) 逻辑已写好但不可达的系统(背包用道具、PC、图鉴、进化
  过场)与完全缺失的系统(钓鱼、通关套件、不服从机制)。

---

## Phase A — 战斗动画接线与修正

### A1 幽灵塔(Ghost / Marowak)揭示流程 ✅ 已修正(Wave 1)
- [x] `IntroAnimState::GhostMarowakReveal` 死代码 → 已接线:新 `IntroPhase::GhostUnveil`,
      有镜 Marowak 战播放揭示动画(闪烁 8 次 + 淡出淡入,动画完成时换 Marowak 图)。
- [x] 三段文本补齐(英文,与全代码库战斗文本惯例一致):"Enemy GHOST appeared!" /
      "Darn! The GHOST can't be ID'd!" / "SILPH SCOPE unveiled the GHOST's identity!"。
- [x] "too scared to move" 机制:无镜幽灵战(塔 1F-7F 野生 + 无镜 Marowak)我方招式
      全部失败(冰冻/睡眠除外,不耗 PP);敌方鬼从不攻击("GHOST: Get out... Get out...");
      逃跑必定成功(原作 core.asm:3283-3324, 1497)。
- [x] PokemonTower6F 改为无论有无镜都开战(RESTLESS_SOUL ≡ MAROWAK,
      pokemon_constants.asm:209),战斗内揭示;is_ghost/揭示判定移入 start_wild_battle。
- [x] SFX_SILPH_SCOPE 播放(sfx_data.rs:157 已存在,接线完成)。
- 注:TUI 此前从不设置 is_ghost(鬼渲染是死代码),已一并修复。

### A2 濒死动画方向与音效 ✅ 已修正(Wave 1)
- [x] 敌方濒死改为向下滑出(新 `SlideKind::Faint`,SlideDownFaintedMonPic 移植,
      8px/2 帧 × 7 行 ≈14 帧;玩家侧原 10 帧/20px 也一并修正)。TUI 同步。
- [x] SFX_FAINT_THUD(训练家战,FALL 播完后接 THUD)+ 玩家怪濒死叫声
      (cry_pending 接 play_species_cry)。野生战不再误播 FALL(走胜利音乐路径)。
- 遗留:TUI 无 message-SFX 块,FALL/THUD 仅 app 侧(低优先)。

### A3 过渡动画修正 ✅ 已修正(Wave 1)
- [x] **FlashScreen 频闪接线**:Circle/DoubleCircle(野生战)进入战斗前先播 12 步 ×3 轮
      调色板频闪(72 帧,原作 `ld b,$3` 循环),再进 wipe;新 intro 起始阶段选择
      `intro_start_phase()`(CORE/battle/mod.rs:278-302)。
- [x] Circle/DoubleCircle 真圆弧:移植 CircleData1-5 + HalfCircle1/2 入口表 +
      Circle_Sub3 行扫描逻辑(RENDER/battle_transition.rs),矩形死代码删除。
- [x] Shrink/Split 改为真瓦片拷贝压缩(CopyTiles1/2 移植,render 已有 source_fb,
      黑块按原作顺序传播);时序 DelayFrames 6 对齐。
- [-] TUI 过渡:保持 8 帧闪烁近似(注明,低优先);TransitionFlash 在 TUI 现可触发。

### A4 非招式动画 ID 接线 ✅ 已修正(Wave 3)
- [x] 捕捉流程动画:新 `BattleAnimEvent`/`BallAnimOutcome` + `pending_anim_events` 队列
      (wPokeBallAnimData 等价);前端 `build_ball_choreo` 完整移植 TossBallAnimation
      序列(捕捉成功 TOSS→POOF→HIDEPIC→SHAKE×3;挣脱 …SHAKE×N→POOF→SHOWPIC;
      未中 TOSS→POOF;鬼闪避 TOSS);球种决定 LOW/MIDDLE/HIGH 子动画;
      SHAKE 每下 40 帧 + SFX_TINK;Safari 球/老人教程同路径。
      附带修复:捕捉成功后战斗未结束的 bug(阶段被 show_text_then 覆盖)。
- [x] X 道具动画(XSTATITEM_ANIM 我方 / XSTATITEM_DUPLICATE_ANIM 敌方 AI);
      Safari ROCK/BAIT 消息触发;STATUS_AFFECTED_ANIM = 蓄力回合闪光
      (core.asm:3196 等 4 处调用点);SHOWPIC 仅捕捉流程使用(与原作一致)。
- [x] SE_TRANSFORM_MON:核对 asm 后真相为 ChangeMonPic 同步换图、无延迟循环
      (审计"阻塞式过渡"表述不准);实现为隐藏→停 12 帧→6 帧后显示(前端按
      TRANSFORMED 标志画目标种),clear_side 可取消。
- 遗留:BALLBLOCK 正常游玩不可达(训练家战背包过滤精灵球),注明未接。

### A5 进出场编排 ✅ 已修正(Wave 3)
- [x] 玩家出场:POOF_ANIM + 球在 hlcoord(4,11) + 3×3(4帧)→5×5(5帧)→7×7
      降采样生长(CopyDownscaledMonTiles 移植)+ 叫声;替代原简单垂直球上升。
      注:核对 asm 后原作 SendOutMon 并无抛物线球轨迹(弧线仅捕捉流程),按原作实现。
- [x] 敌方训练家怪出场:删除多余的滑入/生长动画,怪物在训练家图滑出后直接出现
      (与原作一致;濒死后续怪的 EnemySendingNext 另有 Legacy 滑入,保留)。
- [x] 训练家登场音效:SFX_SILPH_SCOPE(common_text.asm:62-69 核对;tempo 修改对
      非叫声为死代码,按原样播放)。
- [x] Game Freak 流星 logo:新 `gamefreak_splash.rs`(CORE 状态机,asm 帧数精确:
      180f 版权页→64f 黑边+logo→SFX_SHOOTING_STAR+大星 40f→3 次 logo 闪烁→
      6 波小星 144f→40f 收尾;仅星动画期间可按 A/Start 跳过,与原作一致);
      使用真实 gfx/splash 素材,APP/TUI 双前端。
- [x] HP 条逐帧扣血:原作在 engine/gfx/hp_bar.asm(非 core.asm),1px/2帧、48px 条;
      新 `HpBarAnim` 状态机(displayed HP 向实际值缓动,换怪/开战即时同步),
      ShowingText 阶段等扣完才放行;颜色阈值/低血量警报自动跟随。
      偏差:原作扣血无独立音效,按任务要求每次扣血播一次 SFX_BATTLE_DAMAGE。

---

## Phase B — 战斗机制分歧修正

### B1 数值/规则分歧(D1–D5、D7–D9、D11)✅ 已修正(Wave 1)
- [x] **D1 Dream Eater**:目标未睡眠时整招失败(新 Accuracy 钩子 VetoIf 前置否决,
      rules.ron:457 + bridge_accuracy 路由;oracle 同步)。注:原作走通用 miss 文本,
      非 "But it failed!"。
- [x] **D2 束缚回合分布**:Wrap/Bind/Fire Spin/Clamp 改 3/8·3/8·1/8·1/8 加权
      (复用 determine_hit_count;生产 + oracle 两处都修)。
- [x] **D3 Bide**:`(rand & 1) + 2` → 2–3 回合(生产 + oracle 都修)。
- [x] **D4 Fly/Dig 半无敌**:Gen-2+ 穿透表已删,仅 Swift 例外(与原作 ret z 顺序一致)。
- [x] **D5 拒绝采样**:Metronome(battle/mod.rs:571,排除 0/≥$A5/Metronome 自身,
      64 次上限)+ 睡眠回合(jrpg-rules interp.rs RngRange 尾部拒绝,均匀 1-7)。
- [x] **D7 oracle Toxic 分支**:battle/residual.rs 先查 BADLY_POISONED 标志;slice5 parity
      POC 同步修正。
- [x] **D8 经验**:濒死怪不获得经验;EXP ALL 两遍除法移植(含 stat EXP 同除、
      二遍对队伍数再除的原作 in-place 怪癖)。
- [x] **D9 CheckDefrost**:火系招式命中冰冻目标解冻("Fire defrosted <TARGET>!",
      text_3.asm:91;新 pokered_defrost 原生钩子,order 最后保证先判烧伤)。
- [x] **D11 Toxic×寄生种子**:寄生种子走同一扣血例程 → Toxic 计数二次递增、
      吸取量随计数放大(生产 + oracle + parity POC)。
- 已知残留(记录在案):legacy oracle special_effects.rs:86 与 p5_native.rs:859 两处
  Metronome 取模副本受 EffectRandoms 单字节契约限制无法拒绝采样(非生产路径);
  寄生种子先于 Toxic 安装时单回合双方扣血总量一致、仅种者回复差一个基数。

### B2 缺失系统 ✅ 已修正(Wave 2)
- [x] 徽章属性加成 + stat-up glitch:新 `battle/badge_boosts.rs`;原作核对为
      Boulder→攻击、Thunder→防御、Soul→速度、Volcano→特攻(RAM 位序,审计初稿
      属性对应有误,以 asm 为准),×9/8、999 上限;出场/升级/属性变化时重挂
      (glitch 全四项重挂),Haze 清除,X 道具路径同步。伤害/速度判定自动生效。
- [x] 交换怪不服从机制:新 `battle/obedience.rs`;阈值 L10/L30 蓝徽章/L50 虹徽章/
      L70 粉徽章(审计初稿 Marsh/Volcano 互换,以 asm 为准)/地球徽章全服从;
      完整 roll 表(服从/换招/睡觉 1-7 回合/四种 loaf/自伤 40 威力无属性)。
- [x] SRAM per-mon 保真:OT-ID、OT 名、昵称、PP-Ups 全部进出 32KB 原布局
      (无格式破坏);捕捉怪打上玩家 OT;import 派生 is_traded(1.5× EXP 此前从未生效)。
- 已知偏差(代码内注明):ot_id==0 视为己方(保护旧存档);服从判定在混乱/麻痹
  判定之前(回合都丢,仅消息归属不同)。
- [-] 联机对战:维持脚手架,不在本期范围。

---

## Phase C — 非战斗系统接线与补全

### C1 背包道具用于宝可梦 ✅ 已修正(Wave 2)
- [x] bag USE → 分类(`items/bag_use.rs` classify_bag_use)→ 队伍选择新模式
      (PartyScreenMode::UseItem,A 键直接上药)→ use_engine 各效果函数;
      失败文本用原作 "It won't have any effect."。治疗/解状态/PP/维生素/神奇糖果全通。
- [x] 进化石可用 → try_evolve + 图鉴 owned;过场接缝留 `TODO(evolution-cutscene)`(C5)。
- [x] TM/HM 教学接线 + 新增招式替换 UI(原代码库完全没有:PartyScreenPhase::ChooseMove
      + pokered-ui draw_move_choice);HM 招式不可遗忘("HM techniques can't be deleted!")。
- [x] Repel/Super/Max:100/200/250 步 + 消耗侧已有 → 补到期消息
      "REPEL's effect wore off."(原作 .lastRepelStep 语义)。
- [x] 关键道具 TOSS 守卫(InventoryError::KeyItem,"That's too important to toss!";
      卖侧本已有守卫)。

### C2 PC 存储系统 ✅ 已修正(Wave 4)
- [x] `GameScreen::PC` + `pc_screen.rs` 完整状态机(消息分页/YES-NO/SFX 队列/
      换箱存档请求),复用原孤儿逻辑 pc_menu.rs;APP/TUI 双前端渲染与输入。
- [x] Bill's PC:存/取/放生(确认文本按原作)+ STATS 弹窗 + 换箱(12 箱,
      "data will be saved. Is that okay?" + 即时存档,save.asm:358-402);
      最后一只禁止存放(按队伍数,原作 bills_pc.asm:207-213)、箱满/队满守卫。
- [x] 玩家 PC(道具):存/取/扔,数量选择,关键道具固定 1,容量 50 格
      (menu_constants.asm:2),整组失败语义,关键道具禁扔。
- [x] 触发点:`game.openPC()`/`openItemPC()` 脚本 API + 11 个宝可梦中心 sign
      (13,3 坐标)+ RedsHouse2F 卧室 PC(0,1),map.json + script_config.json 双侧接线。
- [x] 大木 PC 图鉴评级:16 档评级文本逐字移植(含原作 "geting" 错拼)。
- 遗留:#MON LEAGUE 名人堂查看器未做(league_pc.asm,归入 C8);
  Bill 家直连 PC 核心已实现未接地图(坐标事件,低优先)。

### C3 图鉴屏 ✅ 已修正(Wave 4)
- [x] 图鉴列表屏:7 行窗口、编号列表、捕获精灵球标记/未捕获虚线、±1/±7 翻页、
      上限=最高已见编号(wDexMaxSeenMon);只见未捕获条目显示 ?'??" / ???lb 无描述
      (asm 513-515 核对);打开条目播放叫声。
- [x] 全 151 种条目数据**早已移植**(pokered-data/src/pokedex.rs,生成自 pokemon/*.json),
      无需重做;补数据测试(数量/编号/妙蛙种子/超梦抽查)。
- [x] 捕捉后图鉴登记:新捕获 → SFX_DEX_PAGE_ADDED + 条目屏(item_effects.asm:510-546
      语义;已拥有/老人教程跳过);旧覆盖层御三家硬编码 bug 一并修复。
- 偏差:DATA/CRY/AREA/QUIT 侧菜单未做(A 直接看 DATA);AREA(分布地图)未实现。

### C4 训练家卡片 ✅ 已修正(Wave 4)
- [x] 卡片屏:NAME/MONEY/TIME(play_time 已有追踪)+ 主角正面图 +
      8 徽章槽(未获得显示道馆馆主头像、获得显示徽章,draw_badges.asm 布局);
      APP 素材渲染 + TUI 文本近似。

### C5 进化过场 ✅ 已修正(Wave 4)
- [x] `evolution_screen.rs` 状态机(asm 帧数精确):"What? X is evolving!"(50f)→
      停乐+SFX_TINK→旧叫声→**MUSIC_SAFARI_ZONE 变形乐**(80f,evolution.asm:44-46
      证实 RB 有 jingle)→ 8 轮取消窗+来回闪烁(共 288f)→ "X evolved into Y!"。
- [x] B 键取消("Huh? X stopped evolving!");**进化石不可取消**(wForceEvolution,
      item_effects.asm:777-778),升级/神奇糖果可取消;取消后下次升级重试
      (wCanEvolveFlags 语义,party_leveled_up_flags)。
- [x] 三条触发路径:战斗升级(settle 仅检测、战后播放;**逃跑/捕捉不再触发进化**,
      end_of_battle.asm:29-33 修正既有错误)、进化石、神奇糖果。
- [x] finalize_evolution:换种+能力值重算+HP 差值+改名语义+图鉴 seen+owned+
      **进化等级学招**(原作确实学,evos_moves.asm:212;此前漏)。
- 偏差:叫声/音乐等待用固定帧数代替 WaitForSoundToFinish;满招时学招静默跳过
  (原作开遗忘提示);进化石战后 bug(原作注释)不还原。

### C6 钓鱼 ✅ 已修正(Wave 5)
- [x] 核对 asm 后:Old Rod 必咬、**Magikarp L5**(非 L10);Good Rod 50% 无咬
      ("Not even a nibble!")+ 全局 2 条表(10 GOLDEEN/10 POLIWAG,非按图);
      **Super Rod 也有 50% 无咬**(asm 注释明说)+ 按图 33 条表(数据已存在,
      一致性测试锁定);无表地图 "Looks like there's nothing here."。
- [x] 面向水面/岸边判定(IsNextTileShoreOrWater,9 个 tileset + tile $14/$32/$48)、
      冲浪时拒绝;咬钩 → "The hooked X attacked!" 开场文本 + 正常野生战。
- [x] FishingAnim 动画已移植:钓竿 OAM 件(按朝向,8×24 fishing_rod 切片)、
  咬钩 ±1px 抖动(10×3 帧)、"!" 气泡(60 帧,朝上时隐藏钓竿);动画期间锁定输入,
  结果文本("Not even a nibble!"/"It's a bite!"/nothing here)在动画后弹出。

### C7 Itemfinder + 隐藏道具 ✅ 已修正(Wave 5)
- [x] 54 处隐藏道具表移植(data/events/hidden_item_coords.asm 原序,含不可达条目
      保 flag 索引与 SRAM 位一致);拾取=A 面朝 tile,先于 sign/NPC 判定
      (home/overworld.asm:89-96 顺序);"found X!" + 包满不设旗标(原作语义)。
- [x] Itemfinder:非对称扫描窗(px−5..px+5, py±4),8 声叮(4×HEALING_MACHINE
      + 4×PURCHASE,30 帧间隔)— 找到/未找到两段文本;不可检测隐藏金币
      (原作如此)。
- 注:不需要地图对象(原作也存数据表);TUI 未从存档播种 flag(与 toggleable 同限)。

### C8 通关套件 ✅ 已修正(Wave 5)
- [x] 名人堂记录 + roll-call:进 HallOfFame 地图 → 重置全区间旗标(10 个,
      scripts/HallOfFame.asm:39-44)→ enterHallOfFame() 脚本命令 → 典礼
      (hof_ceremony.rs:淡出→开场 100f→每只怪滚动 48f/信息 80f/文本 180f→
      玩家统计 360f),音乐 MUSIC_HALL_OF_FAME,队伍 push_team(封顶 50,
      溢出保护 hall_of_fame.asm:66-70)。
- [x] 制作人员名单:credits_order.asm 全部 35 屏 + credits_text.asm 文本 +
      mons 顺序,命令帧数 90/110/120/140、剪影滚动 54f、THE END 600f;
      RED/BLUE 版本区分;MUSIC_CREDITS。
- [x] EVENT_STARTED_ELITE_4 置位(LoreleisRoom @load,对应
      LoreleiShowOrHideExitBlock);Lobby 输后重置改为真·旗标组
      (原 3 个废名,rematch 此前已坏);通关重置含 BEAT_CHAMPION_RIVAL。
- [x] last_blackout_map:Heal 效果时记录进入图(跳过 Safari 休息屋,对应
      rest_house_maps.asm);战败传送改为 fly point(落中心外而非中心内,
      与 Teleport 语义一致),真新镇兜底。
- [x] 继续游戏信息面板:30 帧延迟(continfo delay)、PLAYER/BADGES/#DEX/TIME
      四行按 asm 坐标,真名解码(原 "(save)")。
- [x] #MON LEAGUE PC:名人堂查看器(按队伍序号,怪图+昵称/等级/双属性,
      league_pc.asm:16-35)。
- 偏差:roll-call 单图滑动(原作双图)、无玩家图、淡出用白闪近似;
  通关后存档位置移到真新镇 fly point(避免重播 @load 典礼,可见效果一致)。

### C9 选项生效 ✅ 已修正(Wave 2)
- [x] 文字速度:原作 wOptions 低 3 位 = 每字符帧延迟 FAST/MEDIUM/SLOW = 1/3/5 帧
      (ram_constants.asm:38-40);打字机改为 1 字符/N 帧;选项界面三行全部写回 config
      并持久化到 SRAM;启动时应用存档选项,NEW GAME 重置(原作 InitOptions)。
- [x] 战斗模式 Shift/Set:训练家战 KO 后按原作 ReplaceFaintedEnemyMon 流程提示
      "{TRAINER} is about to use {MON}!" + YES/NO(光标默认 NO);YES 先派怪后
      免费换怪(敌方无自由回合);SET/野生/单怪不提示。新 BattlePhase::ShiftPrompt/
      ShiftSwitchSelect,APP/TUI 渲染齐备。
- 偏差:撤退文本统一 "come back!"(原作按 HP% 变体);APP 的 YES/NO 框沿用共享布局
  (右侧),TUI 按原作左上 hlcoord 0,7。

### C10 其余视觉/细节 ✅ 大部分已修正(Wave 3)
- [x] HP 条逐帧扣血(见 A5)。
- [x] Flash 洞穴变暗:核对原作**无光圈**(后续世代才有);Warp 到 Rock Tunnel 时
      wMapPalOffset=6 → FadePal2(白→深灰、其余→黑),到达时立即生效;
      FLASH 后文字关闭时白闪 3 帧恢复(GBPalWhiteOutWithDelay3)。
- [x] 电梯震动:ShakeElevator 移植(200 帧 ±1px BG 滚动 + SFX_COLLISION×100 +
      结束 PA 音),选层后脚本恢复时触发。
- [x] Teleport/Dig/穿洞绳旋转飞出:PlayerSpinInPlace 16 转(延迟 16→1)+ 上升 5 步 +
      SFX_TELEPORT_EXIT_1/2,完成后淡出到白。
- [x] GB 调色板淡出:transition.rs 首批活调用;warp 淡出黑/白、到达 FadePal7→5 淡入、
      战斗后 MapEntryAfterWhite 淡入(替代原线性逐像素变暗)。
- [x] 水面/花朵瓦片动画:UpdateMovingBgTiles 移植(水 tile $14 行旋转 20 帧节奏、
      花 tile $03 四帧 21 帧节奏,计数器 quirk 保留);存档字节按原作语义
      (每次进图从 tileset 头重取,无法永久关闭——忠实)。
- [x] 软复位:A+B+Start+Select 按住 16 帧 → 回标题(仅 app)。
- [x] NPC 交易:核对 asm 后修正审计假设——npctrade 表只有 给/得/对话组/昵称 四字段;
      OT 名固定 "<TRAINER>",OT-ID 与 DVs **随机**(原硬编码 [0x9A,0x78] 两头都不对),
      等级=给出怪等级;新 pokered-data/trades.rs 全 10 条 + CORE/trade.rs 组装。
- [x] 交易动画:InternalClockTradeFuncSequence 移植(给怪→球进线→滑出→文本→滑回→
      收怪→"Take good care!"),真实 SFX 与叫声,队伍变更在动画后(与原作顺序一致)。
- [x] 交易进化:核对 evolve_trade.asm 后确认 RB 英文版 NPC 交易**从不进化**
      ('G'/"SP" 判定是日版蓝遗留,10 条交易均不匹配)——以测试锁定,不接。
- [x] 关键道具 TOSS 守卫(Wave 2 已完成)。
- [x] SRAM nickname/OT/OT-ID/PP-Ups 保真(Wave 2 已完成)。

### C11 视觉小项收尾(Wave 7,feat/tui-visual-gaps)
- [x] **FLASH 白闪冻结移动**:GBPalWhiteOutWithDelay3 的 Delay3 阻塞整个
  overworld 循环(白闪 3 帧内玩家/NPC 全部冻结),不再只闪不退。
- [x] **电梯边缘回卷**:ShakeElevator ±1px 滚动暴露的 1px 行在贴图边缘处按
  GB tilemap 语义回卷进地图(by.rem_euclid),不再显示 border/空白行;
  Pallet/SilphCo/CeladonMansion 三处渲染测试锁定(边缘行与无震动参考帧
  逐像素一致)。
- [x] **到达侧 EnterMapAnim 旋转进入**:新增 `EnterMapSpinState`
  (presentation.rs):淡入后 5 步 16px 下坠(3 帧间隔,SFX_TELEPORT_ENTER_1/2)
  + 8 转原地旋转(延迟 0..7,36 帧);落地在 warp pad/hole 时跳过原地旋转
  (IsPlayerStandingOnWarpPadOrHole);FLY/TELEPORT/DIG/穿洞绳/传送点抵达
  触发(BIT_FLY_WARP|BIT_DUNGEON_WARP 对应),普通门/连接/黑屏战败抵达不触发。
  注:原作 FLY 抵达走鸟动画(FlyAnimationEnterScreenCoords),本移植用
  spin-in 代替(文档偏差)。
- [x] **FLY 地图鸟 + "To" 提示**:LoadTownMap_Fly 移植——"To" 文本 (0,0)、
  城镇名 (3,0)、▲▼ 游标 (18,0)/(19,0)(▲=gfx/town_map/up_arrow.png,
  charmap $ed;▼=字体光标字形)、Pidgey 鸟精灵(16×16,gfx/sprites/bird.png
  第一帧)居中于选中地标(任务描述中的 "TO>" 实为 "To"+▲▼)。
- [x] **图鉴 AREA 玩家标记**:LoadTownMap_Nest 的 DrawPlayerOrBirdSprite 画
  **玩家精灵**(gfx/sprites/red.png 朝下帧)而非实心黑块,16×16 居中于地标
  (x*8+4,y*8+4)。
- [x] **钓鱼 80 帧抛竿停顿**:FishingInit 在 "You used the ROD!" 文本关闭后
  SFX_HEAL_AILMENT + DelayFrames(80),然后才播 FishingAnim(原任务描述
  "文本框在抛竿期间保持"与 asm 不符——PrintText 先关闭文本,再停顿)。
- [x] **进化满技能格学习**:finalize_evolution 满格时不再静默跳过——返回被
  阻挡的招式,过场结束后进 party screen 的"遗忘哪招?"选择(复用
  ChooseMove 流程,finish 走 replace_move_guarded,含 Gen-1 HM 不可删除
  守卫——learn_move.asm:168-181 确认等级学习同样适用);B/取消=放弃学习
  (AbandonLearning)。偏差:原作内联重问 HM 文本,party screen 无法渲染,
  改为提示后结束流程。
- [x] **Haze 解除冰冻旁白**:核实 haze.asm——Haze **会**解除目标非挥发性
  状态(仅目标方),文本只有 "All STATUS changes are eliminated!"
  (text_3.asm:269)。不再复用 CheckDefrost 的 "Fire defrosted X!",且
  Haze 的 stat 重置不再逐项播报。偏差:Haze 状态清除目前双方都清(原作
  只清目标方),受 parity 测试锁定,记录在案。

---

## 执行记录

### 2026-08-02 审计与文档
- 5 路并行审计完成,结论汇总于本文档;分支 `feat/fidelity-audit-fixes-2026-08` 创建。
- Wave 1(164805ae2):A1 幽灵塔 / A2 濒死 / A3 过渡 / B1 D1-D11。
- Wave 2(21e7675fe):B2 徽章+不服从+SRAM / C1 背包道具 / C9 选项。
- Wave 3(2023efd1e):A4 捕捉与动画触发 / A5 出场+GF logo+HP条 / C10 视觉 / 交易。
- Wave 4(059feb42c):C2 PC / C3 图鉴 / C4 训练家卡 / C5 进化过场。
- Wave 5(440c719b4):C6 钓鱼 / C7 Itemfinder+隐藏道具 / C8 通关套件。
- 收尾:全 8 crate 测试通过(pokered-core 2501 / data 366 / app 51 / ui 66 /
  renderer 396 / rules 48 / engine-script 100),`cargo build --workspace` 0 错误。

### 2026-08-08 过渡/进出场动画补丁(分支 fix/transition-anim-gaps)

对照原版复核全部过渡动画后补的 3 项缺失 + 3 项近似修正:
- [x] **开场默认名菜单头像平移**(本次审计用户报告的缺失):`OakSpeechSlidePicRight/Left`
      (oak_speech2.asm:67-160)移植——新 `OakSpeechPhase::SlidePic`,菜单弹出时
      6×6 头像逐列右滑(6 列 × Delay3 = 18f),选定后滑回(13f 预延迟 + 18f);
      玩家与劲敌两路生效,名字选择停靠 x=96px(hlcoord 12,4)。注:Gen-1 命名
      画面本身**无**整屏滑入(那是黄版/绿宝石效果),原版入场仅白闪。
- [x] **开场头像入场动画**:Nidorino/Red `MovePicLeft` 15f 滑入(rWX 119→$FF,
      8px/帧);Oak/Rival `FadeInIntroPic` 6 档调色板淡入(IntroFadePalettes
      原表,6×10f);入场期间锁定输入与打字机(对齐原版 DelayFrames→PrintText)。
- [x] **ShrinkPlayer 结尾补齐**(oak_speech.asm:103-166):红头像 4f → ShrinkPic1 4f
      → ShrinkPic2 20f → 清除 50f → 白淡出 24f(共 102f),SFX_SHRINK 保留;
      FinalSpeech 头像改为 RedPicFront + GBFadeInFromWhite(24f)。
- [x] **命名画面开/关白闪**:`GBPalWhiteOutWithDelay3`(3f 全白)移植,开场命名与
      捕获取名(AskName)两条路径的开/关均生效,TUI 同步。
- [x] **交易动画忠实化**(替换 C10 的近似版):`Trade_ShowPlayerMon` window 滑入
      63f(rWX/hSCX $7e→0,2px/帧);球 poof(18f)/drop(36f)/shake(16f)/tilt(6f)
      子动画复用战斗动画数据(SUBANIM_DATA 0x48-0x4B + move_anim_0 tileset);
      球过电缆 4px/Delay3(96f,x 24→148,SFX_TINK×31)对齐 trade.asm:299-346;
      收怪 tilt→poof→cry;叫声在 drop/poof 末帧触发;结尾 `Trade_SlideTextBoxOffScreen`
      整屏右滑 137f。偏差保留:文本节拍 80f/段(未移植原作 200f+中间滑出),
      SlideBack 沿用球滑回模型仅对齐时序。
- 验证:`cargo test -p pokered-core`/`pokered-app` 全绿(新增 oak 12 项、trade 10
  项测试),`cargo build -p pokered-tui` 通过;各阶段逐帧截图人工核对。
  注:`battle::recharge_lifecycle_tests::thrash_lock_lifecycle_then_confuses`
  存在并行执行下的偶发失败,单测/全量复跑均通过,与本次改动无关(battle/mod.rs
  未动),master 既有。

## 收尾说明

全部审计条目已按上述记录修正或注明遗留。遗留(均有代码注释/文档注明,多为
纯装饰或需引擎级新能力):
- 联机对战/贸易(真实网络传输,脚手架已有) → **已由联机波处理,见下**;
- TUI 过渡动画近似、TUI 无 message-SFX 块、TUI 不播种隐藏道具 flag → **已修(E2)**;
- 图鉴 AREA(分布地图)与 DATA/CRY/AREA/QUIT 侧菜单 → **已修(E2)**;
- 钓鱼玩家侧动画(气泡渲染器仅 NPC) → **已修(E2)**;
- roll-call 双图滚动、credits 逐扫描线滚动等纯视觉近似 → **已修(E2)**;
- BALLBLOCK(正常游玩不可达)、Bill 家直连 PC(坐标事件未接) → **PC 已接(E2)**;
- #MON LEAGUE 查看器已实现,联赛 PC 入口文本已接。

## 引擎级遗留项处理(分支 feat/link-cable-club → feat/link-unify-wasm-engine)

### 联机波(2026-08-03,bf9c1c96d)
- [x] **TCP 传输层**(pokered-app,零 I/O 规则不破):`TcpTransport`(std::net +
      后台读线程 + mpsc,JSON 行帧,serde_json)+ `LinkServer` 非阻塞 accept +
      `LinkSession`(路由分发到战斗/交易两个 CORE manager,事件合并);
      CLI `--link-listen <port>` / `--link-connect <host:port>`;断线 →
      Disconnected 事件。环回 TCP 集成测试全套。
- [x] **联机战斗**(CORE):`link/rng.rs` LinkRng 移植原版 BattleRandom——
      双方各生成 10 字节、**主机列表双方共用**(cable_club.asm:98-171),
      第 9 次抽取触发本地 `(x*5+1)%256` 再填充(wram.asm 注释"shared list of 9",
      第 10 项永不读——bug 级忠实);协议 v2 增 BattleResult;
      BattleScreen link 模式(行动经线、禁用背包/"Items can't be used here."、
      **无经验**(experience.asm:1-4 忠实)、无 SHIFT 提示、金钱归零);
      **联机可逃跑**(core.asm:1503 核对,审计假设有误)——双逃=平局/单逃=负;
      换怪无免费回合。
- [x] **联机交易**(CORE):LinkTradeDriver(任选队伍成员,含濒死/HM/最后一只——
      Gen-1 无限制,cable_club.asm:393-400,790-800 核对);整结构数据保真;
      **联机交易强制进化**(wForceEvolution,4 种;无确认屏);
      接收怪 OT=对方训练家 → 1.5× EXP/不服从生效;先删后加(cable_club.asm:800-817)。
- [x] **Cable Club 集成**(app + 地图):双端时钟仲裁(主机赢,main_menu.asm:216-220);
      桌上班台 invisible sign 触发(原 hidden_event 5,4/4,4);对端玩家精灵按原作
      坐标 (3,2)/(1,2) 显示;原文文本("Just a moment."/"Waiting...!"/
      "PLEASE WAIT!"/"Too bad! The trade was canceled!"/"The link was canceled.");
      断线错误弹窗(原作拔线死锁,此为改进,用原文文本);
      交易动画复用 + 进化过场接线。

### W1 统一驱动(818e7c19a)
- LinkSession 收缩为传输持有者 + 类型路由器;game.rs 拥有 CORE 双驱动
  (LinkBattleDriver/LinkTradeDriver);CableClubFlow 纯房间 UI;
  删除全部重复的回合/队伍簿记(LinkSessionEvent 等);
  BattleScreen: Clone(镜像渲染/结算);断线弹窗一次且可关(修 bug);
  交易在过场中断线不生效(与核心契约对齐)。
### W2 wasm 适配(909f15ee8)
- 修 wasm32 编译(link 模块去门控,transport 保持原生);
  共享 Frame 信封 codec(TCP 与 Broadcast 逐字节同构);自回显过滤;
  BroadcastChannelTransport:同源两标签页免服务器联机;
  `?link=<channel>[&linkHost=1]` + linkJoin/linkLeave 导出;
  `PokemonGame::attach_link_transport` 公开 API。
### W3 引擎抽象(本分支)
- 传输接缝移入 jrpg-engine:`jrpg_engine::link`(NetworkTransport<M> 泛型 trait、
  TransportError、ChannelTransport<M>、LinkRole{Host,Guest})——零依赖零 I/O,
  424 引擎测试;pokered-core 保留 re-export 兼容;全工作区绿。

### E2 波(2026-08-03,af1537cd3)
- [x] **钓鱼玩家侧动画**:FishingAnim 全移植(presentation.rs FishingAnimState:
      10f 起手→100f 钓竿保持→咬钩 10×(±1px 抖动 3f)→"!" 气泡 60f→结果文本);
      钓竿 OAM 按朝向 4 档(下/上/左/右),玩家相对偏移(原版锚底边,移植锚居中,
      已注明);玩家侧气泡渲染器通泛化;不可取消(原作无取消路径);TUI 同步。
      后续修正:文本关闭后先播 SFX_HEAL_AILMENT + **80 帧停顿**再起手
      (FishingInit: PrintText→SFX→DelayFrames(80)→FishingAnim,核对原文修正
      审计假设——文本盒不保留到起手)。
- [x] **图鉴侧菜单 + AREA**:DATA/CRY/AREA/QUIT 四选菜单(位置/光标/常显列表右侧
      与原作一致,ui_label 双语);CRY 留在菜单内;AREA = 全镇图 + 巢穴标记 +
      "'s NEST" 页头,数据来自 grass+water 野生表并集(排除超梦洞,
      原作用内部索引、须经 dex_order 换算——已按物种身份比较),
      **151 种红版 AREA 全表与 asm 逐条对拍测试**;无栖息地 → "AREA UNKNOWN"
      (80 种;含超梦洞专属与礼物/化石/钓鱼限定)。
- [x] **比尔家直连 PC**:BillsHouse (1,4) PC sign + `openBillsPC()` 脚本 API,
      按 EVENT_MET_BILL 门控(设定前显示监视器文本);"Switch on!" 进入
      PcEntry::BillsPc。
- [x] **TUI 残项**:真过渡 wipe 渲染(直接复用 jrpg-renderer BattleTransitionState,
      同 framebuffer);message-SFX 块补齐(FALL/THUD/SuperEffective 等);
      隐藏道具 flag 从存档播种 + 存回。
- [x] **过场打磨**:credits 逐扫描线滚动(8px 带滚动 + 160px 无缝重复 + 白色窗口
      追踪 + 静止信箱条);HoF roll-call 双图滚动(背图 56f + 前图 40f,
      SCX 语义,玩家双图同);HoF 统计页玩家图补齐。

### B/C 波(feat/tui-visual-gaps,2026-08-04)
**B 类 — TUI 平台补全:**
- [x] **TUI 背包全流程**:开始菜单 ITEM 入口接通(原为死路);浏览/USE/TOSS/
      数量选择;道具→队伍(PartyScreenMode::UseItem)含治疗/解状态/PP/维生素/
      神奇糖果/进化石(进化过场复用)/TM-HM 教学+替换 UI;field 道具
      (Repel/Itemfinder/钓竿/笛子/自行车/穿洞绳);关键道具禁扔;FLY 可达。
- [x] **TUI 飞行地图**:TownMap 全镜像(含 `pending_fly_map`、FlyTo→fly_warp)。
- [x] **TUI Slots/电梯/过滤背包/文凭屏**:渲染+输入+脚本恢复全接通,各有 e2e 测试。
- [x] **TUI 软复位**:A+B+Start+Select 16 帧 → 回标题(无存档文件时内存兜底)。
- [x] **TUI Safari 菜单**:BALL/BAIT/ROCK/RUN 网格 + **球数 BALL×NN**
      (原作 core.asm:2077 在 (7,14) 印 wNumSafariBalls;TUI 显示 ×NN)。
- [x] **app 侧两个潜伏 bug 镜像修复**(TUI 发现):
      ① overworld 重建守卫漏 Bag/PartyScreen/TownMap → 用道具/飞行后回图
      会被传送回存档点;② 软复位组合键按住期间 START 仍打开开始菜单 → 组合
      键按住时抑制 START。
- [x] **Safari 球数(app)**:battle_safari.gui 菜单后 draw_text "×NN"
      (与 TUI 一致;原缺口,FIDELITY_GAPS 记录在案)。

**C 类 — 视觉装饰(C11,agent 已在文档记录,要点):**
- [x] FLY 地图:鸟精灵(Pidgey 帧)+ "To"+ 地名 + ▲▼ 光标(town_map.asm
      LoadTownMap_Fly;注:原作是 "To@" 非 "TO>",按 asm 实现)。
- [x] 到达 EnterMapAnim 旋转进入(SpinDown 17f + SpinInPlace 36f + 双 SFX;
      BIT_FLY_WARP/洞/传送门触发;黑出重置)。
- [x] FLASH 白闪 3 帧冻结移动(GBPalWhiteOutWithDelay3 语义)。
- [x] 电梯边缘行回卷(rem_euclid 瓦片行回卷,GB tilemap-wrap 语义)。
- [x] 图鉴 AREA 玩家标记改玩家精灵(DrawPlayerOrBirdSprite,原作如此)。
- [x] 钓鱼 80 帧起手停顿(见上)。
- [x] 进化满招学招 → 遗忘提示流程(IsMoveHM 守卫同样适用升级学招,
      learn_move.asm:168-181;战斗内路径保留原状并注明)。
- [x] Haze 叙事:"All STATUS changes are eliminated!"(haze.asm 确认
      治愈非挥发性状态;不再误显 "Fire defrosted")。注:双端同治愈仍与
      原作(仅目标)不同,被 parity oracle 锁定,注明。
- [-] SE_FLASH_MON_PIC:确认不可观察(触发动画从未启动),近似保留+注释。
- 遗留注明:FLY 到达用旋转进入替代原版鸟飞动画;战斗内满招学招仍静默
  (需战斗内菜单,超出范围)。

### Wave 9(feat/field-polish,2026-08-04)— 历史文档五项收尾
- [x] **Cycling Road 强制自行车**:FORCED_BIKE_TILES 4 块锁定瓦片(Route16/18,
      force_bike_surf.asm:5-14)、进入自动上车、门卫/黑出/飞行传送解除、
      自行车道具拒绝 "You can't get off here."、冲浪拒绝
      "Cycling is fun! Forget SURFing!";Route 17 位持续语义保留。
- [x] **Softboiled 第 9 个野外招式**:FIELD_MOVE_TABLE 8→9 项;HP>max/5 门禁
      ("Not healthy enough.")、使用者 −maxHP/5 转移、目标上限封顶、
      **不耗 PP 不耗道具**;目标选择复用队伍屏(自选循环、B 取消)。
- [x] **SS Anne 开船动画**:VermilionDock_EraseSSAnne 全移植(flag 先置 →
      停乐+冲浪音乐 → 120f → 船身擦除 → SFX_SS_ANNE_HORN → 8×128f
      列滚动+烟囱冒烟 → 二次鸣笛+erase+warp 移除;1267 帧状态机);
      **原作不持久化空船坞**(每次进入重建,忠实);无 "leaving!" 文本(Gen-1 静默)。
- [x] **巨石尘土粒子**:AnimateBoulderDust 移植(8 步×3f、2×2 冒烟块、
      OBP1 闪烁、逆推方向漂移、横推 3/4 瓦片怪癖)。
- [x] **TUI 老爷爷教程 + 背面精灵**:TUI 接 is_old_man + "OLD MAN" 名;
      双方前端入场剪影按 is_old_man 换 OldManPicBack(仅剪影,忠实)。
- [x] **自行车精灵**:Biking 时双前端(render/overworld.rs)换用 red_bike.png
      (原作 LoadBikePlayerSpriteGraphics → RedBikeSprite,home/overworld.asm:
      1977-1990、gfx/sprites.asm:34),6 帧布局与 red.png 相同
      (DownStand=0/UpStand=1/LeftStand=2/DownWalk=3/UpWalk=4/LeftWalk=5,
      Right 镜像 Left),帧/翻转选择逻辑共用;app 冒烟测试验证换图。
- [x] **巨石尘土完成 SFX_CUT**:DoBoulderDustAnimation(push_boulder.asm:89-103)
      在尘土动画 8 步×3f 完成时播 SFX_CUT 一次(BIT_BOULDER_DUST 同步清除);
      core tick_boulder_push 在尘土完成 tick 推入 audio_requests,单发测试。
- [x] **巨石跨图持久化(核实=忠实)**:原作 LoadMapHeader 每次进图从 ROM
      重读全部对象坐标(home/overworld.asm `.zeroSpriteDataLoop`+`.loadSpriteLoop`),
      普通巨石离图再进即复位——本项"未做"即忠实;推入洞/压开关的"后果"经
      事件标志 + 可切换对象标志持久(wToggleableObjectFlags 在 Main Data 存档区,
      ram/wram.asm:1913;HideObject/ShowObject 翻转,CheckSpriteAvailability→
      IsObjectHidden 逐帧生效,movement.asm:478-490;SeafoamIslands1F/B1F/B2F/
      B3F.asm + VictoryRoad3F.asm 用 BIT_PUSHED_BOULDER 握手 + DOWN_HOLE/ON_SWITCH
      事件),端口已用事件标志 + 各层 @load 场景复现(1F/B1F/B2F/B3F 隐藏、
      B4F 显示、VR3F 瓦片替换)。注:端口 B1F/B2F 巨石默认可见,原作默认隐藏
      待 1F 巨石落下后才出现(链式谜题差异,遗留)。
- 遗留注明:Route 17 下坡自动加速未接;海泡沫强制冲浪流未动。

### Wave 10(feat/field-residuals,2026-08-04)— 移动/场地招式残留三项
- [x] **DIG 语义核实=Gen-1 最后一中心**:asm 证实 `.dig`(start_sub_menus.asm:
      195-199)把 ESCAPE_ROPE 装入 wCurItem+wPseudoItemID 后走完整
      ItemUseEscapeRope 流(item_effects.asm:1492-1528),设
      BIT_FLY_WARP|BIT_ESCAPE_WARP;LoadSpecialWarpData 遇 BIT_ESCAPE_WARP
      读 wLastBlackoutMap(special_warps.asm:76-80)→ FlyWarpDataPtr 查表落点,
      wLastBlackoutMap 由 PokéCenter 治疗时 SetLastBlackoutMap 写为进中心前的
      wLastMap(set_blackout_map.asm:1-23;Safari 休息屋除外)。**即传送到
      "最后一个 PokéCenter",不是洞口**——旧"Gen-2 入口语义(刻意)"注记撤销。
      端口:use_field_item(EscapeRope) 与 field_dig 改按 last_blackout_map
      的 FLY_DESTINATIONS 落点;资格检查改按 EscapeRopeTilesets
      (FOREST/CEMETERY/CAVERN/FACILITY/INTERIOR,不再是不在户外的全集,SS
      Anne/GATE/PC 等处拒绝)+ AgathasRoom 拒绝;DIG 不耗道具(伪道具流)。
      注:拒绝文案仍用端口既有 "Can't use that here."(原作
      ItemUseNotTime 为 "OAK: <PLAYER>! This isn't the time to use that!",
      历史偏差,未随本项改动)。
- [x] **Route 17 下坡(强制下移 + 双速)**:JoypadOverworld(home/overworld.asm:
      1826-1835)在非训练家战斗中且无方向/A/B 输入时模拟 PAD_DOWN——整图
      生效,无瓦片表;DoBikeSpeedup(377-388)在按住 UP/LEFT/RIGHT 时取消
      双速(其余地图自行车恒 2×)。端口:update.rs 移动输入注入 PAD_DOWN
      模拟(pending_trainer_battle 非空时抑制),引擎 PlayerState 新增
      bike_speedup_active 旋钮(Route 17 逆坡按住时关闭,Biking 步进 4→8 帧)。
- [x] **海泡沫强制冲浪流**:ForcedBikeOrSurfMaps 的 4 条 SEAFOAM 项
      (B3F (18,7)/(19,7)、B4F (4,14)/(5,14),force_bike_surf.asm:10-13):
      进图 CheckForceBikeOrSurf 置 wWalkBikeSurfState=2 + 切 MOVE_OBJECT
      脚本(player_state.asm:57-66,78-82)。端口 forced_bike 状态机新增
      ForceSurf 变体:进入这些瓦片强制 TransportMode::Surfing(如从 B3F
      洞摔进 B4F 水流);下岸禁止为地形规则(与原作同为 TilePairCollisions
      + 可通行检查,原文在 current 瓦片四面环水自然拒绝);离开瓦片/地图
      无持久锁,传输状态恢复正常。遗留:B3F 的 MOVE_OBJECT/DEFAULT 强制
      冲浪扫描脚本(scene 层,15,8 与 18,7/19,7 的 RLE)仍未移植(见 B3F
      scene 头注释),B4F 侧 currentWest/currentSouthEast 已有。

---

## 第二轮审计修复(2026-08-15/17,分支 fix/fidelity-audit-high-med-2026-08)

六路并行复审(战斗机制/野外地图/静态数据/菜单存档/音频渲染/自身 bug)在既有
文档之外新发现 8 严重 + 21 中等,已全部修复,每项附测试。聚类 commit:

- `7518a27` **战斗**:PP 消耗+Struggle(force) / WriteMonMoves 按等级补招 /
  野生·礼物随机 DV+训练家 $98$88 / 捕捉 Rand1 拒绝重采样 / 逃跑速度口径
  (麻痹·等级·徽章) / Toxic 换人保留 / 暴击忽略 Reflect·光墙 /
  stat 阶段 999/1 钳制 / 道馆主 LoneMoves·四天王 TeamMoves·冠军劲敌特殊招
  (wGymLeaderNo≡wLoneAttackNo 联合体) / CooltrainerF 1/10 阈值无随机换怪 /
  wAICount 仅实际行动扣(DoNothing 掷骰不耗)。
- `63a7c43` **野外**:双锚点遭遇(rate=右邻格(9,9),表=站立格(8,9),左岸
  quirk)/ 180° 转身掷遭遇 / Safari START 步数·球数框 / repel 在掷骰门控内
  递减(warp·脚本·冷却步不烧) / NPC 轴向漫游(UP_DOWN/LEFT_RIGHT,无径向
  leash)+16 帧/步+delay&7F(引擎侧,tag v0.5.2)/ 步完成检测兼容同帧续走。
- `6f559c4` **菜单存档**:Shop 载荷变体守卫(is_ingame_session_screen,
  app+TUI 双端,修退出商店传送)/ 游戏时间常走(VBlank 语义)/ Day Care
  寄养 EV 清零(33 字节 box_struct)/ 覆盖异 ID 旧档确认(CheckPreviousSaveFile
  +GenRandomTrainerID 随机 ID)/ 当前箱号读回(去死写)/ sCurBoxData 镜像同步 /
  背包 SELECT 交换·合并(≤99 单格,溢出 99 封顶)。
- `62881a5` **音频渲染**:老虎机三专属 SFX(NewSpin/StopWheel/Reward 事件队列)/
  深灰市双带路 MUSIC_MUSEUM_GUY / 玩家行走第 4 帧镜像迈步。
- 严重#5/#6/#8 commit(同分支):训练师遭遇前奏(MEET_* 分派+!气泡 32 帧+
  视线走近)/ Spinner 按每图 RLE 表直线滑行(scripts/extract_spinner_paths.py
  生成 71 条,SFX_ARROW_TILES,B1F/B4F 箭头为装饰无表=忠实)/ SRAM 布局改原版
  兼容(UNION 425 字节·Day Care 33 字节·校验和紧跟 sGameDataEnd)+ 旧格式
  检测迁移(legacy 校验和位验证,损坏档拒绝不"治愈")。

引擎仓库(dotzuki-2)`fix/npc-fidelity-2026-08` → tag **v0.5.2**(已推送),
主仓 git 依赖已指向新 tag,[patch] 段移除。

### FIDELITY_GAPS.md 更正(2026-07-18 wAICount 条目)

原记载"Gen-1 在 wrapper 内于例程运行前递减 wAICount,故 DoNothing 掷骰也耗
一次"系误读:trainer_ai.asm:289-322 的 wrapper 只在 $FF 回卷时重载表值,
不递减;递减仅在 DecrementAICount(:453),其调用点全在道具/换怪/强化路径
(:552/:725/:730)。即预算按"累计 N 次实际使用"消耗,非"出场前 N 回合"。
实现与测试(bruno_idle_spends_no_charge)已按此修正。

### 复核遗留(记录在案,低优先)

- 左岸 quirk 读本图草表(原作读跨图 stale wGrassMons 缓冲=Missingno 机制,
  有意不复现,测试内注明);
- 迁移只在读档时发生:旧档一经保存即升为 canonical 格式。

验证:六 crate 全绿(core 2421 / data 223 / app 60 / ui / renderer 398 /
tui 12),引擎 424;verify_scene_translations 与三个数据 verify 脚本 0 差异
(anim verify 经 crates/dotzuki-renderer symlink 保持历史路径)。
