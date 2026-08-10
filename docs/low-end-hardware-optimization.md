# pokered 低端硬件优化路线(始于 Game Boy 移植可行性调研)

> 分支:`research/low-end-hardware-optimization` · 日期:2026-08
> 本文件从"将 pokered 移植到原版 Game Boy(DMG/CGB,SM83 8-bit CPU,8 KiB WRAM +
> 8 KiB VRAM,32 KiB–1 MiB ROM 卡带)"的可行性调研出发,但**结论是聚焦低端硬件优化**
> 而非 GB 移植:GB 编译器通路不存在(见 §一),而调研揭示的架构问题——堆分配数据模型、
> Boa 解释器依赖、92 KiB RGBA 帧缓冲——对所有低端目标(复古掌机、受限 WASM、嵌入式)
> 都是共同障碍。§四/§五的执行记录即为此优化路线。GB 特化细节保留在文中作为参照
> (这是 8 位目标的极端基准)。

## 结论(TL;DR)

**直接移植不可行,且差距是数量级的。** 两个互相独立的硬伤:

1. **没有编译器通路**:主线 LLVM/rustc 从未合入 SM83 后端(任何 Tier 都没有,
   `rustc --print target-list` 中不存在;对比 AVR 已上游化)。唯一活着的端到端方案
   `zlfn/rust-gb`(fork 的 rustc + fork 的 LLVM + cargo-gb)是单人维护的 alpha 项目,
   作者自述"未在本人开发环境之外测试过"。
2. **代码架构不匹配**:即使编译器问题解决,pokered-core 的运行时模型(堆分配数据模型、
   92 KiB RGBA 帧缓冲)也远超 GB 硬件预算。
   ~~仅 Boa 一个依赖的 rlib 就是整张卡带 ROM 的 25 倍以上~~ —— **此判断已被后续调研
   修正**:pokered 对 Boa 的使用仅是基础剧情脚本引擎,生成的 JS 是刻意简单的子集,可在
   构建期移出 Boa,详见[§四](#四后续更新去-boa-化的可行性验证2026-08同分支)。

**可行路径不是"移植这份代码",而是"把这份仓库当参考实现与数据管线"**,用 GB 原生技术
(GBDK-2020/C 或 RGBDS 汇编)重写,复用本仓库的数据提取、SRAM 布局与 Gen-1 机制考据。
若目标放宽到 GBA,`agb` crate + 主线 nightly rustc 是现成方案。
若坚持"Rust 逻辑上 GB"的路线,去 Boa 化(§四)+ 数据模型定容化(§五)是两个前置
重构,且二者对现代平台(WASM/移动端体积、确定性)也有独立收益。

---

## 一、工具链现状(2024–2026)

### 1.1 LLVM SM83 后端:主线不存在

- 主线 LLVM 从未合入 SM83/GBZ80 后端。rustc 官方 platform-support 页面在任何 Tier
  中均无 `sm83`/`gbz80` 目标;本地 `rustc --print target-list` 验证一致。
  <https://doc.rust-lang.org/nightly/rustc/platform-support.html>
- 历史尝试均已死亡:
  - `euclio/llvm-gbz80`(2015 年代,2022-02 归档)
    <https://github.com/euclio/llvm-gbz80>
  - `Bevinsky/llvm-gbz80` + clang-gbz80(较认真的 Clang 移植,最后提交 2018-10)
    <https://github.com/Bevinsky/llvm-gbz80>
- **2025–2026 的活项目**是全新实现,非旧项目复活:
  - <https://github.com/llvm-z80/llvm-z80> — GlobalISel 的 Z80 后端,含 SM83 target
    (甚至 `sm83-nintendo-none-sdcc` 模式),最后提交 2026-07。
  - <https://github.com/llvm-z80/rust-z80> — 配套 rustc fork,最后提交 2026-07。
- 独立佐证 Alex Jokela 的 Rust-on-Z80 系列(2025-12):仅为 Z80 构建
  `compiler_builtins` 峰值内存 169 GB;`core` 始终无法完整构建(GlobalISel legalizer
  缺口、无软浮点合法化、寄存器分配器在 phi 节点上耗尽)。作者结论:实际可用产出仅限
  "几 KB 手写汇编级别的程序"。
  <https://tinycomputers.io/posts/rust-on-z80-an-llvm-backend-odyssey.html>

### 1.2 rust-gb 及其后继

- `simias/rust-gb`(2015,fork rustc + 定制 LLVM):仓库已 404,无维护血脉。
- **现役唯一项目:<https://github.com/zlfn/rust-gb>**(2024-09 创建,2026-06 仍活跃,
  约 273 star)。管线:rust-z80(fork rustc)→ LLVM IR → llvm-z80 后端 → `cargo gb build`
  (16 KiB bank 打包 + 自带链接器 + 卡带头)。附带 `gbdk-sys`(GBDK-2020 运行时绑定)、
  `gb-bank`(GhostCell 编译期安全的 ROM bank 切换)、图片转换器与示例 ROM。
  README 原文警示:*"This project is still a work in progress, and I haven't tested it
  outside of my development environment."*
