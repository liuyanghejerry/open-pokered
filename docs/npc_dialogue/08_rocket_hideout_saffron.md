# Rocket Hideout / Saffron City / Silph Co. 区域 NPC 对话与剧情脚本

> 来源：pokered 反汇编代码
> 覆盖地图：RocketHideoutB1F-B4F/Elevator、SaffronCity、SaffronGym、SaffronMart、SaffronPokecenter、SaffronPidgeyHouse、FightingDojo、CopycatsHouse1F/2F、SilphCo1F-11F/Elevator
> 用途：Rust 重制版剧情参考

---

## 一、Rocket Hideout B1F（火箭队地下基地一层）

### 地图脚本流程（状态机）

标准三状态训练师战斗状态机，含特殊门机制。

**门机制：** 进入时检查 `EVENT_ENTERED_ROCKET_HIDEOUT`：
- 首次进入 → 检查 `EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_4`，已击败则门打开（播放 SFX_GO_INSIDE）
- 已进入过 → 门始终打开

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_ENTERED_ROCKET_HIDEOUT` | 已首次进入 Rocket Hideout |
| `EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_0~4` | 已击败各 Rocket 训练师 |

### NPC 列表

#### Rocket 1（训练师）

- **挑战前：** "Who are you? How did you get here?"
- **战败时：** "Oww! Beaten!"
- **战后：** "Are you dissing TEAM ROCKET?"

#### Rocket 2（训练师）

- **挑战前：** "You broke into our operation?"
- **战败时：** "Burnt!"
- **战后：** "You're not going to get away with this, brat!"

#### Rocket 3（训练师）

- **挑战前：** "Intruder alert!"
- **战败时：** "I can't do it!"
- **战后：** "SILPH SCOPE? I don't know where it is!"

#### Rocket 4（训练师）

- **挑战前：** "Why did you come here?"
- **战败时：** "This won't do!"
- **战后：** "OK, I'll talk! Take the elevator to see my BOSS!"

#### Rocket 5（训练师，最后一名，击败后触发开门）

- **挑战前：** "Are you lost, you little rat?"
- **战败时：** "Why...?"（设置 `EVENT_BEAT_ROCKET_HIDEOUT_1_TRAINER_4`，触发门打开）
- **战后：** "Uh-oh, that fight opened the door!"

### 可收集物品

- ESCAPE_ROPE、HYPER_POTION

---

## 二、Rocket Hideout B2F（火箭队地下基地二层）

### 地图脚本流程（状态机）

含转向瓷砖（Arrow Tiles）迷宫机制：

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 检查玩家是否踩在转向瓷砖上，触发自动旋转移动 |
| 1 `PLAYER_SPINNING` | 玩家被转向瓷砖旋转，等待移动完成 |
| 2 `START_BATTLE` | 标准战斗流程 |
| 3 `END_BATTLE` | 战斗结束 |

**转向瓷砖机制：** 踩到特定坐标 → 播放 SFX_ARROW_TILES → 禁用玩家输入 → 执行预定义移动序列 → 恢复控制。

### NPC 列表

#### Rocket（训练师）

- **挑战前：** "BOSS said you can see GHOSTs with the SILPH SCOPE!"
- **战败时：** "I surrender!"
- **战后：** "The TEAM ROCKET HQ has 4 basement floors. Can you reach the BOSS?"

### 可收集物品

- MOON_STONE、NUGGET、TM_HORN_DRILL、SUPER_POTION

---

## 三、Rocket Hideout B3F（火箭队地下基地三层）

### 地图脚本流程

同 B2F，使用转向瓷砖迷宫状态机。

### NPC 列表

#### Rocket 1（训练师）

- **挑战前：** "Stop meddling in TEAM ROCKET's affairs!"
- **战败时：** "Oof! Taken down!"
- **战后：** "SILPH SCOPE? The machine the BOSS stole. It's here somewhere."

#### Rocket 2（训练师）

- **挑战前：** "We got word from upstairs that you were coming!"
- **战败时：** "What? I lost? No!"
- **战后：** "Go ahead and go! But, you need the LIFT KEY to run the elevator!"

### 可收集物品

- TM_DOUBLE_EDGE、RARE_CANDY

---

## 四、Rocket Hideout B4F（火箭队地下基地四层，Giovanni 首领战）

### 地图脚本流程（状态机）

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 检查与训练师碰撞 |
| 1 `START_BATTLE` | 标准战斗流程 |
| 2 `END_BATTLE` | 战斗结束 |
| 3 `BEAT_GIOVANNI` | Giovanni 战后特殊演出（对话、褪色、物体显隐） |

**门机制：** 需同时击败 `TRAINER_0` 和 `TRAINER_1` 才能解锁通往 Giovanni 的门（设置 `EVENT_ROCKET_HIDEOUT_4_DOOR_UNLOCKED`）。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_0~2` | 已击败各 Rocket 训练师 |
| `EVENT_ROCKET_HIDEOUT_4_DOOR_UNLOCKED` | 通往 Giovanni 的门已解锁 |
| `EVENT_BEAT_ROCKET_HIDEOUT_GIOVANNI` | 已击败 Giovanni |
| `EVENT_ROCKET_DROPPED_LIFT_KEY` | Rocket 3 战后已掉落 LIFT KEY |

