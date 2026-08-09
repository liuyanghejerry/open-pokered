# Celadon City 区域 NPC 对话与剧情脚本

> 来源：pokered 反汇编代码
> 覆盖地图：CeladonCity、CeladonGym、CeladonPokecenter、CeladonHotel、CeladonDiner、CeladonChiefHouse、CeladonMansion1F/2F/3F/Roof/RoofHouse、GameCorner、GameCornerPrizeRoom、PokemonFanClub、CeladonMart1F
> 用途：Rust 重制版剧情参考

---

## 一、Celadon City（霞关市）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，并重置三个事件标志：

| 事件标志 | 操作 |
|---|---|
| `EVENT_1B8` | 重置 |
| `EVENT_1BF` | 重置 |
| `EVENT_67F` | 重置 |

### NPC 列表

#### Little Girl

> "I got my KOFFING
> in CINNABAR!
>
> It's nice, but it
> breathes poison
> when it's angry!"

#### Gramps 1

> "Heheh! This GYM
> is great! It's
> full of women!"

#### Girl

> "The GAME CORNER
> is bad for our
> city's image!"

#### Gramps 2

> "Moan! I blew it
> all at the slots!
>
> I knew I should
> have cashed in my
> coins for prizes!"

#### Gramps 3（给予 TM41）

**`EVENT_GOT_TM41` 未设置：**

> "Hello, there!
>
> I've seen you,
> but I never had a
> chance to talk!
>
> Here's a gift for
> dropping by!"

- 给予成功：`"<PLAYER> received TM41!"`（设置 `EVENT_GOT_TM41`）
- 背包已满：`"Oh, your pack is full of items!"`

**已领取：**

> "TM41 teaches
> SOFTBOILED!
>
> Only one #MON
> can use it!
>
> That #MON is
> CHANSEY!"

#### Fisher

> "This is my trusted
> pal, POLIWRATH!
>
> It evolved from
> POLIWHIRL when I
> used WATER STONE!"

（触摸 Poliwrath 时播放叫声）

#### Rocket 1

> "What are you
> staring at?"

#### Rocket 2

> "Keep out of TEAM
> ROCKET's way!"

### 标识牌

| 标识 | 文本 |
|---|---|
| 城市路牌 | "CELADON CITY / The City of Rainbow Dreams" |
| 道馆 | "CELADON CITY #MON GYM / LEADER: ERIKA / The Nature Loving Princess!" |
| 豪宅 | "CELADON MANSION" |
| 百货店 | "Find what you need at CELADON DEPT. STORE!" |
| 游戏厅奖品 | "Coins exchanged for prizes! / PRIZE EXCHANGE" |
| 游戏厅 | "ROCKET GAME CORNER / The playground for grown-ups!" |
| Trainer Tips 1 | "X ACCURACY boosts the accuracy of techniques! DIRE HIT jacks up the likelihood of critical hits! Get your items at CELADON DEPT. STORE!" |
| Trainer Tips 2 | "GUARD SPEC. protects #MON against SPECIAL attacks such as fire and water! Get your items at CELADON DEPT. STORE!" |

---

## 二、Celadon Gym（霞关道馆）

### 地图脚本流程（状态机）

进入时加载城市名 `CELADON CITY`、馆主名 `ERIKA`。

