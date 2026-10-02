# 静态数据与野外通用机制审计

基线：`open-pokered@72ff719`；原作：`pret/pokered@fbcf7d0`，Red 与 Blue 条件分别求值。
本文件中的行号是上述基线的行号，修复后可能移动。审计脚本只读取 JSON、原作 ASM 和原始 blockset bytes，不调用游戏的数据转换器作为预期值。结果在 [data-world-check.json](data-world-check.json)，重跑命令：

```sh
# 对冻结基线重跑；把 BASE_TREE 替换为该提交的独立 checkout。
python3 docs/audits/2026-10-02-full-fidelity/check-data-world.py REF_TREE BASE_TREE --implementation open-pokered@72ff719b39634c153cb82d3f3ece200bd413c4e0
# 当前修复树另存，避免覆盖基线证据；脚本自动记录实际提交。
python3 docs/audits/2026-10-02-full-fidelity/check-data-world.py REF_TREE --output docs/audits/2026-10-02-full-fidelity/data-world-check-after.json
```

## 已完成的全量静态核对

| 内容 | 实际核对范围 | 结果 |
| --- | --- | --- |
| 基础物种 | 151 种，五维种族值、双属性、捕获率、基础经验、成长率、初始招式、55 项 TM/HM 位 | 生效的游戏数据全部一致；Mew 最后一个保留位不同，见下文 |
| 招式 | 165 条的效果、威力、属性、命中率百分比、PP | 0 差异；动画 ID 166–203 不属于额外招式 |
| 进化与学招 | 151 种的有序进化路径、最低等级、进化道具、逐级招式 | 0 差异 |
| 图鉴数据 | 151 种的类别、英制身高、体重、原作两页英文描述 | 0 差异 |
| 训练家队伍 | 47 类、391 队、994 只，展开共用等级和逐只等级格式 | 0 差异 |
| 世界数据 | 248 个地图 JSON；有原作实体文件的地图逐条核对尺寸、ID、tileset、connections、border、map.blk，805 warp、918 对象、106 道具对象 | 内容无缺失；Route1 一处随机运动轴错误，见 D-W03 |
| 背景交互 | 339 个 JSON background 对象 | 原作 bg 坐标全部保留；44 张图增加 PC、书架、雕像、老虎机等交互点，属于原作通过通用 tile-handler 实现的内容，不能直接判为新增 bug |
| 野遇表 | 248 个原作 WildDataPointers，189 个零野遇地图，59 个非零/声明野遇地图 × 两版本 × 草水，共 236 张表（包含 Route19/20 共用 SeaRoutes 表） | 0 差异 |
| 钓鱼数据 | Good Rod 两只、Super Rod 十组与33张地图，名称映射和运行时 MapId 映射分别核对 | 0 差异 |

原始 JSON 结果保留全部结构差异，不把结构差异直接等同于 bug。冻结基线共75条；同步Mew保留位和Route1运动轴后剩73条：44条背景坐标表达差异、26条脚本启动战斗的 `isTrainer` 差异、Cerulean Rocket 的2条训练家字段迁移，以及Route22Gate的1条LAST_MAP解析差异。生效内容表零差异，不意味着原始JSON逐字段零差异。26 个原作 `OPP_` 对象在本项目 `isTrainer=false`，但由剧情脚本启动首领、四天王、火箭队等战斗；`CeruleanCity` Rocket 的数据字段改由脚本持有；这些归剧情代理核对，不能凭静态布尔值断言无法战斗。Route22Gate 把原作 LAST_MAP 解析成两侧明确地图，是已实现的门方向修复。UndergroundPathRoute7Copy 原作 header 使用非 copy 常量，但实际 map table ID 是 `$4e`，本项目 ID 78 正确，已从误报排除。25 个 unused/alias JSON 无独立原作实体文件，不声称逐字地图 bytes 对照覆盖到这些空壳。

Mew 的 `tmHmFlags[6]` 为127，原作为255：`pokemon/Mew.json:29` 对 `data/pokemon/base_stats/mew.asm:28` 的 `UNUSED`。差异仅在第56个保留位，实际55个TM/HM都能学习；无可用道具能触发此位，不列为玩法缺陷；本轮仍将该保留位同步为255，保证原始表字节一致。