### NPC 列表

#### Rocket 1（训练师）

- **挑战前：** "I know you! You ruined our plans at MT.MOON!"
- **战败时：** "Burned again!"
- **战后：** "Do you have something against TEAM ROCKET?"

#### Rocket 2（训练师）

- **挑战前：** "How can you not see the beauty of our evil?"
- **战败时：** "Ayaya!"
- **战后：** "BOSS! I'm sorry I failed you!"

#### Rocket 3（训练师，掉落 LIFT KEY）

- **挑战前：** "The elevator doesn't work? Who has the LIFT KEY?"
- **战败时：** "No!"
- **战后：** `"Oh no! I dropped the LIFT KEY!"`（若 `EVENT_ROCKET_DROPPED_LIFT_KEY` 未设置，则显示 LIFT KEY 物体）

---

#### Giovanni（首领）

**战斗前（首次接触）：**

> "So! I must say, I
> am impressed you
> got here!"

（触发战斗）

**战败后（BEAT_GIOVANNI 脚本）：**

> "I see that you
> raise #MON
> with utmost care.
>
> A child like you
> would never
> understand what I
> hope to achieve.
>
> I shall step
> aside this time!
>
> I hope we meet
> again..."

战后处理：
- 设置 `EVENT_BEAT_ROCKET_HIDEOUT_GIOVANNI`
- 隐藏 Giovanni，显示 SILPH_SCOPE 物体
- 激活所有 Saffron Gym 训练师事件

### 可收集物品

- HP_UP、TM_RAZOR_WIND、IRON
- **SILPH_SCOPE**（击败 Giovanni 后出现）
- **LIFT_KEY**（Rocket 3 战后掉落）

---

## 五、Rocket Hideout Elevator

### 电梯机制

**有 LIFT KEY：** 显示楼层选择菜单（B1F / B2F / B4F）。

**无 LIFT KEY：**

> "It appears to
> need a key."

---

## 六、Saffron City（金黄市）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无状态机。

### NPC 列表（Team Rocket 占领期）

#### Rocket 1

> "What do you want?
> Get lost!"

#### Rocket 2

> "BOSS said he'll
> take this town!"

#### Rocket 3

> "Get out of the
> way!"

#### Rocket 4

> "SAFFRON belongs
> to TEAM ROCKET!"

#### Rocket 5

> "Being evil makes
> me feel so alive!"

#### Rocket 6

> "Ow! Watch where
> you're walking!"

#### Rocket 7