| 状态 (`wCeladonGymCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | `CheckFightingMapTrainers` |
| 1 `START_BATTLE` | `DisplayEnemyTrainerTextAndStartBattle` |
| 2 `END_BATTLE` | `EndTrainerBattle` |
| 3 `ERIKA_POST_BATTLE` | 给予 TM21 MEGA DRAIN；设置 RAINBOWBADGE |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_ERIKA` | 已击败馆主 Erika |
| `EVENT_GOT_TM21` | 已领取 TM21 MEGA DRAIN |
| `EVENT_BEAT_CELADON_GYM_TRAINER_0~6` | 已击败各道馆训练师 |

### NPC 列表

#### Erika（馆主）

**战斗前：**

> "Hello. Lovely
> weather isn't it?
> It's so pleasant.
>
> ...Oh dear...
> I must have dozed
> off. Welcome.
>
> My name is ERIKA.
> I am the LEADER
> of CELADON GYM.
>
> I teach the art of
> flower arranging.
> My #MON are of
> the grass-type.
>
> Oh, I'm sorry, I
> had no idea that
> you wished to
> challenge me.
>
> Very well, but I
> shall not lose."

**战败后（自动显示 RAINBOWBADGE 文本）：**

> "Oh!
> I concede defeat.
>
> You are remarkably
> strong.
>
> I must confer you
> the RAINBOWBADGE."

接续：

> "The RAINBOWBADGE
> will make #MON
> up to L50 obey.
>
> It also allows
> #MON to use
> STRENGTH in and
> out of battle.
>
> Please also take
> this with you."

- TM21 给予成功：`"<PLAYER> received TM21!"`
  接续：
  > "TM21 contains
  > MEGA DRAIN.
  >
  > Half the damage
  > it inflicts is
  > drained to heal
  > your #MON!"
- 背包已满：`"You should make room for this."`

**战后再次对话（已给 TM21）：**

> "You are cataloging
> #MON? I must
> say I'm impressed.
>
> I would never
> collect #MON
> if they were
> unattractive."

---

#### 道馆训练师（7 名女性）

| # | 挑战前 | 战败时 | 战后 |
|---|---|---|---|
| 1 | "Hey! You are not allowed in here!" | "You're too rough!" | "Bleaah! I hope ERIKA wipes you out!" |
| 2 | "I was getting bored." | "My makeup!" | "Grass-type #MON are tough against the water-type! They also have an edge on rock and ground #MON!" |
| 3 | "Aren't you the peeping Tom?" | "I'm in shock!" | "Oh, you weren't peeping? We get a lot of gawkers!" |
| 4 | "Look at my grass #MON! They're so easy to raise!" | "No!" | "We only use grass-type #MON at our GYM! We also use them for making flower arrangements!" |
| 5 | "Don't bring any bugs or fire #MON in here!" | "Oh! You!" | "Our LEADER, ERIKA, might be quiet, but she's also very skilled!" |
| 6 | "Pleased to meet you. My hobby is #MON training." | "Oh! Splendid!" | "I have a blind date coming up. I have to learn to be polite." |
| 7 | "Welcome to CELADON GYM! You better not underestimate girl power!" | "Oh! Beaten!" | "I didn't bring my best #MON! Wait 'til next time!" |

---

## 三、Celadon Pokecenter（霞关精灵中心）

### NPC 列表

#### Nurse

标准精灵中心治疗流程。

#### Gentleman

> "# FLUTE awakens
> #MON with a
> sound that only
> they can hear!"

#### Beauty

> "I rode uphill on
> CYCLING ROAD from
> FUCHSIA!"

#### Link Receptionist

标准通信俱乐部接待流程。

---

## 四、Celadon Hotel（霞关酒店）

### NPC 列表

#### Granny

> "#MON? No, this
> is a hotel for
> people.
>
> We're full up."

#### Beauty

> "I'm on vacation
> with my brother
> and boy friend.
>
> CELADON is such a
> pretty city!"

#### Super Nerd

> "Why did she bring
> her brother?"

---

## 五、Celadon Diner（霞关餐厅）

### NPC 列表

#### Cook

> "Hi!
>
> We're taking a
> break now."

#### Middle-Aged Woman

> "My #MON are
> weak, so I often
> have to go to the
> DRUG STORE."

#### Middle-Aged Man

> "Psst! There's a
> basement under
> the GAME CORNER."

#### Fisher

> "Munch...
>
> The man at that
> table lost it all
> at the slots."

#### Gym Guide（给予 COIN CASE）

**`EVENT_GOT_COIN_CASE` 未设置：**

> "Go ahead! Laugh!
>
> I'm flat out
> busted!
>
> No more slots for
> me! I'm going
> straight!
>
> Here! I won't be
> needing this any-
> more!"

- 给予成功：`"<PLAYER> received a COIN CASE!"`（设置 `EVENT_GOT_COIN_CASE`）
- 背包已满：`"Make room for this!"`

**已领取：**

> "I always thought
> I was going to
> win it back..."

---

## 六、Celadon Chief House（霞关游戏厅老板家）

### NPC 列表

#### Chief（老板）

> "Hehehe! The slots
> just reel in the
> dough, big time!"

#### Rocket

> "CHIEF!
>
> We just shipped
> 2000 #MON as
> slot prizes!"

#### Sailor

> "Don't touch the
> poster at the
> GAME CORNER!
>
> There's no secret
> switch behind it!"

（实际上海报背后确有开关——故意误导）

---

## 七、Celadon Mansion 1F（霞关豪宅一楼）

### NPC 列表

#### Granny

> "My dear #MON
> keep me company.
>
> MEOWTH even brings
> money home!"

#### Meowth（宠物精灵）

> "MEOWTH: Meow!"

（播放 Meowth 叫声）

#### Clefairy（宠物精灵）

> "CLEFAIRY: Pi
> pippippi!"

（播放 Clefairy 叫声）

#### Nidoran♀（宠物精灵）

> "NIDORAN: Kya
> kyaoo!"

（播放 Nidoran♀ 叫声）

---

## 八、Celadon Mansion 2F（霞关豪宅二楼）

无 NPC，仅门牌：`"GAME FREAK / Meeting Room"`

---

## 九、Celadon Mansion 3F（霞关豪宅三楼 — Game Freak 开发室）

### NPC 列表

#### Programmer

> "Me? I'm the
> programmer!"

#### Graphic Artist

> "I'm the graphic
> artist!
> I drew you!"

#### Writer

> "I wrote the story!
> Isn't ERIKA cute?
>
> I like MISTY a
> lot too!
>
> Oh, and SABRINA,
> I like her!"

#### Game Designer（图鉴评分 NPC）

**图鉴不足 150 只：**

> "Is that right?
>
> I'm the game
> designer!
>
> Filling up your
> #DEX is tough,
> but don't quit!
>
> When you finish,
> come tell me!"

**图鉴满 150 只：**

> "Wow! Excellent!
> You completed
> your #DEX!
> Congratulations!"

（触发显示文凭 Diploma）

#### 电脑（可检查物体）

| 电脑 | 文本 |
|---|---|
| 游戏程序电脑 | "It's the game program! Messing with it could bug out the game!" |
| 游戏运行电脑 | "Someone's playing a game instead of working!" |
| 游戏脚本电脑 | "It's the script! Better not look at the ending!" |

### 标识牌

> "GAME FREAK / Development Room"

---

## 十、Celadon Mansion Roof（霞关豪宅屋顶）

### 标识牌

> "I KNOW EVERYTHING!"

---

## 十一、Celadon Mansion Roof House（霞关豪宅屋顶小屋 — Eevee 赠送处）

### NPC 列表

#### Hiker

> "I know everything
> about the world
> of #MON in
> your GAME BOY!
>
> Get together with
> your friends and
> trade #MON!"

#### Eevee 精灵球（可拾取物件）

- 首次触摸：给予 Eevee Lv.25（设置 `TOGGLE_CELADON_MANSION_EEVEE_GIFT`，隐藏精灵球）
- 已领取后精灵球消失

---

## 十二、Game Corner（火箭游戏厅）

### 地图脚本流程（状态机）

| 状态 (`wGameCornerCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 初始化幸运老虎机；设置海报门砖块（若 `EVENT_FOUND_ROCKET_HIDEOUT` 未设置） |
| 1 `ROCKET_BATTLE` | 与火箭队成员战斗 |
| 2 `ROCKET_EXIT` | 战斗后火箭队成员离开 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_FOUND_ROCKET_HIDEOUT` | 已发现火箭队隐藏巢穴（推开海报开关） |
| `EVENT_GOT_COIN_CASE` | 已获得硬币盒 |
| `EVENT_GOT_10_COINS` | 已从钓鱼大师获得 10 枚硬币 |
| `EVENT_GOT_20_COINS` | 已从绅士获得 20 枚硬币 |
| `EVENT_GOT_20_COINS_2` | 已从柜台员 2 获得 20 枚硬币 |

### NPC 列表

#### Beauty 1

> "Welcome!
>
> You can exchange
> your coins for
> fabulous prizes
> next door."

#### Clerk 1（硬币兑换，¥1000 → 50 枚）

> "Welcome to ROCKET
> GAME CORNER!
>
> Do you need some
> game coins?
>
> It's ¥1000 for 50
> coins. Would you
> like some?"

- 成功：`"Thanks! Here are your 50 coins!"`
- 拒绝：`"No? Please come play sometime!"`
- 没有硬币盒：`"You don't have a COIN CASE!"`
- 硬币盒已满：`"Oops! Your COIN CASE is full."`
- 金钱不足：`"You can't afford the coins!"`

#### Middle-Aged Man 1

> "Keep this quiet.
>
> It's rumored that
> this place is run
> by TEAM ROCKET."

#### Beauty 2

> "I think these
> machines have
> different odds."

#### Fishing Guru（给予 10 枚硬币）

**`EVENT_GOT_10_COINS` 未设置：**

> "Kid, do you want
> to play?"

- 成功：`"<PLAYER> received 10 coins!"`（设置 `EVENT_GOT_10_COINS`）
- 没有硬币盒：`"Oops! Forgot the COIN CASE!"`
- 硬币盒已满：`"You don't need my coins!"`

**已领取：**

> "Wins seem to come
> and go."

#### Middle-Aged Woman

> "I'm having a
> wonderful time!"

#### Gym Guide（条件对话）

**`EVENT_BEAT_ERIKA` 未设置：**

> "Hey!
>
> You have better
> things to do,
> champ in making!
>
> CELADON GYM's
> LEADER is ERIKA!
> She uses grass-
> type #MON!
>
> She might appear
> docile, but don't
> be fooled!"

**已设置：**

> "They offer rare
> #MON that can
> be exchanged for
> your coins.
>
> But, I just can't
> seem to win!"

#### Gambler

> "Games are scary!
> It's so easy to
> get hooked!"

#### Clerk 2（给予 20 枚硬币）

**`EVENT_GOT_20_COINS_2` 未设置：**

> "What's up? Want
> some coins?"

- 成功：`"<PLAYER> received 20 coins!"`（设置 `EVENT_GOT_20_COINS_2`）
- 硬币过多：`"You have lots of coins!"`

**已领取：**

> "Darn! I need more
> coins for the
> #MON I want!"

#### Gentleman（给予 20 枚硬币）

**`EVENT_GOT_20_COINS` 未设置：**

> "Hey, what? You're
> throwing me off!
> Here are some
> coins, shoo!"

- 成功：`"<PLAYER> received 20 coins!"`（设置 `EVENT_GOT_20_COINS`）
- 硬币过多：`"You've got your own coins!"`

**已领取：**

> "The trick is to
> watch the reels
> closely!"

#### Rocket（海报守卫，坐标触发）

**战斗前（守护海报）：**

> "I'm guarding this
> poster!
> Go away, or else!"

**战败时：**

> "Dang!"

**战斗后离开：**

> "Our hideout might
> be discovered! I
> better tell BOSS!"

（NPC 移动离开，状态机转至 ROCKET_EXIT）

#### 海报（可检查物件）

**`EVENT_FOUND_ROCKET_HIDEOUT` 未设置（击败火箭队守卫后）：**

> "Hey!
>
> A switch behind
> the poster!?
> Let's push it!"

（播放开关音效 → 打开通往火箭队隐藏巢穴的入口 → 设置 `EVENT_FOUND_ROCKET_HIDEOUT`）

---

## 十三、Game Corner Prize Room（游戏厅奖品交换室）

### NPC 列表

#### Balding Guy

> "I sure do fancy
> that PORYGON!
>
> But, it's hard to
> win at slots!"

#### Gambler

> "I had a major
> haul today!"

#### 奖品贩售员（×3）

标准奖品兑换流程（硬币换取奖品宝可梦/TM）。

---

## 十四、Pokemon Fan Club（宝可梦爱好者俱乐部）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无状态机。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_GOT_BIKE_VOUCHER` | 已从主席处获得自行车凭证 |
| `EVENT_PIKACHU_FAN_BOAST` | Pikachu 爱好者正在吹牛 |
| `EVENT_SEEL_FAN_BOAST` | Seel 爱好者正在吹牛 |

### NPC 列表

#### Chairman（主席，关键 NPC）

**未获得自行车相关物品（首次对话）：**

> "I chair the
> #MON Fan Club!
>
> I have collected
> over 100 #MON!
>
> I'm very fussy
> when it comes to
> #MON!
>
> So...
>
> Did you come
> visit to hear
> about my #MON?"

- YES → 听完关于 RAPIDASH 的长篇故事：
  > "Good!
  > Then listen up!
  >
  > My favorite
  > RAPIDASH...
  >
  > It...cute...
  > lovely...smart...
  > plus...amazing...
  > you think so?...
  > oh yes...it...
  > stunning...
  > kindly...
  > love it!
  >
  > Hug it...when...
  > sleeping...warm
  > and cuddly...
  > spectacular...
  > ravishing...
  > ...Oops! Look at
  > the time! I kept
  > you too long!
  >
  > Thanks for hearing
  > me out! I want
  > you to have this!"

  - 给予成功：`"<PLAYER> received a BIKE_VOUCHER!"`（设置 `EVENT_GOT_BIKE_VOUCHER`）
    接续：
    > "Exchange that for
    > a BICYCLE!
    >
    > Don't worry, my
    > FEAROW will FLY
    > me anywhere!
    >
    > So, I don't need a
    > BICYCLE!
    >
    > I hope you like
    > cycling!"
  - 背包已满：`"Make room for this!"`

- NO → `"Oh. Come back when you want to hear my story!"`

**已获得自行车相关物品：**

> "Hello, <PLAYER>!
>
> Did you come see
> me about my
> #MON again?
>
> No? Too bad!"

---

#### Receptionist

> "Our Chairman is
> very vocal about
> #MON."

---

#### Pikachu Fan（与 Seel Fan 互竞）

**非竞争状态：**

> "Won't you admire
> my PIKACHU's
> adorable tail?"

（设置 `EVENT_SEEL_FAN_BOAST`）

**竞争状态（`EVENT_PIKACHU_FAN_BOAST` 已设置）：**

> "Humph! My PIKACHU
> is twice as cute
> as that one!"

（重置 `EVENT_PIKACHU_FAN_BOAST`）

#### Pikachu（宠物精灵）

> "PIKACHU: Chu!
> Pikachu!"

（播放 Pikachu 叫声）

---

#### Seel Fan（与 Pikachu Fan 互竞）

**非竞争状态：**

> "I just love my
> SEEL!
>
> It squeals when I
> hug it!"

（设置 `EVENT_PIKACHU_FAN_BOAST`）

**竞争状态（`EVENT_SEEL_FAN_BOAST` 已设置）：**

> "Oh dear!
>
> My SEEL is far
> more attractive!"

（重置 `EVENT_SEEL_FAN_BOAST`）

#### Seel（宠物精灵）

> "SEEL: Kyuoo!"

（播放 Seel 叫声）

### 标识牌

- `"Let's all listen politely to other trainers!"`
- `"If someone brags, brag right back!"`

---

## 十五、Celadon Mart 1F（霞关百货店一楼 — 服务台）

### NPC 列表

#### Receptionist

> "Hello! Welcome to
> CELADON DEPT.
> STORE.
>
> The board on the
> right describes
> the store layout."

### 标识牌（店铺目录）

> "1F: SERVICE
> COUNTER
>
> 2F: TRAINER'S
> MARKET
>
> 3F: TV GAME SHOP
>
> 4F: WISEMAN GIFTS
>
> 5F: DRUG STORE
>
> ROOFTOP SQUARE:
> VENDING MACHINES"

---

*下一章：Rocket Hideout → Saffron City → Silph Co.*
