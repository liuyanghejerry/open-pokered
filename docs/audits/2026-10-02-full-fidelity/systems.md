# 系统保真审计（基线 72ff719；原作 fbcf7d0）

审计次序：背包/商店 → PC/箱子 → 育成 → NPC/联机交换 → 图鉴 → 存档。下面列的是当前代码的新确认项；`FIDELITY_GAPS.md` 中 PC 无入口、图鉴未填充、联机不存在等旧报告不再成立。原作参考仓库为 sparse checkout，部分 ASM 用 `git show HEAD:path | nl -ba` 读取，行号均属于该固定提交。

验证：当前源码构建；`systems_probe.rs` 临时复制为 `crates/pokered-core/tests/audit_systems_probe.rs` 运行，8/8 观察成立，随后删除临时测试。输出在 `systems-probe.log`。这些观察测试断言的是基线实际问题，并不是对正确行为的验收。

| ID | 严重度 | 触发与实际结果 | 原作预期与精确依据 | 当前实现精确依据 | 验证 |
|---|---|---|---|---|---|
| SYS-01 | 高 | 背包20槽，Potion95；商店买10返回 BagFull，但Potion变99、金钱不扣 | 满格溢出在写99之前拒绝；`engine/items/inventory.asm:68-75` | `items/inventory.rs:95-115` 先改已有槽再报错；`items/shop.rs:223-226` 失败不扣钱 | 运行probe；95→99，10000不变 |
| SYS-02 | 中 | 999999金钱卖Nugget得到1004999；运行时越过原作封顶，存SRAM时又被截为999999 | 3-byte BCD 溢出填$99；`home/inventory.asm:7-11` / `engine/math/bcd.asm:169-188` | `items/shop.rs:257` 只u32 saturating_add；`save/ser_game_data.rs:12-22` 保存另行cap | 运行probe |
| SYS-03 | 高 | 战斗获得stat-exp但未升级，存入取出不刷新能力；PikachuLv50 Attack69应101却仍69 | BOX_TO_PARTY 根据EXP算等级、带stat-exp算能力；`engine/pokemon/add_mon.asm:496-514` | `pokemon/pc_box.rs:71-81` 原样返回；生产`pc_screen.rs:873-875`直接add | 运行probe；原作box trick缺失 |
| SYS-04 | 严重 | 满6只捕获写入`current_box`，PC取出却读`pc_storage`；精灵不可见；切箱又以pc_storage覆盖current_box | 当前箱只有一个wBoxData，捕获、PC、保存共用；`engine/pokemon/bills_pc.asm:262-287`、`engine/menus/save.asm:229-260` | `battle/settlement/writeback.rs:189-195` 仅写current_box；`app/game.rs:5351-5355` PC仅接pc_storage；`app/game.rs:5386-5387`切箱覆盖；`app/game.rs:2909`捕获满箱判定读current_box | 已确认生产调用链；修后真实 settlement→PC→SRAM integration验收 |
| SYS-05 | 高 | 育成无须存读，领取已训练/交换/使用PP Up精灵后stat-exp归0、OT=0/空白、is_traded=false、PP Ups归0 | 33-byte box struct原样复制+OT表复制；`engine/pokemon/add_mon.asm:409-458`、`macros/ram.asm:7-25`；领取计算能力考虑stat-exp `add_mon.asm:511-514` | `save/mod.rs:100`错误存玩家OT；从不存OT name/PP Ups；`save/mod.rs:136-157`create_pokemon后只恢复moves/PP/EXP/DV/name；`pokemon/stats.rs:78-83`零初始化元数据 | 运行probe，错误会破坏交换加经验/不听话语义。注释“原作育成EV wipe”错误，原作宏有10字节stat-exp |
| SYS-06 | 中 | 育成精灵4招已满，升到会新招的等级仍不学招：Pikachu25→30不学26级Swift | 育成WriteMonMoves按FIFO丢第一招、PP及PP Ups一起左移；`scripts/Daycare.asm:178-184`、`engine/pokemon/evos_moves.asm:416-493` | `save/mod.rs:148`调用普通process_level_up_moves；`pokemon/move_learning.rs:263-265`返回pending后被丢弃 | 运行probe |
| SYS-07 | 中 | TM/HM/升级替换一个用了3个PP Up的招式，新招继承3个PP Up；恢复PP可得到额外PP | LearnMove写入新招base PP整字节，清掉高2位PP Ups；`engine/pokemon/learn_move.asm:38-55` | `pokemon/move_learning.rs:157-161`更新moves/pp而不清pp_ups；`items/bag_use.rs:461`生产调用 | 运行probe，Thunderbolt PP15/PP Ups3 |
| SYS-08 | 高 | 仅一只符合NPC交易的精灵时，动画和成功文字照常，completed flag设置，但原精灵没换；此交易永久用掉 | NPC交易可选最后一只；`engine/events/in_game_trades.asm:99-121,129-149`；RemovePokemon无last guard `engine/pokemon/remove_mon.asm:1-10` | `app/game.rs:5995-5997`只查species有无，`7132`调用带last guard的remove；`pokemon/party.rs:140-146`拒绝；`app/game.rs:3453`无条件resume true；各NPC scene成功分支设flag | 静态调用链确认；baseline debug运行已确认：sole Abra保留且交易成功flag=true，证据npc-single-baseline.json |
| SYS-09 | 中 | 有多个同种不同等级精灵时，NPC自动换第一只，缺少原作的选择/取消/选错物种流程 | 原作显示PartyMenu并用wWhichPokemon；`engine/events/in_game_trades.asm:99-121` | `app/game.rs:5996,7131`find_species；OverworldGameDataRequest无party index `overworld/screen.rs:165-174` | 静态确认 |
| SYS-10 | 严重 | “canonical” SRAM中育成box struct只写23字节；另漏wGameProgressFlags尾部2个script byte及78个保留字节，UNION又多填2字节；sprite、party、current box、checksum比原作提前88；双方互导失真/拒绝 | wDayCareMon为33字节宏且含五项stat-exp；`macros/ram.asm:7-25`、`ram/wram.asm:2216-2223`；SRAM紧邻固定区 `ram/sram.asm:16-24` | `save/ser_game_data.rs:363-384`遗漏10；`save/sram_deser_game_data.rs:259-266`同步漏读；`save/sram_import.rs:31-35,189-196`从同一错误serializer导长度 | 运行probe：main1841，daycare23，region3891；原作此尾段应33，根代理独立以RGBDS1.0.1组装原作RAM后确认：Main1929、sprite=$2D2C、party=$2F2C、current box=$30C0、checksum=$3523、region3979；另缺wGameProgressFlags末2个script byte+ds78（ram/wram.asm:2042-2044），同时UNION中50-byte wild branch旧版按48-byte填充多2；净差80-2+10=88。现有`canonical_region_length_matches_original`没有真实原作常量而只自比较（`sram_import_tests.rs:228-250`） |
| SYS-11 | 严重 | .sav物种写dex编号；Pikachu输出$19，原作$54读成Doduo；Rhydon$01读成Bulbasaur，高于151原作物种变None | SRAM结构species用内部ROM ID：Rhydon$01、Pikachu$54；`constants/pokemon_constants.asm:10,93`；图鉴号码另有映射 | `data/species.rs:7-10`枚举dex顺序；`save/ser_pokemon.rs:56,128,160,195`直接as u8/from_index_id；Daycare/HOF同样 | 运行probe；原作互导不能用自回环证明 |
| SYS-12 | 高 | Safari猎场存档没有写活跃步数/球数，Continue没有恢复active；在猎场读档可变普通战斗，下一warp重新得到全额步数球数 | wSafariSteps/wNumSafariBalls属于MainData，SaveGame复制保存；`ram/wram.asm:2062,2211`、`engine/menus/save.asm:220-223`；`engine/events/hidden_events/safari_game.asm:1-24`按保存字段继续 | `overworld/screen.rs:1196-1198`初始化0/inactive；`app/game.rs:1855-1927`save bridge没同步；Continue`app/game.rs:2336-2370`没恢复；`overworld/screen.rs:2479-2482`inactive再入猎场直接补满 | 静态确认；TUI同样独立状态；移动/Web沿用app bridge；debug save_state包含runtime与普通Save不同，应分开验 |
| SYS-13 | 高 | Colosseum战前强制heal、战后却留下HP/status/PP；原作允许带伤入战而战后全队heal | 原作唯一CableClub HealParty在`InitOpponent`返回后：`engine/link/cable_club.asm:280-287` | `battle/link_battle_driver.rs:176,210-212`战前heal；`494-513`结算保留战后party；通用`battle/settlement/writeback.rs:180-184`直接保存；`app/game.rs:4535-4541`rematch reset无heal | 静态确认（源码把asm:292音乐行误引为战前heal）；双进程动态待验 |
| SYS-14 | 高/缺失 | 普通PokéCenter柜员只显示Welcome，游戏内没有选择TradeCenter/Colosseum、存档/入场动作；现有TCP/BC backend需CLI/网页入口并debug warp进入房间 | `scripts/CeruleanPokecenter.asm:12-13`调用cable_club_receptionist；原作LinkMenu选房 `engine/menus/main_menu.asm:182-287` | 所有Pokecenter.scene柜员分支（如`maps/CeruleanPokecenter/script.scene`talkLinkReceptionist）仅文字；TradeCenter桌面linkStart已实现 | 确認内容代码，不应重报为“link不存在”；GBA app的link代码cfg剔除，硬件无后端，属平台未还原 |
| SYS-15 | 高 | 原作首次未CHANGE BOX的合法存档，banks2/3仍可含任意SRAM；import却先无条件校验两bank而拒绝Continue | 原作Load只校验bank1（`engine/menus/save.asm:1-11,31-130`）；首切箱才EmptyAllSRAMBoxes（`save.asm:368-370`） | `save/sram_import.rs:68-69,91-94`无条件validate box banks | 静态确证；修复回归用独立原作布局fixture+0xa7填充未初始化boxbank |
| SYS-16 | 高 | NPC交易完成标记仅runtime script alias，普通.sav没有与completed_in_game_trade_flags同步；重载后可重复交易；原作存档导入的完成位亦不显示 | `engine/events/in_game_trades.asm:47-51`测试wCompletedInGameTradeFlags；FlagAction按byte序bit0-9，`constants/script_constants.asm:23-32`原作10项顺序 | `app/game.rs:1855-1927,2336-2370`没有alias桥接；`save/ser_game_data.rs`对completed_in_game_trade_flags用BE，相反byte顺序 | 静态确认；修复fixture用原作$29E3 bit1(MARCEL)验证导入/导出/alias |

