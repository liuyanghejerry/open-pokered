# Cinnabar Island / Pokemon Mansion / Seafoam Islands / Power Plant / Rock Tunnel 区域 NPC 对话与剧情脚本

> 来源：pokered 反汇编代码
> 覆盖地图：CinnabarIsland、CinnabarGym、CinnabarMart、CinnabarPokecenter、CinnabarLab/FossilRoom/MetronomeRoom/TradeRoom、PokemonMansion1F-3F/B1F、SeafoamIslands1F/B1F-B4F、PowerPlant、RockTunnel1F/B1F、RockTunnelPokecenter
> 用途：Rust 重制版剧情参考

---

## 一、Cinnabar Island（火焰市）

### 地图脚本流程（状态机）

| 状态 (`wCinnabarIslandCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 检测玩家是否未获得 SECRET_KEY 且到达道馆门前位置（Y:4, X:18），触发上锁对话 |
| 1 `PLAYER_MOVING` | 玩家被强制移动（播放门锁住的动画） |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_MANSION_SWITCH_ON` | 精灵大厦开关已激活（影响大厦内部门） |
| `EVENT_LAB_STILL_REVIVING_FOSSIL` | 精灵研究所正在复活化石 |

### NPC 列表

#### Girl

> "CINNABAR GYM's BLAINE is an odd man who has lived here for decades."

#### Gambler

> "Scientists conduct experiments in the burned out building."

### 标识牌

| 标识 | 文本 |
|---|---|
| 城市路牌 | "CINNABAR ISLAND / The Fiery Town of Burning Desire" |
| 精灵研究所 | "#MON LAB" |
| 道馆 | "CINNABAR ISLAND #MON GYM / LEADER: BLAINE / The Hot-Headed Quiz Master!" |
| 道馆门（上锁时） | "The door is locked..." |

---

## 二、Cinnabar Gym（火焰市道馆）

### 地图脚本流程（状态机）

进入时加载城市名 `CINNABAR ISLAND`、馆主名 `BLAINE`。

| 状态 (`wCinnabarGymCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 等待玩家触发训练师 |
| 1 `GET_OPPONENT_TEXT` | 显示对手挑战文本 |
| 2 `OPEN_GATE` | 击败训练师后打开门（播放音效） |
| 3 `BLAINE_POST_BATTLE` | Blaine 战后脚本，给予 TM38 FIRE BLAST 和 VOLCANOBADGE |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_BLAINE` | 已击败馆主 Blaine |
| `EVENT_GOT_TM38` | 已领取 TM38 FIRE BLAST |
| `EVENT_BEAT_CINNABAR_GYM_TRAINER_0~6` | 已击败各道馆训练师 |
| `EVENT_CINNABAR_GYM_GATE0_UNLOCKED` | 对应大门已解锁 |

### NPC 列表

#### Blaine（馆主）

**战斗前：**

> "Hah! I am BLAINE! I am the LEADER of CINNABAR GYM!
>
> My fiery #MON will incinerate all challengers!
>
> Hah! You better have BURN HEAL!"

**战败后（自动显示 VOLCANOBADGE 文本）：**

> "I have burnt out! You have earned the VOLCANOBADGE!"

接续：

> "Hah! The VOLCANOBADGE heightens the SPECIAL abilities of your #MON!
>
> Here, you can have this too!"

- TM38 给予成功：`"<PLAYER> received TM38!"`
  接续：
  > "TM38 contains FIRE BLAST!
  >
  > Teach it to fire-type #MON!
  >
  > CHARMELEON or PONYTA would be good bets!"
- 背包已满：`"Make room for my gift!"`

**战后再次对话（已给 TM38）：**

> "FIRE BLAST is the ultimate fire technique!
>
> Don't waste it on water #MON!"

---

#### Gym Guide

**未击败 Blaine：**

> "Yo! Champ in making!
>
> The hot-headed BLAINE is a fire #MON pro!
>
> Douse his spirits with water!
>
> You better take some BURN HEALs!"

**已击败：**

> "<PLAYER>! You beat that fire brand!"

---

#### Super Nerd 1-7（7 名道馆训练师）

各训练师使用谜题门机制，击败后打开通往下一关的门。代表性对话：

| # | 挑战前 | 战败时 | 战后 |
|---|---|---|---|
| 1 | "Do you know how hot #MON fire breath can get?" | "Yow! Hot, hot, hot!" | "Fire, or to be more precise, combustion... Blah, blah, blah..." |

---

## 三、Cinnabar Mart（火焰市商店）

### NPC 列表

#### Clerk

标准商店购物流程。

#### Silph Worker F

> "Don't they have X ATTACK? It's good for battles!"

#### Scientist

> "It never hurts to have extra items!"

---

## 四、Cinnabar Pokecenter（火焰市精灵中心）

### NPC 列表

#### Nurse

标准精灵中心治疗流程。

#### Cooltrainer F

> "You can cancel evolution. When a #MON is evolving, you can stop it and leave it the way it is."

#### Gentleman

> "Do you have any friends? #MON you get in trades grow very quickly. I think it's worth a try!"

#### Link Receptionist

标准通信俱乐部接待流程。

---

## 五、Cinnabar Lab（火焰市精灵研究所）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无状态机。

### NPC 列表

#### Fishing Guru

> "We study #MON extensively here. People often bring us rare #MON for examination."

#### 照片（可检查物件）

> "A photo of the LAB's founder, DR.FUJI!"

### 标识牌

- `"#MON LAB / Meeting Room"`
- `"#MON LAB / R-and-D Room"`
- `"#MON LAB / Testing Room"`

---

## 六、Cinnabar Lab Fossil Room（化石复活室）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无状态机。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_GAVE_FOSSIL_TO_LAB` | 已将化石交给科学家 |
| `EVENT_LAB_STILL_REVIVING_FOSSIL` | 化石仍在复活中 |
| `EVENT_LAB_HANDING_OVER_FOSSIL_MON` | 准备交付复活后的宝可梦 |

### 可接受化石

- DOME_FOSSIL（圆顶化石）→ 复活 KABUTO
- HELIX_FOSSIL（螺旋化石）→ 复活 OMANYTE
- OLD_AMBER（琥珀）→ 复活 AERODACTYL

### NPC 列表

#### Scientist 1（化石复活主任）

**初次交互（无化石）：**

> "Hiya! I am important doctor! I study here rare #MON fossils!
>
> You! Have you a fossil for me?"

- 无化石：`"No! Is too bad!"`
- 有化石 → 接受并开始复活，设置 `EVENT_GAVE_FOSSIL_TO_LAB` 和 `EVENT_LAB_STILL_REVIVING_FOSSIL`

**复活中：**

> "I take a little time! You go for walk a little while!"

**复活完成（`EVENT_LAB_STILL_REVIVING_FOSSIL` 未设置，`EVENT_GAVE_FOSSIL_TO_LAB` 已设置）：**

> "Where were you? Your fossil is back to life!
>
> It was @[宝可梦名] like I think!"

（给予复活后的宝可梦 Lv.30）

---

## 七、Cinnabar Lab Metronome Room（摇摆摆研究室）

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_GOT_TM35` | 已获得 TM35 METRONOME |

### NPC 列表

#### Scientist 1（TM35 赠送者）

**`EVENT_GOT_TM35` 未设置：**

> "Tch-tch-tch! I made a cool TM! It can cause all kinds of fun!"

- 给予成功：`"<PLAYER> received TM35!"`（设置 `EVENT_GOT_TM35`）
  接续：
  > "Tch-tch-tch! That's the sound of a METRONOME!
  >
  > It tweaks your #MON's brain into using moves it doesn't know!"
- 背包已满：`"Your pack is crammed full!"`

**已领取：** 重复 TM35 说明。

#### Scientist 2

> "EEVEE can evolve into 1 of 3 kinds of #MON."

#### PC Email（可检查物件）

> "There's an e-mail message!
>
> ...
>
> The 3 legendary bird #MON are ARTICUNO, ZAPDOS and MOLTRES. Their whereabouts are unknown. We plan to explore the cavern close to CERULEAN.
>
> From: #MON RESEARCH TEAM
>
> ..."

---

## 八、Cinnabar Lab Trade Room（精灵交易室）

### NPC 列表

#### Super Nerd

> "I found this very strange fossil in MT.MOON! I think it's a rare, prehistoric #MON!"

#### Gramps（游戏内交换 NPC）

交换参数：`TRADE_FOR_DORIS`

#### Beauty（游戏内交换 NPC）

交换参数：`TRADE_FOR_CRINKLES`

---

## 九、Pokemon Mansion 1F（精灵大厦一楼）

### 地图脚本流程（状态机）

检查 `EVENT_MANSION_SWITCH_ON`，根据开关状态加载或卸载门 tile。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_MANSION_SWITCH_ON` | 大厦主开关已激活（影响所有楼层门） |

### NPC 列表

#### Scientist（训练师）

- **挑战前：** "Who are you? There shouldn't be anyone here."
- **战败时：** "Ouch!"
- **战后：** "A key? I don't know what you're talking about."

#### 秘密开关（可检查物件）

> "A secret switch! Press it?"

- YES → 播放 SFX_GO_INSIDE，设置 `EVENT_MANSION_SWITCH_ON`
- NO → `"Not quite yet!"`

### 可收集物品

- ESCAPE_ROPE、CARBOS

---

## 十、Pokemon Mansion 2F（精灵大厦二楼）

### NPC 列表

#### Super Nerd（训练师）

- **挑战前：** "I can't get out! This old place is one big puzzle!"
- **战败时：** "Oh no! My bag of loot!"
- **战后：** "Switches open and close alternating sets of doors!"

#### 日记第一页（可检查物件）

> "Diary: July 5 Guyana, South America
>
> A new #MON was discovered deep in the jungle."

#### 日记第二页（可检查物件）

> "Diary: July 10 We christened the newly discovered #MON, MEW."

### 可收集物品

- CALCIUM

---

## 十一、Pokemon Mansion 3F（精灵大厦三楼）

### 地图脚本流程

检测玩家是否落入洞中（坐标 16,14 / 17,14 / 19,14），根据 `EVENT_MANSION_SWITCH_ON` 状态传送至 1F 或 2F。

### NPC 列表

#### Super Nerd（训练师）

- **挑战前：** "This place is like, huge!"
- **战败时：** "Ayah!"
- **战后：** "I wonder where my partner went."

#### Scientist（训练师）

- **挑战前：** "My mentor once lived here."
- **战败时：** "Whew! Overwhelming!"
- **战后：** "So, you're stuck? Try jumping off over there!"

#### 日记（可检查物件）

> "Diary: Feb. 6 MEW gave birth. We named the newborn MEWTWO."

### 可收集物品

- MAX_POTION、IRON

---

## 十二、Pokemon Mansion B1F（精灵大厦地下一层）

### NPC 列表

#### Burglar（训练师）

- **挑战前：** "Uh-oh. Where am I now?"
- **战败时：** "Awooh!"
- **战后：** "You can find stuff lying around."

#### Scientist（训练师）

- **挑战前：** "This place is ideal for a lab."
- **战败时：** "What was that for?"
- **战后：** "I like it here! It's conducive to my studies!"

#### 日记（可检查物件）

> "Diary; Sept. 1 MEWTWO is far too powerful. We have failed to curb its vicious tendencies..."

### 可收集物品

- RARE_CANDY、FULL_RESTORE
- TM_BLIZZARD（TM14 暴风雪）
- TM_SOLARBEAM（TM22 日光束）
- **SECRET_KEY**（解锁辛纳巴岛道馆大门的关键物品）

---

## 十三、Seafoam Islands 1F（海浪岛一楼）

### 地图脚本流程

检查推动石头机制。当石头被推入洞中时，根据坐标判断，隐藏当前层石头，显示下一层石头。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_SEAFOAM1_BOULDER1_DOWN_HOLE` | 1F 第一块石头已落入洞中 |
| `EVENT_SEAFOAM1_BOULDER2_DOWN_HOLE` | 1F 第二块石头已落入洞中 |

**洞坐标：** (17, 6)、(24, 6)

---

## 十四、Seafoam Islands B1F-B3F（海浪岛地下一至三层）

各层均含石头推动机制，石头需从上层推落至 B4F 才能阻断水流、到达 Articuno。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_SEAFOAM2_BOULDER1_DOWN_HOLE` | B1F 第一块石头已落入洞中 |
| `EVENT_SEAFOAM2_BOULDER2_DOWN_HOLE` | B1F 第二块石头已落入洞中 |
| `EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE` | B2F 第一块石头已落入洞中 |
| `EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE` | B2F 第二块石头已落入洞中 |
| `EVENT_SEAFOAM4_BOULDER1_DOWN_HOLE` | B3F 第一块石头已落入洞中 |
| `EVENT_SEAFOAM4_BOULDER2_DOWN_HOLE` | B3F 第二块石头已落入洞中 |

---

## 十五、Seafoam Islands B3F（海浪岛地下三层，强力水流）

### 地图脚本流程（状态机）

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 检查两个石头是否都落入洞中，若否则触发强力水流 |
| 1 `OBJECT_MOVING1` | 玩家被强制水流冲走（PAD_DOWN 6次、PAD_RIGHT 5次、PAD_DOWN 3次） |
| 2 `MOVE_OBJECT` | 处理强力水流行为 |
| 3 `OBJECT_MOVING2` | 等待冲走动画完成 |

**强力水流触发坐标：** (15, 8)

---

## 十六、Seafoam Islands B4F（海浪岛地下四层，Articuno 最终战）

### 地图脚本流程（状态机）

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 等待玩家到达 Articuno 或水流坐标 |
| 1 `OBJECT_MOVING1` | 玩家浮出水面（上移操作） |
| 2 `MOVE_OBJECT` | 处理强力水流行为 |
| 3 `OBJECT_MOVING2` | 等待水流移动完成，修复冲浪状态 |
| 4 `OBJECT_MOVING3` | 战斗后处理 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_ARTICUNO` | 已击败冻之鸟（Articuno） |

### NPC 列表

#### Articuno（冻之鸟，传说神兽）

> "Gyaoo!"

（播放 Articuno 叫声，进入战斗）

### 标识牌

- `"Boulders might change the flow of water!"`
- `"DANGER Fast current!"`

---

## 十七、Power Plant（发电厂）

### 地图脚本流程（状态机）

| 状态 (`wPowerPlantCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 检查正在战斗的训练师 |
| 1 `START_BATTLE` | 显示训练师文本并开始战斗 |
| 2 `END_BATTLE` | 战斗后处理 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_POWER_PLANT_VOLTORB_0~7` | 已击败 8 个 Voltorb/Electrode 伪装训练师 |
| `EVENT_BEAT_ZAPDOS` | 已击败电击鸟（Zapdos） |

### NPC 列表

#### Voltorb / Electrode 伪装训练师（8 名）

所有 Voltorb/Electrode 伪装均使用相同战斗文本：

- **战前/战中/战后：** `"Bzzzt!"`

#### Zapdos（电击鸟，传说神兽）

> "Gyaoo!"

（播放 Zapdos 叫声，进入战斗）

### 可收集物品

- CARBOS、HP_UP、RARE_CANDY
- TM_THUNDER（TM25 电击）
- TM_REFLECT（TM33 反射盾）

---

## 十八、Rock Tunnel 1F（岩石隧道一楼）

### 地图脚本流程（状态机）

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 检查正在战斗的训练师 |
| 1 `START_BATTLE` | 显示训练师文本并开始战斗 |
| 2 `END_BATTLE` | 战斗后处理 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_ROCK_TUNNEL_1_TRAINER_0~6` | 已击败 1F 7 名训练师 |

### NPC 列表（7 名训练师）

#### Hiker 1

- **挑战前：** "This tunnel goes a long way, kid!"
- **战败时：** "Doh! You win!"
- **战后：** "Watch for ONIX! It can put the squeeze on you!"

#### Hiker 2

- **挑战前：** "Hmm. Maybe I'm lost in here..."
- **战败时：** "Ease up! What am I doing? Which way is out?"
- **战后：** "That sleeping #MON on ROUTE 12 forced me to take this detour."

#### Hiker 3

- **挑战前：** "Outsiders like you need to show me some respect!"
- **战败时：** "I give!"
- **战后：** "You're talented enough to hike!"

#### Super Nerd

- **挑战前：** "#MON fight! Ready, go!"
- **战败时：** "Game over!"
- **战后：** "Oh well, I'll get a ZUBAT as I go!"

#### Cooltrainer F 1

- **挑战前：** "Eek! Don't try anything funny in the dark!"
- **战败时：** "It was too dark!"
- **战后：** "I saw a MACHOP in this tunnel!"

#### Cooltrainer F 2

- **挑战前：** "I came this far for #MON!"
- **战败时：** "I'm out of #MON!"
- **战后：** "You looked cute and harmless!"

#### Cooltrainer F 3

- **挑战前：** "You have #MON! Let's start!"
- **战败时：** "You play hard!"
- **战后：** "Whew! I'm all sweaty now!"

### 标识牌

> "ROCK TUNNEL / CERULEAN CITY - LAVENDER TOWN"

---

## 十九、Rock Tunnel B1F（岩石隧道地下一层）

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_ROCK_TUNNEL_2_TRAINER_0~7` | 已击败 B1F 8 名训练师 |

### NPC 列表（8 名训练师）

#### Cooltrainer F 1

- **挑战前：** "Hikers leave twigs as trail markers."
- **战败时：** "Ohhh! I did my best!"
- **战后：** "I want to go home!"

#### Hiker 1

- **挑战前：** "Hahaha! Can you beat my power?"
- **战败时：** "Oops! Out-muscled!"
- **战后：** "I go for power because I hate thinking!"

#### Super Nerd 1

- **挑战前：** "You have a #DEX? I want one too!"
- **战败时：** "Shoot! I'm so jealous!"
- **战后：** "When you finish your #DEX, can I have it?"

#### Super Nerd 2

- **挑战前：** "Do you know about costume players?"
- **战败时：** "Well, that's that."
- **战后：** "Costume players dress up as #MON for fun."

#### Hiker 2

- **挑战前：** "My #MON techniques will leave you crying!"
- **战败时：** "I give! You're a better technician!"
- **战后：** "In mountains, you'll often find rock-type #MON."

#### Cooltrainer F 2

- **挑战前：** "I don't often come here, but I will fight you."
- **战败时：** "Oh! I lost!"
- **战后：** "I like tiny #MON, big ones are too scary!"

#### Hiker 3

- **挑战前：** "Hit me with your best shot!"
- **战败时：** "Fired away!"
- **战后：** "I'll raise my #MON to beat yours, kid!"

#### Super Nerd 3

- **挑战前：** "I draw #MON when I'm home."
- **战败时：** "Whew! I'm exhausted!"
- **战后：** "I'm an artist, not a fighter."

---

## 二十、Rock Tunnel Pokecenter（岩石隧道精灵中心）

### NPC 列表

#### Nurse

标准精灵中心治疗流程。

#### Gentleman

> "The element types of #MON make them stronger than some types and weaker than others!"

#### Fisher

> "I sold a useless NUGGET for ¥5000!"

#### Link Receptionist

标准通信俱乐部接待流程。

---

*下一章：Route 23-25 → Victory Road → Indigo Plateau → Elite Four → Hall of Fame*