> "With SILPH under
> control, we can
> exploit #MON
> around the world!"

---

#### Scientist

> "You beat TEAM
> ROCKET all alone?
> That's amazing!"

#### Silph Worker M（条件对话）

**`EVENT_BEAT_SILPH_CO_GIOVANNI` 未设置：** 无对话。

**已设置：**

> "Yeah! TEAM ROCKET
> is gone!
> It's safe to go
> out again!"

#### Silph Worker F

> "People should be
> flocking back to
> SAFFRON now."

#### Gentleman

> "I flew here on my
> PIDGEOT when I
> read about SILPH.
>
> It's already over?
> I missed the
> media action."

#### Pidgeot（宠物精灵）

> "PIDGEOT: Bi bibii!"

（播放 Pidgeot 叫声）

#### Rocker

> "I saw ROCKET
> BOSS escaping
> SILPH's building."

### 标识牌

| 标识 | 文本 |
|---|---|
| 城市路牌 | "SAFFRON CITY / Shining, Golden Land of Commerce" |
| Fighting Dojo | "FIGHTING DOJO" |
| 道馆 | "SAFFRON CITY #MON GYM / LEADER: SABRINA / The Master of Psychic #MON!" |
| Silph Co. | "SILPH CO. OFFICE BUILDING" |
| Trainer Tips 1 | "FULL HEAL cures all ailments like sleep and burns. It costs a bit more, but it's more convenient." |
| Trainer Tips 2 | "New GREAT BALL offers improved capture rates. Try it on those hard-to-catch #MON." |

---

## 七、Saffron Gym（金黄市道馆）

### 地图脚本流程（状态机）

进入时加载城市名 `SAFFRON CITY`、馆主名 `SABRINA`。

