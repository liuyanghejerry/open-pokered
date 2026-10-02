# 后半流程还原审计（2026-10-02，修复前）

审计对象为 open-pokered `72ff719b39634c153cb82d3f3ece200bd413c4e0`，原作为 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`。原作 sparse checkout 的 scripts、home、maps、engine 已从该提交的 Git objects 导出到 `/tmp/pokered-late-ref`；下文原作路径相对于其仓库，行号是该提交的准确行号。

方法：按主线先后对照 `.scene`、`script_config.json`、map objects、核心调度和原作汇编失败分支。2026-10-02 根代理完成当前提交 debug-server 构建后，另用隔离 binary/save/sidecar 的 headless 实例验证满容量分支；这些是**定点状态种子复现**，不是未经修改的新游戏全流程证据。代码确认与实际运行分开列出。未声明已完成所有取消/败北/存读/视觉分支。

## 顺序覆盖矩阵

| 顺序 | 区域/事件 | 已对照的边界 | 结果与验证边界 |
|---|---|---|---|
| 1 | 岩山隧道 1F/B1F | map objects、训练家文本/旗标 | 已读脚本；Flash/黑暗渲染由世界/系统审计覆盖，未另做实机全隧道 |
| 2 | 紫苑镇、宝可梦塔 1–7F、富士老人 | 劲敌、幽灵/Scope、Poké Doll/Run/败北、净化区、Rocket离场、救援与笛子满背包 | 鬼嘎拉嘎拉脚本已分开 `fled` 和 `ran`，原版 Doll skip 有意保留；净化区重入未还原（L02）；塔内物品球满背包丢失（L01） |
| 3 | 彩虹百货、餐厅、游戏厅与奖品屋 | 饮料过滤菜单、钱不足、Coin Case、奖励TM/宝可梦失败 | 饮料菜单现在有 `filterBag`，注释称固定choice已过时；满背包/满储存还有实际缺陷（L01/L03）；奖品确认菜单未还原（L07） |
| 4 | Rocket Hideout B1–B4、电梯 | Lift Key显示、Giovanni输赢、Scope显示、门回调、电梯取消 | 电梯 `-1` 不warp；B4若干物品球成功后不隐藏（L04）；B1/B2满背包错误（L01） |
| 5 | 西尔佛公司 1–11F、金黄城与道馆/格斗道场 | Card Key、门回调重入、劲敌、Lapras、Giovanni、解放NPC、MasterBall失败、拳手失败 | MasterBall正确检查bool；CardKey满背包永久丢失（L01）；Lapras/拳手满储存丢失（L03）；Silph9F休息影响黑屏目标（L08） |
| 6 | 浅红、Safari、Koga、牙齿/HM04 | 付费/拒绝/钱不足、早退YES/NO、500步/30球、Secret House HM03失败 | HM03满背包永久丢失，当前binary确认（L01）；Safari计数存读已交系统代理；Koga奖励已检查bool；牙齿1个交出腾位通常正常，不能仅凭未检查bool认定主分支必丢HM04 |
| 7 | 海泡沫 1F–B4F | 上下楼石头flag、双石水流、洞落点、Articuno正常/逃跑/败北 | 当前脚本包含多层显示/隐藏与水流RLE反向修正；原作逃跑后静态遇敌应消失，当前保留（L05）；未完成所有推石路线实际运行 |
| 8 | 红莲、宅邸、实验室、Blaine | 宅邸开关/门、离开重入、Secret Key失败、三化石选择与完成、满储存、道馆奖励 | 宅邸离开不重置switch（L06）；SecretKey/TM35/TM38丢失（L01）；化石满储存丢失，当前binary确认（L03） |
| 9 | 常磐Giovanni与冠军路 | 败北、徽章/TM补领、Route22重开、开路石/掉落石重入、Moltres | Giovanni TM27成功/失败已分支；冠军路2F重置1F临时石flag缺失（L09，世界代理另核）；Moltres逃跑不消失（L05） |
| 10 | 四天王、冠军、名人堂、结局 | 门封锁、返回大厅重置、输赢旗标、冠军队、HallOfFame重置/接管 | 当前脚本已补前3房门block和返回大厅reset，不重复报旧缺陷；Lance入场与打赢后解锁后门仍与原作不同（L10）；结局 autosave/独立Continue由根代理/存档代理核验 |
| 11 | 通关后洞口、神兽、重挑战 | 名人堂打开CeruleanCave、静态遇敌结束、重新载入 | Mewtwo打赢/捕捉后没有hide/toggle且能继续说话（L05）；已存在的神鸟持久flag测试排除“正常胜利后神鸟又刷新”的旧问题 |

## 确认发现

### L01 — 多处领取忽略 GiveItem 失败，包含主线必需 HM03/Card Key/Secret Key（P1）

- 核心链：`crates/pokered-core/src/overworld/update.rs:629–681` 正确把背包是否容纳该物品返回 bool；`crates/pokered-app/src/game.rs:5937–5941` 的 `add_item` 失败不改变背包。脚本必须只在 bool 为 true 后置领取旗标/隐藏物品。
- HM03：`maps/SafariZoneSecretHouse/script.scene:18–23` 却以 `hasItem("HM03")` 代替容量检查，随后无条件 `giveItem` 和 `EVENT_GOT_HM03`；原作 `scripts/SafariZoneSecretHouse.asm:14–23` 在 GiveItem carry clear 时显示 no-room，并不置flag。
- **运行确认**：装满20种非HM03物品，进入Secret House，与NPC1谈话；显示获得HM03、旗标true，背包仍20种且没有HM03。腾位后再次交谈也仅解释Surf。证据 `evidence/late-hm03-full-bag.json`、同名 `-input.json` 和 `.sav`。
- 同根因具体未检查位置（均为修复前行号）：

| 地图 | giveItem 行 | 后果 |
|---|---|---|
| CeladonDiner | 51 | Coin Case领取flag置位但无盒 |
| CeladonMartRoof | 45、63、81 | 交出饮料后TM13/48/49丢失；若饮料还有剩余数量，20种背包没有释放槽位 |
| PokemonTower3F/4F/5F/6F | 43 / 44、57、70 / 70 / 66、79 | 普通物品球隐藏或领取flag置位 |
| RocketHideoutB1F/B2F | 92、100 / 42、55、68、81 | 普通物品球丢失 |
| SilphCo3F/4F/5F/6F/7F/10F | 97 / 74、87、100 / 154、167、180 / 139、150 / 296、303 / 77、90、103 | 普通球丢失；5F:180的CARD_KEY丢失导致主线门打不开 |
| SafariZoneNorth | 15、26 | PROTEIN/TM40球丢失 |
| PokemonMansion1F/2F/3F/B1F | 53、66 / 47 / 88、99 / 66、79、92、105、126 | 普通球丢失；B1F:126的SECRET_KEY丢失导致红莲道馆无法进入 |
| CinnabarLabMetronomeRoom | 18 | TM35领取flag置位但无TM |
| CinnabarGym | 68、93 | TM38首次及补领都忽略失败、flag置位。原作 `scripts/CinnabarGym.asm:151–162` 明确保留补领机会 |
| VictoryRoad1F/2F/3F | 50、63 / 134、147、160、173 / 75、83 | 普通球丢失 |
| CeruleanCave1F | 16、29、42 | 普通球丢失 |
| WardensHouse | 26、35、62 | HM04补领失败分支/普通球需补bool；正常交出唯一GOLD_TEETH会腾位，应区别处理 |

普通地面物品的原作共同预期见 `engine/events/pick_up_item.asm:30–35`（GiveItem失败不得HideObject）；不是要求修掉原版已知bug。父代理的全地图 GiveItem 修复负责同类项。仅HM03做了本报告中的当前binary运行复现，其余是确定代码链。

### L02 — 宝可梦塔净化区离开后未重新触发、缺少原作保护（P2）

`maps/PokemonTower5F/script.scene:15–16` 只在整张地图重入清 `EVENT_IN_PURIFIED_ZONE`，`76–85` 首次进入后永久置位；同楼离开净化区再进入不再治疗。原作 `scripts/PokemonTower5F.asm:16–37` 在四格之外每帧清flag和 `BIT_NO_BATTLES`，入区设禁止遇敌/HealParty/白闪。当前 `.scene` 用 `fadeOutMusic()` 而非白闪，代码也未设置净化区禁战状态（全core搜索无IN_PURIFIED_ZONE读取）。复现：进四格→离开→战斗受伤→同楼再次踏入，应再次治疗，实际flag仍true。代码确认，未另做受伤后运行复现。

### L03 — 满队伍+满当前PC箱时丢失礼物或化石，奖品屋照扣代币（P1/P2）

- 失败语义 `overworld/update.rs:637–686`：队伍6且当前箱20时 GivePokemon返回false；`app/game.rs:4166–4175` 忽略箱存入错误且仍标记Pokédex owned（这最后一点由系统修复负责）。原作 `engine/events/give_pokemon.asm:7–12、40–44` 返回失败，且不更新owned。
- Lapras `maps/SilphCo7F/script.scene:177–180` 无条件置 `EVENT_GOT_LAPRAS`；原作 `scripts/SilphCo7F.asm:310–320` `jr nc,.done`。**运行确认**：队伍6+当前箱20，对话显示Lapras介绍，party仍6、箱无新增，flagtrue；见 `evidence/late-lapras-full-storage.json`。
- 化石 `maps/CinnabarLabFossilRoom/script.scene:103、109、115、118–123` 三物种失败后仍清全部待领取/种类flag；原作 `scripts/CinnabarLabFossilRoom.asm:77–82` carry clear时不清状态。**运行确认**：准备已复活Kabuto且队伍/当前箱均满；领取后全部状态清除、party/box未增长；见 `evidence/late-fossil-full-storage.json`。这次是从已交出/待领取状态开始的定点测试，未模拟化石交出全链。
- 格斗道场 `maps/FightingDojo/script.scene:104–107、127–130` 失败也藏球并锁定选项；原作 `scripts/FightingDojo.asm:245–246、279–280` 失败直接结束。
- 奖品屋 `maps/GameCornerPrizeRoom/script.scene:54–55、63–64、72–73、87–88、96–97、105–106、133–134、142–143、151–152、166–167、175–176、184–185` 失败仍扣对应代币；原作 `engine/events/prize_menu.asm:225–236` 失败不扣。
- 早期补充：MtMoonPokecenter:65用party>=6禁售，当前赠送逻辑已支持送PC；原作 `scripts/MtMoonPokecenter.asm:47–49` 仅GivePokemon失败禁售。party满但PC有位应能买，实际静默拒绝。Oak初始赠送在正常流程容量不可能满，无需因扫描发现而编造可达bug。

### L04 — 部分“已检查失败”的物品球成功后不隐藏，能够重复领取（P2）

例：`maps/RocketHideoutB4F/script.scene:94–98、105–109、116–120、128–132、140–144` 的给道具成功分支仅显示获得文本，没有隐藏/置flag；`overworld/update.rs:1442–1452` 对itemId球直接进入这些脚本，`app/game.rs:5937–5941` 只加背包，不会自动收球。原作共同Pickup路径会HideObject。复现：正常领取B4的HP_UP/TM02/IRON/Scope/LiftKey后再次A，可再获得同物品。代码确认，运行重复领取留给全地图物品代理；不要以map.json itemId存在为理由排除。

### L05 — 静态神兽逃跑可无限重试；Mewtwo正常完成也不隐藏（P2）

原作 `home/trainers.asm:193–212` EndTrainerBattle仅在 `wIsInBattle==$ff`（败北）返回，否则把static enemy标记 fought并隐藏。因此RUN/Poké Doll正常离场也会消耗一次性的静态宝可梦，原作不提供逃跑后重试。

当前 `maps/SeafoamIslandsB4F/script.scene:99–104`、`VictoryRoad2F` Moltres、`PowerPlant`所有伪装球/Zapdos、`CeruleanCaveB1F/script.scene:27–30` 仅对`win/caught`置flag，RUN不藏对象。Mewtwo额外从未调用hide，`CeruleanCaveB1F/script_config.json:2–7` 没有toggle；原作 `scripts/CeruleanCaveB1F.asm:12–14、25、30–31` 走相同EndTrainerBattle隐藏。

复现：面对神兽，进入战斗→RUN→再次交互，会再开始；Mewtwo胜利/捕获后sprite仍在，后续继续“ Mew! ”。失败/blackout必须保留原作可再挑战，不应简单在战斗开始前藏。当前代码确认，测试覆盖会校验所有结果。

两处Snorlax也只在win/caught消失：`maps/Route12/script.scene:34–46`、`maps/Route16/script.scene:92–104`。此处原作与神鸟的败北分支不同：`scripts/Route12.asm:27–39、46–58`（Route16相同流程）先清FIGHT并在开战前HideObject；正常RUN/caught不打印回山文本但置BEAT，win/Poké Doll打印回山文本后置BEAT；败北不置BEAT，但对象已持久隐藏。修复需保留这一原版细节，不能统一成“失败就重新显示”。代码确认，回归将覆盖开战前隐藏的执行顺序。

### L06 — 离开宅邸到红莲岛未清全局开关（P2）

原作 `scripts/CinnabarIsland.asm:5–6` 每次岛上脚本清 `EVENT_MANSION_SWITCH_ON` 和化石等待。当前 `maps/CinnabarIsland/script.scene:13–15` 只清后者，全core/app无前者补偿。复现：宅邸将开关ON→走回红莲岛→再进宅邸，原作回到OFF，当前ON一直保留，门布局延续上次。代码确认。

### L07 — 奖品屋选择奖品即购买，缺少确认步骤（P3，未还原）

当前 `maps/GameCornerPrizeRoom/script.scene:18–22` 明确省略“So, you want <PRIZE>?” Yes/No。原作 `engine/events/prize_menu.asm:197–206` 选择奖品之后另显示确认，可以取消且不扣币；当前选项直接进入给予/收费。属于交互还原缺口，独立于满储存bug。

### L08 — 非精灵中心治疗改变黑屏/Teleport目标（P1/P2）

`overworld/update.rs:3208–3224` 将任何脚本`heal()`（仅排除Safari三个resthouse）都记录 `last_map` 为 SetBlackoutMap。塔5F净化区和Silph9F床也调用heal；原作分别 `scripts/PokemonTower5F.asm:33`、`scripts/SilphCo9F.asm` 的 HealParty，不调用SetLastBlackoutMap。原作设置黑屏目标的唯一事件调用在 `engine/events/pokecenter.asm:17`。复现：在某城市中心登记复活点→进塔/西尔佛内部治疗→之后败北，预期回此前中心，当前目标可能变为之前经过的内部楼层。代码链确定，最终实际落点由存档/系统代理核验与修复。

### L09 — 冠军路跨楼开路旗标未按原作重置（P2）

世界代理指出并在本报告核对：原作 `scripts/VictoryRoad2F.asm:4–5、18–19` 进入2F清1F开路石flag；原作3F没有相应清2F的逻辑，不能类推补重置。当前2F @load 仅重铺本楼开路块，没有清1Fflag；涉及进入别层/再回来时flag与实际石头位置不同步。推石物理/地图代理负责确证全部范围，不把此前已有掉落石修复重新当新bug。

### L10 — Lance进场自动行走和打赢后的后门保持不一致（P2/P3，未还原）

原作 `scripts/LancesRoom.asm:79–85、96–116` 在入口(24,16)用RLE完整带玩家走到主房（缓冲反向消费：LEFT6→DOWN7→LEFT12→UP12）；当前 `maps/LancesRoom/script.scene:26–32` 只走UP1/LEFT2，将玩家留在走廊。另原作 `LancesRoom.asm:154–157` 仅设BEAT_LANCE，没有清门锁；当前scene胜利分支主动清门锁重开后门，允许回前房。不能把scene注释“freely leave once won”的解释当原作证据。代码确认，完整步数/视觉对照未运行。

### L11 — 交换电影将四个文本合并成两行、遗漏文字和独立停留/滑出（P2/P3，未还原）

此项是字体收口时对通用交换电影的追加对照，适用于 NPC/连接交换。baseline `core/trade.rs:152` 把 WentTo/ForSends/Farewell 均设80帧；`435–446` 把 For/Sends、Farewell/Transferred 各自的两个 PrintText 合并成一个两行长句。原作 `data/text/text_2.asm:28–54` 的 For 是“`For <PLAYER>'s` / `MON,`”，Sends 是“`TRAINER sends` / `MON.`”，Farewell 是“`TRAINER waves` / `farewell as`”，Transferred 是“`MON is` / `transferred.`”。原作 `engine/movie/trade.asm:792–826` 给 WentTo 200帧并滑出，For/Sends 各80帧，Farewell/Transferred 各80帧后再次滑出。当前只有结尾 TakeCare 的滑出，NPC TRAINER 控制码还显示为字面角括号。

