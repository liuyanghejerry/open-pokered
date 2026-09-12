# 全技能动画逐帧差分审计

- 分支基线：`746f7dd035c47c9e38a974107a09a56c71aebca5`
- 原作源码：`pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`
- 原作正式 Red ROM SHA-1：`ea9bcae617fdf159b045185467ae58b2e4a48b9a`
- 确定性布置用 DEBUG ROM SHA-1：`5b1456177671b79b263c614ea0e7cc9ac542e9c4`（只负责进入战斗，不产生被比较帧）
- 范围：165 个技能 × 2 个攻方视角 = 330 条轨迹；每条轨迹重复 2 次
- 语义边界：`MoveAnimation` 入口到 `.animationFinished`，强制 `wAnimationType = 0`，不含通用命中反馈

## 结论

- PASS：165 个技能
- FAIL：0 个技能
- 不确定：0 个技能
- 双方视角合计：330 PASS / 0 FAIL / 0 不确定

判定门槛为：原作与当前实现的持续帧数、逐帧实际 OAM、逐帧相对基线的像素变化掩码全部相同，且两次采样可重复。任一通道不同即为 FAIL。像素通道比较各自相对动画前一帧的变化掩码，排除了纯静态区域；调色板和位移效果仍会取样各自底图，因此仅有像素差异时还要复核连续帧。

## 汇总诊断

- 时长不同：0/330；当前更短 0 条、更长 0 条、相同 330 条。
- 实际 OAM 不同：0/330。其中 0 条在播放器内部已生成对象，但前端一帧也没有画出对象。
- 动态像素掩码不同：0/330。
- 坐标证据：36/330 条轨迹的播放器源 OAM 使用硬件坐标；渲染后的 OAM 已按 X−8、Y−16 转为屏幕坐标并纳入上面的实际 OAM 判定。

## 修复覆盖

1. 共享 Gen-I 动画驱动逐条执行原作命令流，保留 frame block、`DelayFrames`、图块上传和 OAM 清理所占的 VBlank。
2. 前端使用前后两级 OAM 缓冲，显示上一 VBlank 已提交的对象，并将硬件 OAM 坐标转换为屏幕坐标。
3. 画面级效果覆盖调色板写入、横纵摇屏、波形扫描线以及宝可梦图片的滑入、滑出、压缩、隐藏和恢复。
4. App 与 TUI 共用相同的解释器、时序表和扫描线渲染辅助函数。

