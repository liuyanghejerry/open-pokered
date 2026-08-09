# Route 3 / Mt. Moon / Cerulean City 区域 NPC 对话与剧情脚本

> 来源：pokered 反汇编代码
> 覆盖地图：Route3、Route4、MtMoon1F、MtMoonB1F、MtMoonB2F、MtMoonPokecenter、CeruleanCity、CeruleanGym、CeruleanMart、CeruleanPokecenter、CeruleanBadgeHouse、CeruleanTradeHouse、CeruleanTrashedHouse、BillsHouse
> 用途：Rust 重制版剧情参考

---

## 一、Route 3（3 号道路）

### 地图脚本流程

标准三状态训练师战斗状态机（DEFAULT / START_BATTLE / END_BATTLE），共 8 名训练师。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_ROUTE_3_TRAINER_0~7` | 击败各训练师 |

### NPC 列表

#### Super Nerd（非战斗 NPC）

> "Whew... I better take a rest... Groan...
> That tunnel from CERULEAN takes a lot out of you!"

---

#### Youngster 1（训练师）

- **挑战前：** "Hey! I met you in VIRIDIAN FOREST!"
- **战败时：** "You beat me again!"
- **战后：** "There are other kinds of #MON than those found in the forest!"

#### Youngster 2（训练师）

- **挑战前：** "Hi! I like shorts! They're comfy and easy to wear!"
- **战败时：** "I don't believe it!"
- **战后：** "Are you storing your #MON on PC? Each BOX can hold 20 #MON!"

#### CooltrainerF 1（训练师）

- **挑战前：** "You looked at me, didn't you?"
- **战败时：** "You're mean!"
- **战后：** "Quit staring if you don't want to fight!"

#### Youngster 3（训练师）

- **挑战前：** "Are you a trainer? Let's fight!"
- **战败时：** "If I had new #MON I would've won!"
- **战后：** "If a #MON BOX on the PC gets full, just switch to another BOX!"

#### CooltrainerF 2（训练师）

- **挑战前：** "That look you gave me, it's so intriguing!"
- **战败时：** "Be nice!"
- **战后：** "Avoid fights by not letting people see you!"

#### Youngster 4（训练师）

- **挑战前：** "Hey! You're not wearing shorts!"
- **战败时：** "Lost! Lost! Lost!"
- **战后：** "I always wear shorts, even in winter!"

#### Youngster 5（训练师）

- **挑战前：** "You can fight my new #MON!"
- **战败时：** "Done like dinner!"
- **战后：** "Trained #MON are stronger than the wild ones!"

#### CooltrainerF 3（训练师）

- **挑战前：** "Eek! Did you touch me?"
- **战败时：** "That's it?"
- **战后：** "ROUTE 4 is at the foot of MT.MOON."

### 标识牌

> "ROUTE 3 / MT.MOON AHEAD"

---

## 二、Route 4（4 号道路）

### 地图脚本流程

标准三状态训练师战斗状态机，1 名可战训练师。

地图道具：TM_WHIRLWIND（`PickUpItemText`）。

### NPC 列表

#### CooltrainerF 1（非战斗 NPC）

> "Ouch! I tripped over a rocky #MON, GEODUDE!"

#### CooltrainerF 2（训练师）

- **挑战前：** "I came to get my mushroom #MON!"
- **战败时：** "Oh! My cute mushroom #MON!"
- **战后：** "There might not be any more mushrooms here. I think I got them all."

### 标识牌

- "MT.MOON / Tunnel Entrance"
- "ROUTE 4 / MT.MOON - CERULEAN CITY"

---

## 三、Mt. Moon 1F（月亮山 1 层）

### 地图脚本流程

标准三状态训练师战斗状态机，7 名训练师。

地图道具：Potion × 2、Moon Stone、Rare Candy、Escape Rope、TM Water Gun。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_MT_MOON_1_TRAINER_0~6` | 击败各训练师 |

### NPC 列表

#### Hiker（训练师）

- **挑战前：** "WHOA! You shocked me! Oh, you're just a kid!"
- **战败时：** "Wow! Shocked again!"
- **战后：** "Kids like you shouldn't be here!"

#### Youngster 1（训练师）

