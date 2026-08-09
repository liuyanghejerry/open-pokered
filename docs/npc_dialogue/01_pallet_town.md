# Pallet Town 区域 NPC 对话与剧情脚本

> 来源：反汇编代码（pokered）
> 覆盖地图：PalletTown、RedsHouse1F、RedsHouse2F、BluesHouse、OaksLab
> 用途：Rust 重制版剧情参考

---

## 一、PalletTown（真红镇）

### 地图脚本流程

PalletTown 有一套基于 `wPalletTownCurScript` 状态机的剧情脚本，按以下顺序推进：

| 阶段编号 | 脚本名 | 触发条件 | 说明 |
|---|---|---|---|
| 0 | `PalletTownDefaultScript` | 玩家 Y 坐标 == 1（接近北出口）且未 `EVENT_FOLLOWED_OAK_INTO_LAB` | 播放 `MUSIC_MEET_PROF_OAK`，锁定输入，触发阶段1 |
| 1 | `PalletTownOakHeyWaitScript` | 接续阶段0 | 显示 Oak "Hey! Wait!" 文本，显示 Oak 精灵，触发阶段2 |
| 2 | `PalletTownOakWalksToPlayerScript` | 接续阶段1 | Oak 精灵寻路走向玩家，触发阶段3 |
| 3 | `PalletTownOakNotSafeComeWithMeScript` | Oak 移动完成后 | 显示 Oak "It's unsafe!" 文本，启动 NPC 移动脚本（玩家跟随 Oak 走向实验室），触发阶段4 |
| 4 | `PalletTownPlayerFollowsOakScript` | NPC 移动脚本结束后 | 触发阶段5（Daisy 脚本） |
| 5 | `PalletTownDaisyScript` | 接续阶段4 | 检查 `EVENT_GOT_TOWN_MAP` 且 `EVENT_ENTERED_BLUES_HOUSE`，若满足则切换 Daisy 精灵（坐姿→行走姿）；获得精灵球后设置 `EVENT_PALLET_AFTER_GETTING_POKEBALLS_2` |
| 6 | `PalletTownNoopScript` | 终态 | 空操作 |

**初始化逻辑：** 若 `EVENT_GOT_POKEBALLS_FROM_OAK` 已设置，则同时设置 `EVENT_PALLET_AFTER_GETTING_POKEBALLS`（控制 Oak 对话内容切换）。

---

### NPC 对话

#### Oak（大木博士）— 出现在镇上

Oak 在镇上出现时（玩家试图向北走出镇子），根据 `wOakWalkedToPlayer` 标志显示不同文本：

**阶段 A：Oak 还未走到玩家面前**（`wOakWalkedToPlayer == 0`）

> "OAK: Hey! Wait!
> Don't go out!"

- 不等待按键，自动继续
- 触发玩家头顶惊叹号气泡
- 玩家朝向变为向下

**阶段 B：Oak 已走到玩家面前**（`wOakWalkedToPlayer == 1`）

> "OAK: It's unsafe!
> Wild #MON live
> in tall grass!
>
> You need your own
> #MON for your
> protection.
> I know!
>
> Here, come with
> me!"

---

#### 女孩（Girl NPC）

> "I'm raising
> #MON too!
>
> When they get
> strong, they can
> protect me!"

- 无触发条件，固定对话

---

#### 渔夫（Fisher NPC）

> "Technology is
> incredible!
>
> You can now store
> and recall items
> and #MON as
> data via PC!"

- 无触发条件，固定对话

---

### 标识牌（Signs）

| 标识 | 文本 |
|---|---|
| Oak 实验室门牌 | "OAK #MON RESEARCH LAB" |
| 镇子路牌 | "PALLET TOWN / Shades of your journey await!" |
| 玩家家门牌 | "\<PLAYER\>'s house" |
| 对手家门牌 | "\<RIVAL\>'s house" |

---

## 二、RedsHouse1F（主角家一楼）

### 地图脚本

无特殊脚本，仅调用 `EnableAutoTextBoxDrawing`。

---

### NPC 对话

#### 妈妈（MOM）

根据 `BIT_GOT_STARTER`（是否已获得初始精灵）显示不同内容：

**未获得初始精灵时：**

