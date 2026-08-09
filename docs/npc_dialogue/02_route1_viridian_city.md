# Route 1 & Viridian City 区域 NPC 对话与剧情脚本

> 来源：pokered 反汇编代码
> 覆盖地图：Route1、ViridianCity、ViridianMart、ViridianPokecenter、ViridianNicknameHouse、ViridianSchoolHouse
> 用途：Rust 重制版剧情参考

---

## 一、Route 1（1 号道路）

### 地图脚本流程

仅执行 `EnableAutoTextBoxDrawing`，无状态机。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_GOT_POTION_SAMPLE` | 已从促销员处领取免费 Potion 样品 |

### NPC 列表

#### Youngster 1（宝可梦商城促销员）

触发逻辑：`CheckAndSetEvent EVENT_GOT_POTION_SAMPLE`

**首次对话（未领取）：**

> "Hi! I work at a
> #MON MART.
>
> It's a convenient
> shop, so please
> visit us in
> VIRIDIAN CITY.
>
> I know, I'll give
> you a sample!
> Here you go!"

- 领取成功：`<PLAYER> got [物品名称]!`（道具音效）
- 背包已满：`"You have too much stuff with you!"`

**已领取后：**

> "We also carry
> # BALLs for
> catching #MON!"

---

#### Youngster 2

> "See those ledges
> along the road?
>
> It's a bit scary,
> but you can jump
> from them.
>
> You can get back
> to PALLET TOWN
> quicker that way."

---

#### 标识牌：Route 1

> "ROUTE 1
> PALLET TOWN -
> VIRIDIAN CITY"

---

## 二、Viridian City（绿野市）

### 地图脚本流程（状态机）

`wViridianCityCurScript` 控制四个子脚本：

| 状态 | 常量名 | 说明 |
|---|---|---|
| 0 | `SCRIPT_VIRIDIANCITY_DEFAULT` | 检查道馆是否开放；检查老爷爷是否挡路 |
| 1 | `SCRIPT_VIRIDIANCITY_OLD_MAN_START_CATCH_TRAINING` | 配置捕捉教学战（Weedle Lv.5，`BATTLE_TYPE_OLD_MAN`） |
| 2 | `SCRIPT_VIRIDIANCITY_OLD_MAN_END_CATCH_TRAINING` | 恢复老爷爷精灵数据，显示说明文本，回到状态0 |
| 3 | `SCRIPT_VIRIDIANCITY_PLAYER_MOVING_DOWN` | 等待模拟输入结束，回到状态0 |

**道馆开放条件：** 除大地徽章外集齐其余7枚徽章（`wObtainedBadges == ~(1 << BIT_EARTHBADGE)`）。

**老爷爷挡路条件：** `EVENT_GOT_POKEDEX` 未设置 && 玩家在坐标 Y=9, X=19。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_VIRIDIAN_GYM_OPEN` | 绿野市道馆已解锁 |
| `EVENT_GOT_POKEDEX` | 已获得图鉴 |
| `EVENT_BEAT_VIRIDIAN_GYM_GIOVANNI` | 已击败馆主 Giovanni |
| `EVENT_GOT_TM42` | 已领取 TM42（DREAM EATER） |

### NPC 列表

#### Youngster 1

> "Those # BALLs
> at your waist!
> You have #MON!
>
> It's great that
> you can carry and
> use #MON any
> time, anywhere!"

---

#### Gambler 1

**除大地徽章外集齐其余徽章，或已击败 Giovanni：**

> "VIRIDIAN GYM's
> LEADER returned!"

**其他情况：**

> "This #MON GYM
> is always closed.
>
> I wonder who the
> LEADER is?"

---

#### Youngster 2（Yes/No 分支）

询问：

> "You want to know
> about the 2 kinds
> of caterpillar
> #MON?"

- YES：
  > "CATERPIE has no
  > poison, but
  > WEEDLE does.
  >
  > Watch out for its
  > POISON STING!"

- NO：
  > "Oh, OK then!"

---

#### Girl

**`EVENT_GOT_POKEDEX` 未设置：**

> "Oh Grandpa! Don't
> be so mean!
> He hasn't had his
> coffee yet."

**已设置：**

> "When I go shop in
> PEWTER CITY, I
> have to take the
> winding trail in
> VIRIDIAN FOREST."

---

#### Old Man（挡路，地图脚本自动触发）

> "You can't go
> through here!
>
> This is private
> property!"

---

#### Old Man（有精神，捕捉教学，Yes/No）

询问：

> "Ahh, I've had my
> coffee now and I
> feel great!
>
> Sure you can go
> through!
>
> Are you in a
> hurry?"

- NO（接受教学，不赶时间）：
  > "I see you're using
  > a #DEX.
  >
  > When you catch a
  > #MON, #DEX
  > is automatically
  > updated.
  >
  > What? Don't you
  > know how to catch
  > #MON?
  >
  > I'll show you
  > how to then."

  教学战结束后：
  > "First, you need
  > to weaken the
  > target #MON."

- YES（拒绝教学，赶时间）：
  > "Time is money...
  > Go along then."

---

#### Fisher（TM42 赠送者）

**未领取（`EVENT_GOT_TM42` 未设置）：**

> "Yawn!
> I must have dozed
> off in the sun.
>
> I had this dream
> about a DROWZEE
> eating my dream.
> What's this?
> Where did this TM
> come from?
>
> This is spooky!
> Here, you can
> have this TM."