所有open-pokered行号基准路径：未写`app/`的逻辑在`crates/pokered-core/src/`；`app/game.rs`为`crates/pokered-app/src/game.rs`；`data/species.rs`为`crates/pokered-data/src/species.rs`。

## 覆盖与边界

| 表面 | 查验 | 未完成/不能宣称 |
|---|---|---|
| 背包/物品 | 20/50槽，溢出，TOSS key/HM，party-target分类，替换学招；PC物品失败用trial clone，排除复制旧推测 | 所有单项物品原作细节的穷举和逐帧动画 |
| 商店 | 生产MartBackend/try_buy/try_sell、价钱cap、BagFull rollback | 全门店交互逐一实玩 |
| PC | 单只/满6/满20 guard、ChangeBox保存请求、物品存取rollback、育成与箱数据桥接 | 所有前端单独回归（尤其TUI/GBA） |
| 育成 | EXP增长，HM/最后一只拒绝，领取元数据，FIFO学招，SRAM结构 | EXP接近原作$50上限的超长步数；L100cap差异暂不单列，因为正常取回会cap100 |
| NPC交易 | 10组数据/共同桥接/动画后mutation/flag、last mon/同种多只 | 所有10处NPC逐一实玩；动画像素代理另审 |
| 图鉴 | 编号列表、seen/owned gated数据、CRY/AREA入口及原作区域排除，PC rating阈值 | 151项描述逐字/中文视觉、全部habitat比较 |
| 联机 | 生产TCP/BC/session/router、trade last-mon专用remove、双方confirm取消和disconnect代码路径、battle healing位置 | 原作硬件串口互通，本项目自定义wire协议不能视作ROM互通；整局双进程及断线时序待验 |
| 存读 | SRAM序列化/反序列化/HOF/当前箱与外部箱/legacy迁移、Safari桥接、OT/PP Ups | 所有原作合法存档逐一导入，真实ROM打开导出文件；8项probe为源码运行证据，尚无ROM差分运行 |