- FOSDEM 2026 有相关综述演讲:<https://fosdem.org/2026/schedule/event/W3UFSK-rust-game-boy/>

### 1.3 SDCC / 转译路线

- SDCC 的 GB 后端成熟且在维护(GBDK-2020 4.1.0+ 中 `gbz80` 更名 `sm83`;
  GBDK-2020 持续发版至 2025 年末)。<https://github.com/gbdk-2020/gbdk-2020>
- Rust→C→SDCC 先例:旧版 rust-gb 管线(AVR target LLVM IR → llvm-cbe → SDCC → GBDK),
  以及其灵感来源 `MartinezTorres/z80_babel`(2022-03 后停更);Go 生态有 gbdk-go
  走同样模式。证明可行,但所有尝试过的项目都已休眠或转向。
- mrustc 只能引导到 Rust 1.54 且官方声明"不适合日常使用",未有人接过 SDCC,不推荐。
  <https://github.com/thepowersgang/mrustc>

### 1.4 社区实际基线

今天做 GB homebrew 的现实选择是 **GBDK-2020(C)** 或 **RGBDS 汇编**(pret 反编译项目
所用),音乐用 hUGEDriver。Rust 生态在 GB 上只有 alpha 级实验。

### 1.5 路径排序

| 路线 | 状态 | 评价 |
|------|------|------|
| 不编译 Rust 到 SM83,共享逻辑/数据,C/asm 重写 | 生态标准做法 | 零工具链风险,推荐 |
| zlfn/rust-gb(rust-z80 + llvm-z80) | 活跃但 alpha、单人、fork 一切 | 可做实验/demo,不可做产品 |
| Rust → llvm-cbe → C → SDCC | 先例存在,项目均休眠 | 需要自己复活管线 |
| mrustc → C → SDCC | 无人做过,Rust ≤1.54 | 不推荐 |
| 主线 LLVM/rustc 支持 | 不存在,无合入迹象 | — |
| (放宽到 GBA)`agb` + `thumbv4t-none-eabi` | 主线 Tier 3 no_std,有已发售游戏 | 务实答案 |

## 二、代码库适配度分析

对 `dotzuki-engine`(16.5k 行)与 `pokered-core`(约 94k 行)的实测:

### 2.1 std 依赖:表面轻微,实质深重

- 两个 crate 均无 `#![no_std]`。`std::` 路径大多是 `std::fmt`/`std::hash` 这类 core
  兼容写法,机械改名即可。
- 真正的问题:**`alloc` 类型就是核心数据模型**。`Vec` 在 pokered-core 出现 644 次/91 个
  文件;`Pokemon.nickname: Option<String>`(`battle/state.rs:92`);事件标志用
  `HashMap<String, bool>`(`overworld/event_flags.rs:7`)——原版是几百字节的位压缩数组,
  此处 RAM 开销约为原版的百倍。无任何分配器抽象,移植 = 全局 bump 分配器或全部改写为
  定长数组/`heapless`。

### 2.2 依赖树:Boa 是最大依赖,但可移除(见 §四)