复现：以原作8px字格绘制最高7字母训练家名及10字母物种名，旧合并行将超过18格框并裁去后半句；即使旧5px字体暂时装下，也丢失独立文本切换、停留与两次滑出。已代码确证，修复 `21ce784` 按原作拆为四个硬换行文本与各80帧停留、补两次137帧window滑出、WentTo200帧、展开TRAINER。17项 `trade::tests` 全通过，包含第79/80帧文本边界、滑出第50/127帧和最大名字宽度。现已以保留72ff719与实际cd9acde集成binary，定点运行Route11Gate2F真实NPC交谈/YES/实际队伍选择（修后）/ConnectCableText/movie/移除和添加，分别在WentTo、ForSends、Farewell的phase frame40截图，完成1016（base）/1570（修后）帧movie并断言Nidorina/Pikachu与EVENT_TRADED_FOR_TERRY。输入/完整命令响应和binary checksum见 `docs/screenshots/2026-10-02-original-font/trade-text-before/after.json` 与README；不是自然全流程或实体连接协议实测。

### L12 — 连接交换电影丢弃已收到的对方训练家名（P3，bug）

baseline `app/game.rs:5846–5854` 错误注释“名字未在线上传输”并以默认NPC TRAINER构造movie。但同一baseline的 `core/link/protocol.rs:75–78` 已包含 `TradeParty.trainer_name`，`core/link/link_trade.rs:354–365` 已收到并保存 `remote_name`，选择菜单也读取它。原作 `data/text/text_2.asm:20–26,35–47` 的 WentTo/Sends/Farewell均读取 `wLinkEnemyTrainerName`。复现：RED与GREEN执行连接交换，选择Pikachu/Charmander并双方确认；预期“to GREEN.”/“GREEN sends”/“GREEN waves”，base movie实际显示默认TRAINER。

