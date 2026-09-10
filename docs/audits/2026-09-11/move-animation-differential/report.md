# 全技能动画逐帧差分审计

- 分支基线：`746f7dd035c47c9e38a974107a09a56c71aebca5`
- 原作源码：`pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`
- 原作正式 Red ROM SHA-1：`ea9bcae617fdf159b045185467ae58b2e4a48b9a`
- 确定性布置用 DEBUG ROM SHA-1：`5b1456177671b79b263c614ea0e7cc9ac542e9c4`（只负责进入战斗，不产生被比较帧）
- 范围：165 个技能 × 2 个攻方视角 = 330 条轨迹；每条轨迹重复 2 次
- 语义边界：`MoveAnimation` 入口到 `.animationFinished`，强制 `wAnimationType = 0`，不含通用命中反馈

## 结论

- PASS：0 个技能
- FAIL：165 个技能
- 不确定：0 个技能
- 双方视角合计：0 PASS / 330 FAIL / 0 不确定

判定门槛为：原作与当前实现的持续帧数、逐帧实际 OAM、逐帧相对基线的像素变化掩码全部相同，且两次采样可重复。任一通道不同即为 FAIL。像素通道比较各自相对动画前一帧的变化掩码，排除了纯静态区域；调色板和位移效果仍会取样各自底图，因此仅有像素差异时还要复核连续帧。

## 汇总诊断

- 时长不同：328/330；当前更短 296 条、更长 32 条、相同 2 条。
- 实际 OAM 不同：328/330。其中 220 条在播放器内部已生成对象，但前端一帧也没有画出对象。
- 动态像素掩码不同：330/330。唯一时长和 OAM 都相同的是 `TAKE_DOWN` 两侧，但连续帧中的位移/闪屏相位仍不同，所以不是 PASS。
- 坐标证据：207/330 条轨迹的可见源 OAM 只有在统一减去 X=8、Y=16 后才与原作关键帧相合。

## 已定位的共性原因

1. `AnimationPlayer` 明确要求调用方在 `WaitDelay` 后等待指定帧数；当前 `advance_move_animation` 却丢弃 `frames` 并把 `anim_wait` 设为 0。
2. 前端只在 `Playing` 分支复制 OAM；带延时的正常帧返回 `WaitDelay`，因此这些帧虽然存在于播放器缓冲区，却没有进入实际渲染层。
3. 原作基准坐标直接写入硬件 OAM；当前播放器把同一数值标作屏幕坐标，未扣除硬件的 X+8/Y+16 偏移。
4. 原作每个子动画都会通过 `CopyVideoData` 上传 64/79 个图块（每帧 8 个），并在多数 frame block 后执行额外的 OAM 清理帧；当前状态机没有建模这些 VBlank 开销。

上述四点是跨技能的共性缺陷，不代表修正它们后即可直接宣告全部通过；特殊效果的逐帧相位仍需用同一套审计重新验证。