- 领取成功：`"<PLAYER> received TM42!"`（道具音效）
- 背包已满：`"You have too much stuff already."`

**已领取：**

> "TM42 contains
> DREAM EATER...
> ...Snore..."

---

### 标识牌

| 标识 | 文本 |
|---|---|
| 城市路牌 | "VIRIDIAN CITY / The Eternally Green Paradise" |
| Trainer Tips 1 | "TRAINER TIPS / Catch #MON and expand your collection! / The more you have, the easier it is to fight!" |
| Trainer Tips 2 | "TRAINER TIPS / The battle moves of #MON are limited by their POWER POINTs, PP. / To replenish PP, rest your tired #MON at a #MON CENTER!" |
| 道馆标识 | "VIRIDIAN CITY #MON GYM" |
| 道馆门锁（地图脚本触发） | "The GYM's doors are locked..." |

---

## 三、Viridian Mart（绿野市商店）

### 地图脚本流程（状态机）

根据 `EVENT_OAK_GOT_PARCEL` 切换文本表：
- 未设置 → 快递剧情版文本（`ViridianMart_TextPointers`）
- 已设置 → 普通商店版文本（`ViridianMart_TextPointers2`）

`wViridianMartCurScript` 状态机：

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 显示"你从真红镇来？"，模拟玩家向左走1格+向上走2格，进入状态1 |
| 1 `OAKS_PARCEL` | 等待移动结束，显示快递台词，给予 Oak's Parcel，进入状态2 |
| 2 `NOOP` | 终态 |

> 注：仅在 `EVENT_OAK_GOT_PARCEL` 未设置时触发快递剧情。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_OAK_GOT_PARCEL` | 大木博士已收到快递 |
| `EVENT_GOT_OAKS_PARCEL` | 玩家已在商店领取 Oak's Parcel |

### NPC 列表

#### 店员（快递剧情版，自动触发）

**进入商店时（DEFAULT 脚本）：**

> "Hey! You came from
> PALLET TOWN?"

**快递台词（OAKS_PARCEL 脚本）：**

> "You know PROF.
> OAK, right?
>
> His order came in.
> Will you take it
> to him?
>
> \<PLAYER\> got
> OAK's PARCEL!"

**普通对话（快递已送出前）：**

> "Okay! Say hi to
> PROF.OAK for me!"

#### 店员（普通商店版）

进入标准商店购物菜单（`TX_SCRIPT_MART`）。

#### Youngster（顾客）

> "This shop sells
> many ANTIDOTEs."

#### Cooltrainer M（顾客）

> "No! POTIONs are
> all sold out."

---

## 四、Viridian Pokecenter（绿野市精灵中心）

### 地图脚本流程

调用 `Serial_TryEstablishingExternallyClockedConnection` + `EnableAutoTextBoxDrawing`，无状态机。

### NPC 列表

#### Nurse（护士）

标准精灵中心治疗流程（`script_pokecenter_nurse`）。

#### Gentleman

> "You can use that
> PC in the corner.
>
> The receptionist
> told me. So kind!"

#### Cooltrainer M

> "There's a #MON
> CENTER in every
> town ahead.
>
> They don't charge
> any money either!"

#### Link Receptionist（通信接待员）

标准通信俱乐部接待流程（`script_cable_club_receptionist`）。

---

## 五、Viridian Nickname House（绿野市起名屋）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无状态机。

### NPC 列表

#### Balding Guy

> "Coming up with
> nicknames is fun,
> but hard.
>
> Simple names are
> the easiest to
> remember."

#### Little Girl

> "My Daddy loves
> #MON too."

#### Spearow（宠物精灵）

> "SPEARY: Tetweet!"

（对话后播放 Spearow 叫声）

#### 标识牌

> "SPEAROW
> Name: SPEARY"

---

## 六、Viridian School House（绿野市学校）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无状态机。黑板为 Hidden Event，由 `engine/events/hidden_events/school_blackboard.asm` 处理。

### 黑板交互流程

双列菜单式交互（SLP / PSN / PAR / BRN / FRZ / QUIT），玩家可反复查询，按 B 或选 QUIT 退出。

### NPC 列表

#### Brunette Girl

> "Whew! I'm trying
> to memorize all
> my notes."

#### Cooltrainer F

> "Okay!
>
> Be sure to read
> the blackboard
> carefully!"

### 黑板文本

#### 介绍

> "The blackboard
> describes #MON
> STATUS changes
> during battles."

#### SLP — 睡眠

> "A #MON can't
> attack if it's
> asleep!
>
> #MON will stay
> asleep even after
> battles.
>
> Use AWAKENING to
> wake them up!"

#### PSN — 毒

> "When poisoned, a
> #MON's health
> steadily drops.
>
> Poison lingers
> after battles.
>
> Use an ANTIDOTE
> to cure poison!"

#### PAR — 麻痹

> "Paralysis could
> make #MON
> moves misfire!
>
> Paralysis remains
> after battles.
>
> Use PARLYZ HEAL
> for treatment!"

#### BRN — 灼伤

> "A burn reduces
> power and speed.
> It also causes
> ongoing damage.
>
> Burns remain
> after battles.
>
> Use BURN HEAL to
> cure a burn!"

#### FRZ — 冰冻

> "If frozen, a
> #MON becomes
> totally immobile!
>
> It stays frozen
> even after the
> battle ends.
>
> Use ICE HEAL to
> thaw out #MON!"

---

*下一章：Viridian Forest → Route 2 → Pewter City*