- pokered-core 解析出 **172 个唯一直接/传递 crate**(workspace 全锁 580 个)。
- **boa_engine 0.20 是 pokered-core(via dotzuki-engine-script)与 pokered-data 的
  非可选依赖**:全部 248 张地图的 NPC 对话/事件/过场都编译成 JS 在 Boa 里跑
  (`overworld/screen.rs:595` 持有常驻 `ScriptEngine`)。Boa 是带 GC 堆、f64 Number
  语义、ICU 数据的完整 ECMAScript 解释器,其 release rlib 为 **24–33 MB**——
  单这一个依赖就是 1 MiB 卡带预算的 25 倍以上。
  **但后续调研(§四)证明:Boa 在 pokered 中只承担了基础剧情脚本引擎的角色,
  生成的 JS 是刻意简单的子集,可在构建期改为 AST 解释/Rust codegen 将其整体移除。**
- `rand` 使用 OS 熵源(`StdRng::from_entropy()` 等),需换成 rDIV 种子 LFSR(原版做法)。
- serde/thiserror/log/strum 均有 no_std 模式,无碍。

### 2.3 内存与数据体量

- 帧缓冲:160×144 RGBA ≈ **92 KiB**,是 GB 全部 VRAM+WRAM 的 11 倍。
- 存档:好消息——`save/serialization.rs` 按原版 4×8 KiB = 32 KiB SRAM 布局序列化,
  **持久化状态本身就是 GB 尺寸**;但运行时工作状态远大于 8 KiB WRAM。
- 数据:`.blk` 地图块仅 23.7 KB(就是原版数据),但 map.json 735 KB、`.scene` 脚本
  965 KB 等源数据共数 MB 冗余文本。信息量与原版 1 MiB ROM 相同——需要 build-time
  打包成二进制表,现有 build.rs 生成器架构(2,949 行,已产出静态表)正好适合做这件事,
  只是当前产出的是面向 32/64 位主机的 Rust 字面量,而非 bank 可寻址的数据块。
- 二进制:pokered-app release 21.7 MB;原版 1 MiB。

### 2.4 数值假设:大体友好

- 伤害公式为纯整数 u8/u16/u32(`battle/damage.rs:95+`),f32 只出现在测试里。
- SM83 无硬件 32 位乘除,编译器会生成软件例程——回合制游戏可接受。
- 浮点例外:`experience/stats.rs:27` 的 `(stat_exp as f64).sqrt()`(需整数 sqrt)、
  过场/标题的 f32 特效、以及 **Boa 把所有脚本数值变成 f64**。

### 2.5 渲染耦合:干净

pokered-core 不含任何渲染代码;绘制在 pokered-renderer / pokered-app 的 render 模块,
通过 `RenderData` trait 读状态。GB 实现可以整体替换渲染层为原生 tile/OAM 代码。
(但渲染代码直接读 core 的公开字段,且心智模型是 RGBA 像素 blit 而非 GB 图块硬件。)

### 2.6 可复用度分级

- **中度改造可复用(约占 pokered-core 20–30% 行数)**:战斗公式、属性/伤害/克制、
  队伍数据结构骨架、SRAM 兼容的存档序列化。仍需 `String`→定长数组、`Vec`→heapless。
- **大手术才能复用**: overworld 逻辑(事件标志 HashMap、地图数据 owned Vec 化、
  `PathBuf`/fs 渗入 screen 构造函数)、数据管线(build.rs 改发 bank 二进制)。
- **本质上绑定宿主**:`rand` 的 OS 熵源(小改,换 rDIV 种子 LFSR)、`image` crate、
  `std::fs` 存档、92 KiB RGBA 帧缓冲。~~脚本架构(Boa)~~ 原列于此,已被 §四推翻——
  Boa 可整体移除;剩余的是定容化工作(§五),量大而非无解。

## 三、现实路径建议

1. **(推荐)参考实现路线**:GB ROM 用 GBDK-2020 或 RGBDS 重写,本仓库提供:
   数据提取管线(build.rs 生成器、`.blk`/map JSON)、SRAM 布局(`save/serialization.rs`)、
   Gen-1 机制考据(含刻意保留的原版 bug 清单)。工具链风险为零。