## 按游玩顺序定位的已确认问题

### D-W01 / P2：在普通地面刷怪、在部分草地不刷怪，Safari 亦受影响

触发例：Route1 的 `(5,28)` 与 `(5,29)` 都站在普通地面 `$2c`，项目却从下一格取到草地 `$52`，roll=0 时可刷怪；`(7,30)`、`(7,31)` 站在草地 `$52`，项目却取下一格普通地面 `$2c`，不刷怪。

当前：`crates/pokered-core/src/overworld/update.rs:1808`、`:1949` 以玩家步坐标 `x+1` 读 rate anchor；`collision.rs:187-197` 中一个步坐标对应两个8px背景tiles，故取到了相邻16px格的底左tile。额外还把 `map.width`（32px block数量）当成16px格数量作边界条件，使地图右半部退回 standing tile。

原作：`engine/battle/wild_encounters.asm:26-37` 明确读玩家**当前16px半块底右8px tile** `(9,9)`；`:64-72` 用同半块底左tile `(8,9)` 选择草/水列表。rate anchor 必须是在当前 block 内 `bottom_left_index+1`，不需要读下一格，也无需右地图边界近似。

证据：机器检查独立展开原始 blocksets，在18张 Overworld/Forest 野遇地图发现581个候选可走/可冲浪格的 rate 不一致（不含NPC占位、warp门禁、可达性判断，数量不是实际玩家刷怪次数）。JSON 的 `encounter_anchor_proof.sample_cells` 保留具体坐标和双方tile/rate。两个入口分别覆盖走一步和原地转向。这些候选由固定的基线旧anchor公式与原作公式静态比较产生；检查脚本的该proof段不读取当前Rust移动实现，所以after结果仍保留581这个基线诊断数，不能解释为修复后仍有581处bug。静态取样与原作坐标已确认；基线完整流程并未逐格穷举。

### D-W02 / P2：零概率表清空使原作左岸旧表刷怪行为消失

触发：先到有草野遇的地图，再通过无草野遇地图抵达可冲浪海岸，站在底左为岸、底右为水的half-block。原作用水rate读先前地图草列表。

当前：`overworld/wild_encounters.rs:114-137` 每次重新读当前地图 JSON，空 grass `mons=[]`；`battle/wild.rs:139` 的空表保护导致无遭遇。仅修 D-W01 的正确右tile，仍然无法还原旧表左岸遭遇。

原作：`engine/overworld/wild_mons.asm:13-20`，grass rate=0跳过复制并保留此前 `wGrassMons`；`:25-30`，water rate=0亦保留此前 `wWaterMons`。`engine/battle/wild_encounters.asm:67-72` 明文解释该left-shore quirk。本项目需要在活跃overworld内缓存最后载入的非零草/水列表，rate仍取当前地图，且状态快照应保留两列表。

证据边界：这里确认的是合法151物种的旧表遭遇。老人捕捉教程临时把玩家姓名写入草野遇RAM（`engine/battle/core.asm:2026-2036`），进而刷出MissingNo等非法内部ID，依赖额外RAM alias及非法species表示；不声称本次基础修复能还原该完整漏洞。

### D-W03 / P2：Route1 赠送 Potion 的店员沿错误方向随机走动

触发：在Route1等待第一位NPC移动。当前横向游走，原作纵向游走。

当前：`crates/pokered-data/maps/Route1/map.json:32` `range=2`；`overworld/npc_movement.rs:42-47` 2映射Horizontal；`overworld/screen.rs:2697` 生产加载实际采用该轴。

原作：`data/maps/objects/Route1.asm:14` 指定 `WALK, UP_DOWN`，第二位在`:15`才是 LEFT_RIGHT。全量98个随机游走NPC的轴核对仅此一个字段错误。该值不是训练家视野，修改应只影响此NPC运动轴。

### D-W04 / P2：自主NPC的受限轴roll丢弃、零delay时长未还原

触发：任意 UP_DOWN/LEFT_RIGHT 随机行走NPC在无遮挡格滚到另一轴方向；或者NPC的 delay roll 为0。