> "MOM: Right.
> All boys leave
> home some day.
> It said so on TV.
>
> PROF.OAK, next
> door, is looking
> for you."

**已获得初始精灵时（回家治疗）：**

触发治疗流程：
1. 显示：
   > "MOM: \<PLAYER\>!
   > You should take a
   > quick rest."
2. 画面淡出为白色
3. 重载地图数据
4. 治疗队伍（`HealParty`）
5. 播放 `MUSIC_PKMN_HEALED`，等待音乐结束
6. 画面淡入
7. 显示：
   > "MOM: Oh good!
   > You and your
   > #MON are
   > looking great!
   > Take care now!"

---

#### 电视（TV）

根据玩家朝向显示不同内容：

**玩家面朝上（正面看电视）：**

> "There's a movie
> on TV. Four boys
> are walking on
> railroad tracks.
>
> I better go too."

（彩蛋：致敬《Stand By Me》）

**玩家从背面触碰：**

> "Oops, wrong side."

---

## 三、RedsHouse2F（主角家二楼）

### 地图脚本

有一个一次性脚本 `RedsHouse2FDefaultScript`：
- 清除手柄输入
- 将玩家朝向设为向上
- 切换到 `SCRIPT_REDSHOUSE2F_NOOP`（终态）

此脚本用于玩家第一次进入二楼时自动朝向调整。

### NPC 对话

无 NPC，无对话文本。

---

## 四、BluesHouse（对手家）

### 地图脚本

进入时触发 `BluesHouseDefaultScript`：
- 设置 `EVENT_ENTERED_BLUES_HOUSE`（记录玩家已进入对手家）
- 切换到 `SCRIPT_BLUESHOUSE_NOOP`（终态，一次性）

---

### NPC 对话

#### Daisy（对手的姐姐）— 坐姿版本

根据事件状态显示不同内容：

**状态 1：未获得图鉴（`EVENT_GOT_POKEDEX` 未设置）**

> "Hi \<PLAYER\>!
> \<RIVAL\> is out at
> Grandpa's lab."

**状态 2：已获得图鉴但未获得城镇地图（`EVENT_GOT_TOWN_MAP` 未设置）**

触发给予城镇地图流程：
1. 显示：
   > "Grandpa asked you
   > to run an errand?
   > Here, this will
   > help you!"
2. 给予 TOWN MAP（如背包已满则提示"You have too much stuff with you."）
3. 隐藏桌上地图精灵（`TOGGLE_TOWN_MAP`）
4. 显示：
   > "\<PLAYER\> got a
   > TOWN MAP!"
5. 设置 `EVENT_GOT_TOWN_MAP`

**状态 3：已获得城镇地图（`EVENT_GOT_TOWN_MAP` 已设置）**

> "Use the TOWN MAP
> to find out where
> you are."

---

#### Daisy — 行走版本（`EVENT_DAISY_WALKING` 触发后）

> "#MON are living
> things! If they
> get tired, give
> them a rest!"

---

#### 桌上的城镇地图道具

> "It's a big map!
> This is useful!"

---

## 五、OaksLab（大木实验室）

### 地图脚本流程

OaksLab 是游戏开场最复杂的剧情脚本，状态机通过 `wOaksLabCurScript` 控制，共 18 个阶段：