2. **实验路线**:用 zlfn/rust-gb 做技术验证 demo(如战斗引擎核心跑在真机上),
   先决工作是把战斗核心抽成 `no_std` + 定长数据的子集 crate。适合做 spike,不适合立项。
3. **GBA 路线**:若"任天堂掌机"即可,`agb` crate(主线 nightly,`thumbv4t-none-eabi`
   Tier 3,已有商业游戏)是现成方案,pokered-core 的 std 改造工作量仍在,但编译器
   问题完全消失。<https://github.com/agbrs/agb>
4. **不值得做**:复活 llvm-cbe→SDCC 管线、等待主线 LLVM 支持、mrustc。

## 四、后续更新:去 Boa 化的可行性验证(2026-08,同分支)

**结论修正:§2.2 原称 Boa 为"一票否决项",经对 DSL 管线的实测,该判断过于悲观。**
pokered 对 Boa 的使用仅是"基础剧情脚本引擎",生成的 JS 是刻意简单的子集,可以在
构建期改为 AST 解释(或 Rust codegen),把 Boa 从 pokered 全链路移除。

实测证据:

- 248 个编译产物(`target/.../pokered-data-*/out/scene_js/*.js`)中:0 闭包、0 原型、
  0 eval、0 动态字符串插值、0 for 循环(`@each`/`@variables` 从未使用)。内容仅为顺序
  `await game.x(...)`、`if/else` 原始类型比较、至多一个 `let result = await
  startBattle(...)` 绑定(30 个文件,全是战斗结果)。
- 效果面已是封闭 Rust 协议:所有 `game.*` 调用汇聚为 `ScriptCommand` 枚举(约 68 变体,
  纯 serde 数据)出、`CommandResult`(Void|Bool|Number|Text 四变体)入——JS 值只是
  传输格式。
- pokered-core 接缝窄:约 30 个调用点集中在 `overworld/{screen,update,script_bridge}.rs`
  三个文件;619 行的 `ScriptEffect` 逐帧调度层与引擎无关,可原样复用。
- 移除面:pokered-core、pokered-data(其直接 boa 依赖仅为 `script_api.rs` 的 JS 闭包
  服务)、pokered-app/tui/web 可彻底去 Boa;`dotzuki-engine-dsl` 本就不依赖 Boa(只发射
  JS 文本),退回纯 build 依赖。`dotzuki-runner`/`dotzuki-cli`/`dotzuki-runner-web` 与 wuxia 示例
  因零 Rust 项目的本质要求仍需运行时脚本引擎,workspace 保留 Boa,但 pokered 不再为其
  付出编译/二进制成本。

推荐路线:**不 codegen Rust async fn,而是直接解释 DSL AST(`dotzuki-engine-dsl/src/ast.rs`
的 `StoryStmt`)**——每步产出 `ScriptCommand`,与现有 `tick`/`signal_done` 协议 1:1
对应,无需 executor,并顺带保留 `--scripts-dir` 磁盘热重载(运行时 .scene→AST 而非
→JS)。剩余工作:

1. `VermilionGym/script.scene:66-125` 唯一一处 `@run` 原生 JS(垃圾桶开关谜题,用了
   `globalThis` 持久状态 + `Math.random`)手写移植,或给 DSL 加 `randInt` + 场景局部
   变量原语。
2. 静态注册表替代 `has_function`/`call_function_no_args` 动态派发;`pokecenter.js` 共享
   模块与 `BridgeView` 同步查询(`hasItem`/`getMoney` 等约 20 个)的原生等价物——
   均为机械工作。
3. 注意:AST 解释器若要服务 GB 目标,产出必须是**扁平化/静态表形式**(bytecode 或
   `&'static` 表);堆上树结构仍是分配源(见 §五 chunk I)。

## 五、后续更新:数据模型堆分配问题的拆解(2026-08,同分支)

> **执行状态(2026-08):chunk A/B/D 与索引帧缓冲(§5.4)已在本分支完成并合并
> (worktree 并行开发 + 逐分支 merge,合并后三 crate 测试全绿)。** chunk A 中
> 修正了一个真实错误:原版 wEventFlags 为 `$A00` 位 = **320 字节**,pokered-data 的
> `EVENT_FLAGS_SIZE = 316` 是错的(316 只是已定义标志的跨度,最大位 0x9DA),已改为
> 320 并与存档层对齐(新增编译期断言)。chunk B 顺带修复:未昵称宝可梦的默认名
> 统一走 `lang_data::species_name`,NidoranF/MrMime 等不再显示 Debug 大写拼写。
> 另新增 `.gitignore` 规则防止 gfx 符号链接被误提交。

