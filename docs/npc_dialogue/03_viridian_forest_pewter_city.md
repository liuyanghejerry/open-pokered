# Viridian Forest / Route 2 / Pewter City 区域 NPC 对话与剧情脚本

> 来源：pokered 反汇编代码
> 覆盖地图：ViridianForest、ViridianForestSouthGate、ViridianForestNorthGate、Route2、Route2Gate、Route2TradeHouse、PewterCity、PewterGym、PewterMart、PewterPokecenter、PewterNidoranHouse、PewterSpeechHouse、Museum1F、Museum2F
> 用途：Rust 重制版剧情参考

---

## 一、Viridian Forest（翠绿森林）

### 地图脚本流程（状态机）

| 状态 (`wViridianForestCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | `CheckFightingMapTrainers` — 检测训练师视野 |
| 1 `START_BATTLE` | `DisplayEnemyTrainerTextAndStartBattle` |
| 2 `END_BATTLE` | `EndTrainerBattle`，回到状态0 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_0` | 已击败 Youngster 2 |
| `EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_1` | 已击败 Youngster 3 |
| `EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_2` | 已击败 Youngster 4 |

### NPC 列表

#### Youngster 1（非战斗 NPC）

> "I came here with
> some friends!
>
> They're out for
> POKÉMON fights!"

---

#### Youngster 2（训练师）

- **挑战前：** "Hey! You have POKÉMON! Come on! Let's battle'em!"
- **战败时：** "No! CATERPIE can't cut it!"
- **战后：** "Ssh! You'll scare the bugs away!"

---

#### Youngster 3（训练师）

- **挑战前：** "Yo! You can't jam out if you're a POKÉMON trainer!"
- **战败时：** "Huh? I ran out of POKÉMON!"
- **战后：** "Darn! I'm going to catch some stronger ones!"

---

#### Youngster 4（训练师）

- **挑战前：** "Hey, wait up! What's the hurry?"
- **战败时：** "I give! You're good at this!"
- **战后：** "Sometimes, you can find stuff on the ground! I'm looking for the stuff I dropped!"

---

#### Youngster 5（非战斗 NPC）

> "I ran out of POKÉ
> BALLs to catch
> POKÉMON with!
>
> You should carry
> extras!"

---

### 地板道具

- ANTIDOTE（解毒药）
- POTION（伤药）
- POKÉ BALL（精灵球）

### 标识牌

- "TRAINER TIPS / If you want to avoid battles, stay away from grassy areas!"
- "For poison, use ANTIDOTE! Get it at POKÉMON MARTs!"
- "TRAINER TIPS / Contact PROF.OAK via PC to get your POKÉDEX evaluated!"
- "TRAINER TIPS / No stealing of POKÉMON from other trainers! Catch only wild POKÉMON!"
- "TRAINER TIPS / Weaken POKÉMON before attempting capture! When healthy, they may escape!"
- "LEAVING VIRIDIAN FOREST / PEWTER CITY AHEAD"

---

## 二、Viridian Forest South Gate（翠绿森林南门）

### NPC 列表

#### Girl

> "Are you going to
> VIRIDIAN FOREST?
> Be careful, it's
> a natural maze!"

#### Little Girl

> "RATTATA may be
> small, but its
> bite is wicked!
> Did you get one?"

---

## 三、Viridian Forest North Gate（翠绿森林北门）

### NPC 列表

#### Super Nerd

> "Many POKÉMON live
> only in forests
> and caves.
>
> You need to look
> everywhere to get
> different kinds!"

#### Gramps

> "Have you noticed
> the bushes on the
> roadside?
>
> They can be cut
> down by a special
> POKÉMON move."

---

## 四、Route 2（2 号道路）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无状态机，无训练师。

### 地板道具

- MOON STONE（月之石）
- HP UP（HP 增强剂）

### 标识牌

- "ROUTE 2 / VIRIDIAN CITY - PEWTER CITY"
- "DIGLETT's CAVE"

---

## 五、Route 2 Gate（2 号道路关卡建筑）

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_GOT_HM05` | 已从 Oak 助手处获得 HM05（FLASH） |

### NPC 列表

#### Oak's Aide（大木博士助手）

需要收集 **10 只** 宝可梦（图鉴数量），达标则给予 HM05 FLASH。

**达标后或已领取：**

> "The HM FLASH
> lights even the
> darkest dungeons."

（通用 OaksAide 对话由 `predef OaksAideScript` 处理）

#### Youngster

> "Once a POKÉMON
> learns FLASH, you
> can get through
> ROCK TUNNEL."

---

## 六、Route 2 Trade House（2 号道路交换小屋）

### NPC 列表

#### Scientist

> "A fainted POKÉMON
> can't fight. But,
> it can still use
> moves like CUT!"

#### Gameboy Kid（游戏内交换 NPC）

交换参数：`TRADE_FOR_MARCEL`（具体宝可梦由 trades.asm 定义）。

---

## 七、Pewter City（深灰市）

### 地图脚本流程（状态机）

`wPewterCityCurScript` 控制：

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 重置博物馆脚本；检测玩家是否往东离开（若 `EVENT_BEAT_BROCK` 未设置则拦截） |
| 1 | Super Nerd 1 引导至博物馆的过场 |
| 2 | 等待 NPC 移动完成后隐藏 Super Nerd 1 |
| 3 | 恢复 Super Nerd 1 位置，回到状态0 |
| 4 | Youngster 引导至道馆的过场 |
| 5 | 等待 NPC 移动完成后隐藏 Youngster |
| 6 | 恢复 Youngster 位置，回到状态0 |

**触发坐标（往东拦截）：** (35,17)、(36,17)、(37,18)、(37,19)

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_BROCK` | 已击败馆主 Brock |
| `EVENT_BOUGHT_MUSEUM_TICKET` | 已购买博物馆门票（每次进城重置） |

### NPC 列表

#### Cooltrainer F

> "It's rumored that
> CLEFAIRYs came
> from the moon!
>
> They appeared
> after MOON STONE
> fell on MT.MOON."

#### Cooltrainer M

> "There aren't many
> serious POKÉMON
> trainers here!
>
> They're all like
> BUG CATCHERs,
> but PEWTER GYM's
> BROCK is totally
> into it!"

#### Super Nerd 1（引导博物馆，Yes/No）

询问：`"Did you check out the MUSEUM?"`

- YES：`"Weren't those fossils from MT.MOON amazing?"`
- NO：`"Really? You absolutely have to go!"` → 过场动画 → NPC 到达目标后：`"It's right here! You have to pay to get in, but it's worth it! See you around!"`

#### Super Nerd 2（花园，Yes/No）

询问：`"Psssst! Do you know what I'm doing?"`

- YES：`"That's right! It's hard work!"`
- NO：`"I'm spraying REPEL to keep POKÉMON out of my garden!"`

#### Youngster（引导道馆，坐标触发或直接对话）

**触发/对话：**

> "You're a trainer
> right? BROCK's
> looking for new
> challengers!
> Follow me!"

**NPC 到达目标后：**

> "If you have the
> right stuff, go
> take on BROCK!"

### 标识牌

| 标识 | 文本 |
|---|---|
| Trainer Tips | "TRAINER TIPS / Any POKÉMON that takes part in battle, however short, earns EXP!" |
| 警方公告 | "NOTICE! Thieves have been stealing POKÉMON fossils at MT.MOON! Please call PEWTER POLICE with any info!" |
| 博物馆 | "PEWTER MUSEUM OF SCIENCE" |
| 道馆 | "PEWTER CITY #MON GYM / LEADER: BROCK / The Rock Solid #MON Trainer!" |
| 城市路牌 | "PEWTER CITY / A Stone Gray City" |

---

## 八、Pewter Gym（深灰道馆）

### 地图脚本流程（状态机）

进入时加载城市名 `PEWTER CITY`、馆主名 `BROCK`。

| 状态 (`wPewterGymCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | `CheckFightingMapTrainers` |
| 1 `START_BATTLE` | `DisplayEnemyTrainerTextAndStartBattle` |
| 2 `END_BATTLE` | `EndTrainerBattle` |
| 3 `BROCK_POST_BATTLE` | 显示"Wait! Take this"文本 → 设置 `EVENT_BEAT_BROCK` → 给予 TM34（或提示背包已满）→ 设置 BOULDERBADGE → 隐藏道馆门口 Youngster；重置 Route22 宿敌事件 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_BROCK` | 已击败 Brock，获得 BOULDERBADGE |
| `EVENT_GOT_TM34` | 已获得 TM34（BIDE） |
| `EVENT_BEAT_PEWTER_GYM_TRAINER_0` | 已击败道馆训练师（击败 Brock 后自动设置） |

### NPC 列表

#### Brock（馆主）

**战斗前：**

> "I'm BROCK!
> I'm PEWTER's GYM
> LEADER!
>
> I believe in rock
> hard defense and
> determination!
>
> That's why my
> POKÉMON are all
> the rock-type!
>
> Do you still want
> to challenge me?
> Fine then! Show
> me your best!"

**战斗胜利（自动显示）：**

> "I took
> you for granted.
>
> As proof of your
> victory, here's
> the BOULDERBADGE!
>
> \<PLAYER\> received
> the BOULDERBADGE!"

接续：

> "That's an official
> POKÉMON LEAGUE
> BADGE!
>
> Its bearer's
> POKÉMON become
> more powerful!
>
> The technique
> FLASH can now be
> used any time!"

**给予 TM34（战后）：**

> "Wait! Take this
> with you!"

- 给予成功：`"\<PLAYER\> received TM34!"`（道具音效）
  接续：
  > "A TM contains a technique that can be taught to #MON!
  >
  > A TM is good only once!
  >
  > So when you use one to teach a new technique, pick the #MON carefully!
  >
  > TM34 contains BIDE!
  >
  > Your #MON will absorb damage in battle then pay it back double!"
- 背包已满：`"You don't have room for this!"`

**战后再次对话（已给 TM34）：**

> "There are all
> kinds of trainers
> in the world!
>
> You appear to be
> very gifted as a
> POKÉMON trainer!
>
> Go to the GYM in
> CERULEAN and test
> your abilities!"

---

#### Gym Guide（道馆向导）

**未击败 Brock（Yes/No）：**

询问：
> "Hiya! I can tell you have what it takes to become a POKÉMON champ! I'm no trainer, but I can tell you how to win! Let me take you to the top!"

- YES/NO 均接续战术建议：
  > "The 1st POKÉMON out in a match is at the top of the POKÉMON LIST! By changing the order of POKÉMON, matches could be made easier!"

**已击败 Brock：**

> "Just as I thought!
> You're POKÉMON
> champ material!"

---

#### Cooltrainer M（道馆训练师）

- **挑战前：** "Stop right there, kid! You're still light years from facing BROCK!"
- **战败时：** "Darn! Light years isn't time! It measures distance!"
- **战后：** "You're pretty hot, but not as hot as BROCK!"

---

## 九、Pewter Mart（深灰商店）

### NPC 列表

#### Clerk（店员）

标准商店购物流程。

#### Youngster

> "A shady, old man
> got me to buy
> this really weird
> fish POKÉMON!
>
> It's totally weak
> and it cost ¥500!"

#### Super Nerd

> "Good things can
> happen if you
> raise POKÉMON
> diligently, even
> the weak ones!"

---

## 十、Pewter Pokecenter（深灰精灵中心）

### NPC 列表

#### Nurse

标准精灵中心治疗流程。

#### Gentleman

> "What!?
>
> TEAM ROCKET is
> at MT.MOON? Huh?
> I'm on the phone!
>
> Scram!"

#### Jigglypuff（胖丁）

> "JIGGLYPUFF: Puu
> pupuu!"

（停止音乐 → 等待32帧 → 播放 `MUSIC_JIGGLYPUFF_SONG` → 胖丁精灵旋转动画 → 等待音乐结束 → 恢复背景音乐）

#### Link Receptionist

标准通信俱乐部接待流程。

---

## 十一、Pewter Nidoran House（深灰角钻牛宅）

### NPC 列表

#### Nidoran♂（宠物精灵）

> "NIDORAN: Bowbow!"

（播放 Nidoran♂ 叫声）

#### Little Boy

> "NIDORAN sit!"

#### Middle Aged Man

> "Our POKÉMON's an
> outsider, so it's
> hard to handle.
>
> An outsider is a
> POKÉMON that you
> get in a trade.
>
> It grows fast, but
> it may ignore an
> unskilled trainer
> in battle!
>
> If only we had
> some BADGEs..."

> **设计说明：** 此对话解释了交换宝可梦（Outsider）机制——交换来的宝可梦升级更快，但若徽章数量不足可能无视指令。

---

## 十二、Pewter Speech House（深灰讲解屋）

### NPC 列表

#### Gambler

> "POKÉMON learn new
> techniques as
> they grow!
>
> But, some moves
> must be taught by
> the trainer!"

#### Youngster

> "POKÉMON become
> easier to catch
> when they are
> hurt or asleep!
>
> But, it's not a
> sure thing!"

---

## 十三、Museum 1F（博物馆一楼）

### 地图脚本流程（状态机）

| 状态 (`wMuseum1FCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 玩家在入口坐标（Y=4，X=9或X=10）时自动触发售票员对话 |
| 1 `NOOP` | 已购票，不再触发 |

> 注：`wMuseum1FCurScript` 和 `EVENT_BOUGHT_MUSEUM_TICKET` 在每次进入深灰市时被重置。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BOUGHT_MUSEUM_TICKET` | 已购买博物馆门票（每次进城重置） |
| `EVENT_GOT_OLD_AMBER` | 已从二楼科学家处获得 OLD AMBER |

### NPC 列表

#### Scientist 1（售票员）

**从背面接近（Y=4，X=13 或 Y=3，X=12）：**

> "You can't sneak
> in the back way!
>
> Oh, whatever!
> Do you know what
> AMBER is?"

- YES：`"There's a lab somewhere trying to resurrect ancient POKÉMON from AMBER."`
- NO：`"AMBER is fossilized tree sap."`

**正面，未购票（Yes/No）：**

> "It's ¥50 for a
> child's ticket.
>
> Would you like to
> come in?"

- YES + 金额足够：扣 ¥50，`"Right, ¥50! Thank you!"`，设置 `EVENT_BOUGHT_MUSEUM_TICKET`
- YES + 金额不足：`"You don't have enough money."`
- NO：`"Come again!"` + 推走玩家

**已购票：**

> "Take plenty of
> time to look!"

---

#### Gambler（参观者）

> "That is one
> magnificent
> fossil!"

---

#### Scientist 2（持有 OLD AMBER）

**未领取（`EVENT_GOT_OLD_AMBER` 未设置）：**

> "Ssh! I think that
> this chunk of
> AMBER contains
> POKÉMON DNA!
>
> It would be great
> if POKÉMON could
> be resurrected
> from it!
>
> But, my colleagues
> just ignore me!
>
> So I have a favor
> to ask!
>
> Take this to a
> POKÉMON LAB and
> get it examined!"

- 给予成功：`"\<PLAYER\> received OLD AMBER!"`（设置 `EVENT_GOT_OLD_AMBER`，隐藏陈列品）
- 背包已满：`"You don't have space for this!"`

**已领取：**

> "Ssh! Get the OLD
> AMBER checked!"

---

#### Scientist 3

> "We are proud of 2
> fossils of very
> rare, prehistoric
> POKÉMON!"

---

#### OLD AMBER 陈列台

> "The AMBER is
> clear and gold!"

---

## 十四、Museum 2F（博物馆二楼）

### NPC 列表

#### Youngster

> "MOON STONE?
>
> What's so special
> about it?"

#### Gramps

> "July 20, 1969!
>
> The 1st lunar
> landing!
>
> I bought a color
> TV to watch it!"

#### Scientist

> "We have a space
> exhibit now."

#### Brunette Girl

> "I want a PIKACHU!
> It's so cute!
>
> I asked my Daddy
> to catch me one!"

#### Hiker（女孩的父亲）

> "Yeah, a PIKACHU
> soon, I promise!"

### 标识牌

| 标识 | 文本 |
|---|---|
| SPACE SHUTTLE | "SPACE SHUTTLE COLUMBIA" |
| MOON STONE | "Meteorite that fell on MT.MOON. (MOON STONE?)" |

---

*下一章：Route 3 → Mt. Moon → Cerulean City*
