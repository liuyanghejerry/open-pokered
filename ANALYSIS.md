# Pokémon Red/Blue 逆向工程项目分析

> **Historical reference only.** This document describes the **original Game Boy assembly disassembly** (from pret/pokered) that served as the source material for the Rust rewrite. The Rust project lives under `workspace/` — see `CLAUDE.md` and `AGENTS.md` for the current architecture. This file is kept for reference when understanding original game mechanics.

## 1. 项目概述

本项目是 **Pokémon Red/Blue（口袋妖怪 红/蓝）** 的完整反汇编（disassembly）项目，由 pret 社区维护。它将原始 Game Boy ROM 的机器码完全还原为可读的汇编源码，并能重新编译生成与原版 ROM **逐字节完全一致** 的输出。

### 关键数据

| 指标 | 数据 |
|------|------|
| 目标平台 | Game Boy (DMG) / Game Boy Color (CGB) |
| CPU | Sharp LR35902 (类Z80, 8位, 4.19 MHz) |
| ROM 大小 | 1 MB (32 个 16KB bank) |
| RAM | 8 KB WRAM + 8 KB HRAM |
| 汇编文件数 | 1,922 个 `.asm` 文件 |
| 代码总行数 | ~173,000 行汇编代码 |
| 图形资源 | 668 个 PNG/BPP 文件 |
| 地图文件 | 228 个 `.blk` 地图块文件 |
| 汇编器 | RGBDS (Rednex Game Boy Development System) v1.0.1 |
| 构建产物 | `pokered.gbc`, `pokeblue.gbc`, `pokeblue_debug.gbc` |

---

## 2. 目录结构

```
pokered/
├── home/           # ROM Bank 0 (常驻内存) - 66 个核心系统文件
├── engine/         # 游戏引擎代码 - 185 个文件, 14 个子系统
├── data/           # 游戏数据定义 - 17 个子目录
├── gfx/            # 图形资源 - 668 个文件
├── constants/      # 汇编常量 - 37 个文件
├── macros/         # 汇编宏 - 11 个文件
├── text/           # 游戏对话文本 - 300+ 个文件
├── scripts/        # 地图脚本 - 750+ 个文件
├── maps/           # 地图数据 - 228 个 .blk 文件
├── audio/          # 音频/音乐 - 多个子目录
├── ram/            # RAM 内存布局定义
├── tools/          # C 语言构建工具
├── vc/             # Virtual Console 补丁
├── main.asm        # 主 ROM 结构入口
├── home.asm        # Home bank 入口
├── audio.asm       # 音频 bank
├── maps.asm        # 地图定义
├── text.asm        # 文本内容
├── ram.asm         # RAM 定义
├── includes.asm    # 主包含文件
├── layout.link     # ROM 内存布局/链接脚本
└── Makefile        # 构建系统
```

---

## 3. 核心架构

### 3.1 ROM Bank 结构

Game Boy 使用 bank 切换机制来访问超过 32KB 的 ROM 空间：

| Bank | 大小 | 内容 |
|------|------|------|
| **ROM0 (Bank 0)** | 16 KB 固定 | 核心系统：中断处理、文本渲染、精灵管理、joypad、bank 切换 |
| **Bank 1** | 16 KB | 混合引擎代码、音频头 |
| **Bank 2** | 16 KB | 音频引擎 1、音效/音乐头 |
| **Bank 3** | 16 KB | 手柄输入、野生精灵、物品系统 |
| **Bank 4-5** | 32 KB | NPC 精灵、字体、战斗引擎 1-2 |
| **Bank 6-7, 11, 14-18, 1A, 1D** | 地图数据 (11 banks) | 地图、瓦片集、隐藏事件 |
| **Bank 8** | 16 KB | 音频引擎 2、Bill的PC |
| **Bank 9-D** | 80 KB | 精灵图形 (5 banks, 151只精灵的压缩图) |
| **Bank F** | 16 KB | **战斗核心** (伤害计算、属性克制) |
| **Bank 13** | 16 KB | 训练师头像图形 |
| **Bank 19-1B** | 48 KB | 瓦片集图形 |
| **Bank 1F** | 16 KB | 音频引擎 3 |
| **Bank 20-2C** | 208 KB | 文本数据 (13 banks) |

### 3.2 RAM 内存布局

```
$C000-$C0FF : 音频 RAM
$C100-$C1AF : 精灵状态数据 (16个精灵 × 0x10字节)
$C1B0-$C1FF : OAM 缓冲区 (40个精灵对象)
$C200-$C3FF : 瓦片地图
$C400-$C7FF : 大地图滚动缓冲区
$C800-$DFFF : 游戏变量 (WRAM)
$DF00-$DFFF : 栈

SRAM (电池备份存储, 4 banks, 32KB):
  Bank 0: 精灵缓冲区
  Bank 1: 主存档数据 (玩家、物品、图鉴)
  Bank 2-3: 存储箱 1-12
```