修复原分支 `fdff65e`（集成 `8837a2a`）将driver真实peer name传入movie；native和web共用该hook，TUI无同构link movie调用。生产前端回归通过ChannelTransport双driver完整request/accept/select/confirm→TradeExecute，验证三段文本；同一Rust视觉fixture另经公共 `PokemonGame::update` 实际启动movie，基线截图“to <TRAINER>.”，8837截图“to GREEN.”，均为WentTo phase frame40。fixture/原始库checksum见 `docs/screenshots/2026-10-02-original-font/link-trade-went-40-before/after.json`。此证据覆盖内存传输与真实app构造器，不宣称TCP、实体串行或原作ROM联机兼容已验证。

## 不重复报告/保留原作行为

- 鬼嘎拉嘎拉无Scope也能触发战斗，Doll跳过和1/256 miss等原版bug应保留。
- 新代码已恢复Tower7F Rocket离场、Saffron解放NPC、多个道馆奖励失败分支、Quiz机器与gate/trainer独立flags、各房门block、冠军按starter选队、HallOfFame旗标重置；旧历史注释说“无API/approximation”不等于当前仍缺失。
- `hasItem`/givePokemon容量结果已经可用；失败处理仍缺是具体脚本未消费结果，不是“引擎不支持”。
- 本文未做红/蓝两个版本全剧情完整运行，也未测全部存读/战斗败北；已指出待验证边界，不以静态覆盖等同“彻底实测”。

