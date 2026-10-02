# 早中期流程逐段原作对照（72ff719）

基线：`open-pokered@72ff719`；原作：`pret/pokered@fbcf7d0`，完整只读副本 `/workspace/onboarding/pokered-reference-full`。下面的行号属于此基线，不是后续修复后的行号。`P1` 表示可永久丢失重要资源或明显破坏游戏状态；`P2` 表示可重复触发的功能/保真错误；`P3` 表示演出、文本、音效差异。没有把旧注释当作证据：已追到当前 native interpreter、OverworldScreen 和 frontend 消费路径。

运行核验使用根代理构建的基线 debug-server 二进制，seed 42，driven-only 模式。`early-runtime.py` 通过 debug API 设置合法边界 fixture，然后用真实按键交互；JSON 记录前后状态和完整命令。它不证明从 NEW GAME 连续打到该位置，原作侧为汇编静态对照，未做 ROM 同帧对拍。

## 按冒险顺序列出的已确认问题

### E01：开局大木叫停缺少主角惊叹气泡（P3，静态）

- 原作：`scripts/PalletTown.asm:177–185`，叫停文本之后 Delay10，然后对主角调用 `EmotionBubble`。
- 当前：`maps/PalletTown/script.scene:39–50`，文本、Delay10、转身、显示大木和移动均有，但没有 `showEmotionBubble("player", 0)` 或等价命令。
- 触发：新游戏第一次北上。实际：没有原作的主角 `!` 演出。验证边界：仅静态，已有 API 是否接受 player ID 需实现时核验。

### E02：御三家选择后对手的移动丢失站位分支（P3，静态）

- 原作：`scripts/OaksLab.asm:194–281`。火/水选择分别按玩家 y=4 与侧面站位选择不同路径；妙蛙种子按玩家 x=9 会重新定位对手再走。
- 当前：`maps/OaksLab/script.scene:166`、`215`、`263`，每一种选择都只有固定路径。
- 触发：从球的侧面选择，而非所有演出假设的下侧。实际：路线与原版不同；某些路径会穿过玩家/桌子。引擎 scripted-path 不做普通碰撞检查（dotzuki `npc_movement.rs:208–225`）。未声称一定卡死。

### E03：包裹送达时对手瞬间出现、领取图鉴阶段音乐未及时恢复（P3，静态）

- 原作：`scripts/OaksLab.asm:510–540`、`554–559`、`674–712`：按主角站位把对手放在 x=4、y=5/6/7，走 UP 2/3/4 到 x=4,y=3；走完立即 `PlayDefaultMusic` 再谈图鉴。
- 当前：`maps/OaksLab/script.scene:393–398`，直接 show 原出生点 (4,3)，无入场移动；从 `389` 的对手音乐到 `439` 才恢复研究所音乐。
- 触发：送包裹。实际：对手瞬现，图鉴整段谈话仍播放对手音乐。注释称“无teleport API”过时，当前有 `setNpcPosition`。

### E04：拿地图后立即再进劲敌家，小茂姐姐仍坐着（P2，已运行复现）

- 原作：`scripts/OaksLab.asm:642–643` 将 Pallet 的当前脚本置为 Daisy；`scripts/PalletTown.asm:133–144` 在真新镇每帧检查两旗标并切换坐姿/走动对象。
- 当前：`maps/PalletTown/script.scene:79–93` 被塞入 `coordNorthExit`；`script_config.json:39–55` 只在 (10,1)/(11,1) 触发，`@load:16–25` 不处理 Daisy。
- 触发：领地图，离开姐姐家，再立即入屋，尚未走到北出口。预期：Daisy走动，显示休息提示；实际：仍是坐姿与地图提示。去北出口后才补做切换。
- 证据：`early-daisy-reentry.json`，地图/进屋旗标均 true，但 Daisy1 visible=true、Daisy2=false，无 EVENT_DAISY_WALKING。

### E05：常磐 TM42 满包仍置领完旗标（P2，静态）