---

## 4. 游戏子系统分析

### 4.1 战斗系统 (`engine/battle/` - 25+ 文件)

战斗系统是最复杂的子系统，分布在多个 ROM bank 中。

**核心文件：**
- `core.asm` (~4000行) - 战斗主循环、回合执行
- `trainer_ai.asm` - 训练师 AI 招式选择
- `move_effects/` (20个文件) - 各招式效果实现
- `animations.asm` - 战斗动画
- `wild_encounters.asm` - 野生精灵遭遇
- `experience.asm` - 经验值计算
- `end_of_battle.asm` - 战后清理

**AI 系统 (4层修正)：**
1. 状态招式优化：目标已有状态时降低状态招式优先级
2. 能力提升偏好：倾向使用增益招式
3. 属性克制：优先使用超有效招式
4. 训练师专属策略（如：四天王会治疗、馆主使用道具）

**招式数据结构 (每招 6 字节)：**
```
偏移0: 动画ID
偏移1: 效果类型 (50+ 种效果)
偏移2: 威力 (0-255)
偏移3: 属性 (15种属性)
偏移4: 命中率 (0-100%)
偏移5: PP (0-40)
```

### 4.2 大地图系统 (`home/overworld.asm` + `engine/overworld/`)

**主循环 (`OverworldLoop`)：**
```
1. 帧同步 (DelayFrame × 2)
2. 加载调色板
3. 处理手柄输入
4. 检查特殊状态 (Safari Zone、传送等)
5. 分支处理：
   - 行走动画未完成 → 继续动画
   - START键 → 显示开始菜单
   - A键 → 检测NPC/告示牌/隐藏事件
   - 方向键 → 移动/碰撞检测
   - 野生遭遇触发 → 进入战斗
```

**关键文件：**
- `map_sprites.asm` - NPC 精灵管理、VRAM 插槽分配
- `movement.asm` - 路径跟随
- `wild_mons.asm` - 遭遇系统
- `doors.asm`, `elevator.asm` - 传送点
- `cut.asm`, `push_boulder.asm` - 秘传技效果

### 4.3 精灵管理系统 (`engine/pokemon/`)

**精灵基础数据 (每只 22 字节)：**
```
偏移0:  图鉴编号 (1字节)
偏移1-5: HP, 攻击, 防御, 速度, 特殊 (各1字节)
偏移6-7: 属性1, 属性2 (各1字节)
偏移8:  捕获率 (1字节)
偏移9:  基础经验值 (1字节)
偏移10: 精灵图尺寸 (1字节)
偏移11-14: 正面/背面图指针 (各2字节)
偏移15-18: 初始招式 (4个, 各1字节)
偏移19: 经验增长类型 (1字节)
偏移20-21: TM/HM 兼容位域 (7字节)
```

**队伍精灵结构 (每只 44 字节 / 0x2C)：**
```
偏移0:    种族ID
偏移1-2:  当前HP
偏移3:    等级
偏移4:    状态异常
偏移5-6:  属性1, 属性2
偏移7:    捕获率
偏移8-11: 4个招式
偏移12-13: 原始训练师ID
偏移14-16: 经验值 (3字节, 大端序)
偏移17-26: 5项努力值 (各2字节)
偏移27-28: 个体值 (DVs, 2字节)
偏移29-32: 4个招式PP
偏移33:   等级 (队伍专用)
偏移34-43: 5项能力值 (各2字节, 计算得出)
```

**关键文件：**
- `evos_moves.asm` - 进化/招式学习
- `experience.asm` - 经验值与等级
- `status_ailments.asm` - 状态异常
- `add_mon.asm` / `remove_mon.asm` - 队伍管理

### 4.4 菜单系统 (`engine/menus/` - 15+ 文件)

- `main_menu.asm` - 标题画面菜单 (继续/新游戏/设置)
- `start_sub_menus.asm` - 暂停菜单分支
- `party_menu.asm` - 队伍选择
- `pokedex.asm` - 图鉴浏览
- `save.asm` - 存读档系统
- `naming_screen.asm` - 命名输入
- `pc.asm` / `players_pc.asm` / `oaks_pc.asm` - PC 系统

**存档系统 (校验和验证)：**
```
SaveGameData:
  1. SaveMainData      → 玩家名、物品、图鉴
  2. SaveCurrentBoxData → 当前存储箱
  3. SavePartyAndDexData → 队伍精灵 + 完成标记
  4. 计算校验和 (所有字节求和取反)

LoadSaveFile:
  1. 加载主数据
  2. 校验和验证 (不匹配则重试一次)
  3. 失败 → 显示 "File Data Destroyed"
```

### 4.5 物品系统 (`engine/items/` - 9 文件)

