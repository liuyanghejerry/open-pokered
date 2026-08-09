# Route 5-10 / Vermilion City / SS Anne 区域 NPC 对话与剧情脚本

> 来源：pokered 反汇编代码
> 覆盖地图：Route5-10（含各 Gate）、UndergroundPath 系列、VermilionCity、VermilionGym、VermilionMart、VermilionPokecenter、VermilionDock、VermilionOldRodHouse、VermilionPidgeyHouse、VermilionTradeHouse、DiglettsCave 系列、SSAnne 系列
> 用途：Rust 重制版剧情参考

---

## 一、Route 5 / 6 / 7 / 8 Gate（萨藩关卡通用机制）

### 共享机制

四个关卡（Route 5/6/7/8 Gate）共享完全相同的守卫文本，差异仅在封锁坐标和推回方向。

**解锁条件：** `BIT_GAVE_SAFFRON_GUARDS_DRINK`（全局标志，一旦在任意关卡给予饮料，所有四个关卡同时解锁）。

**饮料优先级：** FRESH_WATER → SODA_POP → LEMONADE（消耗其中一瓶）。

| 关卡 | 封锁坐标 | 推回方向 |
|---|---|---|
| Route 5 Gate | (3,3) (4,3) | 向上 |
| Route 6 Gate | (3,2) (4,2) | 向下 |
| Route 7 Gate | (3,3) (3,4) | 向左 |
| Route 8 Gate | (2,3) (2,4) | 向右 |

### 守卫对话

**未给饮料（封锁时）：**

> "The guard is
> thirsty and
> blocking the way!
>
> He won't let you
> through!"

（或类似文本，由通用 Gate 守卫脚本处理）

**给予饮料后：**

> "Ahh! That hit the
> spot! Thanks!
>
> You can go
> through now!"

---

## 二、Underground Path（地下通道系列）

### 地图脚本流程

所有地下通道（Route5/6/7/7Copy/8、NorthSouth、WestEast）均仅调用 `EnableAutoTextBoxDrawing`，无状态机、无训练师。

### 特殊说明

- `UndergroundPathRoute7Copy` 是 Route 7 地下通道的镜像副本，结构相同。
- `UndergroundPathNorthSouth` 和 `UndergroundPathWestEast` 为连接南北/东西方向的通道，无 NPC。

---

## 三、Vermilion City（朱紫市）

### 地图脚本流程