| 阶段编号 | 脚本名 | 说明 |
|---|---|---|
| 0 | `OaksLabDefaultScript` | 等待 `EVENT_OAK_APPEARED_IN_PALLET` 且 NPC 移动脚本结束，显示实验室内 Oak（Oak2），推进阶段1 |
| 1 | `OaksLabOakEntersLabScript` | Oak2 精灵执行进入移动（向上走3格），推进阶段2 |
| 2 | `OaksLabToggleOaksScript` | 等待 Oak2 移动完成，切换为 Oak1 精灵，推进阶段3 |
| 3 | `OaksLabPlayerEntersLabScript` | 模拟玩家按上键8次（自动走入实验室），Rival 和 Oak1 面向下，推进阶段4 |
| 4 | `OaksLabFollowedOakScript` | 等待模拟输入结束，设置 `EVENT_FOLLOWED_OAK_INTO_LAB`，恢复地图音乐，推进阶段5 |
| 5 | `OaksLabOakChooseMonSpeechScript` | 连续显示 Rival/Oak 对话（等待选精灵），设置 `EVENT_OAK_ASKED_TO_CHOOSE_MON`，推进阶段6 |
| 6 | `OaksLabPlayerDontGoAwayScript` | 监测玩家是否走向出口（Y==6），若是则 Oak/Rival 面向玩家并提醒，强制玩家走回，推进阶段7 |
| 7 | `OaksLabPlayerForcedToWalkBackScript` | 等待强制移动完成，回到阶段6继续监测 |
| 8 | `OaksLabChoseStarterScript` | 玩家选完初始精灵后，根据选择让 Rival 走向对应精灵球，推进阶段9 |
| 9 | `OaksLabRivalChoosesStarterScript` | 等待 Rival 移动完成，Rival 说台词，隐藏对应精灵球，给 Rival 精灵，推进阶段10 |
| 10 | `OaksLabRivalChallengesPlayerScript` | 等待玩家移动到 Y==6，Rival 面向玩家，播放 `MUSIC_MEET_RIVAL`，Rival 发起挑战，Rival 寻路走向玩家，推进阶段11 |
| 11 | `OaksLabRivalStartBattleScript` | 等待 Rival 移动完成，设置对战参数（`OPP_RIVAL1`），启动战斗，设置胜负文本，推进阶段12 |
| 12 | `OaksLabRivalEndBattleScript` | 战斗结束，恢复 Rival 位置，治疗玩家队伍，设置 `EVENT_BATTLED_RIVAL_IN_OAKS_LAB`，推进阶段13 |
| 13 | `OaksLabRivalStartsExitScript` | 延迟20帧，Rival 说"Smell you later!"，Rival 向下走出实验室，推进阶段14 |
| 14 | `OaksLabPlayerWatchRivalExitScript` | 等待 Rival 走出，隐藏 Rival 精灵，恢复音乐，推进终态（阶段17） |
| 15 | `OaksLabRivalArrivesAtOaksRequestScript` | 玩家交还 Oak's Parcel 后，Rival 被召回，走向 Oak，推进阶段16 |
| 16 | `OaksLabOakGivesPokedexScript` | 等待 Rival 到位，连续播放 Oak/Rival 图鉴对话，给予图鉴，设置相关事件，Rival 离开，推进阶段17 |
| 17 | `OaksLabRivalLeavesWithPokedexScript` | 等待 Rival 走出，隐藏 Rival 精灵，设置 Route22 Rival 出现事件，触发 Daisy 行走脚本，推进终态 |
| 18 | `OaksLabNoopScript` | 终态 |

**特殊逻辑：** 若 `EVENT_PALLET_AFTER_GETTING_POKEBALLS_2` 已设置，则将文本指针切换到 `OaksLab_TextPointers2`（精灵球获得后的简化版文本表）。

---

### NPC 对话

#### Rival（对手）— 可交互状态

根据事件阶段显示不同内容：

**未进入实验室时（`EVENT_FOLLOWED_OAK_INTO_LAB_2` 未设置）：**

> "\<RIVAL\>: Yo
> \<PLAYER\>! Gramps
> isn't around!"

**已进入实验室但未选精灵（`EVENT_GOT_STARTER` 未设置）：**

> "\<RIVAL\>: Heh, I
> don't need to be
> greedy like you!
>
> Go ahead and
> choose, \<PLAYER\>!"

**已选完精灵：**

> "\<RIVAL\>: My
> #MON looks a
> lot stronger."

---

#### 精灵球（Poké Ball 道具）— 三个球的交互

**触摸任意精灵球时（未被 Oak 允许选择前）：**

> "Those are #
> BALLs. They
> contain #MON!"

**被 Oak 允许选择后触摸精灵球：**
- 显示对应精灵的图鉴画面
- 然后询问确认：

| 球 | 确认文本 |
|---|---|
| 小火龙（Charmander）球 | "So! You want the fire #MON, CHARMANDER?" |
| 杰尼龟（Squirtle）球 | "So! You want the water #MON, SQUIRTLE?" |
| 妙蛙种子（Bulbasaur）球 | "So! You want the plant #MON, BULBASAUR?" |

