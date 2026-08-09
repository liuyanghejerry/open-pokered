# pokered 剧情/内容补全计划

跟踪文档:本文件是 pokered 复刻版相对于原版(pret/pokered)缺失内容的补全计划与执行状态。
分支:`feat/pokered-gap-completion`,每阶段独立提交/PR。

## 背景与目标

对照原版 worktree(`/Users/liuyanghe02/develop/pokered-worktree`,224 个 asm 脚本),
复刻版(`crates/pokered-data/maps/`,248 个场景)地图已全覆盖,
但存在约 101 条 TODO/STUB 分布在 53 个地图中,以及若干纯内容缺失(占位对话)。

目标:逐阶段核实并补全,使剧情内容与原版对齐。
验收原则:该阶段场景 TODO 清零 + 新增 API 测试 + pokered-core/引擎全量测试通过。

## Phase 0 审计结论(已完成,2026-08-01)

- 119 行 TODO/STUB 逐条核实(3 组并行审计 + 人工复核矛盾项)。
- **~45 条为过时注释**(能力已实现,注释声称缺 API)→ 已清理/更新措辞(31 个场景文件)。
- **1 处纯内容缺失**:MtMoonPokecenter npc4 实际是**鲤鱼王商人**(¥500 卖 Lv5 MAGIKARP,
  非一句对话)→ 已补完整流程(`talkMagikarpSalesman`)。
- **跨图对象 toggle 已实现**(`hideObjectByName`/`showObjectByName` 写 `__OBJ_HIDDEN_<id>`
  unified_flags + SRAM toggleable 位,目标图 `apply_hidden_object_flags` 加载时应用)
  ——此前误判为缺失,实为接线工作。
- **Safari 计时/球数引擎侧已实现**(warp 进 safari 图自动武装 500 步/30 球、逐步扣减、
  0 步/0 球 PA 提示 + 弹回门)——此前误判为缺失。
- **tradePokemon 已完整实现**(6 个贸易屋 + CinnabarLabTradeRoom + 化石房 + Route18Gate2F 全部在用)。
- **FuchsiaGym 隐形墙 / SaffronGym 传送迷宫 = 纯地图数据,已实现**(blk/warp 与原版逐一致)。
- **推回类全部是固定步数**(单步 PAD_DOWN/UP 或定长几步),`movePlayerRelative` 已足够——只剩接线。

### 真缺口分类(Phase 0 产出)

| 类别 | 条目 | 分布 |
|------|------|------|
| 徽章查询 API | getBadgeCount/hasBadge 脚本侧缺失 | ViridianCity:37、ViridianGym 门锁、CeruleanBadgeHouse:13 |
| `<STARTER>` token | 御三家名字 token 缺失 | ChampionsRoom:79 |
| 区域野生禁用 | 化石区禁止遇敌 | MtMoonB2F:197(x11-14,y5-8) |
| 电梯楼层菜单 | DisplayElevatorFloorMenu 等价缺失 | CeladonMartElevator、SilphCoElevator、RocketHideoutElevator |
| 过滤背包菜单 | filterBag 动态菜单缺失 | CeladonMartRoof:36(饮料)、CinnabarLabFossilRoom:27(化石)、CeruleanBadgeHouse(徽章列表) |
| TM 入包 | ItemId 无 TM/HM 变体,giveItem("TMxx") 静默失败 | GameCornerPrizeRoom:14/145、CeladonMartRoof 小女孩(TM13/48/49) |
| 箭头瓦片/传送带 | 持续强制移动原语缺失 | ViridianGym:164、RocketHideoutB2F、RocketHideoutB3F |
| 巨石落洞 | boulder 推入洞检测缺失 | SeafoamIslands 1F/B1F/B2F/B3F、Route20:16 |
| 开船动画 | 硬件级 OAM/VRAM 特效缺失 | VermilionDock:32 |
| Bill's PC | Cell Separation System 屏幕缺失 + 地图缺机器对象 | BillsHouse |
| HOF/Diploma/save-reset | HOF roll-call、E4 重置、通关后 Init | HallOfFame:29/31/33、CeladonMansion3F:37 |
| link battle/trade | 联机 API 缺失 | Colosseum、TradeCenter、IndigoPlateauLobby:87 |
| 小项 | ShakeElevator(装饰)、MUSIC_SS_ANNE 名 mangling bug | 电梯 ×2、SSAnne2F:101 |

### 已实现(注释清理完成,无需补全)

FuchsiaGym 隐形墙、SaffronGym 迷宫、Safari 计时/球数、tradePokemon(7 处)、
OpenSlots/硬币、MEWTWO/SNORLAX/Voltorb/Zapdos 野生战、跨图 toggle、
MovePlayerRelative(推回)、ReplaceTileBlock(GameCorner 海报/VR2F 地板)、
TMs 之外的一切售货机逻辑。

## 阶段划分(Phase 0 完成后修订)

### Phase 0 — 审计与核实 ✅ 完成
- [x] 119 行 TODO 逐条核实,剔除已实现项
- [x] 清理 ~45 处过时注释(31 文件)+ 4 处清单外过时注释
- [x] 补 MtMoonPokecenter 鲤鱼王商人流程
- [x] 产出核实后缺口清单(见上表)