## 排除的旧报告与额外风险

PC主入口/12箱切换/物品PC/Oak评级/League HOF存在；图鉴seen+owned在捕获赠送交易中已有调用；联机双方交易与战斗driver不是stub；因此不再沿用旧“均未实现”结论。PC物品存取有trial clone（pc_screen.rs:1182,1201），满格库存部分修改不会从clone提交，SYS-01仅商店与其他直接add caller。

原作所谓“育成会擦除EV”和“WriteMonMoves完全重建TM”均没有这份参考ASM支持：box_struct包含stat-exp；育成learning flag只处理deposit起点之后的新招。把这些错误解释写进测试会使全绿测试仍不保真。

修复实施边界（2026-10-02）：systems分支保持JSON model的dex编号，SRAM边界转换原作internal species ID；识别此前native .sav并修正净88字节布局差；新增serde-default尾段，保留原作GP末2个script byte与78个reserved bytes，保留之前项目存档可读。育成恢复原作五项stat-exp、OT、PP Ups和FIFO招式覆盖；NPC交易用真实party selector并允许仅一只精灵交换；每次PC操作与满队捕获同步活动箱镜像。Safari计数、Rod status aliases与垃圾桶索引补普通Save/Continue桥接。Cable Club原作柜员恢复Welcome文本/Pokedex门槛、连接检查、原作apply/save完整文字再确认、保存同意、选房与warp。TCP/网页协议仍是跨平台实现；GBA物理联机后端与原作串行比特流不在本轮修复的实测范围内；入房使用本项目warp桥接，原作是SpecialWarp，未作逐帧差分。