`wVermilionCityCurScript` 控制：

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 检测玩家是否接近码头区域（若 SS Anne 已离港则显示提示） |
| 其他 | 标准 NPC 互动 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_SS_ANNE_LEFT` | SS Anne 已离港 |
| `EVENT_BEAT_LT_SURGE` | 已击败馆主 Lt. Surge |

### NPC 列表

#### Beauty

> "We're careful about
> pollution!
>
> We've heard GRIMER
> multiplies in
> toxic sludge!"

---

#### Gambler 1（条件对话）

**`EVENT_SS_ANNE_LEFT` 未设置：**

> "Did you see
> S.S.ANNE moored
> in the harbor?"

**已设置：**

> "So, S.S.ANNE has
> departed!
>
> She'll be back in
> about a year."

---

#### Sailor 1（码头入口守卫，条件对话）

**`EVENT_SS_ANNE_LEFT` 已设置（船已离港）：**

> "The ship set sail."

**玩家从侧面接近（facing RIGHT）：**

> "Welcome to
> S.S.ANNE!"

**玩家正面接近（持有 S.S.TICKET）：**

> "<PLAYER> flashed
> the S.S.TICKET!
>
> Great! Welcome to
> S.S.ANNE!"

**玩家正面接近（无 S.S.TICKET）：**

> "<PLAYER> doesn't
> have the needed
> S.S.TICKET.
>
> Sorry! You need
> a ticket to get
> aboard."

---

#### Gambler 2

> "I'm putting up a
> building on this
> plot of land.
>
> My #MON is tamping
> the land."

---

#### Machop（宠物精灵）

> "MACHOP: Guoh!
> Gogogoh!
>
> A MACHOP is
> stomping the land
> flat."

（播放 Machop 叫声）

---

#### Sailor 2

> "S.S.ANNE is a
> famous luxury
> cruise ship.
>
> We visit VERMILION
> once a year."

### 标识牌

| 标识 | 文本 |
|---|---|
| 城市路牌 | "VERMILION CITY / The Port of Exquisite Sunsets" |
| 公告 | "NOTICE! ROUTE 12 may be blocked off by a sleeping #MON. Detour through ROCK TUNNEL to LAVENDER TOWN. VERMILION POLICE" |
| 道馆 | "VERMILION CITY #MON GYM / LEADER: LT.SURGE / The Lightning American!" |
| 宝可梦爱好者俱乐部 | "#MON FAN CLUB / All #MON fans welcome!" |
| 港口 | "VERMILION HARBOR" |

---

## 四、Vermilion Gym（朱紫道馆）

### 地图脚本流程（状态机）

进入时加载城市名 `VERMILION CITY`、馆主名 `LT.SURGE`。

| 状态 (`wVermilionGymCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | `CheckFightingMapTrainers` |
| 1 `START_BATTLE` | `DisplayEnemyTrainerTextAndStartBattle` |
| 2 `END_BATTLE` | `EndTrainerBattle` |
| 3 `SURGE_POST_BATTLE` | 给予 TM24 THUNDERBOLT；设置 THUNDERBADGE |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_LT_SURGE` | 已击败 Lt. Surge |
| `EVENT_GOT_TM24` | 已领取 TM24 THUNDERBOLT |
| `EVENT_2ND_LOCK_OPENED` | 垃圾桶双重开关已解锁（打开通往 Surge 的门） |
| `EVENT_BEAT_VERMILION_GYM_TRAINER_0~2` | 已击败 3 名道馆训练师 |

### NPC 列表

#### Lt. Surge（馆主）

**战斗前：**

> "Hey, kid! What do
> you think you're
> doing here?
>
> You won't live
> long in combat!
> That's for sure!
>
> I tell you kid,
> electric #MON
> saved me during
> the war!
>
> They zapped my
> enemies into
> paralysis!
>
> The same as I'll
> do to you!"

**战败后（自动显示 THUNDERBADGE 文本）：**

> "Whoa!
>
> You're the real
> deal, kid!
>
> Fine then, take
> the THUNDERBADGE!"

接续：

> "The THUNDERBADGE
> cranks up your
> #MON's SPEED!
>
> It also lets your
> #MON FLY any
> time, kid!
>
> You're special,
> kid! Take this!"

- TM24 给予成功：`"\<PLAYER\> received TM24!"`（设置 `EVENT_GOT_TM24`）
- 背包已满：`"Yo kid, make room in your pack!"`

**战后再次对话（已给 TM24）：**

> "A little word of
> advice, kid!
>
> Electricity is
> sure powerful!
>
> But, it's useless
> against ground-
> type #MON!"

---

#### Gym Guide（道馆向导）

**未击败 Lt. Surge：**

> "Yo! Champ in
> making!
>
> LT.SURGE has a
> nickname. People
> refer to him as
> the Lightning
> American!
>
> He's an expert on
> electric #MON!
>
> Birds and water
> #MON are at
> risk! Beware of
> paralysis too!
>
> LT.SURGE is very
> cautious!
>
> You'll have to
> break a code to
> get to him!"

**已击败 Lt. Surge：**

> "Whew! That match
> was electric!"

---

#### Gentleman（道馆训练师 0）

- **挑战前：** "When I was in the Army, LT.SURGE was my strict CO!"
- **战败时：** "Stop! You're very good!"
- **战后：** "The door won't open? LT.SURGE always was cautious!"

#### Super Nerd（道馆训练师 1）

- **挑战前：** "I'm a lightweight, but I'm good with electricity!"
- **战败时：** "Fried!"
- **战后：** "OK, I'll talk! LT.SURGE said he hid door switches inside something!"

#### Sailor（道馆训练师 2）

- **挑战前：** "This is no place for kids!"
- **战败时：** "Wow! Surprised me!"
- **战后：** "LT.SURGE set up double locks! Here's a hint! When you open the 1st lock, the 2nd lock is right next to it!"

---

### 道馆垃圾桶谜题

道馆入口有两个垃圾桶（Trash Cans），需要找到隐藏开关才能打开通往 Lt. Surge 的门。

**第一个开关（随机位置）文本：**

> "There's a switch
> in here!"

**第二个开关（必须在第一个开关激活后立即找到，否则重置）文本：**

> "There's a switch
> in here!"

（找到两个开关后门打开；若找错顺序则重置）

**普通垃圾桶文本：**

> "Oops, wrong can!"

---

## 五、Vermilion Mart（朱紫市商店）

### NPC 列表

#### Clerk

标准商店购物流程。

#### Youngster

> "This place sells
> GREAT BALLs.
>
> They're better
> than regular
> # BALLs!"

#### Cooltrainer F

> "I came to buy
> REPEL!
>
> I hate getting
> into battles when
> I don't want to!"

---

## 六、Vermilion Pokecenter（朱紫市精灵中心）

### NPC 列表

#### Nurse

标准精灵中心治疗流程。

#### Gentleman

> "I heard a SAILOR
> from S.S.ANNE
> teaches CUT to
> #MON!"

#### Super Nerd

> "S.S.ANNE is a
> luxury cruise
> liner!
>
> It's full of
> trainers from all
> over the world!"

#### Link Receptionist

标准通信俱乐部接待流程。

---

## 七、Vermilion Dock（朱紫市码头）

### 地图脚本流程

进入时检查 `EVENT_GOT_HM01`：若已获得 HM01 且 `EVENT_SS_ANNE_LEFT` 未设置，则触发 SS Anne 离港动画序列（`VermilionDockSSAnneLeavesScript`）：

1. 设置 `EVENT_SS_ANNE_LEFT`
2. 停止所有音乐，播放冲浪音乐
3. 播放烟雾动画（船头冒烟 × 4）
4. 播放 SFX_SS_ANNE_HORN（船鸣）
5. 用水砖（tile `0x14`）替换船体区域
6. 减少 `wNumberOfWarps`（移除码头传送点）

**关键事件标志：**

| 事件标志 | 含义 |
|---|---|
| `EVENT_GOT_HM01` | 已从船长处获得 HM01，触发离港 |
| `EVENT_SS_ANNE_LEFT` | SS Anne 已离港 |
| `EVENT_STARTED_WALKING_OUT_OF_DOCK` | 玩家开始走出码头 |
| `EVENT_WALKED_OUT_OF_DOCK` | 玩家已走出码头 |

### NPC 列表

码头本身无 NPC 对话文本（仅有一个未使用的 `VermilionDockUnusedText`）。Sailor 守卫位于 **VermilionCity** 地图，不在 Dock 地图内。

---

## 八、Vermilion Old Rod House（旧钓竿小屋）

### NPC 列表

#### Fisher（Old Rod 赠送者）

**未领取（`EVENT_GOT_OLD_ROD` 未设置，Yes/No）：**

询问：`"Do you have time? I'll show you how to fish!"`

- YES：
  > "Here, I'll give
  > you my OLD ROD!
  >
  > It's a basic
  > fishing rod!
  >
  > Use it at the
  > water's edge!"

  给予 OLD ROD：`"\<PLAYER\> received an OLD ROD!"`（设置 `EVENT_GOT_OLD_ROD`）

- NO：`"Okay, come back when you want to learn!"`

**已领取：**

> "Any luck with
> the OLD ROD?"

---

## 九、Vermilion Pidgey House

### NPC 列表

#### Gentleman

> "I like #MON that
> can FLY!
>
> I have a PIDGEY
> that I raised from
> a baby!"

#### Pidgey（宠物精灵）

> "PIDGEY: Coo!"

（播放 Pidgey 叫声）

---

## 十、Vermilion Trade House（交换小屋）

### NPC 列表

#### Little Girl（游戏内交换 NPC）

交换参数：`TRADE_FOR_DUX`

| 参数 | 内容 |
|---|---|
| 给出宝可梦 | SPEAROW |
| 获得宝可梦 | FARFETCH'D（昵称：DUX） |
| 对话套装 | `TRADE_DIALOGSET_HAPPY` |

**对话套装 HAPPY 文本：**

- 询问：`"Hi! Do you have SPEAROW? Want to trade it for FARFETCH'D?"`
- 拒绝：`"That's too bad."`
- 错误宝可梦：`"...This is no SPEAROW. If you get one, trade it with me!"`
- 感谢：`"Thanks pal!"`
- 交换后再对话：`"How is my old FARFETCH'D? My SPEAROW is doing great!"`

---

## 十一、SS Anne 系列

### SS Anne 1F（一等舱）

#### 地图脚本流程

标准三状态训练师战斗状态机，含多名训练师。SS Anne 离港后设置 `EVENT_SS_ANNE_LEFT`。

#### NPC 列表

##### Gentleman 1

> "This luxury liner
> is holding a party
> for trainers!
>
> There are many
> strong trainers
> aboard!"

##### Gentleman 2

> "I came from the
> KANTO region to
> show off my
> #MON!"

---

### SS Anne 1F Rooms（一等舱客房）

各房间内有多名训练师和 NPC，均使用标准训练师战斗状态机。

#### 代表性 NPC 对话

##### Lass（女孩）

> "I'm on a world
> cruise!
>
> I've seen #MON
> from all over!"

##### Gentleman（客房）

> "I collect rare
> #MON from all
> over the world!"

---

### SS Anne 2F（二等舱）

#### 代表性 NPC 对话

##### Sailor

> "This ship goes
> all over the
> world!
>
> We visit all
> kinds of ports!"

---

### SS Anne B1F（底舱）

#### 代表性 NPC 对话

##### Sailor

> "Quit roaming
> around!
>
> You'll get in the
> way of our work!"

---

### SS Anne Captain's Room（船长室）

### 地图脚本流程（状态机）

| 状态 (`wSSAnneCaptainsRoomCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 等待玩家与船长对话 |
| 1 `CAPTAIN_GIVES_HM` | 船长给予 HM01 CUT |
| 2 `NOOP` | 终态 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_GOT_HM01` | 已从船长处获得 HM01（CUT） |
| `EVENT_RUBBED_CAPTAINS_BACK` | 已为船长按摩背部 |

### NPC 列表

#### Captain（船长）

**未给予 HM01（`EVENT_GOT_HM01` 未设置）：**

与船长互动时，自动触发按摩动作（无需确认）：

> "CAPTAIN: Ooargh...
> I feel hideous...
> Urrp! Seasick...
>
> \<PLAYER\> rubbed
> the CAPTAIN's back!
>
> Rub-rub...
> Rub-rub...@"

（设置 `EVENT_RUBBED_CAPTAINS_BACK`，播放治愈音乐）

接续：

> "CAPTAIN: Whew!
> Thank you! I feel
> much better!
>
> You want to see
> my CUT technique?
>
> I could show you
> if I wasn't ill...
>
> I know! You can
> have this!
>
> Teach it to your
> #MON and you can
> see it CUT any
> time!"

- 给予 HM01 成功：`"\<PLAYER\> got @[物品名]!"`（设置 `EVENT_GOT_HM01`）
- 背包已满：`"Oh no! You have no room for this!"`

**已给予 HM01（`EVENT_GOT_HM01` 已设置）：**

> "CAPTAIN: Whew!
>
> Now that I'm not
> sick any more,
> I guess it's time."

---

#### Trash（可检查物件）

> "Yuck! Shouldn't
> have looked!"

#### Seasick Book（可检查物件）

> "How to Conquer
> Seasickness...
>
> The CAPTAIN's
> reading this!"

---

### SS Anne Kitchen（厨房）

#### NPC 列表

##### Cook

> "I'm making food
> for the party!
>
> This is a special
> POKÉMON cuisine
> that raises their
> stats!"

---

### SS Anne Bow（船头）

#### 地图脚本流程

标准三状态训练师战斗状态机，含 2 名 Sailor 训练师。

**SS Anne 离港动画在 VermilionDock 触发**（见第七节），不在 Bow 触发。

#### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_SS_ANNE_5_TRAINER_0` | 已击败 Sailor 2 |
| `EVENT_BEAT_SS_ANNE_5_TRAINER_1` | 已击败 Sailor 3 |

