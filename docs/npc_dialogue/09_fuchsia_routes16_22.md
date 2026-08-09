# Fuchsia City / Safari Zone / Route 16-22 区域 NPC 对话与剧情脚本

> 来源：pokered 反汇编代码
> 覆盖地图：FuchsiaCity、FuchsiaGym、FuchsiaMart、FuchsiaPokecenter、FuchsiaMeetingRoom、FuchsiaGoodRodHouse、FuchsiaBillsGrandpasHouse、WardensHouse、SafariZoneGate/Center/East/West/North/SecretHouse（及各 Rest House）、Route16-22（含 Gate/FlyHouse）、Daycare
> 用途：Rust 重制版剧情参考

---

## 一、Fuchsia City（红紫市）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无状态机。

### NPC 列表

#### Youngster 1

> "Did you try the SAFARI GAME? Some #MON can only be caught there."

#### Gambler

> "SAFARI ZONE has a zoo in front of the entrance.
>
> Out back is the SAFARI GAME for catching #MON."

#### Erik（与 Safari Zone Center Rest House 的 Sara 相关）

> "ERIK: Where's SARA? I said I'd meet her here."

#### Youngster 2

> "That item ball in there is really a #MON."

#### 化石标志（条件对话）

**`EVENT_GOT_DOME_FOSSIL` 和 `EVENT_GOT_HELIX_FOSSIL` 均未设置：** `"..."`

**已获得圆形化石（Dome Fossil）：**

> "Name: KABUTO
>
> A #MON that was resurrected from a fossil."

**已获得螺旋化石（Helix Fossil）：**

> "Name: OMANYTE
>
> A #MON that was resurrected from a fossil."

### 标识牌

| 标识 | 文本 |
|---|---|
| 城市路牌 | "FUCHSIA CITY / Behold! It's Passion Pink!" |
| Safari Game | "SAFARI GAME / #MON-U-CATCH!" |
| Warden's Home | "SAFARI ZONE / WARDEN's HOME" |
| Safari Zone | "#MON PARADISE / SAFARI ZONE" |
| 道馆 | "FUCHSIA CITY #MON GYM / LEADER: KOGA / The Poisonous Ninja Master" |

---

## 二、Fuchsia Gym（红紫道馆）

### 地图脚本流程（状态机）

进入时加载城市名 `FUCHSIA CITY`、馆主名 `KOGA`。