当前锁定依赖：`dotzuki@7efac8a` `workspace/crates/dotzuki-engine/src/overworld/npc_movement.rs:242-261`，用低两位加index选方向，轴不符就重新delay；`:232-235`，delay0会立即再次roll。`pokered-core/src/overworld/update.rs:2185`实际调用这个函数，不是测试孤立helper。

原作：`engine/overworld/movement.asm:206-251` 按四个64-byte区间选方向，轴不符roll改映射到同轴方向，每一个roll都会尝试移动；`:353-361` 的 `Random & $7f=0` 会因 `:390-392` 的 `dec [hl]` 下溢而等待256帧。项目对受限轴有额外50%停顿，且删掉了原作的0→256计数器quirk。

证据边界：双方分支/计数器静态确认；不同RNG算法本来不会产生相同输入序列，故没有以“随机步路线不同”作为bug依据。通用引擎默认行为不宜直接改成Gen I专有规则；应由pokered运动provider/适配层实现，并增加控制随机输入的回归。该项与仅修Route1数据轴是独立问题。

### D-W05 / P2（版本能力）：Blue 数据已还原，生产野遇仍取Red

触发：用核心API `PokemonGame::new(GameVersion::Blue)`进入Route2等有版本差异的地图。原作Blue应为Caterpie，当前生产roll仍可能选到Red的Weedle。

当前：`overworld/update.rs:3813` 固定 `GameVersion::Red`；overworld没有自己的版本字段。原作：`data/wild/maps/Route2.asm:8-24` 分别声明Red的Weedle和Blue的Caterpie。JSON双版本全部核对正确，问题在版本状态未贯通。默认Red游戏不把Weedle判作错误；仅把已有Blue核心能力及Red/Blue承诺的不完整贯通列为问题。

### D-W06 / P1：全地图地面球失败后消失，或成功后可反复拾取

触发：把背包装满20类，且不持有地面球的物品，拾取后腾一格再重试；另一路是成功拾取后再次对同一个球按A。对248地图中104个非零道具ID的PokeBall逐一连接map JSON → trigger/config → storyline，67个handler未检查giveItem结果，27个成功分支没有hideObject。两类数量独立统计，其他缺陷可与它们重叠；所有记录、基线文件行号和toggle名称保存在 [ground-pickup-baseline.json](ground-pickup-baseline.json)。

当前例：`maps/Route2/script.scene:11-16` 无条件置收集旗并隐藏，失败会永远丢失Moon Stone；`maps/MtMoonB2F/script.scene:193-205`、`:208-220` 成功给HP Up/TM01但不隐藏，造成无限重复拾取。`overworld/update.rs:1442-1452` 的生产ItemPickup路径只调用场景；不会代替场景判断容量或隐藏。道具容量的数量上限由systems审计另修，本项专指场景如何处理真实返回值。

原作：`engine/events/pick_up_item.asm:20-42` call GiveItem后检查carry；失败只显示满包文字，成功才`:33-35` HideObject。对象原始toggle位负责持久化。修复让所有104个球只有given成功才设置原有旗、hide并显示found，失败保留球与旗；原有旗名字保留以兼容存档。静态扫描修后0缺陷，运行时覆盖全部104个编译后场景及5个真实A键入口，见修复验证附件。

### D-W07 / P2：七个地面球发错TM，map.json却已正确

触发：背包留空位并拾取下列球。物种与招式表0差异无法保证奖励脚本正确。

| 地图 / NPC | 基线实际奖励 | 原作奖励 | 基线scene行 / 原作objects行 |
| --- | --- | --- | --- |
| MtMoon1F / 13 | TM34 Bide | TM12 Water Gun | 151 / MtMoon1F.asm:42 |
| Route25 / 10 | TM39 Swift | TM19 Seismic Toss | 119 / Route25.asm:32 |
| RocketHideoutB3F / 3 | TM44 Rest | TM10 Double-Edge | 48 / RocketHideoutB3F.asm:19 |
| SafariZoneEast / 4 | TM39 Swift | TM37 Egg Bomb | 69 / SafariZoneEast.asm:26 |
| SafariZoneWest / 2 | TM49 Tri Attack | TM32 Double Team | 39 / SafariZoneWest.asm:28 |
| PokemonMansionB1F / 5 | TM18 Counter | TM14 Blizzard | 92 / PokemonMansionB1F.asm:24 |
| PokemonMansionB1F / 6 | TM26 Earthquake | TM22 SolarBeam | 105 / PokemonMansionB1F.asm:25 |