#### NPC 列表

##### Super Nerd（非战斗 NPC）

> "The party's over.
> The ship will be
> departing soon."

##### Sailor 1（非战斗 NPC）

> "Scrubbing decks
> is hard work!"

##### CooltrainerM（非战斗 NPC）

> "Urf. I feel ill.
> I stepped out to
> get some air."

##### Sailor 2（训练师 0）

- **挑战前：** "Hey matey! Let's do a little jig!"
- **战败时：** "You're impressive!"
- **战后：** "How many kinds of #MON do you think there are?"

##### Sailor 3（训练师 1）

- **挑战前：** "Ahoy there! Are you seasick?"
- **战败时：** "I was just careless!"
- **战后：** "My Pa said there are 100 kinds of #MON. I think there are more."

---

## 十二、Diglett's Cave（地鼠洞）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无 NPC、无对话。

---

## 十三、Diglett's Cave Route 11 入口

### NPC 列表

#### Gambler

> "What a surprise!
> DIGLETTs dug this
> long tunnel!
>
> It goes right to
> VIRIDIAN CITY!"

---

## 十四、Diglett's Cave Route 2 入口

### NPC 列表

#### Fishing Guru

> "I went to ROCK
> TUNNEL, but it's
> dark and scary.
>
> If a #MON's
> FLASH could light
> it up..."

---

## 附录：游戏内交换对话套装

| 套装编号 | 常量名 | 风格 |
|---|---|---|
| 0 | `TRADE_DIALOGSET_CASUAL` | 随意风格 |
| 1 | `TRADE_DIALOGSET_EVOLUTION` | 进化型（交换后宝可梦会进化） |
| 2 | `TRADE_DIALOGSET_HAPPY` | 开心风格 |

每套含五条文本：询问、拒绝、错误宝可梦、感谢、交换后再对话。

---

*下一章：Route 11-15 → Lavender Town → Pokemon Tower*