源码定位：当前前端的 [`advance_move_animation`](../../../../crates/pokered-app/src/render/battle.rs#L1186-L1223)；锁定依赖中的 [`AnimationPlayer` 调用约定](https://github.com/liuyanghejerry/dotzuki/blob/88f1fccd72c62fcb51ce036e5d2b211e37ac9051/workspace/crates/dotzuki-renderer/src/battle_anim/player.rs#L31-L42) 与 [`WaitDelay` 返回](https://github.com/liuyanghejerry/dotzuki/blob/88f1fccd72c62fcb51ce036e5d2b211e37ac9051/workspace/crates/dotzuki-renderer/src/battle_anim/player.rs#L344-L354)；原作 [`PlayAnimation` / `PlaySubanimation`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/animations.asm#L164-L268) 与 [`CopyVideoData`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/copy2.asm#L62-L111)。

## 逐技能结果

| ID | 技能 | 玩家视角（原作→当前） | 敌方视角（原作→当前） | 总结 |
|---:|---|---|---|---|
| 1 | `POUND` | FAIL (29→3) | FAIL (29→3) | FAIL |
| 2 | `KARATE_CHOP` | FAIL (47→5) | FAIL (47→5) | FAIL |
| 3 | `DOUBLESLAP` | FAIL (45→5) | FAIL (45→5) | FAIL |
| 4 | `COMET_PUNCH` | FAIL (51→7) | FAIL (51→7) | FAIL |
| 5 | `MEGA_PUNCH` | FAIL (44→16) | FAIL (44→16) | FAIL |
| 6 | `PAY_DAY` | FAIL (74→10) | FAIL (74→10) | FAIL |
| 7 | `FIRE_PUNCH` | FAIL (126→16) | FAIL (126→16) | FAIL |
| 8 | `ICE_PUNCH` | FAIL (110→8) | FAIL (110→8) | FAIL |
| 9 | `THUNDERPUNCH` | FAIL (117→17) | FAIL (117→17) | FAIL |
| 10 | `SCRATCH` | FAIL (39→13) | FAIL (39→13) | FAIL |
| 11 | `VICEGRIP` | FAIL (47→5) | FAIL (47→5) | FAIL |
| 12 | `GUILLOTINE` | FAIL (55→21) | FAIL (55→21) | FAIL |
| 13 | `RAZOR_WIND` | FAIL (33→13) | FAIL (33→13) | FAIL |
| 14 | `SWORDS_DANCE` | FAIL (115→13) | FAIL (115→13) | FAIL |
| 15 | `CUT` | FAIL (37→18) | FAIL (37→18) | FAIL |
| 16 | `GUST` | FAIL (98→12) | FAIL (98→12) | FAIL |
| 17 | `WING_ATTACK` | FAIL (32→4) | FAIL (32→4) | FAIL |
| 18 | `WHIRLWIND` | FAIL (91→34) | FAIL (91→34) | FAIL |
| 19 | `FLY` | FAIL (35→5) | FAIL (35→5) | FAIL |
| 20 | `BIND` | FAIL (41→5) | FAIL (41→5) | FAIL |
| 21 | `SLAM` | FAIL (32→4) | FAIL (32→4) | FAIL |
| 22 | `VINE_WHIP` | FAIL (46→15) | FAIL (46→15) | FAIL |
| 23 | `STOMP` | FAIL (20→2) | FAIL (20→2) | FAIL |
| 24 | `DOUBLE_KICK` | FAIL (57→5) | FAIL (57→5) | FAIL |
| 25 | `MEGA_KICK` | FAIL (44→16) | FAIL (44→16) | FAIL |
| 26 | `JUMP_KICK` | FAIL (32→4) | FAIL (32→4) | FAIL |
| 27 | `ROLLING_KICK` | FAIL (36→9) | FAIL (36→9) | FAIL |
| 28 | `SAND_ATTACK` | FAIL (30→4) | FAIL (30→4) | FAIL |
| 29 | `HEADBUTT` | FAIL (22→6) | FAIL (22→6) | FAIL |
| 30 | `HORN_ATTACK` | FAIL (56→6) | FAIL (56→6) | FAIL |
| 31 | `FURY_ATTACK` | FAIL (57→13) | FAIL (57→13) | FAIL |
| 32 | `HORN_DRILL` | FAIL (66→6) | FAIL (66→6) | FAIL |
| 33 | `TACKLE` | FAIL (7→6) | FAIL (7→6) | FAIL |
| 34 | `BODY_SLAM` | FAIL (15→16) | FAIL (15→16) | FAIL |
| 35 | `WRAP` | FAIL (61→7) | FAIL (61→7) | FAIL |
| 36 | `TAKE_DOWN` | FAIL (11→11) | FAIL (11→11) | FAIL |
| 37 | `THRASH` | FAIL (32→4) | FAIL (32→4) | FAIL |
| 38 | `DOUBLE_EDGE` | FAIL (63→19) | FAIL (63→19) | FAIL |
| 39 | `TAIL_WHIP` | FAIL (43→44) | FAIL (43→44) | FAIL |
| 40 | `POISON_STING` | FAIL (18→2) | FAIL (18→2) | FAIL |
| 41 | `TWINEEDLE` | FAIL (45→5) | FAIL (45→5) | FAIL |
| 42 | `PIN_MISSILE` | FAIL (19→3) | FAIL (19→3) | FAIL |
| 43 | `LEER` | FAIL (9→13) | FAIL (9→13) | FAIL |
| 44 | `BITE` | FAIL (38→4) | FAIL (38→4) | FAIL |
| 45 | `GROWL` | FAIL (66→10) | FAIL (66→10) | FAIL |
| 46 | `ROAR` | FAIL (73→7) | FAIL (73→7) | FAIL |
| 47 | `SING` | FAIL (298→22) | FAIL (298→22) | FAIL |
| 48 | `SUPERSONIC` | FAIL (81→11) | FAIL (81→11) | FAIL |
| 49 | `SONICBOOM` | FAIL (132→14) | FAIL (132→14) | FAIL |
| 50 | `DISABLE` | FAIL (9→13) | FAIL (9→13) | FAIL |
| 51 | `ACID` | FAIL (114→16) | FAIL (114→16) | FAIL |
| 52 | `EMBER` | FAIL (95→13) | FAIL (95→13) | FAIL |
| 53 | `FLAMETHROWER` | FAIL (104→15) | FAIL (104→15) | FAIL |
| 54 | `MIST` | FAIL (139→68) | FAIL (139→68) | FAIL |
| 55 | `WATER_GUN` | FAIL (81→13) | FAIL (81→13) | FAIL |
| 56 | `HYDRO_PUMP` | FAIL (245→33) | FAIL (245→33) | FAIL |
| 57 | `SURF` | FAIL (261→82) | FAIL (261→82) | FAIL |
| 58 | `ICE_BEAM` | FAIL (111→19) | FAIL (111→19) | FAIL |
| 59 | `BLIZZARD` | FAIL (189→65) | FAIL (189→65) | FAIL |
| 60 | `PSYBEAM` | FAIL (81→64) | FAIL (81→64) | FAIL |
| 61 | `BUBBLEBEAM` | FAIL (100→21) | FAIL (100→21) | FAIL |
| 62 | `AURORA_BEAM` | FAIL (53→37) | FAIL (53→37) | FAIL |
| 63 | `HYPER_BEAM` | FAIL (187→148) | FAIL (187→148) | FAIL |
| 64 | `PECK` | FAIL (29→3) | FAIL (29→3) | FAIL |
| 65 | `DRILL_PECK` | FAIL (32→4) | FAIL (32→4) | FAIL |
| 66 | `SUBMISSION` | FAIL (52→29) | FAIL (52→29) | FAIL |
| 67 | `LOW_KICK` | FAIL (59→30) | FAIL (59→30) | FAIL |
| 68 | `COUNTER` | FAIL (59→30) | FAIL (59→30) | FAIL |
| 69 | `SEISMIC_TOSS` | FAIL (272→201) | FAIL (272→201) | FAIL |
| 70 | `STRENGTH` | FAIL (38→9) | FAIL (38→9) | FAIL |
| 71 | `ABSORB` | FAIL (112→17) | FAIL (112→17) | FAIL |
| 72 | `MEGA_DRAIN` | FAIL (120→27) | FAIL (120→27) | FAIL |
| 73 | `LEECH_SEED` | FAIL (115→8) | FAIL (115→8) | FAIL |
| 74 | `GROWTH` | FAIL (111→109) | FAIL (111→109) | FAIL |
| 75 | `RAZOR_LEAF` | FAIL (218→182) | FAIL (218→182) | FAIL |
| 76 | `SOLARBEAM` | FAIL (78→17) | FAIL (78→17) | FAIL |
| 77 | `POISONPOWDER` | FAIL (67→9) | FAIL (67→9) | FAIL |
| 78 | `STUN_SPORE` | FAIL (67→9) | FAIL (67→9) | FAIL |
| 79 | `SLEEP_POWDER` | FAIL (67→9) | FAIL (67→9) | FAIL |
| 80 | `PETAL_DANCE` | FAIL (167→160) | FAIL (167→160) | FAIL |
| 81 | `STRING_SHOT` | FAIL (101→11) | FAIL (101→11) | FAIL |
| 82 | `DRAGON_RAGE` | FAIL (135→24) | FAIL (135→24) | FAIL |
| 83 | `FIRE_SPIN` | FAIL (94→19) | FAIL (94→19) | FAIL |
| 84 | `THUNDERSHOCK` | FAIL (81→30) | FAIL (81→30) | FAIL |
| 85 | `THUNDERBOLT` | FAIL (127→83) | FAIL (127→83) | FAIL |
| 86 | `THUNDER_WAVE` | FAIL (117→34) | FAIL (117→34) | FAIL |
| 87 | `THUNDER` | FAIL (174→53) | FAIL (174→53) | FAIL |
| 88 | `ROCK_THROW` | FAIL (56→19) | FAIL (56→19) | FAIL |
| 89 | `EARTHQUAKE` | FAIL (145→147) | FAIL (145→147) | FAIL |
| 90 | `FISSURE` | FAIL (153→157) | FAIL (153→157) | FAIL |
| 91 | `DIG` | FAIL (46→26) | FAIL (46→26) | FAIL |
| 92 | `TOXIC` | FAIL (200→75) | FAIL (200→75) | FAIL |
| 93 | `CONFUSION` | FAIL (49→50) | FAIL (49→50) | FAIL |
| 94 | `PSYCHIC_M` | FAIL (192→306) | FAIL (192→306) | FAIL |
| 95 | `HYPNOSIS` | FAIL (49→50) | FAIL (49→50) | FAIL |
| 96 | `MEDITATE` | FAIL (36→11) | FAIL (36→11) | FAIL |
| 97 | `AGILITY` | FAIL (1→3) | FAIL (1→3) | FAIL |
| 98 | `QUICK_ATTACK` | FAIL (59→30) | FAIL (59→30) | FAIL |
| 99 | `RAGE` | FAIL (25→3) | FAIL (25→3) | FAIL |
| 100 | `TELEPORT` | FAIL (47→37) | FAIL (47→37) | FAIL |
| 101 | `NIGHT_SHADE` | FAIL (192→306) | FAIL (192→306) | FAIL |
| 102 | `MIMIC` | FAIL (112→15) | FAIL (112→15) | FAIL |
| 103 | `SCREECH` | FAIL (74→10) | FAIL (74→10) | FAIL |
| 104 | `DOUBLE_TEAM` | FAIL (180→139) | FAIL (180→139) | FAIL |
| 105 | `RECOVER` | FAIL (189→170) | FAIL (189→170) | FAIL |
| 106 | `HARDEN` | FAIL (36→11) | FAIL (36→11) | FAIL |
| 107 | `MINIMIZE` | FAIL (124→116) | FAIL (124→116) | FAIL |
| 108 | `SMOKESCREEN` | FAIL (160→113) | FAIL (160→113) | FAIL |
| 109 | `CONFUSE_RAY` | FAIL (137→21) | FAIL (137→21) | FAIL |
| 110 | `WITHDRAW` | FAIL (77→32) | FAIL (77→32) | FAIL |
| 111 | `DEFENSE_CURL` | FAIL (36→11) | FAIL (36→11) | FAIL |
| 112 | `BARRIER` | FAIL (105→13) | FAIL (105→13) | FAIL |
| 113 | `LIGHT_SCREEN` | FAIL (105→15) | FAIL (105→15) | FAIL |
| 114 | `HAZE` | FAIL (139→68) | FAIL (139→68) | FAIL |
| 115 | `REFLECT` | FAIL (153→63) | FAIL (153→63) | FAIL |
| 116 | `FOCUS_ENERGY` | FAIL (111→107) | FAIL (111→107) | FAIL |
| 117 | `BIDE` | FAIL (32→4) | FAIL (32→4) | FAIL |
| 118 | `METRONOME` | FAIL (43→44) | FAIL (43→44) | FAIL |
| 119 | `MIRROR_MOVE` | FAIL (29→3) | FAIL (29→3) | FAIL |
| 120 | `SELFDESTRUCT` | FAIL (115→42) | FAIL (115→42) | FAIL |
| 121 | `EGG_BOMB` | FAIL (61→9) | FAIL (61→9) | FAIL |
| 122 | `LICK` | FAIL (62→10) | FAIL (62→10) | FAIL |
| 123 | `SMOG` | FAIL (95→15) | FAIL (95→15) | FAIL |
| 124 | `SLUDGE` | FAIL (114→16) | FAIL (114→16) | FAIL |
| 125 | `BONE_CLUB` | FAIL (38→4) | FAIL (38→4) | FAIL |
| 126 | `FIRE_BLAST` | FAIL (152→19) | FAIL (152→19) | FAIL |
| 127 | `WATERFALL` | FAIL (195→64) | FAIL (195→64) | FAIL |
| 128 | `CLAMP` | FAIL (95→9) | FAIL (95→9) | FAIL |
| 129 | `SWIFT` | FAIL (83→19) | FAIL (83→19) | FAIL |
| 130 | `SKULL_BASH` | FAIL (18→2) | FAIL (18→2) | FAIL |
| 131 | `SPIKE_CANNON` | FAIL (26→4) | FAIL (26→4) | FAIL |
| 132 | `CONSTRICT` | FAIL (73→7) | FAIL (73→7) | FAIL |
| 133 | `AMNESIA` | FAIL (57→5) | FAIL (57→5) | FAIL |
| 134 | `KINESIS` | FAIL (29→3) | FAIL (29→3) | FAIL |
| 135 | `SOFTBOILED` | FAIL (215→135) | FAIL (215→135) | FAIL |
| 136 | `HI_JUMP_KICK` | FAIL (32→4) | FAIL (32→4) | FAIL |
| 137 | `GLARE` | FAIL (9→13) | FAIL (9→13) | FAIL |
| 138 | `DREAM_EATER` | FAIL (86→55) | FAIL (86→55) | FAIL |
| 139 | `POISON_GAS` | FAIL (95→13) | FAIL (95→13) | FAIL |
| 140 | `BARRAGE` | FAIL (47→7) | FAIL (47→7) | FAIL |
| 141 | `LEECH_LIFE` | FAIL (157→28) | FAIL (157→28) | FAIL |
| 142 | `LOVELY_KISS` | FAIL (74→10) | FAIL (74→10) | FAIL |
| 143 | `SKY_ATTACK` | FAIL (81→41) | FAIL (81→41) | FAIL |
| 144 | `TRANSFORM` | FAIL (156→31) | FAIL (178→31) | FAIL |
| 145 | `BUBBLE` | FAIL (100→5) | FAIL (100→5) | FAIL |
| 146 | `DIZZY_PUNCH` | FAIL (146→16) | FAIL (146→16) | FAIL |
| 147 | `SPORE` | FAIL (99→41) | FAIL (99→41) | FAIL |
| 148 | `FLASH` | FAIL (9→13) | FAIL (9→13) | FAIL |
| 149 | `PSYWAVE` | FAIL (224→267) | FAIL (224→267) | FAIL |
| 150 | `SPLASH` | FAIL (109→110) | FAIL (109→110) | FAIL |
| 151 | `ACID_ARMOR` | FAIL (24→18) | FAIL (24→18) | FAIL |
| 152 | `CRABHAMMER` | FAIL (56→6) | FAIL (56→6) | FAIL |
| 153 | `EXPLOSION` | FAIL (115→42) | FAIL (115→42) | FAIL |
| 154 | `FURY_SWIPES` | FAIL (31→13) | FAIL (31→13) | FAIL |
| 155 | `BONEMERANG` | FAIL (32→4) | FAIL (32→4) | FAIL |
| 156 | `REST` | FAIL (123→7) | FAIL (123→7) | FAIL |
| 157 | `ROCK_SLIDE` | FAIL (179→68) | FAIL (179→68) | FAIL |
| 158 | `HYPER_FANG` | FAIL (32→4) | FAIL (32→4) | FAIL |
| 159 | `SHARPEN` | FAIL (36→11) | FAIL (36→11) | FAIL |
| 160 | `CONVERSION` | FAIL (120→25) | FAIL (120→25) | FAIL |
| 161 | `TRI_ATTACK` | FAIL (61→17) | FAIL (61→17) | FAIL |
| 162 | `SUPER_FANG` | FAIL (32→6) | FAIL (32→6) | FAIL |
| 163 | `SLASH` | FAIL (39→13) | FAIL (39→13) | FAIL |
| 164 | `SUBSTITUTE` | FAIL (72→33) | FAIL (72→33) | FAIL |
| 165 | `STRUGGLE` | FAIL (29→3) | FAIL (29→3) | FAIL |

## 可复核数据

`summary.json` 是紧凑索引；完整的逐帧变化哈希、变化像素包围盒、实际 OAM、播放器源 OAM 及其 RLE 计数位于 `frame-traces.json.gz`。`evidence/` 保存抽样技能的原作与当前实现连续 PNG 帧；它们不是挑选关键帧，而是从语义入口到结束的完整窗口。

复现入口为 `scripts/move_animation_differential.py`；脚本会拒绝非锁定 SHA-1 的 ROM、符号文件或非锁定 commit 的 pret 源码。