选 YES 后：
> "This #MON is
> really energetic!"

> "\<PLAYER\> received
> a \<MON NAME\>!"

**最后一个球（已被 Rival 选走一个）：**

> "That's PROF.OAK's
> last #MON!"

---

#### Oak（大木博士）— Oak1（实验室主要 NPC）

根据游戏进度显示不同内容：

**未选精灵，Oak 已邀请选择：**

> "OAK: Now, \<PLAYER\>,
> which #MON do
> you want?"

**已选精灵但未与 Rival 战斗：**

> "OAK: If a wild
> #MON appears,
> your #MON can
> fight against it!"

**已与 Rival 战斗，未交 Parcel：**

> "OAK: \<PLAYER\>,
> raise your young
> #MON by making
> it fight!"

**玩家携带 Oak's Parcel 返回：**

> "OAK: Oh, \<PLAYER\>!
>
> How is my old
> #MON?
>
> Well, it seems to
> like you a lot.
>
> You must be
> talented as a
> #MON trainer!
>
> What? You have
> something for me?
>
> \<PLAYER\> delivered
> OAK's PARCEL."

接续：

> "Ah! This is the
> custom # BALL
> I ordered!
> Thank you!"

**已获得图鉴，未获得精灵球：**

> "#MON around the
> world wait for
> you, \<PLAYER\>!"

**Route22 Rival 战斗后，给予精灵球（首次）：**

> "OAK: You can't get
> detailed data on
> #MON by just
> seeing them.
>
> You must catch
> them! Use these
> to capture wild
> #MON.
>
> \<PLAYER\> got 5
> # BALLs!"

接续：

> "When a wild
> #MON appears,
> it's fair game.
>
> Just throw a #
> BALL at it and try
> to catch it!
>
> This won't always
> work, though.
>
> A healthy #MON
> could escape. You
> have to be lucky!"

**已给过精灵球（后续再次对话）：**

> "OAK: Come see me
> sometimes.
>
> I want to know how
> your #DEX is
> coming along."

**已拥有 2 只以上精灵（图鉴进度对话）：**

> "OAK: Good to see
> you! How is your
> #DEX coming?
> Here, let me take
> a look!"

（触发图鉴评分展示 `DisplayDexRating`）

---

#### Oak（大木博士）— 剧情阶段台词（脚本触发，不可直接交互）

**阶段5：OakChooseMon 剧情**

Rival 先说：
> "\<RIVAL\>: Gramps!
> I'm fed up with
> waiting!"

Oak 说：
> "OAK: \<RIVAL\>?
> Let me think...
>
> Oh, that's right,
> I told you to
> come! Just wait!
>
> Here, \<PLAYER\>!
>
> There are 3
> #MON here!
>
> Haha!
>
> They are inside
> the # BALLs.
>
> When I was young,
> I was a serious
> #MON trainer!
>
> In my old age, I
> have only 3 left,
> but you can have
> one! Choose!"

Rival 说：
> "\<RIVAL\>: Hey!
> Gramps! What
> about me?"

Oak 说：
> "OAK: Be patient!
> \<RIVAL\>, you can
> have one too!"

**阶段6：玩家走向出口时**

> "OAK: Hey! Don't go
> away yet!"

**阶段9：Rival 选精灵**

Rival 说：
> "\<RIVAL\>: I'll take
> this one, then!"

Rival 收到精灵后：
> "\<RIVAL\> received
> a \<MON NAME\>!"

**阶段10：Rival 发起挑战**

Rival 说：
> "\<RIVAL\>: Wait
> \<PLAYER\>!
> Let's check out
> our #MON!
>
> Come on, I'll take
> you on!"

**阶段12：战斗结束**

玩家败北时：
> "WHAT?
> Unbelievable!
> I picked the
> wrong #MON!"

玩家胜利时：
> "\<RIVAL\>: Yeah! Am
> I great or what?"

**阶段13：Rival 离开**

> "\<RIVAL\>: Okay!
> I'll make my
> #MON fight to
> toughen it up!
>
> \<PLAYER\>! Gramps!
> Smell you later!"

**阶段15：Rival 被召回（交 Parcel 后）**

> "\<RIVAL\>: Gramps!"

**阶段16：Oak 给图鉴**