表中的scene行以基线具体giveItem行计；原作路径均在 `data/maps/objects/`。map.json itemId已经和原作一致，错误来自scene奖励覆盖。修复奖励及显示TM号，不重命名既有保存旗。机器检查将七个修复分别与map.json字节ID核对；全部104个运行时测试也验证最终请求的ItemId。

### D-W08 / P2：PowerPlant五个道具和ViridianGym Revive没有拾取逻辑

触发：对PowerPlant NPC10–14或ViridianGym NPC11按A。当前只显示JSON fallback文本，不给道具、不隐藏；原有scene注释误称运行时自动处理。

当前：`maps/PowerPlant/script.scene:173-176` 与 `maps/ViridianGym/script.scene:171-172` 注释排除handler，两个script_config也没有对应talk。`overworld/update.rs:1442-1452` 并不存在注释描述的自动GiveItem路径。

原作：`data/maps/objects/PowerPlant.asm:37-41` 的Carbos、HP Up、Rare Candy、TM25 Thunder、TM33 Reflect；`ViridianGym.asm:34` 的Revive均走统一PickUpItem。修复六个handler和对应script_config，加入原始TOGGLE位 `$56..$5a` 与 `$33` 的映射；容量不足不领取，成功隐藏且重入持久化。

### D-W09 / P2：成功拾取缺提示音、文本需要额外A键，原作会等音效自动结束

触发：成功拾取任意一个地面球，等待found文字打完。基线104个成功分支全是普通ShowDialogue，未调用playSound；`overworld/update.rs:3238-3244` 只发GiveItem数据请求，`pokered-app/src/game.rs:5937-5944` 只修改背包，没有补声音。普通对话在 `update.rs:2364-2398` 等待A/B。

原作：`engine/events/pick_up_item.asm:33-50` 成功Hide之后设置NoWaitA，执行FoundItemText；`home/text.asm:529-531` PlaySound并WaitForSoundToFinish，`home/text_script.asm:87-103` 跳过额外A确认，但仍保留A持续按住时HoldTextDisplayOpen的原作行为。

修复采用游戏专用 `showItemDialogue(t(en,zh))`，仅连接104个成功分支，不修改通用引擎或GiveItem全局行为。流程为逐字打印→GET_ITEM_1→按frontend实际sequencer状态等待→自动关闭；音效未结束时A/B不能提前收框，音效结束后持续按A仍等松手；不显示手动翻页箭头。失败继续普通满包文字，不播放拾取音效。effect的sound_started与等待状态纳入快照；测试明确验证打字先于音效、音效恰一次、失败无音效、等待中按键不关闭、快照相位及无A自动收框。

## 通用机制已排除的旧问题与边界

逐项阅读了Repel扣步/到期替代遭遇、首只等级保护、战后三步cooldown、三种钓竿水边/冲浪限制与roll循环、三种杆sprite演出入口、强制自行车/Seafoam强制Surf、HM徽章/party菜单显示、Cut、Surf上岸、Dig/Teleport出口、Strength双次接触/障碍检测、胜利之路开关和Seafoam落洞。既有测试加源码确认这些路径不是空壳，旧审计称“钓鱼未实现”“HM缺少接线”“Strength未实现”的项不再重复列为未还原。

未把“数据0差异”扩展成“所有运行逻辑完全保真”：隐藏道具/剧情条件的逐流程行为交由剧情代理；cross-floor boulder switch flags交给后期剧情审计；完整原作随机RNG、越界RAM故障、MissingNo和非法地图不在这些静态数值检查的覆盖声明中。这里的全量指可穷举的内容表及明确入口，不是151种×全部战斗状态×全部地图路径的动态穷举。