## 修复验收（systems 分支）

所有 SYS-01 至 SYS-16 均纳入本轮修复。core lib 全量 2605/2605 通过；新增独立 integration 14/14 通过，包含原作地址 fixture、旧版 native layout migration、背包失败原子性、sale cap、151个物种边界映射、育成元数据/FIFO/PP Ups、PC box trick、12个活动箱满队捕获→PC→普通SRAM，10项NPC完成位、Safari/Rod状态恢复及联机战后治疗。治疗测试按12个护士位置和6个其他治疗位置矩阵，只有护士登记回城目标。

原作地址 fixture 依据 root 独立 RGBDS 1.0.1 assemble/link 得到的 `reference-ram.sym`，手工构造 `$2598..$3523` bank1，不调用 serializer 布置字段。它故意把 Pikachu 写为 `$54`、stat-exp 写为54321、Safari写123步/7球、MARCEL位写 `$29E3 bit1`，把未首次切箱的bank2/3填 `$A7`。尾段写非零值后必须原样导出。另一个 matrix 固定主数据边界：

| 字段 | 原作 .sav offset | 断言 |
|---|---|---|
| GameProgressFlags | `$289C..$2964` | 120个旧model字节+80个新增尾段，总200 |
| SafariSteps / NumSafariBalls | `$29B9` / `$2CF3` | BE16步数 / u8球数 |
| statusFlags1 / NPC trade flags | `$29D4` / `$29E3` | Rod bits3/4/5、低字节bit0–7先写 |
| Grass / Water table | `$2B34..$2B48` / `$2B51..$2B65` | 各20字节；UNION剩余375 |
| TrainerHeader / Daycare species | `$2CDC` / `$2D0B` | 地址BE16 / internal species `$54` |
| Daycare stat-exp / Sprite start | `$2D1C` / `$2D2C` | 五项10字节EV，完整box33 |
| Party / CurrentBox / checksum | `$2F2C` / `$30C0` / `$3523` | 原作独立常量，region3979 |