### Phase 1 — 查询/数据 API ✅ 完成
- [x] `getBadgeCount` / `hasBadge` 脚本 API — ViridianCity talkGambler(all-but-EARTHBADGE)+ (32,8) gym 门锁 coord 接线
- [x] `<STARTER>` token — ChampionsRoom "first left with <STARTER>!"
- [x] 区域野生禁用 — MtMoonB2F 化石区(x11-14,y5-8,Super Nerd 战后)
- [x] `elevatorMenu(floors)` — CeladonMart(5 层)/SilphCo(11 层)/RocketHideout(B1F/B2F/B4F)+ app 菜单屏 + tui 取消
- [x] 跨图 toggle 接线 — BillsHouse→CeruleanCity 守卫切换;PokemonTower7F/OaksLab/Route25 确认为功能已等价或内容缺口(注释更新)
- [x] **修复既有 bug A**:DSL codegen 把 Assign(含命令调用)无条件提前到块头 → `result = startBattle(...)`/`floor = elevatorMenu(...)` 在对话前执行;现仅提升纯变量声明,命令调用保持原位(回归测试 `test_call_assign_stays_in_place`)
- [x] **修复既有 bug B**:`ScriptEffect::WarpTo` 在 apply_finished_effect 为空实现 → `warpTo` 命令从不生效;现走 pending_warp + fade(端到端验证:电梯选 3F → warp 到 CeladonMart3F)
- 验收:新 API 单测 + 场景接线 + 全量测试通过;headless 端到端验证电梯全流程

### Phase 2 — 移动/谜题(接线为主)✅ 完成
- [x] 推回接线 — Route23 ×7、LoreleisRoom、AgathasRoom、Route18Gate1F(coordIndex 路径)、VermilionCity(票检 (18,30))
- [x] 落洞接线 — PokemonMansion3F 3 洞(1F/2F)+ Seafoam 四层玩家落洞(coordEvents + warpTo)
- [x] CinnabarLabFossilRoom 就绪状态对齐(CinnabarIsland @load resetFlag + 分支修正)
- [x] 售货机购买接线 — CeladonMartRoof 3 台(¥200/¥300/¥350 + 包满/钱不够分支)
- [x] 箭头瓦片/传送带 — **已由引擎 spinner 强制移动实现**(update.rs),审计误判;Viridian Gym/RocketHideout 注释更新
- [x] 巨石落洞 — core `tick_boulder_push` 检测 Seafoam 洞表(推巨石进洞 → 隐藏 + DOWN_HOLE flag);B1F 巨石 `hidden=true` bug 修正;各层 @load 保持隐藏
- [x] 强制冲浪(B3F 巨石全落洞后)未复现(注明)
- 验收:场景 TODO 清零(仅剩 Phase 3 filterBag)+ 全量测试通过

### Phase 3 — 经济/菜单 ✅ 完成
- [x] **TM 入包** — ItemId 增 Hm01-05/Tm01-50 变体(GB id $C4-$FA),giveItem("TMxx") 解析;GameCornerPrizeRoom TM 奖品(TM23/15/50)全流程、CeladonMartRoof 小女孩 TM13/48/49 自动生效
- [x] **filterBag** — 新命令:过滤背包菜单(只显示携带的候选),返回选中物品名;挂起/恢复模式复用 elevatorMenu 机制。接线:CeladonMartRoof 饮料菜单、CinnabarLabFossilRoom 化石菜单
- [x] **徽章列表** — CeruleanBadgeHouse 8 徽章描述按 hasBadge 门控(未拥有显示 "You don't have that BADGE yet.")
- [x] Safari 查询 API — **跳过**:引擎已内置计时/球数,原版对话文本固定不显示数量,无场景需求
- 验收:场景 TODO 清零 + 新 API 测试 + 全量测试通过

### Phase 4 — 贸易/联机/通关后 ✅ 完成
- [x] **MUSIC_SS_ANNE 修复** — `MusicId::from_name` 大小写/下划线归一化("MUSIC_SS_ANNE"/"SsAnne"/"SSAnne" 均解析);SSAnne2F 战后恢复主题
- [x] **Diploma 屏** — `showDiploma()` 命令 + 全屏文凭(app 屏:玩家名 + 祝贺 + GAME FREAK);CeladonMansion3F 接线
- [x] **Bill's PC** — 机器对象(sign)+ 两段式流程:对话 Bill → 引导去机器 → Cell Separation System 转换(Bill 人类形态);map.json 加 PC sign
- [x] **HOF** — roll-call 文本登记 + E4 重置由 IndigoPlateauLobby @load 覆盖;save-restart 无脚本 API(注明,留前端)
- [x] **link 占位** — Colosseum/TradeCenter/IndigoPlateauLobby 保留 "!" / "making preparations" 占位并注明联机缺失
- [x] ShakeElevator(装饰)— 保留 TODO(纯装饰,低优先)
- 验收:全量测试通过(失败 0)+ roundtrip 通过

## 提交历史

- Phase 0(df6834ddb):审计 + 注释清理 + MtMoonPokecenter 商人
- Phase 1(9e3effe1c):badge 查询 / `<STARTER>` / 化石区野生禁用 / elevatorMenu / 跨图 toggle 接线 + 修复 DSL Assign 顺序与 warpTo 空实现两个既有 bug
- Phase 2(965d2ff8b):推回/落洞/就绪/售货机接线 + 巨石落洞引擎检测 + spinner 瓦片确认已实现
- Phase 3(1ebafdcb5):TM 入包(ItemId TM/HM 变体)+ filterBag 过滤背包菜单 + 徽章列表 hasBadge 门控
- Phase 4(本分支):Diploma 屏 / Bill's PC 两段式 / HOF roll-call / link 占位 / MUSIC_SS_ANNE 修复

## 收尾说明

四个阶段全部完成:pokered 复刻版的剧情/内容缺口已按计划核实并补全。
遗留(非剧情缺口,需引擎级新能力):link battle/trade 联机、Bill's PC 完整
Cell Separation 屏幕、HOF roll-call 全屏、通关后 save-restart、电梯到达震动
(装饰)。这些在计划文档各阶段注明。