Rival 说：
> "\<RIVAL\>: What did
> you call me for?"

Oak 说：
> "OAK: Oh right! I
> have a request
> of you two."

Oak 介绍图鉴：
> "On the desk there
> is my invention,
> #DEX!
>
> It automatically
> records data on
> #MON you've
> seen or caught!
>
> It's a hi-tech
> encyclopedia!"

Oak 给予图鉴：
> "OAK: \<PLAYER\> and
> \<RIVAL\>! Take
> these with you!
>
> \<PLAYER\> got
> #DEX from OAK!"

Oak 说明任务：
> "To make a complete
> guide on all the
> #MON in the
> world...
>
> That was my dream!
>
> But, I'm too old!
> I can't do it!
>
> So, I want you two
> to fulfill my
> dream for me!
>
> Get moving, you
> two!
>
> This is a great
> undertaking in
> #MON history!"

Rival 说：
> "\<RIVAL\>: Alright
> Gramps! Leave it
> all to me!
>
> \<PLAYER\>, I hate to
> say it, but I
> don't need you!
>
> I know! I'll
> borrow a TOWN MAP
> from my sis!
>
> I'll tell her not
> to lend you one,
> \<PLAYER\>! Hahaha!"

---

#### Oak2（实验室内另一个 Oak 精灵，剧情过渡用）

> "?"

（仅在过渡动画期间短暂出现，无实际对话内容）

---

#### 女孩助手（Girl NPC）

> "PROF.OAK is the
> authority on
> #MON!
>
> Many #MON
> trainers hold him
> in high regard!"

---

#### 科学家助手（Scientist NPC，两个）

> "I study #MON as
> PROF.OAK's AIDE."

（两个科学家 NPC 共用同一段文本）

---

#### 图鉴道具（Pokédex，桌上）

> "It's encyclopedia-
> like, but the
> pages are blank!"

---

## 附录：关键事件标志一览

| 事件标志 | 含义 |
|---|---|
| `EVENT_OAK_APPEARED_IN_PALLET` | Oak 已在镇上出现 |
| `EVENT_FOLLOWED_OAK_INTO_LAB` | 玩家已跟随 Oak 进入实验室（控制北出口封锁） |
| `EVENT_FOLLOWED_OAK_INTO_LAB_2` | 同上（用于 Rival 对话判断） |
| `EVENT_OAK_ASKED_TO_CHOOSE_MON` | Oak 已邀请玩家选精灵 |
| `BIT_GOT_STARTER` | 玩家已选择初始精灵 |
| `EVENT_GOT_STARTER` | 同上（事件版本） |
| `EVENT_BATTLED_RIVAL_IN_OAKS_LAB` | 已在实验室与 Rival 战斗 |
| `EVENT_GOT_POKEDEX` | 已获得图鉴 |
| `EVENT_GOT_POKEBALLS_FROM_OAK` | 已从 Oak 处获得精灵球 |
| `EVENT_PALLET_AFTER_GETTING_POKEBALLS` | 获得精灵球后的镇子状态标志 |
| `EVENT_GOT_TOWN_MAP` | 已获得城镇地图 |
| `EVENT_ENTERED_BLUES_HOUSE` | 已进入对手家 |
| `EVENT_DAISY_WALKING` | Daisy 已切换为行走状态 |
| `EVENT_OAK_GOT_PARCEL` | Oak 已收到包裹 |
| `EVENT_1ST_ROUTE22_RIVAL_BATTLE` | Route22 第一次 Rival 战斗已触发 |
| `EVENT_ROUTE22_RIVAL_WANTS_BATTLE` | Route22 Rival 等待战斗 |

---

## 附录：Rival 初始精灵选择逻辑

Rival 选择的精灵与玩家相克：

| 玩家选择 | Rival 选择 |
|---|---|
| Charmander（小火龙）| Squirtle（杰尼龟）|
| Squirtle（杰尼龟）| Bulbasaur（妙蛙种子）|
| Bulbasaur（妙蛙种子）| Charmander（小火龙）|

Rival 战斗队伍由 `OPP_RIVAL1` + `wTrainerNo`（1/2/3）决定，对应三种不同阵容。

---

*下一章：Route 1 → Viridian City*