| 状态 (`wSaffronGymCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | `CheckFightingMapTrainers` |
| 1 `START_BATTLE` | `DisplayEnemyTrainerTextAndStartBattle` |
| 2 `END_BATTLE` | `EndTrainerBattle` |
| 3 `SABRINA_POST_BATTLE` | 给予 TM46 PSYWAVE；设置 MARSHBADGE |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_SABRINA` | 已击败馆主 Sabrina |
| `EVENT_GOT_TM46` | 已领取 TM46 PSYWAVE |
| `EVENT_BEAT_SAFFRON_GYM_TRAINER_0~6` | 已击败各道馆训练师 |

### NPC 列表

#### Sabrina（馆主）

**战斗前：**

> "I had a vision of
> your arrival!
>
> I have had psychic
> powers since I
> was a child.
>
> I first learned
> to bend spoons
> with my mind.
>
> I dislike fight-
> ing, but if you
> wish, I will show
> you my powers!"

**战败后（自动显示 MARSHBADGE 文本）：**

> "I'm
> shocked!
> But, a loss is a
> loss.
>
> I admit I didn't
> work hard enough
> to win!
>
> You earned the
> MARSHBADGE!"

接续：

> "The MARSHBADGE
> makes #MON up
> to L70 obey you!
>
> Stronger #MON
> will become wild,
> ignoring your
> orders in battle!
>
> Just don't raise
> your #MON too
> much!
>
> Wait, please take
> this TM with you!"

- TM46 给予成功：`"<PLAYER> received TM46!"`
  接续：
  > "TM46 is PSYWAVE!
  > It uses powerful
  > psychic waves to
  > inflict damage!"
- 背包已满：`"Your pack is full of other items!"`

**战后再次对话（已给 TM46）：**

> "Everyone has
> psychic power!
> People just don't
> realize it!"

---

#### Gym Guide

**未击败 Sabrina：**

> "Yo! Champ in
> making!
>
> SABRINA's #MON
> use psychic power
> instead of force!
>
> Fighting #MON
> are weak against
> psychic #MON!
>
> They get creamed
> before they can
> even aim a punch!"

**已击败：**

> "Psychic power,
> huh?
>
> If I had that,
> I'd make a bundle
> at the slots!"

---

#### 道馆训练师（7 名）

| # | 挑战前 | 战败时 | 战后 |
|---|---|---|---|
| 1 Channeler | "SABRINA is younger than I, but I respect her!" | "Not good enough!" | "In a battle of equals, the one with the stronger will wins! If you wish to beat SABRINA, focus on winning!" |
| 2 Youngster | "Does our unseen power scare you?" | "I never foresaw this!" | "Psychic #MON fear only ghosts and bugs!" |
| 3 Channeler | "#MON take on the appearance of their trainers. Your #MON must be tough, then!" | "I knew it!" | "I must teach better techniques to my #MON!" |
| 4 Youngster | "You know that power alone isn't enough!" | "I don't believe this!" | "SABRINA just wiped out the KARATE MASTER next door!" |
| 5 Channeler | "You and I, our #MON shall fight!" | "I lost after all!" | "I knew that this was going to take place." |
| 6 Youngster | "SABRINA is young, but she's also our LEADER! You won't reach her easily!" | "I lost my concentration!" | "There used to be 2 #MON GYMs in SAFFRON. The FIGHTING DOJO next door lost its GYM status when we went and creamed them!" |
| 7 Youngster | "SAFFRON #MON GYM is famous for its psychics! You want to see SABRINA! I can tell!" | "Arrrgh!" | "That's right! I used telepathy to read your mind!" |

---

## 八、Saffron Mart（金黄市商店）

### NPC 列表

#### Clerk

标准商店购物流程。

#### Super Nerd

> "MAX REPEL lasts
> longer than SUPER
> REPEL for keeping
> weaker #MON
> away!"

#### Cooltrainer F

> "REVIVE is costly,
> but it revives
> fainted #MON!"

---

## 九、Saffron Pokecenter（金黄市精灵中心）

### NPC 列表

#### Nurse

标准精灵中心治疗流程。

#### Beauty

> "#MON growth
> rates differ from
> specie to specie."

#### Gentleman

> "SILPH CO. is very
> famous. That's
> why it attracted
> TEAM ROCKET!"

#### Link Receptionist

标准通信俱乐部接待流程。

---

## 十、Saffron Pidgey House（金黄市比比鸟之家）

### NPC 列表

#### Brunette Girl

> "Thank you for
> writing. I hope
> to see you soon!
>
> Hey! Don't look
> at my letter!"

#### Pidgey（宠物精灵）

> "PIDGEY: Kurukkoo!"

（播放 Pidgey 叫声）

#### Youngster

> "The COPYCAT is
> cute! I'm getting
> her a # DOLL!"

#### 报纸（可检查物件）

> "I was given a PP
> UP as a gift.
>
> It's used for
> increasing the PP
> of techniques!"

---

## 十一、Fighting Dojo（格斗道场）

### 地图脚本流程（状态机）

| 状态 (`wFightingDojoCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 检查训练师碰撞；若未击败 Karate Master 且玩家在坐标 (4,3)，触发首领对话 |
| 1 `START_BATTLE` | `DisplayEnemyTrainerTextAndStartBattle` |
| 2 `END_BATTLE` | `EndTrainerBattle` |
| 3 `KARATE_MASTER_POST_BATTLE` | 首领战后脚本，设置所有训练师事件，显示礼物精灵选择 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_KARATE_MASTER` | 已击败 Karate Master |
| `EVENT_DEFEATED_FIGHTING_DOJO` | 已完全击败道场（选取礼物精灵后） |
| `EVENT_BEAT_FIGHTING_DOJO_TRAINER_0~3` | 已击败各 Blackbelt 训练师 |
| `EVENT_GOT_HITMONLEE` | 已选择 Hitmonlee |
| `EVENT_GOT_HITMONCHAN` | 已选择 Hitmonchan |

### NPC 列表

#### Karate Master（首领）

**战斗前（首次到达坐标 (4,3)）：**

> "Grunt!
>
> I am the KARATE
> MASTER! I am the
> LEADER here!
>
> You wish to
> challenge us?
> Expect no mercy!
>
> Fwaaa!"

**战败后（首次）：**

> "Indeed, I have
> lost!
>
> But, I beseech
> you, do not take
> our emblem as
> your trophy!
>
> In return, I will
> give you a prized
> fighting #MON!
>
> Choose whichever
> one you like!"

**战后再次对话：**

> "Ho!
>
> Stay and train at
> Karate with us!"

---

#### Blackbelt 1（训练师）

- **挑战前：** "Hoargh! Take your shoes off!"
- **战败时：** "I give up!"
- **战后：** "You wait 'til you see our Master! I'm a small fry compared to him!"

#### Blackbelt 2（训练师）

- **挑战前：** "I hear you're good! Show me!"
- **战败时：** "Judge! 1 point!"
- **战后：** "Our Master is a pro fighter!"

#### Blackbelt 3（训练师）

- **挑战前：** "Nothing tough frightens me! I break boulders for training!"
- **战败时：** "Yow! Stubbed fingers!"
- **战后：** "The only thing that frightens us is psychic power!"

#### Blackbelt 4（训练师）

- **挑战前：** "Hoohah! You're trespassing in our FIGHTING DOJO!"
- **战败时：** "Oof! I give up!"
- **战后：** "The prime fighters across the land train here."

---

#### Hitmonlee 精灵球（可选礼物）

询问：`"You want the hard kicking HITMONLEE?"`

- YES + 队伍未满：给予 Hitmonlee Lv.30（设置 `EVENT_GOT_HITMONLEE` 和 `EVENT_DEFEATED_FIGHTING_DOJO`，隐藏精灵球）
- 已获得其中一个：`"Better not get greedy..."`

#### Hitmonchan 精灵球（可选礼物）

询问：`"You want the piston punching HITMONCHAN?"`

- YES + 队伍未满：给予 Hitmonchan Lv.30（设置 `EVENT_GOT_HITMONCHAN` 和 `EVENT_DEFEATED_FIGHTING_DOJO`，隐藏精灵球）
- 已获得其中一个：`"Better not get greedy..."`

---

## 十二、Copycat's House 1F（仿真者之家一楼）

### NPC 列表

#### Middle-Aged Woman（母亲）

> "My daughter is so
> self-centered.
> She only has a
> few friends."

#### Middle-Aged Man（父亲）

> "My daughter likes
> to mimic people.
>
> Her mimicry has
> earned her the
> nickname COPYCAT
> around here!"

#### Chansey（宠物精灵）

> "CHANSEY: Chaan!
> Sii!"

（播放 Chansey 叫声）

---

## 十三、Copycat's House 2F（仿真者之家二楼）

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_GOT_TM31` | 已从 Copycat 获得 TM31 MIMIC |

### NPC 列表

#### Copycat（关键 NPC）

**首次对话（未获得 TM31，无 POKE_DOLL）：**

> "<PLAYER>: Hi! Do
> you like #MON?
>
> <PLAYER>: Uh no, I
> just asked you.
>
> <PLAYER>: Huh?
> You're strange!
>
> COPYCAT: Hmm?
> Quit mimicking?
>
> But, that's my
> favorite hobby!"

**持有 POKE_DOLL 时：**

> "Oh wow!
> A # DOLL!
>
> For me?
> Thank you!
>
> You can have
> this, then!"

- 给予成功：`"<PLAYER> received TM31!"`（移除 POKE_DOLL，设置 `EVENT_GOT_TM31`）
  接续：
  > "TM31 contains my
  > favorite, MIMIC!
  >
  > Use it on a good
  > #MON!"
- 背包已满：`"Don't you want this?"`

**已领取后：**

> "<PLAYER>: Hi!
> Thanks for TM31!
>
> <PLAYER>: Pardon?
>
> <PLAYER>: Is it
> that fun to mimic
> my every move?
>
> COPYCAT: You bet!
> It's a scream!"

#### Doduo（宠物精灵，特殊文本）

> "DODUO: Giiih!
>
> MIRROR MIRROR ON
> THE WALL, WHO IS
> THE FAIREST ONE
> OF ALL?"

#### 稀有娃娃（3 个可检查物件）

> "This is a rare
> #MON! Huh?
> It's only a doll!"

#### 超级任天堂（可检查物件）

> "A game with MARIO
> wearing a bucket
> on his head!"

---

## 十四、Silph Co. 1F（Silph 公司一楼）

### 地图脚本流程

检查 `EVENT_BEAT_SILPH_CO_GIOVANNI`：若已击败且接待员未恢复，则显示接待员（设置 `EVENT_SILPH_CO_RECEPTIONIST_AT_DESK`）。

### NPC 列表

#### Link Receptionist（条件出现，击败 Giovanni 后）

> "Welcome!
>
> The PRESIDENT is
> in the boardroom
> on 11F!"

---

## 十五、Silph Co. 2F-10F（Silph 公司各楼层通用机制）

### 门卡系统（Card Key）

每层含 2-4 扇电子门，需使用 CARD_KEY（从 5F 获得）解锁。

**未解锁时：** 门块阻挡（tile 0x54）。

**解锁机制：** `SilphCoXF_SetCardKeyDoorYScript` 检查 `wCardKeyDoorY` 坐标，自动解锁对应门并设置 `EVENT_SILPH_CO_X_UNLOCKED_DOORX`。

### 转向瓷砖（Teleport Tiles）

> "Diamond shaped
> tiles are
> teleport blocks!
>
> They're hi-tech
> transporters!"

### Silph Worker 通用条件对话模板

**`EVENT_BEAT_SILPH_CO_GIOVANNI` 未设置：** 显示惊慌/躲藏文本。

**已设置：** 显示感谢/庆祝文本，如：

> "<PLAYER>! You and
> your #MON
> saved us!"

---

## 十六、Silph Co. 2F（Silph 公司二楼）

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_SILPH_CO_2F_TRAINER_0~3` | 已击败各训练师 |
| `EVENT_GOT_TM36` | 已领取 TM36 SELFDESTRUCT |
| `EVENT_SILPH_CO_2_UNLOCKED_DOOR1~2` | 两扇门解锁状态 |

### NPC 列表

#### Silph Worker F（给予 TM36）

**`EVENT_GOT_TM36` 未设置：**

> "Eeek!
> No! Stop! Help!
>
> Oh, you're not
> with TEAM ROCKET.
> I thought...
> I'm sorry. Here,
> please take this!"

- 给予成功：`"<PLAYER> got TM36!"`（设置 `EVENT_GOT_TM36`）
  接续：
  > "TM36 is
  > SELFDESTRUCT!
  >
  > It's powerful, but
  > the #MON that
  > uses it faints!
  > Be careful."
- 背包已满：`"You don't have any room for this."`

**已领取：** 重复 TM36 说明。

#### Scientist 1（训练师，双重特务）

- **挑战前：** "Help! I'm a SILPH employee."
- **战败时：** "How did you know I was a ROCKET?"
- **战后：** "I work for both SILPH and TEAM ROCKET!"

#### Scientist 2（训练师）

- **挑战前：** "It's off limits here! Go home!"
- **战败时：** "You're good."
- **战后：** "Can you solve the maze in here?"

#### Rocket 1（训练师）

- **挑战前：** "No kids are allowed in here!"
- **战败时：** "Tough!"
- **战后：** "Diamond shaped tiles are teleport blocks! They're hi-tech transporters!"

#### Rocket 2（训练师）

- **挑战前：** "Hey kid! What are you doing here?"
- **战败时：** "I goofed!"
- **战后：** "SILPH CO. will be merged with TEAM ROCKET!"

### 可收集物品

- MOON_STONE、NUGGET、TM_HORN_DRILL、SUPER_POTION

---

## 十七、Silph Co. 3F（Silph 公司三楼）

### NPC 列表

#### Rocket（训练师）

- **挑战前：** "Quit messing with us, kid!"
- **战败时：** "I give up!"
- **战后：** "A hint? You can open doors with a CARD KEY!"

#### Scientist（训练师，双重特务）

- **挑战前：** "I support TEAM ROCKET more than I support SILPH!"
- **战败时：** "You really got me!"
- **战后：** "Humph... TEAM ROCKET said that if I helped them, they'd let me study #MON!"

### 可收集物品

- HYPER_POTION

---

## 十八、Silph Co. 4F（Silph 公司四楼）

### NPC 列表

#### Rocket 1（训练师）

- **挑战前：** "TEAM ROCKET has taken command of SILPH CO.!"
- **战败时：** "Arrgh!"
- **战后：** "Fwahahaha! My BOSS has been after this place!"

#### Scientist（训练师）

- **挑战前：** "My #MON are my loyal soldiers!"
- **战败时：** "Darn! You weak #MON!"
- **战后：** "The doors are electronically locked! A CARD KEY opens them!"

#### Rocket 2（训练师）

- **挑战前：** "Intruder spotted!"
- **战败时：** "Who are you?"
- **战后：** "I better tell the BOSS on 11F!"

### 可收集物品

- FULL_HEAL、MAX_REVIVE、ESCAPE_ROPE

---

## 十九、Silph Co. 5F（Silph 公司五楼，CARD KEY 获得地点）

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_SILPH_CO_5F_TRAINER_0~3` | 已击败各训练师 |
| `EVENT_SILPH_CO_5_UNLOCKED_DOOR1~3` | 三扇门解锁状态 |

### NPC 列表

#### Rocket 1（训练师）

- **挑战前：** "I heard a kid was wandering around."
- **战败时：** "Boom!"
- **战后：** "It's not smart to pick a fight with TEAM ROCKET!"

#### Scientist（训练师）

- **挑战前：** "We study # BALL technology on this floor!"
- **战败时：** "Dang! Blast it!"
- **战后：** "We worked on the ultimate # BALL which would catch anything!"

#### Rocker（训练师）

- **挑战前：** "Whaaat? There shouldn't be any children here?"

### 可收集物品

- TM_TAKE_DOWN、PROTEIN
- **CARD_KEY**（关键物品，用于解锁 Silph Co. 所有楼层的电子门）

### 可检查物件（研究报告）

三份精灵研究报告（报告内容为实验室研究数据）。

---

## 二十、Silph Co. 11F（Silph 公司顶楼，Giovanni 最终战）

### 地图脚本流程

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 检查训练师碰撞 |
| 1 `START_BATTLE` | 标准战斗流程 |
| 2 `END_BATTLE` | 战斗结束 |
| 3 `BEAT_GIOVANNI` | Giovanni 最终战后演出，触发 Team Rocket 撤离脚本 |

**Team Rocket 撤离脚本：** 击败 Giovanni 后，隐藏所有 Rocket 对象，显示 Saffron City 正常居民，设置 `EVENT_BEAT_SILPH_CO_GIOVANNI`。

### NPC 列表

#### Giovanni（最终首领，Silph Co. 版本）

**战斗前：**（首次接触，进入战斗）

**战败后：**

> （同 Rocket Hideout 版本，Giovanni 离开的台词）

---

## 二十一、Silph Co. Elevator

连接 Silph Co. 各楼层，功能与 Rocket Hideout Elevator 相同（无需钥匙）。

---

*下一章：Fuchsia City → Safari Zone → Route 16-22*
