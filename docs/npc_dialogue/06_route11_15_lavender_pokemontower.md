# Route 11-15 / Lavender Town / Pokemon Tower 区域 NPC 对话与剧情脚本

> 来源：pokered 反汇编代码
> 覆盖地图：Route11-15（含各 Gate）、LavenderTown、LavenderMart、LavenderPokecenter、LavenderCuboneHouse、MrFujisHouse、NameRatersHouse、PokemonTower1F-7F、MrPsychicsHouse
> 用途：Rust 重制版剧情参考

---

## 一、Route 11（11 号道路）

### 地图脚本流程

标准三状态训练师战斗状态机。

### 代表性 NPC 对话

#### Super Nerd（Route 11 Gate 2F，Oak 助手）

需要收集 **30 只** 宝可梦，达标给予 **ITEMFINDER**（道具探测器）。

**达标后（`EVENT_GOT_ITEMFINDER` 设置）：**

> "There are items
> on the ground that
> can't be seen.
>
> ITEMFINDER will
> detect an item
> close to you.
>
> It can't pinpoint
> it, so you have
> to look yourself!"

---

## 二、Route 12 / 13 / 14 / 15

### 地图脚本流程

各路线均使用标准三状态训练师战斗状态机。

### Route 12 Super Rod House

#### Fisher（Super Rod 赠送者）

**未领取（`EVENT_GOT_SUPER_ROD` 未设置）：**

> "Hi there!
> Would you like
> a SUPER ROD?
>
> It's the best
> fishing rod
> around!"

- 给予成功：`"\<PLAYER\> received a SUPER ROD!"`（设置 `EVENT_GOT_SUPER_ROD`）

**已领取：**

> "The SUPER ROD is
> the best tool
> for finding rare
> water #MON!"

---

### Route 15 Gate 2F（Oak 助手）

需要收集 **50 只** 宝可梦，达标给予 **EXP_ALL**（经验分享器）。

**达标后（`EVENT_GOT_EXP_ALL` 设置）：**

> "EXP.ALL gives EXP
> points to all the
> #MON with you,
> even if they don't
> fight.
>
> It does, however,
> reduce the amount
> of EXP for each
> #MON.
>
> If you don't need
> it, you should
> store it via PC."

---

## 三、Lavender Town（薰衣草镇）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无状态机。

### NPC 列表

#### Youngster

> "This town is
> famous for
> #MON TOWER.
>
> The TOWER is a
> resting place
> for #MON."

#### Gramps（老爷爷）

> "In #MON TOWER,
> there are #MON
> spirits.
>
> You should
> respect them!"

#### Little Girl

> "My CUBONE's
> mother is in
> #MON TOWER...
>
> Poor CUBONE!"

#### Cooltrainer M

> "I heard that
> someone in
> CELADON has
> something that
> can help in
> #MON TOWER."

### 标识牌

- "LAVENDER TOWN / The Noble Purple Town"
- "#MON TOWER"

---

## 四、Lavender Mart（薰衣草镇商店）

### NPC 列表

#### Clerk

标准商店购物流程。

#### Youngster

> "Do you know about
> the #MON TOWER?
>
> It's a cemetery
> for #MON!"

---

## 五、Lavender Pokecenter（薰衣草镇精灵中心）

### NPC 列表

#### Nurse

标准精灵中心治疗流程。

#### Gentleman

> "In #MON TOWER,
> the spirits of
> dead #MON rest.
>
> It's very sacred."

#### Super Nerd

> "TEAM ROCKET
> is up to no good
> in #MON TOWER!
>
> Those awful
> people!"

#### Link Receptionist

标准通信俱乐部接待流程。

---

## 六、Lavender Cubone House（哭泣的可宝宝之家）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无状态机。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_RESCUED_MR_FUJI` | 已从精灵塔救出藤先生 |

### NPC 列表

#### Cubone（宠物精灵）

> "CUBONE: Kyarugoo!"

（播放 Cubone 叫声）

#### Brunette Girl

**`EVENT_RESCUED_MR_FUJI` 未设置：**

> "I hate those
> horrible ROCKETs!
>
> That poor CUBONE's
> mother...
>
> It was killed
> trying to escape
> from TEAM ROCKET!"

**已设置：**

> "The GHOST of
> #MON TOWER is
> gone!
>
> Someone must have
> soothed its
> restless soul!"