> **执行状态(2026-08,第二轮):chunk C(队伍/箱子/HoF 定长数组)、去 Boa(DSL AST
> 解释器 + 原生脚本引擎,默认启用,Boa 退居 `script-boa` feature)、索引帧缓冲接线
> (pokered 全链路渲染进打包 2bpp 的 `IndexedFrameBuffer<GbColor>`,92 KiB → 5.7 KiB)
> 已并行完成并合并。**至此 §5.2 的 A/B/C/D 与 §5.4 全部落地;pokered 的游戏运行时
> 堆分配已基本清零(持久状态定长 + 战斗外的运行结构定容)。** 关键实现注记:
> - 去 Boa:解释器语义以 JS 为规范(Boa 生成代码的 hoisting/真值/`===`/短路语义逐项
>   复刻,含 `showEmotionBubble(id, 0)` 这类 Boa 传 `"0"` 字符串的怪癖);VermilionGym
>   的 `@run` 垃圾桶谜题已手写移植为原生命令;`--scripts-dir` 热重载原生可用。
> - 索引帧缓冲:渲染代码经 RGBA 门面逐像素量化进 2bpp 缓冲,淡入淡出/闪光改为
>   调色板寄存器操作(`remap_shades`/`scale_shades`/`apply_bgp`);战斗转场两张 92 KiB
>   临时缓冲消失;firered/dotzuki-runner 真彩色路径保留 RGBA 不动。
> - 遗留:pokered-data 的 `boa_engine` 直接依赖仅剩 `script-boa` feature 需要,全量
>   移除待 feature 删除;AST 仍是每地图 `Vec<StoryStmt>` 反序列化(chunk I 扁平化);
>   `pokered-runner-web` 的 chunk A 遗留引用已补修。SRAM 字节布局全程未变。

去 Boa 之后,§2.2 遗留的最大障碍是 `alloc` 数据模型。实测结论:**问题比预想的小——
GB 尺寸的定型布局已经以存档格式的形式存在于仓库里**,改造是按类别机械替换,不是重新设计。

### 5.1 关键事实

- `save/serialization.rs` 已按原版 4×8 KiB = 32 KiB SRAM 布局序列化,`GameData`
  (`save/game_data.rs:248-331`)已是 wram 镜像:约 30 个定长数组,仅 5 个小 Vec 字段
  待转换。这证明定容设计可行。
- `Pokemon` 定容化后有原版尺寸直接对应:`PARTY_STRUCT_SIZE=44`、`BOX_STRUCT_SIZE=33`
  (`ser_pokemon.rs:12-13`),两个 `Option<String>` 名字(各 ≤10 字符)对应原版 11 字节
  charmap 编码,`serialize_name` 已在存档层做同样的事。
- 事件标志最夸张:运行时 `HashMap<String,bool>`(还重复存了两份:`EventFlags` +
  `SaveData.script_flags`)最坏约 **60 KB**,而 pokered-data 已定义完整的位数组映射
  (`EVENT_FLAGS_SIZE = 316` 字节,507 个标志的 `bit_index/byte_offset/bit_mask` 均为
  const fn)。一个数量级是 200 倍。(注意 316 vs save 层 `NUM_EVENTS_BYTES = 320` 的
  不一致,需在改造时对齐。)
- 地图数据已是 `&'static` 源(`get_block_data`/`get_map_json`),加载时 `.to_vec()` 成
  owned 副本纯属习惯;改为借用 + `set_block` 脏覆盖层即可。
- 战斗侧:每场战斗克隆双方整个队伍(`BattlerState.party: Vec<Pokemon>`);战斗文本是最热
  分配源(仅 `battle/mod.rs` 就 77 个 `format!`);engine 的 stack 规则层已是零分配
  (fn 指针 + `&'static` hooks),只需给容器加容量上限。