## late 修复验证补记

`fix/fidelity-late-events` 已对 L02/L03（场景部分）/L05/L06/L07/L10 落实修复。原作给道具失败/物品球隐藏由早期代理，给宝可梦失败的底层 Pokédex/BoxFull 与非中心 heal 黑屏目标由系统代理处理；不能从本独立 late binary 日志宣称这些集成改动已通过。

当前修复构建运行确认：满队/满箱的 Lapras 不置已领取flag；Kabuto 交付失败保留复活/待领取flag；PrizeRoom 满队/满箱仍为9999币（baseline为9819）。三种化石、两拳手、12种红蓝宝可梦奖品、3种TM奖品均由实际 native AST 结果矩阵覆盖成功/失败与确认取消，另覆盖静态遇敌win/caught/ran/fled/lose、Snorlax原版败北前隐藏细节、Cinnabar双重置、Lance完整37步和保持门锁、净化区同楼重入/禁战/白闪6帧保持。`cargo test -p pokered-core --lib fidelity` 27项通过（11项新增），静态持久事件恢复矩阵12对象通过。

同屏同帧截图与完整命令响应位于修复提交 `docs/screenshots/2026-10-02-late-fidelity/`：Lance600帧、净化区122帧、Prize确认223帧；before/after分别以已核验baseline和late修复binary产生。未声称完成红蓝全流程、全部存档/敗北与岩山隧道/石头路由实机覆盖。