- **挑战前：** "Did you come to explore too?"
- **战败时：** "Losing stinks!"
- **战后：** "I came down here to show off to girls."

#### CooltrainerF 1（训练师）

- **挑战前：** "Wow! It's way bigger in here than I thought!"
- **战败时：** "Oh! I lost it!"
- **战后：** "How do you get out of here?"

#### Super Nerd（训练师）

- **挑战前：** "What! Don't sneak up on me!"
- **战败时：** "My #MON won't do!"
- **战后：** "I have to find stronger #MON."

#### CooltrainerF 2（训练师）

- **挑战前：** "What? I'm waiting for my friends to find me here."
- **战败时：** "I lost?"
- **战后：** "I heard there are some very rare fossils here."

#### Youngster 2（训练师）

- **挑战前：** "Suspicious men are in the cave. What about you?"
- **战败时：** "You got me!"
- **战后：** "I saw them! I'm sure they're from TEAM ROCKET!"

#### Youngster 3（训练师）

- **挑战前：** "Go through this cave to get to CERULEAN CITY!"
- **战败时：** "I lost."
- **战后：** "ZUBAT is tough! But, it can be useful if you catch one."

### 标识牌

> "Beware! ZUBAT is a blood sucker!"

---

## 四、Mt. Moon B1F（月亮山地下 1 层）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无训练师、无道具、无实质交互。

存在一个 `MtMoonB1FUnusedText`（内容为空），**未使用，不会显示**。

---

## 五、Mt. Moon B2F（月亮山地下 2 层）

### 地图脚本流程（状态机）