### 5.2 改造分块(每块独立可交付,对 WASM/移动端也有独立收益)

| Chunk | 内容 | 触及面 | 规模 |
|---|---|---|---|
| A | 事件标志 → `[u8; 316]` 位数组,删除 `script_flags` 重复存储 | pokered-core 约 40 调用点;需 DSL 编译期把标志名解析为位索引 | ~800–1,200 行 |
| B | 名字 → `[u8; 11]` charmap;`Pokemon` 变为可 `Copy` | 仅 pokered-core | ~400–600 行 |
| C | 队伍/箱子/HoF → 定长数组(6/20×12/50,原版上限) | 仅 pokered-core | ~600–900 行 |
| D | 背包/PC 道具 → 定长槽位 | 波及 dotzuki-engine 的 `Inventory<I>` pub 字段 | ~300–500 行 |
| E | MapData → `&'static` 借用 + 脏覆盖 | 波及 engine `MapData` pub 字段 | ~300–500 行 |
| F | 战斗文本 → 类型化消息枚举 + 定长缓冲 | pokered-core + pokered-app 的 `battle_i18n.rs`(目前靠匹配英文字符串做翻译,类型化后反而更干净) | ~1,000–1,500 行 |
| G | 战斗借用队伍 + 有界 arena | engine battle pub 字段 | ~400–700 行 |

建议顺序 A→B→C→D(A、B 让 `Pokemon` 可 `Copy`,解锁 C),再 E/F/G。A–D 完成后持久
状态约为固定 ~15 KB、零堆分配;全部完成后工作集:全箱子常驻约 20 KB,按原版方式
bank 化约 7–9 KB, comfortably 低于存档格式已对准的 32 KiB。

### 5.3 残余障碍(去 Boa + 定容化之后)

1. `rand` OS 熵 → rDIV 种子 LFSR(原版做法),小改。
2. 帧缓冲/渲染模型 → GB tile/VRAM(chunk H,真正 GB 目标才需要;索引化后几乎直译,
   见 §5.4)。
3. **编译器通路本身**(§一)——这是唯一仍然"无解"的部分,主线 LLVM 无 SM83 后端。

### 5.4 帧缓冲索引化:RGBA 只是终点格式(2026-08,同分支)

针对"92 KiB RGBA 帧缓冲能否优化成真正的灰阶"的专项调研,结论:**可以,且比数据模型
各 chunk 都顺——内部渲染管线本来就已经是 2bpp 索引 + 调色板,RGBA 只存在于最后一张
帧缓冲上。**

现状证据:

- `dotzuki-renderer/src/palette.rs` 已有完整的 GB 原生色彩模型:`GbColor`(2-bit 四阶灰)、
  `Palette`(含 BGP 寄存器仿真 `from_bgp_register`、white-out 状态),并为 firered demo
  准备了 `GbaColor`(4-bit 16 色)——`Palette` 已对色深泛型化(`ColorIndex` trait)。
- 图块资源按 GB 格式存储:`png_to_2bpp`(`pokered-renderer/src/resource.rs:104`)、
  `TileSet::from_2bpp`,字体 1bpp。真彩色路径 `RgbaTileSet` 仅 `pokered-app/src/demo.rs`
  使用,主游戏不碰。
- RGBA 只出现在 `FrameBuffer`(160×144×4 = 92,160 B)与边缘消费者(native/web 的
  `pixels` 纹理、TUI halfblocks、PNG 截图)。

改造方案:新增 `IndexedFrameBuffer<C: ColorIndex>`(不动 engine 的通用 `FrameBuffer`——
dotzuki-runner 的 320×240 真彩色项目仍在用),2bpp 打包 `[u8; 5760]`(与 GB VRAM 图块同构)
或 u8 索引 `[u8; 23040]`,调色板推迟到上屏时应用(native/web 查表展开直接写 GPU 帧;
TUI/PNG 各加一个转换函数)。

波及面(实测):

- 直接操作 `fb.data` 字节的仅 **8 处**,全在 pokered-app render(淡入淡出、战斗闪光)。
  索引化后这些应改为**调色板寄存器操作**——这正是 GB 硬件做淡入淡出的方式,
  `PaletteState` 已有现成支撑,比逐像素 RGBA 运算更原教旨也更快。