- 原作：`scripts/ViridianCity.asm:234–249` 的 `ViridianCityFisherText` 在 GiveItem carry 失败时进入 `.bag_full`，保持未领。
- 当前：`maps/ViridianCity/script.scene:121–122` 无条件 giveItem 后置 EVENT_GOT_TM42。
- 触发：包里20类道具且没有 TM42。实际：TM42添加失败，后续对话只剩完成提示，无法重领。需要正常使用 CUT/SURF 到达，此问题不限定“第一次经过常磐”。

### E06：大量地面球在满包时被永久清掉（P1/P2；代表项已运行）

- 原作统一机制：`engine/events/pick_up_item.asm:20–42`，GiveItem失败只显示 NoMoreRoomForItemText；成功才 HideObject。
- 当前入口确实执行 `.scene`：`overworld/update.rs:1442–1452`。没有覆盖场景返回值的自动隐藏/回滚；`game.rs:5937–5944` 忽略 bag.add_item 的 Err。
- 本阶段明确受影响：Route2 `script.scene:13–15,26–28`；MtMoon1F `86–89,99–102,112–115,125–128,138–141,151–154`；Route4 `31–33`；Route24 `116–118`；Route25 `119–121`；SSAnne2FRooms `69–71,94–96`；SSAnneB1FRooms `119–122,135–138,151–154`（B1F球未隐藏，但完成旗标让它永远变空）。
- 触发：20类道具且不存在目标道具。预期：球保留，可以清包后再拿；实际：物品未入包但物件隐藏/完成旗标已置。Route24 TM45、MtMoon1F 月之石实测符合。
- 证据：`early-route24-tm45-full-bag.json`；`early-mtmoon-moonstone-full-bag.json`。未逐球运行，其他列出的同构路径静态确认。

### E07：鲤鱼王商人拒绝原作允许的“队伍满→箱子”（P2，已运行）

- 原作：`scripts/MtMoonPokecenter.asm:46–62` 调用 GivePokemon，仅失败时不扣钱；`engine/events/give_pokemon.asm:7–39` 队伍6只时会进当前PC箱，队伍和箱都满才失败。
- 当前：`maps/MtMoonPokecenter/script.scene:65–73` 先用队伍数量阻断，因此即使箱空也不能买。过时注释称 givePokemon 无返回值/满队静默丢弃，与当前 `update.rs:637–687`、`game.rs:4163` 不符。
- 触发：6只队伍、PC空箱、≥500元，选择 YES。预期：得到Lv5鲤鱼王存箱，扣500且置买过旗标；实际：静默结束，钱与旗标均不变。
- 证据：`early-magikarp-party-full.json`：party_count=6，money仍3000，EVENT_BOUGHT_MAGIKARP不成立；fixture通过 `--save` 指向不存在的临时存档启动空PC，再用 `interact_with npc:3` 实际寻路并对话（商人会走动，固定朝上坐标不足以证明交互）。初次固定坐标的无对话记录已由该记录替换。

### E08：月见山 Water Gun 球错误给 Bide TM34（P2，静态）

- 原作：`data/maps/objects/MtMoon1F.asm:42` npc13 是 `TM_WATER_GUN`；`constants/item_constants.asm:172,194` WATER_GUN=TM12，BIDE=TM34。
- 当前 map.json:204–213 的 itemId=212 ($D4=TM12) 正确，但 `.scene:145–154` 声称“TM34 / WATER GUN”、实际 `giveItem("TM34",1)`。
- 触发：正常拾取月见山1F (5,32) 地面球。预期：TM12 WATER GUN；实际：TM34 BIDE。必须以真实场景奖励为准，静态 map 数据正确没有消除此错误。

### E09：月见山B2F HP_UP / TM01 可无限重复领（P2，HP_UP已运行）