边界：已证明固定布局与字段转换，尚未用真实原作 ROM 打开导出存档；未知运行时指针和未建模的原作状态仍有零填充。旧版native育成未保存的EV/OT等历史信息无法恢复。NPC选择/取消和最后一只交易已修；有效party选择后才显示“connect cable”，关闭后才开始动画与最终mutation。默认Native VM的垃圾桶索引有普通SRAM桥接；可选Boa引擎未作等价实玩。联机柜员房间选择恢复，但SpecialWarp入房时序、自定义TCP/Web协议与原作物理串口互通未声明完成。

最终本分支验收：core lib2605，systems integration14，app NPC/柜员fidelity4，existing cable flow8全过；TUI cargo check、debug-server app build成功。联机前不heal的双channel driver测试与战后heal settlement测试已纳入core。截图及双进程TCP app桥接动态结果另补后续证据commit。

动态时序补验发现并追加修复：NPC选对后的ConnectCableText需在外部await期间正常推进，而NativeVM WaitingForCommand会重发同一tradePokemon。core现在在await trade/battle/elevator/filterBag期间跳过VM polling，继续处理手动dialogue，直到frontend resume；追加复现该重发问题的core测试，最终集成分支统一执行。

存读边界追加：原作/新canonical SRAM的所有已映射event/status/NPC完成位为authority，不允许companion的false覆盖，也不允许companion的旧true伪造尚未完成的NPC/Rod位。已独立识别为旧native布局的文件，仅迁移历史sidecar中为true的NPC/Rod alias；新Save导出后provenance清除。此标记为serde-skip运行态，不改变SRAM或JSON格式。显式 `--save slot.sav` 的companion改绑定 `slot.script_flags.json`；默认pokered路径兼容旧文件名。旧native首次迁移且缺绑定sidecar时允许读取旧exe旁global文件，下一次保存写绑定路径；历史global文件没有trainer身份，无法证明它属于哪个旧自定义save，这是旧格式本身的信息缺失，不适用于新/raw SRAM。追加回归覆盖canonical与legacy两类false/true、mapped event、未知extra以及2条不同save路径隔离/默认旧路径。

SYS-15进一步按原作 `wCurrentBoxNum` bit7判断箱bank是否已初始化，不能把“checksum有效”等同于已初始化。原作NEW GAME可以保留前一位trainer的有效bank2/3，而bit7为0；新增独立fixture把前局有效箱bank复制进原作新局存档，验证12箱仍为空。原作bit7为0时忽略这些bank，已识别旧native布局仍可迁移其有效bank；本项目导出每次均写完整箱bank，所以同时设置bit7。whole-image及GBA逐bank导入采取同一策略。

Web旧JSON仅由实际localStorage读取入口检查原始JSON缺少 `game_data.game_progress_tail` 后标记一次迁移；同slot extras只接收旧NPC/Rod alias的true。新JSON和原作SRAM保持存档位权威，普通debug snapshot反序列化不会获得此标记。追加旧/新JSON同slot迁移、snapshot不迁移及无效JSON回归；上述追加source测试等待根代理最终集成检查，未沿用此前14项通过结果宣称已运行。