- `inventory.asm` - 背包/存储管理
- `item_effects.asm` - 使用效果 (40种物品各自的函数指针)
- `tms.asm` - TM/HM 教学
- `itemfinder.asm` - 寻宝器

**物品分类：**
- 精灵球 (4种) → `ItemUseBall` 捕获逻辑
- 药品 (12种) → `ItemUseMedicine` 回复逻辑
- 进化石 (5种) → `ItemUseEvoStone` 进化逻辑
- 战斗增益 (4种) → `ItemUseXStat` 提升逻辑
- 关键道具 → 特殊效果

### 4.6 音频系统 (`audio/`)

- 3 个音频引擎副本分布在 Bank 2, 8, 1F
- 4 声道：2× 方波, 1× 波形, 1× 噪声
- ~30 首音乐曲目 + 100+ 音效
- 自定义音序器格式
- 状态变量：`wAudioROMBank`, `wNewSoundID`, `wAudioFadeOutControl`

### 4.7 连接/对战系统 (`engine/link/`)

- 串口通信 (Cable Club)
- 交换/对战菜单
- 数据同步协议 (前导码 $9C + 10个随机数)
- 支持交易中心和斗技场

### 4.8 图形系统

- 瓦片地图渲染 (160×144 像素, 4色)
- OAM DMA 传输
- 精灵压缩/解压 (RLE/LZ77 变种)
- PNG → 1bpp/2bpp 构建管线

### 4.9 数学工具 (`engine/math/`)

- `random.asm` - 16位伪随机数生成器 (基于硬件定时器 rDIV)
- `multiply_divide.asm` - 16位乘法 (移位-加法算法)
- `bcd.asm` - BCD 除法 (金钱计算)

---

## 5. 数据格式总览

| 数据类型 | 大小 | 位置 | 关键字段 |
|----------|------|------|----------|
| 精灵基础数据 | 22 字节 | `data/pokemon/base_stats/` | 图鉴号, 5项能力, 2属性, 捕获率, 经验, 图, 初始招式, 成长率, TM兼容 |
| 招式定义 | 6 字节 | `data/moves/moves.asm` | 动画, 效果, 威力, 属性, 命中, PP |
| 队伍精灵 | 44 字节 | RAM | 种族, HP, 等级, 状态, 招式, 经验, 努力值, 个体值, PP, 能力值 |
| 存储箱精灵 | 33 字节 | SRAM | (同队伍但无计算能力值) |
| 野生遭遇 | 21 字节 | `data/wild/maps/` | 遭遇率, 10×(等级+种族) |
| 训练师队伍 | 可变 | `data/trainers/parties.asm` | 等级, 1-6只精灵, 终止符 |
| 属性克制表 | 3 字节/条 | `data/types/type_matchups.asm` | 攻击属性, 防御属性, 效果 |
| 地图头 | 可变 | `data/maps/headers/` | 尺寸, 瓦片集, 音乐, 连接 |
| 物品名称 | 可变 | `data/items/names.asm` | 字符串 |
| 存档数据 | ~32KB | SRAM | 玩家数据, 队伍, 12个箱子, 校验和 |

---

## 6. 版本差异 (Red vs Blue)

通过条件编译实现：
- `-D _RED` / `-D _BLUE` 预处理器标志
- 野生精灵分布不同 (如：绿色森林中 红版=独角虫, 蓝版=绿毛虫)
- 可获得精灵种类不同
- 标题画面精灵不同
- Virtual Console 版本有额外补丁

---

## 7. 构建系统

```bash
make            # 构建所有 ROM (red, blue, blue_debug)
make red        # 仅构建 红版
make blue       # 仅构建 蓝版
make compare    # 验证 SHA1 校验和
make clean      # 清除所有构建产物
```

**构建流程：**
```
PNG 图形 → (rgbgfx) → .1bpp/.2bpp
ASM 源码 + 常量 + 宏 → (rgbasm) → .o 目标文件
目标文件 → (rgblink + layout.link) → .gbc ROM
ROM → (rgbfix) → 修正头部校验和
```

---

## 8. 项目特点与挑战

### 优势
- **完全逆向**：每一行代码都有对应的汇编实现
- **字节级精确**：可重建完全一致的 ROM
- **模块化良好**：子系统划分清晰
- **数据驱动**：大量游戏内容由数据表定义

### 重写挑战
- **Bank 切换**：原始代码高度依赖 Game Boy 的 bank 切换机制
- **硬件依赖**：直接操作 GPU、音频、串口等硬件寄存器
- **精确计时**：帧同步、V-Blank 中断
- **位级操作**：大量位域、nibble操作
- **指针算术**：16位地址空间下的复杂指针操作
- **压缩算法**：自定义的精灵/数据压缩格式
- **状态机**：许多系统实现为隐式状态机