- 原作：`scripts/MtMoonB2F.asm:167–168` 绑定 PickUpItem；上面原作统一机制成功后隐藏一次。
- 当前：`maps/MtMoonB2F/script.scene:193–205,208–220` 检查given并显示文本，却没有HideObject、完成旗标或其他成功后清理。
- 触发：空包，连续对同一个地面球按A。实际：HP_UP数量增至2，球仍可见；TM01结构相同。
- 证据：`early-mtmoon-hp-up-repeat.json` after bag=[HpUp×2]，npc8 visible=true。队伍属性永久增强可无限获得，改变资源平衡。其他地图同型遗漏需全局清单，不能从“已检查given”推断拾取完整。

### E10：分化石后理科男直接隔空收走另一块（P3，静态）

- 原作：`scripts/MtMoonB2F.asm:90–155`，按照玩家靠近哪块化石，理科男走 RIGHT+UP 或 UP 到另一块球，等移动完成再说话、隐藏球。
- 当前：`maps/MtMoonB2F/script.scene:142–146,176–180` 直接说话并隐藏，未移动理科男。
- 原作满包/取消化石选择保留两球的分支在当前 `135–151,169–185` 已实现，不重复报旧缺口。

### E11：小霞 TM11 满包仍置领完旗标（P2，已运行）

- 原作：`scripts/CeruleanGym.asm:46–62` 失败只显示NoRoom，不置 EVENT_GOT_TM11；`92–99` 重谈重试。
- 当前：`maps/CeruleanGym/script.scene:15–17,40–42` 两分支都未检查given，且无条件置旗标。
- 触发：满包打赢小霞，或胜后重领TM时满包。实际：没有TM11但旗标已真，再谈无法拿到。
- 证据：`early-misty-full-bag.json`，fixture选择胜后补领分支；同构胜利路径静态核对。

### E12：华蓝桥右侧遇敌时把NPC内部坐标误当地图坐标（P2，静态）

- 原作：`scripts/CeruleanCity.asm:83–99`，右侧主角 x=21 时把 rival SPRITESTATEDATA2_MAPX 写25，然后DOWN×3。原作NPC坐标包含4格边框，`macros/scripts/maps.asm:16–19` 的 object_event 写 y+4/x+4；真实NPC地图x=21。
- 当前：`maps/CeruleanCity/script.scene:34–47` 把25当真实地图x，`setNpcPosition(...,25,2)`，然后在25列往下走；退场 `84` 则沿24列。
- 触发：从桥右侧(21,6)触发。预期：对手到(21,5)贴近玩家；实际：隔4格出现在东边院落，仍隔空开战。左侧(20,6)路径未发现此问题。

### E13：华蓝桥胜利后跳过对手介绍Bill的大段对话（P3，静态）

- 原作：`scripts/CeruleanCity.asm:175–178` 先置 EVENT_BEAT_CERULEAN_RIVAL，再调用 RivalText；`255–268` 因旗标为真显示 `CeruleanCityRivalIWentToBillsText`。
- 当前：`.scene:69–88` 置旗标后直接走出场；虽然`talkBlue:94–99`保存了Bill台词，但对手随后隐藏，普通玩家不能再触发。
- 触发：华蓝桥胜利。实际：战败评语之后立刻退场，没有 PC/Bill/稀有宝可梦的引导文本。

### E14：华蓝盗贼满包后隐藏，永久丢TM28（P2，已运行）

- 原作：`scripts/CeruleanCity.asm:305–320` GiveItem失败显示NoRoom且不隐藏；胜败旗标已立仍可重谈领奖。
- 当前：`maps/CeruleanCity/script.scene:123–130,138–142,167–174` 全部无条件给物、显示成功、隐藏。
- 触发：20类且无TM28，战胜或胜后补领。实际：无TM28却把盗贼永久隐藏。
- 证据：`early-thief-full-bag.json` 用原作存在的“已胜但尚未拿到TM”补领分支，after 无TM28且隐藏旗标true。首次胜利的两入口静态确认相同问题。

### E15：金珠桥满包时没有原作的退一步（P3，静态）