- 走 `set_pixel`/`fill_rect`/`copy_line` 正常 API 的 22 个文件可不动(保持签名,
  `Rgba` 入参内部量化到调色板项,或改 `GbColor` 入参)。
- `battle_transition.rs` 转场峰值 3×92 KB → 索引化后约 17 KB。

收益:单帧 92 KB → 5.7 KB(2bpp),每帧上屏带宽同比例下降(WASM 的 JS↔wasm 拷贝
直接受益);对真 GB 目标,2bpp 索引缓冲与 VRAM tilemap 1:1 对应,渲染代码几乎直译。

### 5.5 对 dotzuki 引擎通用性的影响评估(2026-08,同分支)

结论:**按上述分工设计,通用性基本不受损——前提是守住"限制进参数、不进引擎类型"
这条线。**逐项:

- **去 Boa(AST 解释器):对通用性是加分,但有一个真实风险。** 解释器落在
  dotzuki-engine-dsl,`ScriptCommand`/`CommandResult` 协议本就在引擎侧、与游戏无关;
  `PokemonScriptApi` 变纯 Rust 是游戏侧资产,不碰引擎。dotzuki-runner 将来也可切到 AST
  解释器,WASM bundle 直接瘦 20+ MB,对零 Rust 项目是净收益。**风险是语义分裂**:
  Boa 运行时与 AST 解释器并存期间,`@if` 求值(f64 vs 整数)、`@run` 原生 JS、运算符
  优先级等角落会悄悄 drift。缓解:AST 解释器定为**规范语义**,Boa 降级为 legacy/dev
  路径,场景测试两边跑。
- **Chunk A/B/C(标志、名字、队伍):零影响。** 全在 pokered-core 内部;引擎的
  `EventFlags`/`MonsterInstance` 等通用类型原样保留,pokered 只是不再包它们——这正是
  provider 模式的设计意图。
- **Chunk D/E/G(Inventory、MapData、battle 结构):触及引擎 pub 类型,唯一需要小心。**
  坏走法是把 Gen-1 上限(20 格背包、12 箱子)硬编码进引擎类型,引擎会从"任意 DOTZUKI"
  悄悄收窄成"宝可梦-like"。好走法是容量做成 **const generic**(`Inventory<I, const N:
  usize>`)、借用做成 `Cow<'static>` 或存储泛型——与引擎现有 GAT/provider 风格同一
  成语,firered/minimon 改一行类型签名即可过。G 的借用队伍会引入生命周期参数,可用
  "有界 arena 但保持所有权"替代以回避。
- **索引帧缓冲:纯增量。** `IndexedFrameBuffer` 是新类型,`FrameBuffer` RGBA 原样留给
  dotzuki-runner 真彩色项目;`Palette` 已通过 `ColorIndex` 泛型化(firered 16 色路径证明了
  该抽象)。引擎多了一项能力(索引渲染),可覆盖 NES/SNES 风格项目——通用性反而变宽。