| 状态 (`wMtMoonB2FCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 检查化石/Super Nerd 触发坐标；若 `EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD` 未触发且玩家在 (X=13, Y=8) 则强制进入 Super Nerd 对话 |
| 1 `START_BATTLE` | 显示训练师文本并进入战斗 |
| 2 `END_BATTLE` | 战斗结算 |
| 3 `DEFEATED_SUPER_NERD` | 打败 Super Nerd 后的过场 |
| 4 `MOVE_SUPER_NERD` | Super Nerd 移动去拿另一块化石 |
| 5 `SUPER_NERD_TAKES_OTHER_FOSSIL` | Super Nerd 的台词 + 隐藏另一块化石 |

**战斗禁区：** `EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD` 已触发时，玩家在化石区域（X=11~14, Y=5~8）内设置 `BIT_NO_BATTLES`。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_MT_MOON_3_TRAINER_0~3` | 击败各 Team Rocket 训练师 |
| `EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD` | 打败出口处 Super Nerd，化石区域已解锁 |
| `EVENT_GOT_DOME_FOSSIL` | 玩家拿走了 Dome Fossil |
| `EVENT_GOT_HELIX_FOSSIL` | 玩家拿走了 Helix Fossil |

### NPC 列表

#### Super Nerd（剧情核心 NPC，坐标触发）

**分支 A：首次接触（`EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD` 未设置）：**

> "Hey, stop!
>
> I found these fossils! They're both mine!"

→ 进入战斗。

战败文本：`"OK! I'll share!"`

**分支 B：已击败 Super Nerd，且两块化石均未取走：**

> "We'll each take one!
> No being greedy!"

**分支 C：已击败 Super Nerd，且玩家已拿走其中一块化石：**

> "Far away, on CINNABAR ISLAND, there's a #MON LAB.
> They do research on regenerating fossils."

**Super Nerd 拿走另一块化石时：**

> "All right. Then this is mine!"

---

#### Dome Fossil（Yes/No 交互）

- 提示：`"You want the DOME FOSSIL?"`
- YES + 背包有空位：`"\<PLAYER\> got the DOME FOSSIL!"` → 触发 `EVENT_GOT_DOME_FOSSIL`，进入 MOVE_SUPER_NERD 状态
- YES + 背包已满：`"Look, you've got no room for this."`
- NO：直接关闭

#### Helix Fossil（Yes/No 交互，逻辑与 Dome Fossil 对称）

- 提示：`"You want the HELIX FOSSIL?"`
- YES + 背包有空位：`"\<PLAYER\> got the HELIX FOSSIL!"`

---

#### Team Rocket 1（训练师）

- **挑战前：** "TEAM ROCKET will find the fossils, revive and sell them for cash!"
- **战败时：** "Urgh! Now I'm mad!"
- **战后：** "You made me mad! TEAM ROCKET will blacklist you!"

#### Team Rocket 2（训练师）

- **挑战前：** "We, TEAM ROCKET, are #MON gangsters!"
- **战败时：** "I blew it!"
- **战后：** "Darn it all! My associates won't stand for this!"

#### Team Rocket 3（训练师）

- **挑战前：** "We're pulling a big job here! Get lost, kid!"
- **战败时：** "So, you are good."
- **战后：** "If you find a fossil, give it to me and scram!"

#### Team Rocket 4（训练师）

- **挑战前：** "Little kids should leave grown-ups alone!"
- **战败时：** "I'm steamed!"
- **战后：** "#MON lived here long before people came."

---

### 地板道具

- HP Up
- TM Mega Punch

---

## 六、Mt. Moon Pokecenter（月亮山口宝可梦中心）

### NPC 列表

#### Nurse

标准精灵中心治疗流程。

#### Youngster

> "I've 6 # BALLs set in my belt.
> At most, you can carry 6 #MON."

#### Gentleman

> "TEAM ROCKET attacks CERULEAN citizens...
> TEAM ROCKET is always in the news!"

#### Magikarp Salesman（关键 NPC）

**未购买（`EVENT_BOUGHT_MAGIKARP` 未设置，Yes/No）：**

> "MAN: Hello, there! Have I got a deal just for you!
> I'll let you have a swell MAGIKARP for just ¥500!
> What do you say?"

- YES + 金额不足：`"You'll need more money than that!"`
- YES + 金额足够：给予 Lv.5 MAGIKARP，扣 ¥500，设置 `EVENT_BOUGHT_MAGIKARP`
- NO：`"No? I'm only doing this as a favor to you!"`

**已购买：**

> "MAN: Well, I don't give refunds!"

#### Link Receptionist

标准通信俱乐部接待流程。

---

## 七、Cerulean City（浅蓝市）

### 地图脚本流程（状态机）

`wCeruleanCityCurScript` 控制：

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 检测火箭队小混混触发坐标（X=30，Y=7或9）；检测宿敌战斗触发坐标（X=20或21，Y=6） |
| 1 `RIVAL_BATTLE` | 宿敌移动完毕后显示对话并进入战斗 |
| 2 `RIVAL_DEFEATED` | 宿敌被打败后的过场 |
| 3 `RIVAL_CLEANUP` | 宿敌离开，恢复默认音乐 |
| 4 `ROCKET_DEFEATED` | 打败火箭队小混混后发放 TM28 DIG |

**宿敌战斗触发：** `EVENT_BEAT_CERULEAN_RIVAL` 未设置 && 玩家在桥上坐标（X=20或21，Y=6）→ 播放 `Music_MeetRival`，宿敌精灵出现并向下走3步。

**宿敌队伍编号：**

| 宿敌起始精灵 | 训练师编号 |
|---|---|
| STARTER2（Squirtle） | 7 |
| STARTER3（Bulbasaur） | 8 |
| STARTER1（Charmander） | 9 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_CERULEAN_RIVAL` | 在桥上打败宿敌 |
| `EVENT_BEAT_CERULEAN_ROCKET_THIEF` | 打败火箭队小混混 |

### NPC 列表

#### Rival（桥上，自动触发）

**战斗前：**

> "\<RIVAL\>: Yo! \<PLAYER\>!
> You're still struggling along back here?
> I'm doing great! I caught a bunch of strong and smart #MON!
> Here, let me see what you caught, \<PLAYER\>!"

**宿敌败北时：**

> "Hey! Take it easy! You won already!"

**宿敌胜利时：**

> "Heh! You're no match for my genius!"

**战斗后再次对话：**

> "\<RIVAL\>: Hey, guess what?
> I went to BILL's and got him to show me his rare #MON!
> That added a lot of pages to my #DEX!
> After all, BILL's world famous as a #MANIAC!
> He invented the #MON Storage System on PC!
> Since you're using his system, go thank him!
> Well, I better get rolling! Smell ya later!"

---

#### Rocket Thief（火箭队小混混，坐标触发）

**战斗前（自动触发）：**

> "Hey! Stay out! It's not your yard! Huh? Me?
> I'm an innocent bystander! Don't you believe me?"

战败台词：`"Stop! I give up! I'll leave quietly!"`

**战斗后（归还 TM28）：**

> "OK! I'll return the TM I stole!"

- 给予成功：`"\<PLAYER\> recovered TM28!"` → `"I better get moving! Bye!"`
- 背包已满：`"Make room for this! I can't run until I give it to you!"`

---

#### CooltrainerM

> "You're a trainer too? Collecting, fighting, it's a tough life."

#### Super Nerd 1

> "That bush in front of the shop is in the way.
> There might be a way around."

#### Super Nerd 2

> "You're making an encyclopedia on #MON? That sounds amusing."

#### Guard（×2，同一文本）

> "The people here were robbed.
> It's obvious that TEAM ROCKET is behind this most heinous crime!
> Even our POLICE force has trouble with the ROCKETs!"

#### CooltrainerF 1（随机对话，3 条，基于 `hRandomAdd`）

| 条件 | 文本 |
|---|---|
| `hRandomAdd >= 180` | "OK! SLOWBRO! Use SONICBOOM! Come on, SLOWBRO pay attention!" |
| `100 <= hRandomAdd < 180` | "SLOWBRO punch! No! You blew it again!" |
| `hRandomAdd < 100` | "SLOWBRO, WITHDRAW! No! That's wrong! It's so hard to control #MON! Your #MON's obedience depends on your abilities as a trainer!" |

#### Slowbro（随机对话，4 条，基于 `hRandomAdd`）

| 条件 | 文本 |
|---|---|
| `>= 180` | "SLOWBRO took a snooze..." |
| `120~179` | "SLOWBRO is loafing around..." |
| `60~119` | "SLOWBRO turned away..." |
| `< 60` | "SLOWBRO ignored orders..." |

#### CooltrainerF 2

> "I want a bright red BICYCLE!
> I'll keep it at home, so it won't get dirty!"

#### Super Nerd 3（洞窟入口前）

> "This is CERULEAN CAVE! Horribly strong #MON live in there!
> The #MON LEAGUE champion is the only person who is allowed in!"

### 标识牌

| 标识 | 文本 |
|---|---|
| 城市路牌 | "CERULEAN CITY / A Mysterious, Blue Aura Surrounds It" |
| Trainer Tips | "TRAINER TIPS / Pressing B Button during evolution cancels the whole process." |
| 自行车店 | "Grass and caves handled easily! BIKE SHOP" |
| 道馆 | "CERULEAN CITY #MON GYM / LEADER: MISTY / The Tomboyish Mermaid!" |

---

## 八、Cerulean Gym（浅蓝道馆）

### 地图脚本流程（状态机）

进入时加载城市名 `CERULEAN CITY`、馆主名 `MISTY`。

| 状态 (`wCeruleanGymCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | `CheckFightingMapTrainers` |
| 1 `START_BATTLE` | `DisplayEnemyTrainerTextAndStartBattle` |
| 2 `END_BATTLE` | `EndTrainerBattle` |
| 3 `MISTY_POST_BATTLE` | 发放 CASCADE BADGE 和 TM11 BUBBLEBEAM |

**Misty 被打败后：** 显示徽章文本 → 设置 `EVENT_BEAT_MISTY` → 给予 TM11 → 设置 `BIT_CASCADEBADGE` → 强制设置两名训练师的战败事件。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_MISTY` | 已击败馆主 Misty |
| `EVENT_GOT_TM11` | 已领取 TM11 BUBBLEBEAM |
| `EVENT_BEAT_CERULEAN_GYM_TRAINER_0` | 已击败道馆训练师 CooltrainerF |
| `EVENT_BEAT_CERULEAN_GYM_TRAINER_1` | 已击败道馆训练师 Swimmer |

### NPC 列表

#### Misty（馆主）

**战斗前：**

> "Hi, you're a new face!
> Trainers who want to turn pro have to have a policy about #MON!
> What is your approach when you catch #MON?
> My policy is an all-out offensive with water-type #MON!"

**战败后（自动显示 CASCADE BADGE 文本）：**

> "Wow! You're too much!
> All right!
> You can have the CASCADEBADGE to show you beat me!"

接续说明：

> "The CASCADEBADGE makes all #MON up to L30 obey!
> That includes even outsiders!
> There's more, you can now use CUT any time!
> You can CUT down small bushes to open new paths!
> You can also have my favorite TM!"

- TM11 给予成功：`"\<PLAYER\> received TM11!"`
- 背包已满：`"You better make room for this!"`

**已击败 Misty，已领 TM11：**

> "TM11 teaches BUBBLEBEAM!
> Use it on an aquatic #MON!"

---

#### Gym Guide（道馆向导）

**未击败 Misty：**

> "Yo! Champ in making!
> Here's my advice!
> The LEADER, MISTY, is a pro who uses water #MON!
> You can drain all their water with plant #MON!
> Or, zap them with electricity!"

**已击败 Misty：**

> "You beat MISTY! What'd I tell ya?
> You and me kid, we make a pretty darn good team!"

---

#### CooltrainerF（道馆训练师）

- **挑战前：** "I'm more than good enough for you! MISTY can wait!"
- **战败时：** "You overwhelmed me!"
- **战后：** "You have to face other trainers to find out how good you really are."

#### Swimmer（道馆训练师）

- **挑战前：** "Splash! I'm first up! Let's do it!"
- **战败时：** "That can't be!"
- **战后：** "MISTY is going to keep improving! She won't lose to someone like you!"

---

## 九、Cerulean Mart（浅蓝市商店）

### NPC 列表

#### Clerk

标准商店购物流程。

#### CooltrainerM

> "Use REPEL to keep bugs and weak #MON away.
> Put your strongest #MON at the top of the list for best results!"

#### CooltrainerF

> "Have you seen any RARE CANDY?
> It's supposed to make #MON go up one level!"

---

## 十、Cerulean Pokecenter（浅蓝市宝可梦中心）

### NPC 列表

#### Nurse

标准精灵中心治疗流程。

#### Super Nerd

> "That BILL!
> I heard that he'll do whatever it takes to get rare #MON!"

#### Gentleman

> "Have you heard about BILL?
> Everyone calls him a #MANIAC!
> I think people are just jealous of BILL, though.
> Who wouldn't want to boast about their #MON?"

#### Link Receptionist

标准通信俱乐部接待流程。

---

## 十一、Cerulean Badge House（徽章说明屋）

### 地图脚本流程

设置 `BIT_NO_AUTO_TEXT_BOX` 和 `wDoNotWaitForButtonPressAfterDisplayingText = 1`。NPC 展示 8 项徽章菜单，玩家可反复查询直至按 B 退出。

### NPC 列表

#### Middle-Aged Man

**开场：**

> "#MON BADGEs are owned only by skilled trainers.
> I see you have at least one.
> Those BADGEs have amazing secrets!"

**选择提示：**

> "Now then... Which of the 8 BADGEs should I describe?"

**结束：**

> "Come visit me any time you wish."

**各徽章说明：**

| 徽章 | 说明 |
|---|---|
| BOULDERBADGE | "The ATTACK of all #MON increases a little bit. It also lets you use FLASH any time you desire." |
| CASCADEBADGE | "#MON up to L30 will obey you. Any higher, they become unruly! It also lets you use CUT outside of battle." |
| THUNDERBADGE | "The SPEED of all #MON increases a little bit. It also lets you use FLY outside of battle." |
| RAINBOWBADGE | "#MON up to L50 will obey you. Any higher, they become unruly! It also lets you use STRENGTH outside of battle." |
| SOULBADGE | "The DEFENSE of all #MON increases a little bit. It also lets you use SURF outside of battle." |
| MARSHBADGE | "#MON up to L70 will obey you. Any higher, they become unruly!" |
| VOLCANOBADGE | "Your #MON's SPECIAL abilities increase a bit." |
| EARTHBADGE | "All #MON will obey you!" |

---

## 十二、Cerulean Trade House（浅蓝市交换屋）

### NPC 列表

#### Granny

> "My husband likes trading #MON.
> If you are a collector, would you please trade with him?"

#### Gambler（游戏内交换 NPC）

交换参数：`TRADE_FOR_LOLA`

| 参数 | 内容 |
|---|---|
| 给出宝可梦 | POLIWHIRL |
| 获得宝可梦 | JYNX（昵称：LOLA） |
| 对话套装 | `TRADE_DIALOGSET_EVOLUTION` |

---

## 十三、Cerulean Trashed House（浅蓝市被洗劫的房屋）

### NPC 列表

#### Fishing Guru（条件对话）

**背包中没有 TM DIG：**

> "Those miserable ROCKETs!
> Look what they did here!
> They stole a TM for teaching #MON how to DIG holes!
> That cost me a bundle, it did!"

**背包中已有 TM DIG（已从火箭队小混混处拿回）：**

> "I figure what's lost is lost!
> I decided to teach DIGLETT how to DIG without a TM!"

#### Girl

> "TEAM ROCKET must be trying to DIG their way into no good!"

#### 墙洞（可检查物体）

> "TEAM ROCKET left a way out!"

---

## 十四、Bill's House（比尔的小屋）

### 地图脚本流程（状态机）

| 状态 (`wBillsHouseCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 无操作（ret） |
| 1 `POKEMON_WALK_TO_MACHINE` | Bill（变身精灵）走向传送机 |
| 2 `POKEMON_ENTERS_MACHINE` | 等待移动完成，隐藏精灵，设置 `EVENT_BILL_SAID_USE_CELL_SEPARATOR` |
| 3 `BILL_EXITS_MACHINE` | 等待 Cell Separator 使用后，人类 Bill 精灵出现并走出机器 |
| 4 `CLEANUP` | 移动完成后设置 `EVENT_MET_BILL`/`EVENT_MET_BILL_2`，返回状态0 |
| 5 `PC` | 激活 Bill's PC（`script_bills_pc`） |

**Bill 精灵移动路径：**
- 正常路径：上 3 步
- 绕行路径（玩家朝下）：右1 → 上2 → 左1 → 上1
- 人类 Bill 走出机器：下1 → 右3 → 下1

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BILL_SAID_USE_CELL_SEPARATOR` | Bill 进入机器，PC 可用 Cell Separation System |
| `EVENT_USED_CELL_SEPARATOR_ON_BILL` | 玩家已执行 Cell Separation System |
| `EVENT_MET_BILL` | 与人类 Bill 会面完成 |
| `EVENT_GOT_SS_TICKET` | 已从 Bill 处获得 S.S. TICKET |

### NPC 列表

#### Bill（变身为精灵的状态，Yes/No）

询问（强制帮助，选 No 后会再次询问直到同意）：

> "Hiya! I'm a #MON... ...No I'm not!
> Call me BILL! I'm a true blue #MANIAC! Hey!
> What's with that skeptical look?
> I'm not joshing you, I screwed up an experiment and got combined with a #MON!
> So, how about it? Help me out here!"

- YES（或最终同意）：
  > "When I'm in the TELEPORTER, go to my PC and run the Cell Separation System!"
  → 进入 POKEMON_WALK_TO_MACHINE 状态

- NO（第一次拒绝）：
  > "No!? Come on, you gotta help a guy in deep trouble!
  > What do you say, chief? Please? OK? All right!"
  → 强制进入帮助流程

---

#### Bill（人类形态恢复后）

**首次对话且 `EVENT_GOT_SS_TICKET` 未触发：**

> "BILL: Yeehah! Thanks, bud! I owe you one!
> So, did you come to see my #MON collection? You didn't? That's a bummer.
> I've got to thank you... Oh here, maybe this'll do."

- 给予 S.S. TICKET 成功：`"\<PLAYER\> received an S.S.TICKET!"`
- 背包已满：`"You've got too much stuff, bud!"`

接续（无论是否已拿 SS Ticket）：

> "That cruise ship, S.S.ANNE, is in VERMILION CITY. Its passengers are all trainers!
> They invited me to their party, but I can't stand fancy do's. Why don't you go instead of me?"

---

#### Bill 的 PC（互动物品）

**尚未帮助 Bill 前：**

> "BILL: Look, bud, just check out some of my rare #MON on my PC!"

**`EVENT_BILL_SAID_USE_CELL_SEPARATOR` 触发后：** 执行 Cell Separation System（`script_bills_pc`）。

---

*下一章：Route 5-10 → Vermilion City → SS Anne*