源码定位：共享驱动与扫描线效果位于 [`gen1_battle_anim.rs`](../../../../crates/pokered-renderer/src/gen1_battle_anim.rs)，App/TUI 接入分别位于各自的 `render/battle.rs`；原作依据为 [`PlayAnimation` / `PlaySubanimation`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/animations.asm#L164-L268) 与 [`CopyVideoData`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/copy2.asm#L62-L111)。

## 逐技能结果

| ID | 技能 | 玩家视角（原作→当前） | 敌方视角（原作→当前） | 总结 |
|---:|---|---|---|---|
| 1 | `POUND` | PASS (29→29) | PASS (29→29) | PASS |
| 2 | `KARATE_CHOP` | PASS (47→47) | PASS (47→47) | PASS |
| 3 | `DOUBLESLAP` | PASS (45→45) | PASS (45→45) | PASS |
| 4 | `COMET_PUNCH` | PASS (51→51) | PASS (51→51) | PASS |
| 5 | `MEGA_PUNCH` | PASS (44→44) | PASS (44→44) | PASS |
| 6 | `PAY_DAY` | PASS (74→74) | PASS (74→74) | PASS |
| 7 | `FIRE_PUNCH` | PASS (126→126) | PASS (126→126) | PASS |
| 8 | `ICE_PUNCH` | PASS (110→110) | PASS (110→110) | PASS |
| 9 | `THUNDERPUNCH` | PASS (117→117) | PASS (117→117) | PASS |
| 10 | `SCRATCH` | PASS (39→39) | PASS (39→39) | PASS |
| 11 | `VICEGRIP` | PASS (47→47) | PASS (47→47) | PASS |
| 12 | `GUILLOTINE` | PASS (55→55) | PASS (55→55) | PASS |
| 13 | `RAZOR_WIND` | PASS (33→33) | PASS (33→33) | PASS |
| 14 | `SWORDS_DANCE` | PASS (115→115) | PASS (115→115) | PASS |
| 15 | `CUT` | PASS (37→37) | PASS (37→37) | PASS |
| 16 | `GUST` | PASS (98→98) | PASS (98→98) | PASS |
| 17 | `WING_ATTACK` | PASS (32→32) | PASS (32→32) | PASS |
| 18 | `WHIRLWIND` | PASS (91→91) | PASS (91→91) | PASS |
| 19 | `FLY` | PASS (35→35) | PASS (35→35) | PASS |
| 20 | `BIND` | PASS (41→41) | PASS (41→41) | PASS |
| 21 | `SLAM` | PASS (32→32) | PASS (32→32) | PASS |
| 22 | `VINE_WHIP` | PASS (46→46) | PASS (46→46) | PASS |
| 23 | `STOMP` | PASS (20→20) | PASS (20→20) | PASS |
| 24 | `DOUBLE_KICK` | PASS (57→57) | PASS (57→57) | PASS |
| 25 | `MEGA_KICK` | PASS (44→44) | PASS (44→44) | PASS |
| 26 | `JUMP_KICK` | PASS (32→32) | PASS (32→32) | PASS |
| 27 | `ROLLING_KICK` | PASS (36→36) | PASS (36→36) | PASS |
| 28 | `SAND_ATTACK` | PASS (30→30) | PASS (30→30) | PASS |
| 29 | `HEADBUTT` | PASS (22→22) | PASS (22→22) | PASS |
| 30 | `HORN_ATTACK` | PASS (56→56) | PASS (56→56) | PASS |
| 31 | `FURY_ATTACK` | PASS (57→57) | PASS (57→57) | PASS |
| 32 | `HORN_DRILL` | PASS (66→66) | PASS (66→66) | PASS |
| 33 | `TACKLE` | PASS (7→7) | PASS (7→7) | PASS |
| 34 | `BODY_SLAM` | PASS (15→15) | PASS (15→15) | PASS |
| 35 | `WRAP` | PASS (61→61) | PASS (61→61) | PASS |
| 36 | `TAKE_DOWN` | PASS (11→11) | PASS (11→11) | PASS |
| 37 | `THRASH` | PASS (32→32) | PASS (32→32) | PASS |
| 38 | `DOUBLE_EDGE` | PASS (63→63) | PASS (63→63) | PASS |
| 39 | `TAIL_WHIP` | PASS (43→43) | PASS (43→43) | PASS |
| 40 | `POISON_STING` | PASS (18→18) | PASS (18→18) | PASS |
| 41 | `TWINEEDLE` | PASS (45→45) | PASS (45→45) | PASS |
| 42 | `PIN_MISSILE` | PASS (19→19) | PASS (19→19) | PASS |
| 43 | `LEER` | PASS (9→9) | PASS (9→9) | PASS |
| 44 | `BITE` | PASS (38→38) | PASS (38→38) | PASS |
| 45 | `GROWL` | PASS (66→66) | PASS (66→66) | PASS |
| 46 | `ROAR` | PASS (73→73) | PASS (73→73) | PASS |
| 47 | `SING` | PASS (298→298) | PASS (298→298) | PASS |
| 48 | `SUPERSONIC` | PASS (81→81) | PASS (81→81) | PASS |
| 49 | `SONICBOOM` | PASS (132→132) | PASS (132→132) | PASS |
| 50 | `DISABLE` | PASS (9→9) | PASS (9→9) | PASS |
| 51 | `ACID` | PASS (114→114) | PASS (114→114) | PASS |
| 52 | `EMBER` | PASS (95→95) | PASS (95→95) | PASS |
| 53 | `FLAMETHROWER` | PASS (104→104) | PASS (104→104) | PASS |
| 54 | `MIST` | PASS (139→139) | PASS (139→139) | PASS |
| 55 | `WATER_GUN` | PASS (81→81) | PASS (81→81) | PASS |
| 56 | `HYDRO_PUMP` | PASS (245→245) | PASS (245→245) | PASS |
| 57 | `SURF` | PASS (261→261) | PASS (261→261) | PASS |
| 58 | `ICE_BEAM` | PASS (111→111) | PASS (111→111) | PASS |
| 59 | `BLIZZARD` | PASS (189→189) | PASS (189→189) | PASS |
| 60 | `PSYBEAM` | PASS (81→81) | PASS (81→81) | PASS |
| 61 | `BUBBLEBEAM` | PASS (100→100) | PASS (100→100) | PASS |
| 62 | `AURORA_BEAM` | PASS (53→53) | PASS (53→53) | PASS |
| 63 | `HYPER_BEAM` | PASS (187→187) | PASS (187→187) | PASS |
| 64 | `PECK` | PASS (29→29) | PASS (29→29) | PASS |
| 65 | `DRILL_PECK` | PASS (32→32) | PASS (32→32) | PASS |
| 66 | `SUBMISSION` | PASS (52→52) | PASS (52→52) | PASS |
| 67 | `LOW_KICK` | PASS (59→59) | PASS (59→59) | PASS |
| 68 | `COUNTER` | PASS (59→59) | PASS (59→59) | PASS |
| 69 | `SEISMIC_TOSS` | PASS (272→272) | PASS (272→272) | PASS |
| 70 | `STRENGTH` | PASS (38→38) | PASS (38→38) | PASS |
| 71 | `ABSORB` | PASS (112→112) | PASS (112→112) | PASS |
| 72 | `MEGA_DRAIN` | PASS (120→120) | PASS (120→120) | PASS |
| 73 | `LEECH_SEED` | PASS (115→115) | PASS (115→115) | PASS |
| 74 | `GROWTH` | PASS (111→111) | PASS (111→111) | PASS |
| 75 | `RAZOR_LEAF` | PASS (218→218) | PASS (218→218) | PASS |
| 76 | `SOLARBEAM` | PASS (78→78) | PASS (78→78) | PASS |
| 77 | `POISONPOWDER` | PASS (67→67) | PASS (67→67) | PASS |
| 78 | `STUN_SPORE` | PASS (67→67) | PASS (67→67) | PASS |
| 79 | `SLEEP_POWDER` | PASS (67→67) | PASS (67→67) | PASS |
| 80 | `PETAL_DANCE` | PASS (167→167) | PASS (167→167) | PASS |
| 81 | `STRING_SHOT` | PASS (101→101) | PASS (101→101) | PASS |
| 82 | `DRAGON_RAGE` | PASS (135→135) | PASS (135→135) | PASS |
| 83 | `FIRE_SPIN` | PASS (94→94) | PASS (94→94) | PASS |
| 84 | `THUNDERSHOCK` | PASS (81→81) | PASS (81→81) | PASS |
| 85 | `THUNDERBOLT` | PASS (127→127) | PASS (127→127) | PASS |
| 86 | `THUNDER_WAVE` | PASS (117→117) | PASS (117→117) | PASS |
| 87 | `THUNDER` | PASS (174→174) | PASS (174→174) | PASS |
| 88 | `ROCK_THROW` | PASS (56→56) | PASS (56→56) | PASS |
| 89 | `EARTHQUAKE` | PASS (145→145) | PASS (145→145) | PASS |
| 90 | `FISSURE` | PASS (153→153) | PASS (153→153) | PASS |
| 91 | `DIG` | PASS (46→46) | PASS (46→46) | PASS |
| 92 | `TOXIC` | PASS (200→200) | PASS (200→200) | PASS |
| 93 | `CONFUSION` | PASS (49→49) | PASS (49→49) | PASS |
| 94 | `PSYCHIC_M` | PASS (192→192) | PASS (192→192) | PASS |
| 95 | `HYPNOSIS` | PASS (49→49) | PASS (49→49) | PASS |
| 96 | `MEDITATE` | PASS (36→36) | PASS (36→36) | PASS |
| 97 | `AGILITY` | PASS (1→1) | PASS (1→1) | PASS |
| 98 | `QUICK_ATTACK` | PASS (59→59) | PASS (59→59) | PASS |
| 99 | `RAGE` | PASS (25→25) | PASS (25→25) | PASS |
| 100 | `TELEPORT` | PASS (47→47) | PASS (47→47) | PASS |
| 101 | `NIGHT_SHADE` | PASS (192→192) | PASS (192→192) | PASS |
| 102 | `MIMIC` | PASS (112→112) | PASS (112→112) | PASS |
| 103 | `SCREECH` | PASS (74→74) | PASS (74→74) | PASS |
| 104 | `DOUBLE_TEAM` | PASS (180→180) | PASS (180→180) | PASS |
| 105 | `RECOVER` | PASS (189→189) | PASS (189→189) | PASS |
| 106 | `HARDEN` | PASS (36→36) | PASS (36→36) | PASS |
| 107 | `MINIMIZE` | PASS (124→124) | PASS (124→124) | PASS |
| 108 | `SMOKESCREEN` | PASS (160→160) | PASS (160→160) | PASS |
| 109 | `CONFUSE_RAY` | PASS (137→137) | PASS (137→137) | PASS |
| 110 | `WITHDRAW` | PASS (77→77) | PASS (77→77) | PASS |
| 111 | `DEFENSE_CURL` | PASS (36→36) | PASS (36→36) | PASS |
| 112 | `BARRIER` | PASS (105→105) | PASS (105→105) | PASS |
| 113 | `LIGHT_SCREEN` | PASS (105→105) | PASS (105→105) | PASS |
| 114 | `HAZE` | PASS (139→139) | PASS (139→139) | PASS |
| 115 | `REFLECT` | PASS (153→153) | PASS (153→153) | PASS |
| 116 | `FOCUS_ENERGY` | PASS (111→111) | PASS (111→111) | PASS |
| 117 | `BIDE` | PASS (32→32) | PASS (32→32) | PASS |
| 118 | `METRONOME` | PASS (43→43) | PASS (43→43) | PASS |
| 119 | `MIRROR_MOVE` | PASS (29→29) | PASS (29→29) | PASS |
| 120 | `SELFDESTRUCT` | PASS (115→115) | PASS (115→115) | PASS |
| 121 | `EGG_BOMB` | PASS (61→61) | PASS (61→61) | PASS |
| 122 | `LICK` | PASS (62→62) | PASS (62→62) | PASS |
| 123 | `SMOG` | PASS (95→95) | PASS (95→95) | PASS |
| 124 | `SLUDGE` | PASS (114→114) | PASS (114→114) | PASS |
| 125 | `BONE_CLUB` | PASS (38→38) | PASS (38→38) | PASS |
| 126 | `FIRE_BLAST` | PASS (152→152) | PASS (152→152) | PASS |
| 127 | `WATERFALL` | PASS (195→195) | PASS (195→195) | PASS |
| 128 | `CLAMP` | PASS (95→95) | PASS (95→95) | PASS |
| 129 | `SWIFT` | PASS (83→83) | PASS (83→83) | PASS |
| 130 | `SKULL_BASH` | PASS (18→18) | PASS (18→18) | PASS |
| 131 | `SPIKE_CANNON` | PASS (26→26) | PASS (26→26) | PASS |
| 132 | `CONSTRICT` | PASS (73→73) | PASS (73→73) | PASS |
| 133 | `AMNESIA` | PASS (57→57) | PASS (57→57) | PASS |
| 134 | `KINESIS` | PASS (29→29) | PASS (29→29) | PASS |
| 135 | `SOFTBOILED` | PASS (215→215) | PASS (215→215) | PASS |
| 136 | `HI_JUMP_KICK` | PASS (32→32) | PASS (32→32) | PASS |
| 137 | `GLARE` | PASS (9→9) | PASS (9→9) | PASS |
| 138 | `DREAM_EATER` | PASS (86→86) | PASS (86→86) | PASS |
| 139 | `POISON_GAS` | PASS (95→95) | PASS (95→95) | PASS |
| 140 | `BARRAGE` | PASS (47→47) | PASS (47→47) | PASS |
| 141 | `LEECH_LIFE` | PASS (157→157) | PASS (157→157) | PASS |
| 142 | `LOVELY_KISS` | PASS (74→74) | PASS (74→74) | PASS |
| 143 | `SKY_ATTACK` | PASS (81→81) | PASS (81→81) | PASS |
| 144 | `TRANSFORM` | PASS (156→156) | PASS (178→178) | PASS |
| 145 | `BUBBLE` | PASS (100→100) | PASS (100→100) | PASS |
| 146 | `DIZZY_PUNCH` | PASS (146→146) | PASS (146→146) | PASS |
| 147 | `SPORE` | PASS (99→99) | PASS (99→99) | PASS |
| 148 | `FLASH` | PASS (9→9) | PASS (9→9) | PASS |
| 149 | `PSYWAVE` | PASS (224→224) | PASS (224→224) | PASS |
| 150 | `SPLASH` | PASS (109→109) | PASS (109→109) | PASS |
| 151 | `ACID_ARMOR` | PASS (24→24) | PASS (24→24) | PASS |
| 152 | `CRABHAMMER` | PASS (56→56) | PASS (56→56) | PASS |
| 153 | `EXPLOSION` | PASS (115→115) | PASS (115→115) | PASS |
| 154 | `FURY_SWIPES` | PASS (31→31) | PASS (31→31) | PASS |
| 155 | `BONEMERANG` | PASS (32→32) | PASS (32→32) | PASS |
| 156 | `REST` | PASS (123→123) | PASS (123→123) | PASS |
| 157 | `ROCK_SLIDE` | PASS (179→179) | PASS (179→179) | PASS |
| 158 | `HYPER_FANG` | PASS (32→32) | PASS (32→32) | PASS |
| 159 | `SHARPEN` | PASS (36→36) | PASS (36→36) | PASS |
| 160 | `CONVERSION` | PASS (120→120) | PASS (120→120) | PASS |
| 161 | `TRI_ATTACK` | PASS (61→61) | PASS (61→61) | PASS |
| 162 | `SUPER_FANG` | PASS (32→32) | PASS (32→32) | PASS |
| 163 | `SLASH` | PASS (39→39) | PASS (39→39) | PASS |
| 164 | `SUBSTITUTE` | PASS (72→72) | PASS (72→72) | PASS |
| 165 | `STRUGGLE` | PASS (29→29) | PASS (29→29) | PASS |

## 可复核数据

`summary.json` 是紧凑索引；完整的逐帧变化哈希、变化像素包围盒、实际 OAM、播放器源 OAM 及其 RLE 计数位于 `frame-traces.json.gz`。`evidence/` 保存抽样技能的原作与当前实现连续 PNG 帧；它们不是挑选关键帧，而是从语义入口到结束的完整窗口。

复现入口为 `scripts/move_animation_differential.py`；脚本会拒绝非锁定 SHA-1 的 ROM、符号文件或非锁定 commit 的 pret 源码。