- 原作：`scripts/Route24.asm:35–44,142–145` 满包置EVENT_NUGGET_REWARD_AVAILABLE，DefaultScript随后模拟PAD_DOWN一步。
- 当前：`maps/Route24/script.scene:43–47` 置旗标却不movePlayer。注释说DSL不能读取玩家坐标过时，现有 `getPlayerX/Y` 与相对移动。
- 触发：在(10,15)自动领取金珠时满包。当前正确保留金珠待领，错误仅退位演出/重新进入的交互位置；不把失败时不给金珠当未实现。

### E16：Bill绕人路径与出机坐标错误（P2/P3，静态）

- 原作：`scripts/BillsHouse.asm:19–46` 主角 facing DOWN 时Bill先RIGHT，UP×2，LEFT，UP绕开玩家；其余UP×3。`62–97` 出机时 internal map=(5,6)，实际=(1,2)，DOWN、RIGHT×3、DOWN到(4,4)。原作内部+4规则见E12。
- 当前：`maps/BillsHouse/script.scene:35` 只有UP×3，如果玩家在(6,4)向下说话会直接穿过主角；`57–60` 出机设置实际(6,2)，再向左往(4,4)，是另一台机器与反向路径。
- 触发：在Bill上方说话/运行细胞分离。旗标和票的满包重试当前正确，本条不声称主线不能完成。

### E17：自行车满包仍销毁兑换券并锁死领取（P1，已运行）

- 原作：`scripts/BikeShop.asm:23–36` 先GiveItem，成功才移除兑换券、置完成旗标；失败保留兑换券并BagFull。
- 当前：`maps/BikeShop/script.scene:19–23` 无条件giveItem→takeItem→setFlag。
- 触发：20类含兑换券、没有自行车。实际：Bicycle未加入，券消失，EVENT_GOT_BICYCLE=true，店员再也不兑换。自行车道通行受影响。
- 证据：`early-bike-full-bag.json`，20类变19类、无Bicycle，旗标true。玩家站在柜台外合法可达(6,4)执行交互。

### E18：船长揉背音乐缺少等待结束与恢复船音乐（P3，静态）

- 原作：`scripts/SSAnneCaptainsRoom.asm:45–69` 播放治愈音乐、等待声道不再为 MUSIC_PKMN_HEALED、`PlayDefaultMusic` 后才置揉背旗标。
- 当前：`maps/SSAnneCaptainsRoom/script.scene:33–37` playMusic后直接继续台词，无等待/恢复地图BGM。船长转身保护亦未实现（原作:5–10,36–37,67–68，当前注释:38–40,56–58主动省略）。
- HM01满包可重试当前已正确，且揉背不治疗队伍亦正确，均非新缺口。

### E19：离船演出不检查入码头来自哪一侧（P2，静态）

- 原作：`scripts/VermilionDock.asm:5–11` 只有 GOT_HM01 且 destinationWarpID=1（从船返回）才开走船脚本。
- 当前：`maps/VermilionDock/script.scene:38` 仅GOT_HM01与自定义一次旗标，忽略来源。
- 正常可达触发：先拿HM01，在船上后续训练家战全灭回中心，再从枯叶市进入码头（尚未从船口返回）。预期：船仍在，可以回船；实际：从城市进入也驶离。
- 验证边界：来源判定差异代码确认，未连续打败黑化复现场景。不要将调试warp本身当正常玩家入口。

### E20：马志士垃圾桶谜题使用了另一个随机算法与存档生命周期（P2，静态）

- 原作：`scripts/VermilionCity.asm:16–20` 入城把RandomSub AND $0E作为第一桶，仅偶数索引；失败重置 `engine/events/hidden_events/vermilion_gym_trash.asm:86–91` 同样AND $0E。第二桶原作:50–74保留著名AND/DEC错误，可能落索引0，并非均匀四邻。第一锁事件与两桶索引位于存档MainData内，`ram/wram.asm:1751,2138–2143,2223`。
- 当前实际native实现：`overworld/native_script.rs:535–557,586–590` 用%15（允许奇数桶），第二桶均匀有效四邻；`465–491` 引擎map实例新建phase=0，`start:528–535` 不读取 EVENT_1ST_LOCK_OPENED。
- 触发：随机找桶、找到第一锁后保存读档/重建地图实例。实际：与原版候选桶、概率、存读状态不同。注意scene的 Math.random 注释过时，当前走native特例；不能报告成@run未实现。
- 本条是原版行为（包含原版bug）的保真差异，当前谜题本身能解。没有把“修复原版著名bug”直接描述成Rust死锁。