视角总结:本轮改造改的是引擎的**平台轴**(更小内存、无 JS 依赖、可 no_std),而非
**游戏轴**(数据/规则泛型化),两轴基本正交;平台轴变宽本身就是通用性("任意经典
DOTZUKI"若包含"跑在复古级硬件上",这些改动是把引擎往宣言方向推)。

三条纪律:

1. 容量上限永远以 const generic/参数形式进引擎,Gen-1 数字只出现在 pokered-data;
2. AST 解释器是唯一规范语义,Boa 路径冻结不再加特性,场景测试两边跑;
3. 引擎 pub 类型的破坏性改动(D/E/G)以 firered/minimon/dotzuki-template 能小改通过为
   验收标准。

### 5.6 Review 结果与遗留清单(2026-08,同分支)

对 `master..research/low-end-hardware-optimization` 全量 diff 的双路 review(核心逻辑 + 前端/编辑器)
结论:**ship-with-fixes**。review 发现的问题已修复:

- **F1(已修)**:原生共享场景(pokecenter)被 `load_map` 的 `functions.clear()` 清掉,
  共享模块机制实际是死的——改为共享函数键豁免清理,地图自有同名 storyline 覆盖共享,
  新增回归测试。
- **F2(已修)**:热注入/编辑器注入一次后污染 AST provider(`scenes` 非空即全量遮蔽
  embedded),导致其他 247 张地图脚本全部失效——`SceneAstProvider` 新增 `disk_mode`
  区分 `--scripts-dir` 全量模式与嵌入式覆盖模式,缺键回退 embedded,新增回归测试。
- **Android/iOS(已修)**:fb-wire 未同步两个移动端壳(`fb.data` 直读)——iOS 增加
  `fb_rgba` 展开缓冲,Android 改走 RGBA 门面 + `to_rgba` 上屏。
- **import_flags(已修)**:编辑器标志恢复只写持久层、未达实时脚本引擎——改走
  `set_flag_live` + `apply_hidden_object_flags`;TS 侧 retry/快照导入补恢复。
- **F8(已修)**:`ot_name` 缺省 serde 零填充(会写成 SRAM 控制字节)→ 默认
  `NO_NAME`(0x50 填充)。

遗留(记录在案,暂缓):

- **F3 已知行为**:中文昵称(pinyin 命名屏)在输入时即静默丢弃(编码遇不可编码字符
  截断为物种名)——SRAM 往返后本就会丢,但现在 UI 无反馈。建议后续在命名屏给出提示
  或走 extras sidecar 暂存显示名。
- **F4**:`script-boa` feature 不门控依赖,Boa 仍编译进所有构建;彻底移除需拆分
  dotzuki-engine-script(§四遗留)。
- **F5**:PC/商店菜单渲染路径每帧 `bag.items()` 分配(小,≤20 槽)。
- **F6/F7**:`<TRAINER>` OT 名丢失尖括号;GB 引号字形(0x70)昵称经解码重编码路径会
  截断(导入路径保留原始字节,无损)。
- 渲染侧:淡入淡出调色板现在影响同帧后绘制内容(Start/保存菜单)——更贴近原版
  rBGP 语义,已记录;`Flash::Short` 反相从恒等变为真反相(原版正确);索引化混合
  量化中间色调(仅影响未使用的 `render_layers` 索引路径)。

### 5.7 Review 结果与遗留清单(2026-08,第二轮,同分支)

本轮 review 发现的问题已修复:

- **wuxia-app(已修)**:漏掉 `GameLoop::Fb` 适配——补上索引 fb 分支。
- **共享 pokecenter 场景(已修)**:原生引擎上共享场景遮蔽了各图自有护士/接待员
  storyline——改为地图自有 storyline 优先,共享仅作缺省回退。
- **昵称序列化(已修)**:昵称 serde 缺省值 + 旧存档 `<TRAINER>` 占位 + 0x00 前导
  字节的加固。
- **companion sidecar(已修)**:extras 为空时旧 sidecar 文件/web `pokered.script_flags`
  键不删除,上一存档的 extras 会在下次加载时重新并入——现为空时删除该键/文件。

记录在案的行为变化(有意为之):

- `Flash::Short`(dotzuki-renderer battle_anim/effects.rs)现在执行真实的颜色反相——
  master 的重映射意外是恒等映射;属可见变化,但是向原版语义的保真修复。
- Android 加载屏现经 4 级灰阶调色板渲染——索引帧缓冲的固有量化,接受。

## 附:主要参考链接

- rustc platform support: <https://doc.rust-lang.org/nightly/rustc/platform-support.html>
- zlfn/rust-gb: <https://github.com/zlfn/rust-gb>
- llvm-z80: <https://github.com/llvm-z80/llvm-z80> / <https://github.com/llvm-z80/rust-z80>
- Rust on Z80 系列: <https://tinycomputers.io/posts/rust-on-z80-an-llvm-backend-odyssey.html>
- GBDK-2020: <https://github.com/gbdk-2020/gbdk-2020> · RGBDS: <https://github.com/gbdev/rgbds>
- agb (GBA): <https://github.com/agbrs/agb>
- awesome-gbdev: <https://github.com/gbdev/awesome-gbdev>