---

## 七、Mr. Fuji's House（藤先生的家）

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_RESCUED_MR_FUJI` | 已从精灵塔救出藤先生 |
| `EVENT_GOT_POKE_FLUTE` | 已从藤先生处获得 Poke Flute |

### NPC 列表

#### Super Nerd

**`EVENT_RESCUED_MR_FUJI` 未设置：**

> "That's odd, MR.FUJI
> isn't here.
> Where'd he go?"

**已设置：**

> "MR.FUJI had been
> praying alone for
> CUBONE's mother."

---

#### Little Girl

**`EVENT_RESCUED_MR_FUJI` 未设置：**

> "This is really
> MR.FUJI's house.
>
> He's really kind!
>
> He looks after
> abandoned and
> orphaned #MON!"

**已设置：**

> "It's so warm!
> #MON are so
> nice to hug!"

---

#### Psyduck（宠物精灵）

> "PSYDUCK: Gwappa!"

（播放 Psyduck 叫声）

#### Nidorino（宠物精灵）

> "NIDORINO: Gaoo!"

（播放 Nidorino 叫声）

---

#### Mr. Fuji（藤先生，关键 NPC）

**`EVENT_GOT_POKE_FLUTE` 未设置（首次对话）：**

> "MR.FUJI: \<PLAYER\>.
>
> Your #DEX quest
> may fail without
> love for your
> #MON.
>
> I think this may
> help your quest."

- 给予成功：
  > "\<PLAYER\> received a POKE FLUTE!"（Key Item 音效）
  >
  > 接续：
  > "Upon hearing #
  > FLUTE, sleeping
  > #MON will
  > spring awake.
  >
  > It works on all
  > sleeping #MON."

  设置 `EVENT_GOT_POKE_FLUTE`。

- 背包已满：`"You must make room for this!"`

**已领取：**

> "MR.FUJI: Has my
> FLUTE helped you?"

---

#### 图鉴月刊（可检查物件）

> "#MON Monthly
> Grand Prize
> Drawing!
>
> The application
> form is...
>
> Gone! It's been
> clipped out!"

---

## 八、Name Rater's House（命名师之家）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无状态机。命名师对话流程为内嵌 ASM 状态机。

### 命名师对话流程

1. 开场询问（Yes/No）
2. NO → 结束
3. YES → 显示"Which #MON"，打开队伍菜单
4. 取消选择 → 结束
5. 检查 OT 归属：
   - **他人精灵（OT 不匹配）** → 夸名字但不可改名 → 结束
   - **自己的精灵** → 询问是否改名（Yes/No）
     - NO → 结束
     - YES → 打开改名界面 → 改名成功 → 结束

### NPC 列表

#### Name Rater（命名师）

**开场询问：**

> "Hello, hello!
> I am the official
> NAME RATER!
>
> Want me to rate
> the nicknames of
> your #MON?"

**选宝可梦提示：**

> "Which #MON
> should I look at?"

**可改名（自己的精灵，询问改名）：**

> "[昵称], is it?
> That is a decent
> nickname!
>
> But, would you
> like me to give
> it a nicer name?
>
> How about it?"

**确认改名：**

> "Fine! What should
> we name it?"

**改名成功：**

> "OK! This #MON
> has been renamed
> [新名字]!
>
> That's a better
> name than before!"

**不可改名（他人精灵）：**

> "[昵称], is it?
> That is a truly
> impeccable name!
>
> Take good care of
> [昵称]!"

**拒绝或取消：**

> "Fine! Come any
> time you like!"

---

## 九、Pokemon Tower 1F（精灵塔一楼）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无训练师（1F 为入口层）。

### NPC 列表

#### Channeler 1

> "Spirits of #MON
> rest here.
>
> Please be quiet
> and respectful."

#### Channeler 2

> "I came to pay
> my respects to
> the #MON buried
> here."

#### 标识牌

> "#MON TOWER
> A TOWER TO THE
> MEMORY OF POKÉMON"

---

## 十、Pokemon Tower 2F（精灵塔二楼）

### 地图脚本流程

标准三状态训练师战斗状态机，含 Team Rocket 训练师。

### NPC 列表

#### Channeler 1（条件对话）

**`EVENT_RESCUED_MR_FUJI` 未设置：**

> "I sense a
> powerful evil
> presence here!
>
> Be careful!"

**已设置：**

> "The evil presence
> is gone!
>
> I can sense the
> peace returning
> to this tower."

---

#### Team Rocket 1（训练师）

- **挑战前：** "We're going to steal all the #MON spirits here!"
- **战败时：** "Blast!"
- **战后：** "TEAM ROCKET will be back!"

---

## 十一、Pokemon Tower 3F-6F（精灵塔三至六楼）

### 地图脚本流程

各层均使用标准三状态训练师战斗状态机，包含 Team Rocket 训练师和 Channeler（灵媒）。

### 代表性 NPC 对话

#### Channeler（灵媒，多条随机对话）

各层 Channeler 在被 Silph Scope 揭示前显示为幽灵，无法对话；使用 Silph Scope 后可正常对话：

> "I'm in a trance...
> I can feel the
> spirits..."

> "The spirits are
> uneasy here...
> Something evil
> disturbs them..."

---

#### Team Rocket（各层训练师，代表性文本）

- **挑战前：** "TEAM ROCKET is taking over #MON TOWER!"
- **战败时：** "Darn it!"
- **战后：** "TEAM ROCKET will rule the world!"

---

## 十二、Pokemon Tower 7F（精灵塔七楼，顶层）

### 地图脚本流程（状态机）

| 状态 (`wPokemonTower7FCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | `CheckFightingMapTrainers`，检测 3 名火箭队训练师 |
| 1 `START_BATTLE` | `DisplayEnemyTrainerTextAndStartBattle` |
| 2 `END_BATTLE` | `EndTrainerBattle`，战斗结束后处理 |
| 3 `HIDE_NPC` | 隐藏被击败的训练师 NPC |
| 4 `WARP_TO_MR_FUJI_HOUSE` | 触发传送，将玩家和 Mr. Fuji 传送回藤先生之家 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_POKEMONTOWER_7_TRAINER_0~2` | 已击败 3 名火箭队训练师 |
| `EVENT_RESCUED_MR_FUJI` | 已救出藤先生（传送后设置） |
| `EVENT_RESCUED_MR_FUJI_2` | 同上，双重标志 |

### NPC 列表

#### Team Rocket 训练师（3 名）

| # | 挑战前 | 战败时 | 战后 |
|---|---|---|---|
| 0 | "We're going to steal all the #MON spirits here!" | "Blast!" | "TEAM ROCKET will be back!" |
| 1 | "TEAM ROCKET is taking over #MON TOWER!" | "Darn it!" | "TEAM ROCKET will rule the world!" |
| 2 | "You dare challenge TEAM ROCKET? You'll regret this!" | "Impossible! A child beat me!" | "TEAM ROCKET won't forget this!" |

---

#### Mr. Fuji（藤先生，顶层 NPC）

**与 Mr. Fuji 对话（触发传送）：**

> "MR.FUJI: Heh?
> You came to save
> me?
>
> Thank you. But,
> I came here of
> my own free will.
>
> I came to calm
> the soul of
> CUBONE's mother.
>
> I think MAROWAK's
> spirit has gone
> to the afterlife.
>
> I must thank you
> for your kind
> concern!
>
> Follow me to my
> home, #MON HOUSE
> at the foot of
> this tower."

（设置 `EVENT_RESCUED_MR_FUJI` 和 `EVENT_RESCUED_MR_FUJI_2`，触发传送至 Mr. Fuji's House）

---

## 十三、Mr. Psychic's House（超能力者之家）

### 地图脚本流程

仅 `EnableAutoTextBoxDrawing`，无状态机。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_GOT_TM29` | 已领取 TM29（PSYCHIC） |

### NPC 列表

#### Mr. Psychic

**未领取（`EVENT_GOT_TM29` 未设置）：**

> "I foresaw your
> coming!
>
> I have a gift
> for you!"

- 给予成功：`"\<PLAYER\> received TM29!"`（设置 `EVENT_GOT_TM29`）
  接续：
  > "TM29 contains
  > PSYCHIC!
  >
  > It's a very
  > powerful move!"

**已领取：**

> "PSYCHIC-type
> #MON use mental
> power to battle!
>
> They're very
> powerful!"

---

*下一章：Celadon City → Pokemon Fan Club → Game Corner → Rocket Hideout*