| 状态 (`wFuchsiaGymCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | `CheckFightingMapTrainers` |
| 1 `START_BATTLE` | `DisplayEnemyTrainerTextAndStartBattle` |
| 2 `END_BATTLE` | `EndTrainerBattle` |
| 3 `KOGA_POST_BATTLE` | 给予 TM06 TOXIC；设置 SOULBADGE |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_KOGA` | 已击败馆主 Koga |
| `EVENT_GOT_TM06` | 已领取 TM06 TOXIC |
| `EVENT_BEAT_FUCHSIA_GYM_TRAINER_0~5` | 已击败各道馆训练师 |

### NPC 列表

#### Koga（馆主）

**战斗前：**

> "KOGA: Fwahahaha! A mere child like you dares to challenge me?
>
> Very well, I shall show you true terror as a ninja master!
>
> You shall feel the despair of poison and sleep techniques!"

**战败后（自动显示 SOULBADGE 文本）：**

> "Humph! You have proven your worth!
>
> Here! Take the SOULBADGE!"

接续：

> "Now that you have the SOULBADGE, the DEFENSE of your #MON increases!
>
> It also lets you SURF outside of battle!"

- TM06 给予成功：`"<PLAYER> received TM06!"`
  接续：
  > "TM06 contains TOXIC!
  >
  > It is a secret technique over 400 years old!"
- 背包已满：`"You don't have room for this!"`

**战后再次对话（已给 TM06）：**

> "When afflicted by TOXIC, #MON suffer more and more as battle progresses!
>
> It will surely terrorize foes!"

---

#### Gym Guide

**未击败 Koga：**

> "Yo! Champ in making!
>
> FUCHSIA GYM is riddled with invisible walls!
>
> KOGA might appear close, but he's blocked off!
>
> You have to find gaps in the walls to reach him!"

**已击败：**

> "It's amazing how ninja can terrify even now!"

---

#### 道馆训练师（6 名 Rocker）

| # | 挑战前 | 战败时 | 战后 |
|---|---|---|---|
| 1 | "Strength isn't the key for #MON! It's strategy! I'll show you how strategy can beat brute strength!" | "What? Extraordinary!" | "So, you mix brawn with brains? Good strategy!" |
| 2 | "I wanted to become a ninja, so I joined this GYM!" | "I'm done for!" | "I will keep on training under KOGA, my ninja master!" |
| 3 | "Let's see you beat my special techniques!" | "You had me fooled!" | "I like poison and sleep techniques, as they linger after battle!" |
| 4 | "Stop right there! Our invisible walls have you frustrated?" | "Whoa! He's got it!" | "You impressed me! Here's a hint! Look very closely for gaps in the invisible walls!" |
| 5 | "I also study the way of the ninja with master KOGA! Ninja have a long history of using animals!" | "Awoo!" | "I still have much to learn!" |
| 6 | "Master KOGA comes from a long line of ninjas! What did you descend from?" | "Dropped my balls!" | "Where there is light, there is shadow! Light and shadow! Which do you choose?" |

---

## 三、Fuchsia Mart（红紫市商店）

### NPC 列表

#### Clerk

标准商店购物流程。

#### Middle-Aged Man

> "Do you have a SAFARI ZONE flag?
>
> What about cards or calendars?"

#### Cooltrainer F

> "Did you try X SPEED? It speeds up a #MON in battle!"

---

## 四、Fuchsia Pokecenter（红紫市精灵中心）

### NPC 列表

#### Nurse

标准精灵中心治疗流程。

#### Rocker

> "You can't win with just one strong #MON.
>
> It's tough, but you have to raise them evenly."

#### Cooltrainer F

> "There's a narrow trail west of VIRIDIAN CITY.
>
> It goes to #MON LEAGUE HQ. The HQ governs all trainers."

#### Link Receptionist

标准通信俱乐部接待流程。

---

## 五、Fuchsia Meeting Room（红紫市会议室）

### NPC 列表

#### Safari Zone Worker 1

> "We nicknamed the WARDEN SLOWPOKE.
>
> He and SLOWPOKE both look vacant!"

#### Safari Zone Worker 2

> "SLOWPOKE is very knowledgeable about #MON!
>
> He even has some fossils of rare, extinct #MON!"

#### Safari Zone Worker 3

> "SLOWPOKE came in, but I couldn't understand him.
>
> I think he's got a speech problem!"

---

## 六、Fuchsia Good Rod House（红紫市好钓竿之家）

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `BIT_GOT_GOOD_ROD` | 已获得 Good Rod |

### NPC 列表

#### Fishing Guru（关键 NPC）

**未领取（`BIT_GOT_GOOD_ROD` 未设置）：**

> "I'm the FISHING GURU's older brother!
>
> I simply Looove fishing!
>
> Do you like to fish?"

- YES → 给予 **Good Rod**（设置 `BIT_GOT_GOOD_ROD`）
- NO → `"Oh... That's so disappointing..."`
- 背包已满：`"Oh no! You have no room for my gift!"`

**已领取：**

> "Hello there, <PLAYER>!
>
> How are the fish biting?"

---

## 七、Fuchsia Bill's Grandpa's House（红紫市比尔外公之家）

### NPC 列表

#### Middle-Aged Woman

> "SAFARI ZONE's WARDEN is old, but still active!
>
> All his teeth are false, though."

#### Bill's Grandpa

> "Hmm? You've met BILL?
>
> He's my grandson!
>
> He always liked collecting things even as a child!"

#### Youngster

> "BILL files his own #MON data on his PC!
>
> Did he show you?"

---

## 八、Warden's House（Safari Zone 守卫之家）

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_GOT_HM04` | 已从守卫处获得 HM04 STRENGTH |
| `EVENT_GOT_WARDENS_GOLD_TEETH` | 守卫已找回金牙 |

### NPC 列表

#### Warden（守卫，关键 NPC）

**`EVENT_GOT_WARDENS_GOLD_TEETH` 未设置（金牙丢失）：**

> "（口齿不清的说话声）"

（无法听懂，提示玩家去找金牙）

**已找回金牙（背包中有 GOLD_TEETH）：**

> "My GOLD TEETH!
>
> You found them!
>
> Thank you!
>
> Here, take this
> as a reward!"

- 给予成功：`"<PLAYER> received HM04!"`（设置 `EVENT_GOT_HM04`，移除 GOLD_TEETH）
  接续：
  > "HM04 is STRENGTH!
  >
  > It's a powerful
  > move, and it
  > also lets you
  > push heavy
  > boulders!"
- 背包已满：`"You don't have room for this!"`

**已领取：**

> "STRENGTH lets you
> push boulders!
>
> Use it well!"

---

## 九、Safari Zone Gate（Safari Zone 大门）

### 地图脚本流程（状态机）

| 状态 (`wSafariZoneGateCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 自动触发 Worker 1 首次欢迎对话 |
| 1 `PLAYER_MOVING_RIGHT` | 玩家向右移动 |
| 2 `WOULD_YOU_LIKE_TO_JOIN` | Worker 1 询问是否参加（¥500） |
| 3 `PLAYER_MOVING` | 玩家进入 Safari Zone |
| 4 `PLAYER_MOVING_DOWN` | 玩家向下移动（离开） |
| 5 `LEAVING_SAFARI` | 离开 Safari Zone 处理（返还剩余 Safari Ball） |

### Safari Zone 机制

| 变量/标志 | 含义 |
|---|---|
| `EVENT_IN_SAFARI_ZONE` | 当前在 Safari Zone 内 |
| `EVENT_SAFARI_GAME_OVER` | Safari 游戏结束 |
| `wNumSafariBalls` | 剩余 Safari Ball 数（初始 30） |
| `wSafariSteps` | 剩余步数（初始 502 步） |

**入场费：¥500** → 获得 30 个 Safari Ball，步数限制 502。

### NPC 列表

#### Safari Zone Worker 1（关键 NPC）

**欢迎词：**

> "Welcome to the SAFARI ZONE!"

**询问参加：**

> "For just ¥500, you can catch all the #MON you want in the park!
>
> Would you like to join the hunt?"

**付款成功：**

> "That'll be ¥500 please!
>
> We only use a special # BALL here.
>
> <PLAYER> received 30 SAFARI BALLs!
>
> We'll call you on the PA when you run out of time or SAFARI BALLs!"

**付款不足：** `"Oops! Not enough money!"`

**拒绝：** `"OK! Please come again!"`

**提前离开询问：** `"Leaving early?"`

**游戏结束：** `"Did you get a good haul? Come again!"`

---

#### Safari Zone Worker 2（教育 NPC）

**首次访问（Yes/No）：**

> "Hi! Is it your first time here?"

- YES：
  > "SAFARI ZONE has 4 zones in it.
  >
  > Each zone has different kinds of #MON. Use SAFARI BALLs to catch them!
  >
  > When you run out of time or SAFARI BALLS, it's game over for you!
  >
  > Before you go, open an unused #MON BOX so there's room for new #MON!"

- NO：`"Sorry, you're a regular here!"`

---

## 十、Safari Zone Center（Safari Zone 中心区）

无 NPC，仅可收集物品（Nugget）和标识牌。

### 标识牌

- `"REST HOUSE"`
- `"TRAINER TIPS / Press the START Button to check remaining time!"`

---

## 十一、Safari Zone Center Rest House（Safari Zone 中心休息所）

### NPC 列表

#### Girl（Sara，与 Fuchsia City Erik 相关）

> "SARA: Where did my boy friend, ERIK, go?"

#### Scientist

> "I'm catching #MON to take home as gifts!"

---

## 十二、Safari Zone East（Safari Zone 东区）

### 可收集物品

- Full Restore、Max Restore、Carbos、TM_EGG_BOMB

### 标识牌

- `"TRAINER TIPS / The remaining time declines only while you walk!"`
- `"CENTER AREA / NORTH: AREA 2"`

---

## 十三、Safari Zone West（Safari Zone 西区）

### 可收集物品

- Max Potion、TM_DOUBLE_TEAM、Max Revive
- **GOLD_TEETH**（交给守卫换取 HM04）

### 标识牌

- `"TRAINER TIPS / Zone Exploration Campaign! The Search for the SECRET HOUSE!"`
- `"AREA 3 / EAST: CENTER AREA"`

---

## 十四、Safari Zone North（Safari Zone 北区）

### 可收集物品

- Protein、TM_SKULL_BASH

### 标识牌

- `"TRAINER TIPS / The SECRET HOUSE is still ahead!"`
- `"AREA 2"`
- `"TRAINER TIPS / #MON hide in tall grass! Zigzag through grassy areas to flush them out."`
- `"TRAINER TIPS / Win a free HM for finding the SECRET HOUSE!"`

---

## 十五、Safari Zone Secret House（Safari Zone 秘密之家）

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_GOT_HM03` | 已获得 HM03 SURF |

### NPC 列表

#### Fishing Guru（秘密之家居民，关键 NPC）

**`EVENT_GOT_HM03` 未设置（首次到达）：**

> "Ah! Finally!
>
> You're the first person to reach the SECRET HOUSE!
>
> I was getting worried that no one would win our campaign prize.
>
> Congratulations! You have won!"

- 给予成功：`"<PLAYER> received HM03!"`（设置 `EVENT_GOT_HM03`）
  接续：
  > "HM03 is SURF!
  >
  > #MON will be able to ferry you across water!
  >
  > And, this HM isn't disposable! You can use it over and over!
  >
  > You're super lucky for winning this fabulous prize!"
- 背包已满：`"You don't have room for this fabulous prize!"`

**已领取：** 重复 HM03 说明。

---

## 十六、Route 16（16 号道路，Cycling Road 起点）

### 地图脚本流程（状态机）

| 状态 (`wRoute16CurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 检查 Snorlax 和训练师战斗 |
| 1 `START_BATTLE` | 开始战斗 |
| 2 `END_BATTLE` | 结束战斗 |
| 3 `SNORLAX_POST_BATTLE` | Snorlax 战后处理（Snorlax 离开） |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_ROUTE16_SNORLAX` | 已击败 Snorlax |
| `EVENT_FIGHT_ROUTE16_SNORLAX` | 已触发 Snorlax 战斗 |
| `EVENT_BEAT_ROUTE_16_TRAINER_0~5` | 已击败各 Biker 训练师 |

### Snorlax 文本

- 阻挡时：`"A sleeping #MON blocks the way!"`
- 激活时：`"SNORLAX woke up! It attacked in a grumpy rage!"`
- 战胜后：`"With a big yawn, SNORLAX returned to the mountains!"`

### NPC 列表（6 名 Biker 训练师）

| # | 挑战前 | 战败时 | 战后 |
|---|---|---|---|
| 1 | "What do you want?" | "Don't you dare laugh!" | "We like just hanging here, what's it to you?" |
| 2 | "Nice BIKE! Hand it over!" | "Knock out!" | "Forget it, who needs your BIKE!" |
| 3 | "Come out and play, little mouse!" | "You little rat!" | "I hate losing! Get away from me!" |
| 4 | "Hey, you just bumped me!" | "Kaboom!" | "You can also get to FUCHSIA from VERMILION using a coastal road." |
| 5 | "I'm feeling hungry and mean!" | "Bad, bad, bad!" | "I like my #MON ferocious! They tear up enemies!" |
| 6 | "Sure, I'll go!" | "Don't make me mad!" | "I like harassing people with my vicious #MON!" |

### 标识牌

- `"Enjoy the slope! CYCLING ROAD"`
- `"ROUTE 16 / CELADON CITY - FUCHSIA CITY"`

---

## 十七、Route 16 Fly House（16 号道路 Fly 之家）

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_GOT_HM02` | 已获得 HM02 FLY |

### NPC 列表

#### Brunette Girl（关键 NPC）

**`EVENT_GOT_HM02` 未设置：**

> "Oh, you found my secret retreat!
>
> Please don't tell anyone I'm here. I'll make it up to you with this!"

- 给予成功：`"<PLAYER> received HM02!"`（设置 `EVENT_GOT_HM02`）
  接续：
  > "HM02 is FLY. It will take you back to any town.
  >
  > Put it to good use!"
- 背包已满：`"You don't have any room for this."`

**已领取：** 重复 HM02 说明。

#### Fearow（宠物精灵）

> "FEAROW: Kyueen!"

（播放 Fearow 叫声）

---

## 十八、Route 17（17 号道路，Cycling Road 主体）

### 地图脚本流程

标准三状态训练师战斗状态机（10 名 Biker）。

### 代表性 NPC 对话

| Biker | 挑战前 | 战后 |
|---|---|---|
| 1 | "There's no money in fighting kids!" | "Good stuff is lying around on CYCLING ROAD!" |
| 2 | "What do you want, kiddo?" | "I could belly-bump you outta here!" |
| 3 | "You heading to FUCHSIA?" | "I love racing downhill!" |
| 5 | "Let VOLTORB electrify you!" | "I got my VOLTORB at the abandoned POWER PLANT." |
| 6 | "My #MON won't evolve! Why?" | "Maybe some #MON need element STONEs to evolve." |

### 标识牌

- `"TRAINER TIPS / All #MON are unique. Even #MON of the same type and level grow at different rates."`
- `"TRAINER TIPS / Press the A or B Button to stay in place while on a slope."`
- `"ROUTE 17 / CELADON CITY - FUCHSIA CITY"`
- `"CYCLING ROAD / Slope ends here!"`

---

## 十九、Route 18（18 号道路，Cycling Road 终点）

### 训练师列表（3 名 Cooltrainer M）

| # | 挑战前 | 战后 |
|---|---|---|
| 1 | "I always check every grassy area for new #MON." | "I wish I had a BIKE!" |
| 2 | "Kurukkoo! How do you like my bird call?" | "I also collect sea #MON on weekends!" |
| 3 | "This is my turf! Get out of here!" | "This is my fave #MON hunting area!" |

### 标识牌

- `"ROUTE 18 / CELADON CITY - FUCHSIA CITY"`
- `"CYCLING ROAD / No pedestrians permitted!"`

---

## 二十、Route 19（19 号水上道路）

### 训练师列表（10 名，含 Cooltrainer 和 Swimmer）

代表性对话：

- Swimmer：`"I love swimming! What about you?"` / `"What's beyond the horizon?"`
- Cooltrainer M 1：`"Have to warm up before my swim!"` / `"Thanks, kid! I'm ready for a swim!"`
- Cooltrainer M 2：`"Wait! You'll have a heart attack!"` / `"Watch out for TENTACOOL!"`

### 标识牌

> "SEA ROUTE 19 / FUCHSIA CITY - SEAFOAM ISLANDS"

---

## 二十一、Route 20（20 号水上道路）

### 训练师列表（10 名 Swimmer + 1 名 Cooltrainer）

代表性对话：

- Swimmer：`"The water is shallow here."` / `"SEAFOAM is a quiet getaway!"`
- Cooltrainer M：`"I rode my bird #MON here!"`
- Swimmer：`"CINNABAR, in the west, has a LAB for #MON."`

### 标识牌

> "SEAFOAM ISLANDS"

---

## 二十二、Route 21（21 号水上道路）

### 训练师列表（9 名，含 Fisher 和 Swimmer）

代表性对话：

- Fisher 1：`"You want to know if the fish are biting?"` / `"I can't catch anything good!"`
- Fisher 2：`"I got a big haul! Wanna go for it?"` / `"I seem to only catch MAGIKARP!"`
- Swimmer 1：`"The sea cleanses my body and soul!"` / `"I like the mountains too!"`
- Swimmer 4：`"Right now, I'm in a triathlon meet!"` / `"I'm beat! But, I still have the bike race and marathon left!"`

---

## 二十三、Route 22（22 号道路，宿敌战斗点）

### 地图脚本流程（状态机）

| 状态 (`wRoute22CurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 检查宿敌战斗触发坐标 |
| 1 `RIVAL1_START_BATTLE` | 第一次宿敌战斗开始 |
| 2 `RIVAL1_AFTER_BATTLE` | 第一次宿敌战斗后 |
| 3 `RIVAL1_EXIT` | 第一次宿敌离开 |
| 4 `RIVAL2_START_BATTLE` | 第二次宿敌战斗开始 |
| 5 `RIVAL2_AFTER_BATTLE` | 第二次宿敌战斗后 |
| 6 `RIVAL2_EXIT` | 第二次宿敌离开 |
| 7 `NOOP` | 终态 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_1ST_ROUTE22_RIVAL_BATTLE` | 第一次宿敌战斗已触发 |
| `EVENT_2ND_ROUTE22_RIVAL_BATTLE` | 第二次宿敌战斗已触发 |
| `EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE` | 已赢第一次宿敌战 |
| `EVENT_BEAT_ROUTE22_RIVAL_2ND_BATTLE` | 已赢第二次宿敌战 |

### NPC 列表

#### 宿敌（第一次战斗，游戏初期）

**战斗前：**

> "<RIVAL>: Hey! <PLAYER>!
>
> You're going to #MON LEAGUE?
>
> Forget it! You probably don't have any BADGEs!
>
> The guard won't let you through!
>
> By the way, did your #MON get any stronger?"

**战败后：** `"Awww! You just lucked out!"`

**战后：**

> "<RIVAL>: What? Why do I have 2 #MON?
>
> You should catch some more too!"

---

#### 宿敌（第二次战斗，集齐徽章后）

**战斗前：**

> "<RIVAL>: What? <PLAYER>! What a surprise to see you here!
>
> So you're going to #MON LEAGUE?
>
> You collected all the BADGEs too? That's cool!
>
> Then I'll whip you <PLAYER> as a warm up for #MON LEAGUE!
>
> Come on!"

**战败后：** `"What!? I was just careless!"`

**战后：**

> "<RIVAL>: Hahaha! <PLAYER>! That's your best? You're nowhere near as good as me, pal!
>
> Go train some more! You loser!"

### 标识牌

> "#MON LEAGUE / Front Gate"

---

## 二十四、Route 22 Gate（22 号道路关卡，League 前守卫）

### NPC 列表

#### Gate Guard（守卫）

**未拥有 Boulder Badge：**

> "Only truly skilled trainers are allowed through.
>
> You don't have the BOULDERBADGE yet!
>
> The rules are rules. I can't let you pass."

（播放拒绝音效，强制玩家向下移动）

**已拥有 Boulder Badge：**

> "Oh! That is the BOULDERBADGE! Go right ahead!"

---

## 二十五、Daycare（保育所）

### 保育所机制

| 变量 | 含义 |
|---|---|
| `wDayCareInUse` | 0=无宝可梦，1=有宝可梦在保育 |
| `wDayCareMonName` | 寄养宝可梦昵称 |
| `wDayCareStartLevel` | 寄养时的等级 |
| `wDayCareNumLevelsGrown` | 成长的等级数 |
| `wDayCareTotalCost` | 总费用（¥100 × (成长等级数 + 1)） |

### NPC 列表

#### Gentleman（保育所老板，关键 NPC）

**无宝可梦在保育（询问寄养）：**

> "I run a DAYCARE. Would you like me to raise one of your #MON?"

- NO → `"OK! Please come again!"`
- YES → 显示队伍菜单
- 只有 1 只宝可梦 → `"You only have 1 #MON. You need a reserve!"`

**确认寄养：**

> "Fine, I'll look after @[宝可梦昵称] for a while.
>
> Come see me in a while."

**有宝可梦在保育（返回取回）：**

**宝可梦已成长：**

> "Your @[宝可梦昵称] has grown a lot!
>
> By level, it's grown by @[成长等级数]!
>
> Aren't I great?"

**宝可梦未成长：**

> "Back already? Your @[宝可梦昵称] needs some more time with me."

**收费与返还：**

> "You owe me ¥@[费用] for the return of this #MON."

- 付款成功：`"<PLAYER> got @[宝可梦昵称] back!"`
- 付款不足：保留宝可梦
- 队伍已满：`"You don't have any room for this #MON."` 保留宝可梦
- 宝可梦会 HM 招式：`"I can't accept #MON with HM moves!"`

---

*下一章：Cinnabar Island → Pokemon Mansion → Seafoam Islands → Power Plant → Rock Tunnel*