## 覆盖矩阵与已排除旧结论

| 流程 | 已读到原作并对照的分支 | 当前结论 / 验证边界 |
|---|---|---|
| 主角家母亲 | 初见/已有伙伴治疗、床/PC入口 | 本轮未确认新的分支错误；未ROM对拍 |
| 真新镇叫停/跟随 | 北出口双格、对象隐藏、入研究所 | 主干已有修复；E01，Daisy见E04 |
| 御三家/首战 | 三种选择、NO、类型优势、败后治疗、告别 | 主干和胜败继续已存在；侧面路径E02，未完整运行三种首战 |
| 包裹/图鉴 | 包裹给物、交付移除、老头换人、Route22激活 | flag/item主干存在；E03。原作包裹本来不检查GiveItem，不能机械套满包bug模板 |
| 城镇地图 | 未有图鉴/领完/满包 | 已有given与失败保留，旧“不给返回值”注释不算缺口；Daisy时机E04 |
| 常磐老人教学 | NO/YES、教程、领取图鉴之后对象切换 | 真实oldManTutorial已接线；TM42见E05 |
| Route1/森林 | 伤药赠送/满包，3个地面球成功与失败 | 有given与失败保留；森林拾取有成功隐藏；未重跑所有训练家战 |
| 尼比/博物馆 | 向导YES/NO、东出口阻挡、小刚败/胜/TM重领、门票方位 | 门票方位及小刚TM满包分支已补；escort仍有演出近似，未把旧stub断言为主线阻塞 |
| 月见山 | 商人YES/NO/钱不足、1F全部球、理科男败/胜、两化石NO/满包/成功 | E06–E10。化石互斥/失败保留已存在 |
| 华蓝/桥 | 左右桥触发、小霞败/胜/TM、盗贼两入口/满包、金珠满包 | E11–E15；左桥主干已存在 |
| Bill | YES/NO都帮助、PC帮助前后、票满包重领/警卫切换 | 票成功/满包正确，Bill移动E16 |
| 枯叶/自行车 | fanclub接受/拒绝/满包、兑换/买不起/已有自行车、钓竿接受拒绝满包 | fanclub、OldRod满包已正确；自行车E17 |
| S.S.Anne | rival两站位/胜后提示、船长HM满包重领、房间道具、驶离入口 | reward主干存在；房间道具E06、船长E18、入口E19 |
| 马志士 | 未解/解谜入场块、馆主败/胜/TM满包重领、3训练家停战 | 道馆主奖励已given处理；谜题实际native已实现；随机与存读E20 |
| 前期帮助道具 | Route2Gate FLASH、Route11Gate2F Itemfinder | FLASH已有given保留；Itemfinder:script.scene71–72仍无条件完成，需与全局奖励清单一起修 |

## 全地图返回值补充清单

`unhandled-gift-calls.json` 从当前 `.scene` 源解析出112处未赋值的giveItem/givePokemon调用，保留文件、行号与后8行。**112不是bug数量**：包裹与大木的赠球原作本来也不检查；初始御三家没有正常满队入口；一些givePokemon先阻止满队，错误是禁止箱回退；另一些场景先消耗兑换材料，是否释放格位需要数量>1边界。需要逐项依原作决定。

本轮明确没有重新报 `docs/FIDELITY_GAPS.md` 已修复的 fanclub满包、Eevee箱满、森林拾取、博物馆方位、field move基本接线等。此处“未发现新问题”不等于“全部像素/帧/随机种子100%一致”。
