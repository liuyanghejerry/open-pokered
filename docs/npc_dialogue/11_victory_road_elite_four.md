# Route 23-25 / Victory Road / Indigo Plateau / Elite Four / Hall of Fame 区域 NPC 对话与剧情脚本

> 来源：pokered 反汇编代码
> 覆盖地图：Route23、Route24（Nugget Bridge）、Route25（Bill 海角）、VictoryRoad1F/2F/3F、IndigoPlateau、IndigoPlateauLobby、LoreleisRoom、BrunosRoom、AgathasRoom、LancesRoom、ChampionsRoom、HallOfFame
> 用途：Rust 重制版剧情参考

---

## 一、Route 23（23 号道路，联盟徽章检查门廊）

### 地图脚本流程（状态机）

| 状态 (`wRoute23CurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 根据玩家 Y 坐标检查对应徽章；未持有则强制向下移动并显示拒绝文本 |
| 1 `PLAYER_MOVING` | 播放被拒绝后的玩家移动动画 |
| 2 `RESET_TO_DEFAULT` | 移动完成后重置为 DEFAULT |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_PASSED_CASCADEBADGE_CHECK` | 已通过第 1 关（CASCADE BADGE） |
| `EVENT_PASSED_THUNDERBADGE_CHECK` | 已通过第 2 关（THUNDER BADGE） |
| `EVENT_PASSED_RAINBOWBADGE_CHECK` | 已通过第 3 关（RAINBOW BADGE） |
| `EVENT_PASSED_SOULBADGE_CHECK` | 已通过第 4 关（SOUL BADGE） |
| `EVENT_PASSED_MARSHBADGE_CHECK` | 已通过第 5 关（MARSH BADGE） |
| `EVENT_PASSED_VOLCANOBADGE_CHECK` | 已通过第 6 关（VOLCANO BADGE） |
| `EVENT_PASSED_EARTHBADGE_CHECK` | 已通过第 7 关（EARTH BADGE） |

### NPC 列表（7 名守卫，按 Y 坐标排列）

#### 守卫 1（Y:35）— 检查 CASCADE BADGE

**未持有：**

> \"You can pass here only if you have the CASCADEBADGE!
>
> You don't have the CASCADEBADGE yet!
>
> You have to have it to get to POKEMON LEAGUE!\"

**持有：**

> \"Oh! That is the CASCADEBADGE!
>
> OK then! Please, go right ahead!\"

---

#### 守卫 2（Y:56）— 检查 THUNDER BADGE

（文本格式同上，替换徽章名为 THUNDERBADGE）

---

#### 守卫 3（Y:85）— 检查 RAINBOW BADGE

（文本格式同上，替换徽章名为 RAINBOWBADGE）

---

#### 游泳者 1（Y:96）— 检查 SOUL BADGE

（文本格式同上，替换徽章名为 SOULBADGE）

---

#### 游泳者 2（Y:105）— 检查 MARSH BADGE

（文本格式同上，替换徽章名为 MARSHBADGE）

---

#### 守卫 4（Y:119）— 检查 VOLCANO BADGE

（文本格式同上，替换徽章名为 VOLCANOBADGE）

---

#### 守卫 5（Y:136）— 检查 EARTH BADGE

（文本格式同上，替换徽章名为 EARTHBADGE）

### 标识牌

> \"VICTORY ROAD GATE / POKEMON LEAGUE\"

---

## 二、Route 24（24 号道路，Nugget Bridge）

### 地图脚本流程（状态机）

| 状态 (`wRoute24CurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 检查 5 名桥上训练师是否全部击败，若是则触发奖品发放 |
| 1 `START_BATTLE` | 显示训练师文本并开始战斗 |
| 2 `END_BATTLE` | 战斗后处理 |
| 3 `AFTER_ROCKET_BATTLE` | 火箭队成员战斗后处理（NPC 离开） |
| 4 `PLAYER_MOVING` | 玩家移动动画 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_GOT_NUGGET` | 已获得金砖奖励 |
| `EVENT_BEAT_ROUTE24_ROCKET` | 已击败火箭队成员 |
| `EVENT_BEAT_ROUTE_24_TRAINER_0~5` | 已击败各桥上训练师 |

### NPC 列表

#### CooltrainerM（桥头奖励发放者 / 火箭队成员）

**击败全部 5 名训练师后（首次）：**

> \"Congratulations! You beat our 5 contest trainers!
>
> You just earned a fabulous prize!\"

- 给予成功：`\"You received a NUGGET!\"`（设置 `EVENT_GOT_NUGGET`）
- 背包已满：提示无法接收

接续（火箭队招募）：

> \"By the way, would you like to join TEAM ROCKET?
>
> We're a group dedicated to evil using POKEMON!
>
> Want to join?\"

- YES → `\"Are you sure?\"`
  - YES → `\"Come on, join us!\"`
    - YES → `\"I'm telling you to join!\"`
      - YES → `\"OK, you need convincing! I'll make you an offer you can't refuse!\"`（强制进入战斗）
      - NO → 强制战斗
    - NO → 强制战斗
  - NO → 强制战斗
- NO → 强制战斗

**战败后：**

> \"Arrgh! You are good!\"

（设置 `EVENT_BEAT_ROUTE24_ROCKET`，NPC 离开）

**已获得金砖后再次对话：**

> \"With your ability, you could become a top leader in TEAM ROCKET!\"

---

#### 5 名桥上训练师（Youngster × 2、CooltrainerM × 2、CooltrainerF × 1）

代表性对话：

| # | 挑战前 | 战后 |
|---|---|---|
| Youngster 1 | \"Local trainers come here to practice!\" | - |
| Youngster 2 | \"Dad took me to a great party on S.S.ANNE at VERMILION CITY!\" | - |
| CooltrainerM | \"I'm a cool guy. I've got a girl friend!\" | - |
| CooltrainerF | \"Hi! My boy friend is cool!\" | - |

### 可收集物品

- TM_THUNDER_WAVE（雷电波）

---

## 三、Route 25（25 号道路，Bill 的海角）

### 地图脚本流程（状态机）

标准三状态训练师战斗状态机。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING` | 已帮助 Bill 并离开其小屋 |
| `EVENT_MET_BILL_2` | 已与 Bill 二次会面 |
| `EVENT_GOT_SS_TICKET` | 已从 Bill 处获得 SS 票 |
| `EVENT_BEAT_ROUTE_25_TRAINER_0~8` | 已击败各训练师 |

### NPC 列表（9 名训练师）

| # | 职业 | 挑战前 | 战后 |
|---|---|---|---|
| 1 | Youngster | \"Local trainers come here to practice!\" | - |
| 2 | Youngster | \"Dad took me to a great party on S.S.ANNE at VERMILION CITY!\" | - |
| 3 | CooltrainerM | \"I'm a cool guy. I've got a girl friend!\" | - |
| 4 | CooltrainerF | \"Hi! My boy friend is cool!\" | - |
| 5 | Youngster | \"I knew I had to fight you!\" | - |
| 6 | CooltrainerF | \"My friend has a cute POKEMON. I'm so jealous!\" | - |
| 7 | Hiker | \"I just got down from MT.MOON, but I'm ready!\" | - |
| 8 | Hiker | \"I'm off to see a POKEMON collector at the cape!\" | - |
| 9 | Hiker | \"You're going to see BILL? First, let's fight!\" | - |

### 标识牌

> \"SEA COTTAGE / BILL lives here!\"

### 可收集物品

- TM_SEISMIC_TOSS（地震拳）

---

## 四、Victory Road 1F（胜利之路一楼）

### 地图脚本流程（状态机）

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 检查 `EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH`，若已设置则替换地板砖块 |
| 1 `START_BATTLE` | 标准战斗流程 |
| 2 `END_BATTLE` | 战斗后处理 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH` | 1F 石头已推到开关上 |
| `EVENT_BEAT_VICTORY_ROAD_1_TRAINER_0~1` | 已击败 2 名训练师 |

### NPC 列表（2 名训练师）

#### CooltrainerF

- **挑战前：** \"I wonder if you are good enough for me!\"
- **战败时：** \"I lost out!\"
- **战后：** \"I never wanted to lose to anybody!\"

#### CooltrainerM

- **挑战前：** \"I can see you're good! Let me see exactly how good!\"
- **战败时：** \"I had a chance...\"
- **战后：** \"I concede, you're better than me!\"

### 可收集物品

- TM43（SKY ATTACK 天空攻击）
- RARE_CANDY（稀有糖果）
- 石头 × 3（需推到开关解锁通道）

---

## 五、Victory Road 2F（胜利之路二楼）

### 地图脚本流程（状态机）

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 检查两个石头开关状态；进入时重置 1F 开关标志 |
| 1 `START_BATTLE` | 标准战斗流程 |
| 2 `END_BATTLE` | 战斗后处理 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1` | 2F 第一块石头已推到开关上 |
| `EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH2` | 2F 第二块石头已推到开关上 |
| `EVENT_BEAT_MOLTRES` | 已击败火焰鸟（Moltres） |
| `EVENT_BEAT_VICTORY_ROAD_2_TRAINER_0~4` | 已击败 5 名训练师 |

### NPC 列表

#### Hiker

- **挑战前：** \"VICTORY ROAD is the final test for trainers!\"
- **战败时：** \"Aiyah!\"
- **战后：** \"If you get stuck, try moving some boulders around!\"

#### SuperNerd 1

- **挑战前：** \"Ah, so you wish to challenge the ELITE FOUR?\"
- **战败时：** \"You got me!\"
- **战后：** \"<RIVAL> also came through here!\"

#### CooltrainerM

- **挑战前：** \"Come on! I'll whip you!\"
- **战败时：** \"I got whipped!\"
- **战后：** \"You earned the right to be on VICTORY ROAD!\"

#### SuperNerd 2

- **挑战前：** \"If you can get through here, you can go meet the ELITE FOUR!\"
- **战败时：** \"No! Unbelievable!\"
- **战后：** \"I can beat you when it comes to knowledge about POKEMON!\"

#### SuperNerd 3

- **挑战前：** \"Is VICTORY ROAD too tough?\"
- **战败时：** \"Well done!\"
- **战后：** \"Many trainers give up the challenge here.\"

#### Moltres（火焰鸟，传说神兽）

> \"Gyaoo!\"

（播放 Moltres 叫声，进入战斗）

### 可收集物品

- TM17（SUBMISSION 投降）
- FULL_HEAL（全满药）
- TM05（MEGA KICK 超级踢）
- GUARD_SPEC（特防屏障）
- 石头 × 3

---

## 六、Victory Road 3F（胜利之路三楼）

### 地图脚本流程（状态机）

| 状态 | 说明 |
|---|---|
| 0 `DEFAULT` | 检查石头开关状态和洞穴机制；进入时重置 2F 开关标志 |
| 1 `START_BATTLE` | 标准战斗流程 |
| 2 `END_BATTLE` | 战斗后处理 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH1` | 3F 第一块石头已推到开关上 |
| `EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2` | 3F 第二块石头已落入洞中 |
| `BIT_PUSHED_BOULDER` | 玩家刚推动石头（临时标志） |
| `EVENT_BEAT_VICTORY_ROAD_3_TRAINER_0~3` | 已击败 4 名训练师 |

### NPC 列表（4 名 CooltrainerM/F）

#### CooltrainerM 1

- **挑战前：** \"I heard rumors of a child prodigy!\"
- **战败时：** \"The rumors were true!\"
- **战后：** \"You beat GIOVANNI of TEAM ROCKET?\"

#### CooltrainerF 1

- **挑战前：** \"I'll show you just how good you are!\"
- **战败时：** \"I'm furious!\"
- **战后：** \"You showed me just how good I was!\"

#### CooltrainerM 2

- **挑战前：** \"Only the chosen can pass here!\"
- **战败时：** \"I don't believe it!\"
- **战后：** \"All trainers here are headed to the POKEMON LEAGUE! Be careful!\"

#### CooltrainerF 2

- **挑战前：** \"Trainers live to seek stronger opponents!\"
- **战败时：** \"Oh! So strong!\"
- **战后：** \"By fighting tough battles, you get stronger!\"

### 可收集物品

- MAX_REVIVE（复活药 MAX）
- TM47（EXPLOSION 大爆炸）
- 石头 × 4

---

## 七、Indigo Plateau（靛蓝高原，室外区域）

无 NPC，无脚本，仅作为联盟建筑群的室外入口。

---

## 八、Indigo Plateau Lobby（靛蓝高原大厅）

### 地图脚本流程

进入时初始化串口连接；若玩家已开始四天王挑战但未完成，重置所有四天王事件标志及胜利之路开关标志，允许重新挑战。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `BIT_STARTED_ELITE_4` | 已开始四天王挑战（进入 Lorelei 房间时设置） |
| `EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH` | 进入大厅时重置 |
| `INDIGO_PLATEAU_EVENTS_START ~ INDIGO_PLATEAU_EVENTS_END` | 四天王事件范围（重置区间） |

### NPC 列表

#### Nurse（护士）

标准精灵中心治疗流程。

#### Gym Guide（道馆向导）

> \"Yo! Champ in making!
>
> At POKEMON LEAGUE, you have to face the ELITE FOUR in succession.
>
> If you lose, you have to start all over again!
>
> This is it! Go for it!\"

#### CooltrainerF

> \"From here on, you face the ELITE FOUR one by one!
>
> If you win, a door opens to the next trainer!
>
> Good luck!\"

#### Link Receptionist（通信俱乐部接待员）

标准通信俱乐部接待流程。

---

## 九、Lorelei's Room（洛雷莱之间）

### 地图脚本流程（状态机）

| 状态 (`wLoreleisRoomCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 检查是否已自动进场；若未进场则触发自动走路进场序列，锁闭出口 |
| 1 `LORELEI_START_BATTLE` | 显示 Lorelei 战前文本，开始战斗 |
| 2 `LORELEI_END_BATTLE` | 战斗后处理，解锁通往 Bruno 的出口 |
| 3 `PLAYER_IS_MOVING` | 等待自动走路完成 |
| 4 `NOOP` | 终态 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_LORELEIS_ROOM_TRAINER_0` | 已击败 Lorelei |
| `EVENT_AUTOWALKED_INTO_LORELEIS_ROOM` | 已完成进场自动走路 |
| `BIT_STARTED_ELITE_4` | 进入本房间时设置，标记四天王挑战已开始 |

**出口机制：** 通往 Bruno 的出口在战斗前以地砖 `$24` 封锁；击败 Lorelei 后替换为可通行砖块 `$05`。

### NPC 列表

#### Lorelei（四天王第一位）

**战斗前：**

> \"Welcome to POKEMON LEAGUE!
>
> I am LORELEI of the ELITE FOUR!
>
> No one can best me when it comes to icy POKEMON!
>
> Freezing moves are powerful!
>
> Your POKEMON will be at my mercy when they are frozen solid!
>
> Hahaha! Are you ready?\"

**战败后：**

> \"How dare you!
>
> You're better than I thought!
>
> Go on ahead!
>
> You only got a taste of POKEMON LEAGUE power!\"

#### 声音提示（出口封锁时）

> \"Someone's voice: Don't run away!\"

---

## 十、Bruno's Room（武天之间）

### 地图脚本流程（状态机）

结构与 Lorelei's Room 相同（DEFAULT → START_BATTLE → END_BATTLE → PLAYER_IS_MOVING → NOOP）。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_BRUNOS_ROOM_TRAINER_0` | 已击败 Bruno |
| `EVENT_AUTOWALKED_INTO_BRUNOS_ROOM` | 已完成进场自动走路 |

### NPC 列表

#### Bruno（四天王第二位）

**战斗前：**

> \"I am BRUNO of the ELITE FOUR!
>
> Through rigorous training, people and POKEMON can become stronger!
>
> I've weight trained with my POKEMON!
>
> <PLAYER>! We will grind you down with our superior power!
>
> Hoo hah!\"

**战败后：**

> \"Why? How could I lose?
>
> My job is done! Go face your next challenge!\"

#### 声音提示（出口封锁时）

> \"Someone's voice: Don't run away!\"

---

## 十一、Agatha's Room（菊子之间）

### 地图脚本流程（状态机）

结构与 Lorelei's Room 相同（DEFAULT → START_BATTLE → END_BATTLE → PLAYER_IS_MOVING → NOOP）。

击败 Agatha 后，将 ChampionsRoom 的脚本状态推进至"宿敌准备战斗"阶段。

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_AGATHAS_ROOM_TRAINER_0` | 已击败 Agatha |
| `EVENT_AUTOWALKED_INTO_AGATHAS_ROOM` | 已完成进场自动走路 |

**出口机制：** 以地砖 `$0E`（通道）和 `$3B`（封锁）控制。

### NPC 列表

#### Agatha（四天王第三位）

**战斗前：**

> \"I am AGATHA of the ELITE FOUR!
>
> OAK's taken a lot of interest in you, child!
>
> That old duff was once tough and handsome!
>
> That was decades ago!
>
> Now he just wants to fiddle with his POKEDEX!
>
> He's wrong! POKEMON are for fighting!
>
> <PLAYER>! I'll show you how a real trainer fights!\"

**战败后：**

> \"Oh ho! You're something special, child!
>
> You win! I see what the old duff sees in you now!
>
> I have nothing else to say! Run along now, child!\"

#### 声音提示（出口封锁时）

> \"Someone's voice: Don't run away!\"

---

## 十二、Lance's Room（龙之间）

### 地图脚本流程（状态机）

| 状态 (`wLancesRoomCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 检查入口封锁状态；触发自动走路进场（12 步上、12 步左、7 步下、6 步左） |
| 1 `LANCE_START_BATTLE` | 显示 Lance 战前文本，开始战斗 |
| 2 `LANCE_END_BATTLE` | 战斗后处理，解锁通往 ChampionsRoom 的出口 |
| 3 `PLAYER_IS_MOVING` | 等待自动走路完成 |
| 4 `NOOP` | 终态 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_LANCE` | 已击败 Lance |
| `EVENT_LANCES_ROOM_LOCK_DOOR` | 入口已封锁（进入时设置） |

**入口机制：** 进入时以地砖 `$72/$73` 封锁入口；战斗结束后替换为 `$31/$32`。

### NPC 列表

#### Lance（四天王第四位）

**战斗前：**

> \"Ah! I heard about you <PLAYER>!
>
> I lead the ELITE FOUR!
>
> You can call me LANCE the dragon trainer!
>
> You know that dragons are mythical POKEMON!
>
> They're hard to catch and raise, but their powers are superior!
>
> They're virtually indestructible!
>
> Well, are you ready to lose?
>
> Your LEAGUE challenge ends with me, <PLAYER>!\"

**战败后：**

> \"That's it!
>
> I hate to admit it, but you are a POKEMON master!
>
> I still can't believe my dragons lost to you, <PLAYER>!
>
> You are now the POKEMON LEAGUE champion!
>
> ...Or, you would have been, but you have one more challenge ahead.
>
> You have to face another trainer!
>
> His name is... <RIVAL>!
>
> He beat the ELITE FOUR before you!
>
> He is the real POKEMON LEAGUE champion!\"

---

## 十三、Champion's Room（宿敌最终战）

### 地图脚本流程（状态机）

| 状态 (`wChampionsRoomCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 空闲等待（由 Agatha 战后推进至下一状态） |
| 1 `PLAYER_ENTERS` | 玩家自动走路进场 |
| 2 `RIVAL_READY_TO_BATTLE` | 显示宿敌战前文本，进入战斗 |
| 3 `RIVAL_DEFEATED` | 宿敌战败处理，触发大木博士进场 |
| 4 `OAK_ARRIVES` | 大木博士进入房间，音乐变更 |
| 5 `OAK_CONGRATULATES_PLAYER` | 大木博士向玩家道贺 |
| 6 `OAK_DISAPPOINTED_WITH_RIVAL` | 大木博士训斥宿敌 |
| 7 `OAK_COME_WITH_ME` | 大木博士引导玩家前往名人堂 |
| 8 `OAK_EXITS` | 大木博士离开 |
| 9 `PLAYER_FOLLOWS_OAK` | 玩家自动走路前往名人堂 |
| 10 `CLEANUP_SCRIPT` | 最终清理 |

### 关键事件标志

| 事件标志 | 含义 |
|---|---|
| `EVENT_BEAT_CHAMPION_RIVAL` | 已击败宿敌（最终 Champion） |
| `wRivalStarter` | 宿敌的初始精灵（决定其队伍组成） |
| `wPlayerStarter` | 玩家的初始精灵（用于大木博士台词） |

### NPC 列表

#### 宿敌（最终 Champion）

**战斗前：**

> \"<RIVAL>: Hey! I was looking forward to seeing you, <PLAYER>!
>
> My rival should be strong to keep me sharp!
>
> While working on POKEDEX, I looked all over for powerful POKEMON!
>
> Not only that, I assembled teams that would beat any POKEMON type!
>
> And now! I'm the POKEMON LEAGUE champion!
>
> <PLAYER>! Do you know what that means? I'll tell you!
>
> I am the most powerful trainer in the world!\"

**战败后（玩家获胜）：**

> \"NO! That can't be! You beat my best!
>
> After all that work to become LEAGUE champ?
>
> My reign is over already?
>
> It's not fair!\"

接续（宿敌独白）：

> \"Why? Why did I lose?
>
> I never made any mistakes raising my POKEMON...
>
> Darn it! You're the new POKEMON LEAGUE champion!
>
> Although I don't like to admit it.\"

**战败后（玩家失败，宿敌胜利）：**

> \"Hahaha! I won, I won!
>
> I'm too good for you, <PLAYER>!
>
> You did well to even reach me, <RIVAL>, the POKEMON genius!
>
> Nice try, loser! Hahaha!\"

---

#### 大木博士（OAK，过场 NPC）

**进场（称呼玩家）：**

> \"OAK: <PLAYER>!\"

**向玩家道贺：**

> \"OAK: So, you won! Congratulations!
>
> You're the new POKEMON LEAGUE champion!
>
> You've grown up so much since you first left with @<初始精灵名>!
>
> <PLAYER>, you have come of age!\"

**训斥宿敌：**

> \"OAK: <RIVAL>! I'm disappointed!
>
> I came when I heard you beat the ELITE FOUR!
>
> But, when I got here, you had already lost!
>
> <RIVAL>! Do you understand why you lost?
>
> You have forgotten to treat your POKEMON with trust and love!
>
> Without them, you will never become a champ again!\"

**引导玩家前往名人堂：**

> \"OAK: <PLAYER>! You understand that your victory was not just your own doing!
>
> The bond you share with your POKEMON is marvelous!
>
> <PLAYER>! Come with me!\"

---

## 十四、Hall of Fame（名人堂）

### 地图脚本流程（状态机）

| 状态 (`wHallOfFameCurScript`) | 说明 |
|---|---|
| 0 `DEFAULT` | 玩家自动走路进场（5 步向上） |
| 1 `OAK_CONGRATULATIONS` | 大木博士发表最终演讲，登录名人堂 |
| 2 `RESET_EVENTS_AND_SAVE` | 重置所有四天王事件、保存游戏数据 |
| 3 `NOOP` | 终态 |

### 关键操作

- 临时保存并恢复 `wLetterPrintingDelayFlags`
- 清除 `BIT_NO_MAP_MUSIC`
- 重置四天王房间脚本状态至 DEFAULT
- 重置 `INDIGO_PLATEAU_EVENTS_START` 至 `INDIGO_PLATEAU_EVENTS_END` 范围内的所有事件标志
- 设置 `wLastBlackoutMap` 为 `PALLET_TOWN`（失败后重生地点）
- 调用 `SaveGameData` 保存游戏

### NPC 列表

#### 大木博士（名人堂演讲）

> \"OAK: Er-hem! Congratulations <PLAYER>!
>
> This floor is the POKEMON HALL OF FAME!
>
> POKEMON LEAGUE champions are honored for their exploits here!
>
> Their POKEMON are also recorded in the HALL OF FAME!
>
> <PLAYER>! You have endeavored hard to become the new LEAGUE champion!
>
> Congratulations, <PLAYER>, you and your POKEMON are HALL OF FAMERs!\"

（显示玩家队伍，录入名人堂 PC，播放片尾曲）

---

## 附：胜利之路与四天王挑战流程总览

```
Route 23（7 个徽章检查点）
    ↓
Victory Road 1F/2F/3F（石头谜题 + Moltres）
    ↓
Indigo Plateau Lobby（精灵中心 + 向导）
    ↓
Lorelei's Room（冰系四天王）
    ↓
Bruno's Room（格斗系四天王）
    ↓
Agatha's Room（幽灵系四天王）→ 推进 ChampionsRoom 脚本
    ↓
Lance's Room（龙系四天王）→ 揭露宿敌为 Champion
    ↓
Champion's Room（宿敌最终战 + 大木博士过场）
    ↓
Hall of Fame（名人堂录入 + 游戏保存 + 重置四天王事件）
```

---

*全部地图 NPC 对话与剧情脚本文档完成*
*文档覆盖范围：PalletTown → ViridianCity → PewterCity → CeruleanCity → VermilionCity → LavenderTown → CeladonCity → SaffronCity → FuchsiaCity → CinnabarIsland → Victory Road → Elite Four → Hall of Fame*
