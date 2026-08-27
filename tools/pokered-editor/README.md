# Pokered Editor 使用说明

workspace 游戏编辑器，集成了地图编辑（248 张地图）、训练师队伍编辑、Pokemon/招式数据编辑和存档构造功能。包括 Map（地图查看/编辑）、Script（脚本编辑）、Save（存档状态构造）、Trainer（训练师队伍编辑）、Pokemon（宝可梦数据编辑）、Move（招式数据编辑）、Layout（UI布局编辑）、Pixel（像素画编辑）八个标签页。

**试玩（Playtest）是一个悬浮浮窗**：点击左侧 ActivityBar 的 🎮 按钮可在任意编辑界面打开，
浮窗可拖动、置顶显示，内含两种模式：
- **▶ Play** — 完整通关流程（进度自动保存在浏览器，New Game / Continue / Restart）
- **🧪 Test** — 免存档的临时会话，用于快速验证改动；下方调试条提供文本速度（快/中/慢）、
  加速（1x/2x/4x）、队伍全恢复、游戏存档快照导出/导入

**快速试玩入口**（无需保存即可把当前编辑数据实时注入运行中的游戏并打开浮窗定位到目标）：

| 入口 | 位置 | 效果 |
|------|------|------|
| ⚔ 试玩战斗 | 宝可梦编辑器 | 直接进入对战该宝可梦（Lv5）的野生战斗 |
| 📖 试玩图鉴 | 宝可梦编辑器 | 直接打开该宝可梦的图鉴条目（完整数据 + 叫声） |
| ✨ 试玩进化 | 宝可梦编辑器（进化条目） | 播放当前物种 → 目标物种的进化动画 |
| ⚔ 试玩对战 | 训练师编辑器 / 地图 NPC 详情 | 与该训练师当前队伍对战 |
| ⚔ 试玩招式 | 招式编辑器 | Lv25 测试员使用该招式 vs 野生 Lv25 |
| ▶（每槽位） | 地图编辑器 → Wild Encounters | 触发该野生遭遇槽位 |
| ▶ Test this map | 地图编辑器工具栏 | warp 到当前地图 |
| ▶ In Game | 脚本编辑器 | warp 到脚本所属地图 |
| ▶ 用此存档试玩 | 存档编辑器 | 用构造的存档（队伍/道具/徽章/旗帜）启动游戏 |

Map 编辑器的 **🎮 Game** 模式与 Script 编辑器的 **▶ In Game** 按钮基于
`pokered-runner-web`（wasm 内嵌完整游戏本体）提供所见即所得预览：保存地图/脚本后
实时注入运行中的游戏（脚本热重载、遇敌表覆盖、warp 传送），无需重新构建。

**AI 助手（静态部署也可用）**：左侧 ActivityBar 的 ✨ 按钮打开 AI Assistant。开发模式
（`npm run dev` / Electron）经本地 `/api/ai/*` 后端运行，可读取项目数据、提出编辑提案
（审查后应用）。**静态部署（GitHub Pages）** 时助手改为**浏览器直连**：无需本地后端，
默认提供 DeepSeek profile（OpenAI 兼容，模型 `deepseek-chat`），也可在助手设置（⚙）中
配置 OpenAI / OpenRouter 等任意 OpenAI-compatible 端点；API Key 仅存于浏览器
localStorage（`jrpg-ai-key-<id>`）。静态模式下助手经 IndexedDB delta 层读写项目数据
（读取基线 + 编辑覆盖），提案同样写入 delta——"导出编辑"按钮可备份全部本地改动。

## 启动

```bash
cd workspace/tools/pokered-editor

# 安装依赖（首次使用）
npm install

# 启动开发服务器
npm run dev
```

启动后访问终端输出的地址（默认 `http://localhost:5173`）。编辑器会自动加载全部地图，侧边栏显示进度。

### 构建检查

```bash
# 仅类型检查
npx vue-tsc --noEmit

# 类型检查 + 生产构建
npm run build
```

## 界面布局

```
┌──────────────────┬──────────────────────────────────────┐
│                  │                                      │
│    侧边栏        │          工具栏                       │
│                  │  [View] [Edit Collision] [-] 2x [+]  │
│  地图选择         │──────────────────────────────────────│
│  搜索过滤         │                                      │
│  显示选项         │          地图画布                      │
│  保存/导航        │       （可缩放、可点击）                │
│  { } Script 按钮 │     显示连接箭头和目标地图名            │
│  World Map小地图  │                                      │
│  Passable Tiles  │                                      │
│  实体详情         │──────────────────────────────────────│
│  Map Header编辑   │  脚本编辑器（可折叠面板）               │
│  (BGM/连接编辑)   │  [函数列表] | [CodeMirror 代码编辑区]  │
│  地图信息         │                                      │
│  图例            │                                      │
│                  │                                      │
└──────────────────┴──────────────────────────────────────┘
```

## 基本操作

### 选择地图

- **下拉菜单**：侧边栏顶部的下拉框，按地图 ID 排序列出全部地图
- **搜索过滤**：在搜索框输入地图名称关键字（如 `Pallet`、`Route`、`Gym`），下拉列表实时过滤
- **前后翻页**：点击 ◀ ▶ 按钮或按 `←` `→` 方向键切换上/下一张地图

### 缩放

工具栏中的 `-` / `+` 按钮调整缩放倍数（1x ~ 4x），默认 2x。

### 显示图层

侧边栏复选框控制画布上显示哪些图层：

| 选项 | 说明 |
|------|------|
| Show Tiles | 渲染 tileset 贴图（关闭时按 blockID 着色） |
| Show Collision | 叠加碰撞信息（绿色=可通行，红色=不可通行） |
| Show Warps | 显示传送点（蓝色方块） |
| Show Signs | 显示路牌/告示牌（黄色方块，标记 S） |
| Show NPCs | 显示 NPC（紫色=普通，红色=训练师，绿色=道具） |
| Show Coord Events | 显示坐标触发事件（橙色方块，标记 C） |
| Show Connections | 显示地图连接信息 |
| Show Grid | 显示 block 网格线 |

### 鼠标交互

- **悬停**：画布上移动鼠标显示 tooltip，包含当前 tile 坐标、block 坐标、blockID、tileID、碰撞状态，以及该位置的实体信息（Warp/Sign/NPC/Coord Event）
- **点击 Warp**：如果该 Warp 有目标地图，自动跳转到目标地图；否则选中该 Warp 显示详情
- **点击 Sign / NPC / Coord Event**：选中实体，侧边栏显示详细信息
- **点击空地**：取消选中

### 键盘快捷键

| 按键 | 功能 |
|------|------|
| `←` / `→` | 切换上/下一张地图 |
| `V` | 切换到 View 模式 |
| `E` | 切换到 Edit Collision 模式 |
| `T` | 切换到 Edit Tiles 模式 |
| `Ctrl+S` / `Cmd+S` | 保存当前脚本（脚本编辑器打开时） |
| `Esc` | 取消选中当前实体 |

## 查看地图信息

### Map Info 面板

侧边栏底部 **Map Info** 区域显示当前地图的结构化数据：

- **基本属性**：名称、ID、尺寸（block 数）、tileset、音乐
- **Connections**：上下左右连接的相邻地图（可点击跳转）
- **Warps 列表**：所有传送点坐标及目标（可点击选中/高亮）
- **Signs 列表**：所有路牌坐标及 textId、绑定的脚本函数（可点击跳转到脚本编辑器）
- **Map Scripts**：地图级脚本函数列表（可点击跳转到脚本编辑器）
- **Coord Events**：坐标触发事件及其触发函数（可点击跳转到脚本编辑器）
- **NPCs 列表**：所有 NPC，含精灵名、坐标、训练师/道具信息、脚本绑定（可点击跳转到脚本编辑器）
- **Wild Pokemon**：野生宝可梦遭遇表（分 Red/Blue 版本，显示草地/水面遭遇率和前 5 种）。完整查看与编辑请使用侧边栏的 **Wild Encounters** 编辑器

### Entity Detail 面板

点击地图上的实体后，侧边栏中间显示详细信息：

- **NPC**：精灵名（颜色标识类型）、坐标、移动方式/朝向、视野范围、训练师职业和编号、道具 ID、脚本绑定（可点击函数名打开脚本编辑器并跳转到定义）
- **Sign**：坐标、textId、脚本绑定（可点击函数名打开脚本编辑器并跳转到定义）
- **Warp**：坐标、目标地图名、目标 warp ID，以及"Go to"跳转按钮
- **Coord Event**：坐标、名称（name）、触发函数名（可点击打开脚本编辑器并跳转到定义）

每个 Coord Event 拥有一个唯一的 camelCase `name`（如 `northExit1`），用于在 `.scene` 文件中通过 `@trigger(name = "...")` 引用该事件。名称在同一地图内必须唯一。

### Passable Tiles 面板

显示当前地图 tileset 对应的所有可通行 tile ID。这些数据按 tileset 分组（非按地图），来源于 `collision.rs`。

## 编辑功能

### 编辑 BGM 音乐

侧边栏 **Map Header Editor** 区域提供 BGM 音乐下拉选择器：

- 显示当前地图的音乐名称
- 可选择全部 45 种音乐（从 `music.rs` 中定义的 `MusicId` 枚举）
- 选择后自动标记为"未保存"，点击 Save 按钮保存到 `map.json`

### 编辑地图连接关系

Map Header Editor 区域同时提供地图连接的可视化编辑功能：

- **四个方向**：North / South / West / East
- **每个连接显示**：目标地图名、偏移量（offset）
- **操作按钮**：
  - `Go` — 跳转到目标地图
  - `Edit` — 编辑连接（选择目标地图、设置偏移量）
  - `✕` — 删除连接
- **画布可视化**：开启 Show Connections 后，地图边缘显示绿色箭头指示连接方向和目标地图名

### 编辑野外遇敌（Wild Encounters）

侧边栏的 **Wild Encounters** 面板用于查看和编辑当前地图的野生宝可梦遭遇表：

- **结构**：按版本（Red / Blue）× 地形（Grass / Water）分组
- **Encounter Rate**：每张表的遭遇率（0–255，0 表示该地形无遭遇）
- **遭遇槽位列表**：每个槽位显示编号、出现概率（20/20/10/10/10/10/5/5/5/5%）、等级（1–100）和物种（下拉选择，覆盖全部 151 种宝可梦及 `None`）
- **操作**：
  - `+ Add slot` — 在该表末尾追加一条遭遇（默认 Lv1 None）
  - `Fill to 10` — 自动补齐到原版 ROM 期望的 10 槽位
  - `✕` — 删除某个槽位
  - `Red → Blue` / `Blue → Red` — 在两个版本之间快速复制整张遭遇表
  - `Disable` — 清除当前地图的全部 wild 数据（`map.json` 中变为 `null`）
  - `+ Enable wild encounters` — 当地图本来没有 wild 数据时一键创建空的遭遇表
- **校验提示**：遭遇率 > 0 但槽位数为 0 或非 10 时，编辑器会显示黄色警告
- **保存**：所有改动会标记"未保存"，点击侧边栏 Save 按钮写回 `map.json` 中的 `wild` 字段


### 编辑地图 tile（地图外观）

侧边栏的 **Block Palette** 面板显示当前地图 tileset 的全部 block。操作步骤：

1. 在工具栏点击 **Edit Tiles**（或按快捷键 `T`）切换到 tile 编辑模式
2. 在 Block Palette 中点击想要绘制的 block（高亮表示已选中）
3. 在地图画布上点击或按住鼠标拖动绘制；每次点击会把对应位置的 block 替换为所选 block
4. 完成后点击侧边栏的 **Save** 按钮，改动会写回 `map.blk`

提示：

- 绘制时支持拖动连续涂抹，画布光标变为 `cell` 形状
- 切换地图后会自动加载该地图 tileset 对应的 block 调色板
- Block Palette 上方的按钮可以快速激活 / 显示当前 tile 编辑状态

### 编辑 block / tileset

侧边栏 Block Palette 提供两种深度编辑：

1. **编辑单个 block**（4×4 tile 重排）
   - 在 Block Palette 中**双击**一个 block（或选中后点击 **Edit Block**）打开 Block Editor 模态
   - 左侧显示 4×4 tile 网格，右侧是当前 tileset 的全部 tile 缩略图
   - 在右侧调色板中点击想要的 tile（蓝白选中框），再到左侧 4×4 中点击对应位置即可替换
   - 在右侧调色板中**右键**任一 tile 可切换其"可通行 / 阻挡"状态（绿色 = 可通行，红色 = 阻挡）
   - 关闭对话框后点击侧边栏 Save：blockset (`gfx/blocksets/<name>.bst`) 和 collision overrides (`pokered-data/tileset_passable_overrides.json`) 都会写回磁盘

2. **新建 tileset**（克隆已有 tileset）
   - 在 Block Palette 上方点击 **+ New Tileset**
   - 填入名称（PascalCase 标识符）、显示名、要克隆的 base tileset、分类（室外 / 室内 / 洞穴）
   - 编辑器会复制 base 的 `.bst` 与 `.png` 到新名字（snake_case 文件名），并把元数据写入 `pokered-data/tileset_extras.json`
   - 新 tileset 立即出现在 New Map 对话框的 tileset 下拉中，可以基于它创建地图、然后用 Block Editor 修改它的 block 排布
   - **运行时支持**：`pokered-data` 的 `build.rs` 会在编译时读取 `tileset_extras.json` 与对应的 `.bst` / `tileset_passable_overrides.json`，把每个自定义 tileset 嵌入到游戏运行时。`TilesetId::Custom(slot)` 会在 `from_name` / `blockset_for_tileset` / `is_tile_passable` 等路径上自动生效，新 tileset 的 **block 排布** 与 **tile 碰撞** 在游戏中也会正确生效；palettes、counter / grass tile、动画与门 / warp / spinner 行为继承自所选 base tileset

### 在大世界中新建地图

侧边栏的 **World Map** 面板支持创建新地图：

1. 点击 **+ Place** 按钮进入"放置模式"，然后在小地图任意空白格上点击，即可弹出"Create New Map"对话框，并预填该坐标
   - 也可以点击 **+ New Map** 直接打开对话框（不在世界地图上放置标记）
2. 在对话框中填写：
   - **Map Name** — 标识符，必须以字母开头，仅含字母 / 数字 / 下划线（用作目录名 / `name` 字段）
   - **Display Name** — 可选，显示在世界地图悬浮提示中（默认大写化标识符）
   - **Tileset** — 从全部支持的 tileset 中选择
   - **Music** — 从 45 种音乐中选择
   - **Width / Height** — 地图大小（block 数，1–255）
   - **Border Block ID** — 初始填充 block（同时作为 `borderBlock`）
   - **Place on World Map** — 勾选后填入 X/Y（0–15）
3. 点击 **Create** 即可完成创建，编辑器会自动：
   - 在 `crates/pokered-data/maps/<Name>/` 目录写入 `map.json`、`map.blk`（按 borderBlock 填充）、`script_config.json` 和空的 `script.js`
   - 自动分配 `id`（当前最大 id + 1）
   - 把世界地图坐标写入 `crates/pokered-data/town_map_extras.json`
   - 切换到新地图，并在小地图上以青色圆点显示

### 小地图导航

侧边栏的 **World Map** 区域显示关都地区（Kanto）的小地图：

- **城市（黄色）**：Pallet Town、Viridian City、Pewter City 等
- **道路（绿色）**：Route 1-25、Sea Route 19-21
- **当前位置（红色圆圈）**：高亮显示当前选中的地图
- **交互**：
  - 鼠标悬停显示地图名称
  - 点击跳转到对应地图

### 编辑脚本绑定

选中 NPC 或 Sign 后，Entity Detail 面板中有 **Script Function** 输入框。修改后按回车确认，编辑器会同步更新 `script_config.json` 中的绑定关系。

### 地图间导航

编辑器维护一个导航历史栈：

- 点击 Warp / Connection / "Go to" 按钮跳转到目标地图
- 侧边栏出现 **← Back** 按钮，可返回上一张地图

### 保存

点击侧边栏 **Save** 按钮（有未保存修改时可用）。保存操作会：

1. 将 `map.json` 写回 `crates/pokered-data/maps/{MapName}/map.json`（自动剥离运行时字段 `talk`）
2. 将 `script_config.json` 写回同目录

保存成功后状态栏显示 `Saved {MapName}`，未保存标记消失。

## 脚本编辑器

编辑器内嵌了基于 **CodeMirror 6** 的代码编辑器，可以直接查看和编辑每张地图的 `script.js` 事件脚本。

### 打开脚本编辑器

有三种方式打开脚本编辑器：

1. **侧边栏按钮**：点击 `{ } Script` 按钮，面板在地图画布下方展开
2. **点击函数名**：在 Map Info 面板或 Entity Detail 面板中，点击任何高亮的函数名（如 NPC 的 talk 函数、Sign 的 talk 函数、Map Script、Coord Event trigger），编辑器自动打开并跳转到该函数的定义行
3. **再次点击 `✕ Script` 按钮**关闭面板

### 编辑器界面

```
┌─────────────────────────────────────────────────────┐
│ ═══ 可拖拽调整高度的把手 ═══                          │
├──────────────────────────────────────────────────────┤
│ { } PalletTown/script.js  [Modified]    [Save] [✕] │
├──────────────┬──────────────────────────────────────┤
│ Functions(5) │                                      │
│              │   // CodeMirror 代码编辑区            │
│ ▸ enterMap   │   export async function enterMap() { │
│ ▸ talkOak    │     if (!game.getFlag("...")) {      │
│ ▸ signPallet │       ...                            │
│              │     }                                │
│              │   }                                  │
│              │                                      │
└──────────────┴──────────────────────────────────────┘
```

- **顶部栏**：显示当前地图的脚本文件名，修改状态标记（Modified），Save 按钮和关闭按钮
- **左侧函数列表**：自动解析 `script.js` 中的所有函数定义，显示函数名、行号、是否为 `export`。点击函数名跳转到对应行
- **右侧编辑区**：CodeMirror 6 编辑器，支持 JavaScript 语法高亮、行号、代码折叠、括号匹配、搜索替换、撤销/重做
- **可拖拽调整高度**：顶部有拖拽把手，可上下拖动调整脚本编辑器的面板高度

### 编辑器功能

| 功能 | 说明 |
|------|------|
| JavaScript 语法高亮 | 基于 `@codemirror/lang-javascript`，支持 ES module 语法 |
| 暗色主题 | One Dark 主题，与编辑器整体风格一致 |
| 行号 | 左侧显示行号 |
| 代码折叠 | 点击行号旁的折叠图标可折叠/展开代码块 |
| 括号匹配 | 自动高亮匹配的括号对 |
| 搜索替换 | Ctrl+F 搜索，Ctrl+H 替换 |
| 撤销/重做 | Ctrl+Z / Ctrl+Shift+Z |
| 快捷保存 | Ctrl+S / Cmd+S 保存脚本到磁盘 |
| 函数跳转 | 从侧边栏函数列表或实体面板点击函数名跳转 |

### 保存脚本

- 点击顶部栏的 **Save** 按钮，或按 `Ctrl+S` / `Cmd+S`
- 脚本内容直接写回 `crates/pokered-data/maps/{MapName}/script.js`
- 保存后 "Modified" 标记消失，状态栏显示 `Saved script for {MapName}`

### 切换地图时

当在侧边栏切换到另一张地图时，脚本编辑器自动加载新地图的 `script.js`。如果新地图没有 `script.js`，编辑器显示为空白。

## 训练师队伍编辑（Trainer）

侧边栏的 **Trainer** 活动用于编辑全部 47 个训练师 class（Youngster、Brock、Lance、Rival1/2/3、Rocket 等）的所有可用队伍配置。

### 打开方式

- 在侧边栏左侧 ActivityBar 点击 **Trainer** 图标
- 在地图上点击训练师 NPC，详情面板会出现 **🎓 Edit Trainer Team** 按钮直达对应 class
- 直接访问 URL `/trainer/Brock`（class 名为 PascalCase）

### 编辑界面

- 左侧 `TrainerSidebar` 列出全部 47 个 class，可搜索过滤
- 右侧 `TrainerEditor` 以 tab 形式展示该 class 的所有队伍（同一 class 下不同训练师的队伍配置）
  - 每个 tab 内可调整队伍中每只宝可梦的 **等级（1–100）** 和 **物种**（下拉选择 151 种）
  - 每队最多 6 只，可增删宝可梦
  - 可增删整支队伍
- 修改后点击 **Save** 写回 `crates/pokered-data/trainers/<ClassName>.json`

### 单一数据源（Single Source of Truth）

`crates/pokered-data/trainers/*.json`（47 个文件）是训练师数据的**唯一**正本：

- 编辑器保存 → 写入 JSON
- `cargo build` 时，`pokered-data/build.rs` 自动读取所有 JSON 并生成 `OUT_DIR/trainer_data_gen.rs`
- 该文件被 `pokered-data::trainer_data::trainer_data()` 通过 `include!()` 嵌入，编译期还原为 `Vec<TrainerClassData>`
- 因此**修改 JSON → 重新 `cargo build` → 游戏自动使用新数据**，无需任何手工导入/导出步骤
- `cargo:rerun-if-changed` 已对每个 JSON 文件单独注册，编辑单个文件即可触发增量重建

### API 端点（Trainer）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/trainers` | 全部 trainer class 列表 |
| GET | `/api/trainers/:class` | 单个 class 的 JSON 数据 |
| PUT | `/api/trainers/:class` | 保存单个 class（写回 `<ClassName>.json`，2 空格缩进 + 末尾换行） |

## Pokemon 数据编辑（Pokemon）

侧边栏的 **Pokemon** 活动用于编辑全部 151 种宝可梦的数据，每只宝可梦一个 JSON 文件，覆盖战斗属性、招式、进化等所有可调字段。

### 打开方式

- 在侧边栏左侧 ActivityBar 点击 **Pokemon** 图标（🐾）
- 直接访问 URL `/pokemon/Bulbasaur`（物种名为 PascalCase）

### 快速试玩（WYSIWYG）

编辑器头部提供两个试玩按钮，无需保存即可把当前编辑的宝可梦数据（基础能力等）实时注入
运行中的游戏并打开悬浮试玩窗：

- **⚔ 试玩战斗** — 直接进入一场对战该宝可梦（Lv5）的野生战斗，可即时验证基础能力、
  类型、初始招式等改动
- **📖 试玩图鉴** — 直接打开该宝可梦的图鉴条目（完整数据 + 叫声），可即时验证图鉴
  分类、身高体重、描述文字等改动

### 可编辑字段

每只宝可梦的 JSON 涵盖以下分组：

| 分组 | 字段 | 范围 / 说明 |
|------|------|-------------|
| **Sprite Preview** | Front / Back 缩略图 | 顶部展示 `gfx/pokemon/front/<stem>.png` 与 `gfx/pokemon/back/<stem>b.png`，pixelated 渲染。当前为只读预览 |
| **Pokédex Entry** | Category | 分类名（最长 11 字符，大写 ASCII，如 `SEED` / `MOUSE`） |
|  | Height (ft / in) | 身高，两字段：英尺（0–99）+ 英寸（0–11） |
|  | Weight (lbs) | 体重，磅，精度 0.1（内部存为 u16 十分之一磅） |
|  | Flavor Text Pages | 0–4 页图鉴描述；每页 textarea，页内 `\n` 换行；`#MON` 是 `POKéMON` 控制 token |
| **Base Stats** | HP / Attack / Defense / Speed / Special | 各 1–255，编辑器底部显示总和 (BST) |
| **Types & Growth** | Type 1 / Type 2 | 15 种属性下拉（Normal..Dragon），单系宝可梦 Type1 = Type2 |
|  | Growth Rate | 6 档（MediumFast / SlightlyFast / SlightlySlow / MediumSlow / Fast / Slow） |
|  | Catch Rate | 0–255 |
|  | Base Exp | 0–255 |
| **Initial Moves** | Slot 1–4 | 4 个槽位，每个可选 165 种招式或 `None`（新捕获时自带的招式） |
| **TM / HM Flags** | 7 字节位标记 + 单独 TM01..HM05 复选框 | 决定该宝可梦能学习哪些 TM01-TM50 / HM01-HM05；右上角显示总数 |
| **Evolutions** | 多条进化路径 | 三种方式：`Level`（到达指定等级）/ `Item`（使用进化石，可选 5 种）/ `Trade`（交换） |
| **Learnset** | 升级学习招式表 | 任意条目，每条 `Lv. + Move`；提供 "Sort by Level" 一键排序 |

### 单一数据源（Single Source of Truth）

`crates/pokered-data/pokemon/*.json`（151 个文件）是 Pokemon 数据的**唯一**正本：

- 编辑器保存 → 写入 JSON
- `cargo build` 时，`pokered-data/build.rs` 的 `generate_pokemon_and_evos_data` 函数读取所有 JSON，生成：
  - `OUT_DIR/pokemon_data_gen.rs` — `BASE_STATS: [BaseStats; 151]` 数组字面量
  - `OUT_DIR/evos_moves_gen.rs` — `evos_moves_data()` 返回的 `vec![...]`
- 这两个文件被 `pokemon_data.rs` 与 `evos_moves.rs` 用 `include!()` 引入
- 修改 JSON → 重新 `cargo build` → 游戏自动使用新数据；战斗、捕获、进化、学招式表全部生效

### 一次性导出（首次迁移用）

若 `pokemon/` 目录被删除或需要从源码重新种子化，依次运行：

```bash
# 1. 从 Rust 源 BASE_STATS / MOVES / evos_moves_data() 重建 pokemon/*.json + moves/*.json
cargo run --example dump_pokemon_and_moves -p pokered-data

# 2. 从汇编 data/pokemon/dex_entries.asm + dex_text.asm 合并 pokedex 块到 pokemon/*.json
cargo run --example seed_pokedex_from_asm -p pokered-data
```

日常编辑无需运行这些命令——直接编辑 JSON 或使用编辑器即可。Pokedex 数据现已由 `crates/pokered-data/src/pokedex.rs::POKEDEX_ENTRIES`（在 build 时由 `build.rs` 从 `pokemon/*.json` 的 `pokedex` 块生成）暴露给 Rust 运行时。

### 精灵图预览 (Sprite Preview)

编辑器顶部展示 front + back 两张精灵图（来自 `gfx/pokemon/front/<stem>.png` 与 `gfx/pokemon/back/<stem>b.png`），由 Vite 的 `/gfx/*` 中间件直接提供。文件名规则：

```ts
species.toLowerCase().replace(/[ \-']/g, '')
```

唯一特例：`MrMime` → `mr.mime`（磁盘文件名保留了点号）。当前为**只读预览**——要替换精灵图，直接编辑/替换 `gfx/pokemon/front/<stem>.png` 即可。下次刷新页面预览会更新；Rust 运行时也会读取新的 PNG。

### API 端点（Pokemon）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/pokemon` | 全部 species 文件名列表 |
| POST | `/api/pokemon` | 新建 species（body `{"name":"NewMon"}`，写模板 JSON；重建后成为 `Species` 枚举变体） |
| GET | `/api/pokemon/:species` | 单个 species 的 JSON 数据 |
| PUT | `/api/pokemon/:species` | 保存（写回 `<Species>.json`，2 空格缩进 + 末尾换行；`species` 字段必须与文件名一致） |

## 招式数据编辑（Move）

侧边栏的 **Move** 活动用于编辑全部 165 个招式的战斗参数，每个招式一个 JSON 文件。

### 打开方式

- 在侧边栏左侧 ActivityBar 点击 **Move** 图标（⚔）
- 直接访问 URL `/move/Tackle`（招式名为 PascalCase）

### 可编辑字段

| 字段 | 范围 / 说明 |
|------|-------------|
| **Power** | 0–255。0 表示纯状态招式或特殊伤害公式（如 SeismicToss） |
| **Accuracy** | 0–100。100 = 总是命中（受 1/256 miss bug 影响） |
| **PP** | 1–40 |
| **Move Type** | 15 种属性下拉 |
| **Effect** | 81 种 `MoveEffect` 枚举（如 `NoAdditionalEffect`、`BurnSideEffect1`、`TwoToFiveAttacksEffect`、`OhkoEffect` 等），决定招式行为；完整列表见 `crates/pokered-data/src/moves.rs::MoveEffect` |

> **注意**：只改 `power / accuracy / pp` 是安全的；改 `effect` 会改变招式行为类型，必须配合 `pokered-core` 中对应分支的逻辑使用，否则可能让原本带状态效果的招式失去效果。

## 扩展数据（新增宝可梦 / 招式 / 道具）

Pokemon / Move / Item 编辑器支持**新建**记录：侧边栏顶部的 **＋ New Pokemon / ＋ New Move /
＋ New Item** 按钮，输入 PascalCase 名称（`^[A-Z][A-Za-z]+$`，如 `Pikachu2`，不能与现有条目
大小写不敏感重名）后创建一份模板 JSON。名称即 Rust 枚举变体（`Species` / `MoveId` / `ItemId`），
因此：

- 宝可梦/招式：新增文件写回 `crates/pokered-data/pokemon/<Name>.json` / `moves/<Name>.json`
- 道具：新增文件写回 `crates/pokered-data/data/items/<Name>.json`，并**自动登记到
  `data/items/item_list.json`**（枚举顺序源，追加到末尾）
- 下次 `cargo build` 时 `build.rs` 自动发现新文件并重新生成枚举与数据表：
  - 既有编号保持稳定（物种 1..=151 / 招式 0x01..=0xA5 / 道具 0x01..=0x53）
  - 新条目追加编号（物种 152+，招式 0xA6+，道具 0x54+）
- 重建游戏后新条目即可实际使用：宝可梦/招式可被战斗、学习招、进化、图鉴、存档、训练师队伍、
  野遇引用；道具可被商店、剧情脚本、拾取物引用

**NPC 新增（地图内）**：Map 编辑器的 **Map Info → NPCs** 区块有 **＋ Add** 按钮，在当前地图
中心添加一个默认 NPC（`textId` 自动分配、`Stationary/Down`、常用精灵贴图），并同步写入
`script_config.json` 的脚本绑定；选中 NPC 后可在 **Entity Detail** 面板改脚本绑定或用
**🗑 Delete NPC** 删除（绑定同步清理）。

**注意事项**

- 新宝可梦暂无专属素材与本地化：英文名显示 `???`、默认叫声/图标降级为占位；front/back 精灵图
  缺失时战斗画面不绘制，可通过既有 Pixel 编辑器 / AI 精灵工具生成
  `gfx/pokemon/front|back/<stem>.png` 补充
- 名称不可更改、不可删除——重命名/删除会让后续条目的编号漂移，破坏存档字节兼容
- 重建前（仅编辑器内）的 WYSIWYG 预览无法识别新条目；重建后运行时覆盖注入自动生效
- 静态部署（GitHub Pages 模式）不支持新建（无 /api 后端），仅在 `npm run dev` / Electron 下可用

### API 端点（Move）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/moves` | 全部 move 文件名列表 |
| POST | `/api/moves` | 新建招式（body `{"name":"NewMove"}`，写模板 JSON；重建后成为 `MoveId` 枚举变体） |
| GET | `/api/moves/:id` | 单个 move 的 JSON 数据 |
| PUT | `/api/moves/:id` | 保存（写回 `<MoveId>.json`，2 空格缩进 + 末尾换行；`id` 字段必须与文件名一致） |

## AI 助手技能（Skills）

AI 助手内置了一套**可加载的任务手册**（skills）：系统提示词里只有技能名 + 一句话描述的索引，
当请求命中某个技能时，助手先调用 `read_skill` 工具加载完整手册，再按其流程执行。技能是
`SKILL.md` 文件（frontmatter 写 `name` / `description`），按目录扫描发现，无需注册表：

- **内置技能**在 `tools/pokered-editor/skills/<name>/SKILL.md`，随编辑器发布：
  - `new-map` —— 新增地图：建目录（map.json/map.blk/script.scene/script_config.json）、
    摆放 NPC/warp/sign、配置野遇与地图连接、编写事件脚本，以及 Rust 侧注册清单（MapId /
    MAP_DIMENSIONS / embedded_blk_sources / map_names）
  - `new-trainer` —— 新增训练师：给既有职业加阵容（`OPP_<CLASS><N>` 引用），或新建
    职业（JSON + TrainerClass/build.rs/贴图/中文名的完整清单），含道馆主（对话驱动）与路上
    训练师（视线触发）两种布阵模式
  - `new-pokemon` —— 新增宝可梦：物种 JSON 全字段指南（种族值/属性/努力曲线/初始招式/
    TM-HM 位标志/图鉴/进化/升级招）、自动编号规则、贴图与本地化补全、以及如何让它可被获得
  - `save-construction` —— 存档构造：Save 页签、完整 JSON 快照格式（队伍/背包/徽章/
    320 字节事件旗标位集）、export/import-snapshot CLI 与 debug server 实时修改
- **项目级技能**放在项目根目录的 `skills/` 下，同名时覆盖内置技能（可用于项目自定义流程）。

配套的工具面调整：`list_skills` / `read_skill` 两个读取工具（开发服务器与静态托管的编辑器
助手均可用；静态构建会把 SKILL.md 内联进 bundle）；`propose_map_file`（写地图目录内的
`map.json` / `script_config.json`，走提案审查托盘）；`propose_map_create` 接受 `tileset` / `width` /
`height` / `music` / `borderBlock` / `townMap` 参数，在 pokered 项目里生成完整可构建的地图目录。
打包 Electron 应用内可用环境变量 `DOTZUKI_SKILLS_DIR` 指定内置技能目录。

## 数据目录结构

```
crates/pokered-data/maps/
├── PalletTown/
│   ├── map.json              # 地图主数据（header, warps, npcs, signs, text, wild）
│   ├── map.blk               # block 数据（二进制，每字节一个 blockID）
│   ├── script_config.json    # 脚本配置（mapScripts, npc/sign 绑定, coordEvents）
│   └── script.js             # 事件脚本（可通过内嵌脚本编辑器查看和编辑）
├── OaksLab/
│   ├── map.json
│   ├── map.blk
│   └── ...
└── ... (248 张地图)
```

```
crates/pokered-data/trainers/
├── Brock.json                 # 单个 trainer class 的全部队伍配置
├── Lance.json
├── Rival1.json
└── ... (47 个 class)
```

```
crates/pokered-data/pokemon/
├── Bulbasaur.json             # 单只宝可梦的全部数据
├── Ivysaur.json               # （baseStats, types, growthRate, initialMoves,
├── Venusaur.json              #   tmHmFlags, evolutions, learnset）
└── ... (151 个 species，编辑器可新增 → 152+)
```

```
crates/pokered-data/moves/
├── Pound.json                 # 单个招式的全部数据
├── Tackle.json                # （id, effect, power, type, accuracy, pp）
├── Earthquake.json
└── ... (165 个招式，编辑器可新增 → 166+)
```

> Trainer / Pokemon / Move 三类 JSON 都是单一数据源；`build.rs` 在编译期生成 Rust 数据，无需手动 dump/import。

## API 端点

编辑器通过 Vite 开发服务器内置的 API 访问数据：

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/maps` | 所有地图目录名列表 |
| POST | `/api/maps` | 创建新地图（body: `{ name, displayName?, tileset, width, height, music?, borderBlock?, townMap?: { x, y } }`） |
| GET | `/api/maps/:name/map.json` | 地图 JSON 数据 |
| GET | `/api/maps/:name/map.blk` | block 数据（JSON 字节数组） |
| GET | `/api/maps/:name/script_config.json` | 脚本配置 |
| GET | `/api/maps/:name/script.js` | 脚本文件（纯文本） |
| PUT | `/api/maps/:name/map.json` | 保存地图 JSON |
| PUT | `/api/maps/:name/script_config.json` | 保存脚本配置 |
| PUT | `/api/maps/:name/script.js` | 保存脚本文件 |
| PUT | `/api/maps/:name/map.blk` | 保存 block 数据（tile 编辑） |
| GET | `/api/blocksets` | 所有 tileset 的 blockset 数据（含用户新建） |
| PUT | `/api/blocksets/:name` | 写回某个 tileset 的 `.bst`（body: `{ blocks: { [id]: number[16] } }`，稀疏更新） |
| GET | `/api/passable-tiles` | 各 tileset 的可通行 tile 列表（默认值 + 用户 overrides） |
| PUT | `/api/passable-tiles/:name` | 写回单个 tileset 的可通行 tile 列表（body: `{ tiles: number[] }`） |
| GET | `/api/tileset-extras` | 用户新建的 tileset 元数据 |
| POST | `/api/tilesets` | 通过克隆 base tileset 创建新 tileset（body: `{ name, base, category?, displayName? }`） |
| GET | `/api/town-map-extras` | 用户新增的世界地图位置 |
| PUT | `/api/town-map-extras` | 保存用户新增的世界地图位置 |
| GET | `/api/trainers` | 全部 trainer class 列表（详见 Trainer 章节） |
| GET / PUT | `/api/trainers/:class` | 单个 trainer class 的 JSON 读 / 写 |
| GET | `/api/pokemon` | 全部 151 个 species 文件名列表 |
| GET / PUT | `/api/pokemon/:species` | 单个 Pokemon 的 JSON 读 / 写 |
| GET | `/api/moves` | 全部 165 个 move 文件名列表 |
| GET / PUT | `/api/moves/:id` | 单个招式的 JSON 读 / 写 |
| GET | `/gfx/tilesets/*.png` | tileset 贴图 |

## Layout Editor

Layout Editor 提供了一个可视化的界面来编辑游戏中 15 个菜单的屏幕布局（定义在 `crates/pokered-data/ui_layouts/` 中）。每个菜单以 JSON 格式存储，包含变体（variant）和矩形框（box）定义，用户可以通过拖拽或直接编辑 JSON 来调整菜单元素的位置和大小。

### 启动

```bash
cd workspace/tools/pokered-editor

# 构建 WebAssembly 预览模块（首次或 wasm 变更后需要）
npm run build:wasm

# 启动开发服务器
npm run dev
```

### 使用方法

1. 在左侧 Activity Bar 点击 **Layout** 图标进入布局编辑模式
2. 在侧边栏选择要编辑的菜单（共 15 个：bag、battle_bag、battle_main、battle_move、battle_party、battle_text、dialog、main、mart、naming、options、party、save、start、stats）
3. **JSON 编辑**：左侧面板为 CodeMirror 6 编辑器，可直接编辑布局 JSON
4. **拖拽编辑**：右侧预览画布显示菜单渲染结果，可拖拽矩形框调整位置，拖拽右下角手柄调整大小
5. **Mock 状态**：通过下拉菜单切换不同的渲染状态预览
6. **保存**：`Cmd+S` / `Ctrl+S` 或点击 Save 按钮保存到对应 JSON 文件
7. 切换布局时如果有未保存的修改，会弹出确认对话框

### 键盘快捷键

| 快捷键 | 功能 |
|--------|------|
| `Cmd+Z` / `Ctrl+Z` | 撤销（CodeMirror 内使用 CM 自身撤销，画布上使用布局撤销） |
| `Cmd+Shift+Z` / `Ctrl+Shift+Z` | 重做 |
| `Cmd+S` / `Ctrl+S` | 保存当前布局（始终生效，即使在编辑器中） |

### API 端点（Layout）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/ui-layouts` | 全部布局文件列表 |
| GET | `/api/ui-layouts/:name` | 单个布局的 JSON 数据 |
| PUT | `/api/ui-layouts/:name` | 保存单个布局（写回对应 `.json` 文件） |

## 图例

### 地图画布

| 颜色 | 含义 |
|------|------|
| 🟢 绿色半透明 | 可通行 tile |
| 🔴 红色半透明 | 不可通行 tile |
| 🔵 蓝色 | Warp（传送点） |
| 🟡 黄色 | Sign（路牌） |
| 🔴 红色实心 | NPC — 训练师（标记 T） |
| 🟢 绿色实心 | NPC — 道具（标记 I） |
| 🟣 紫色 | NPC — 普通（标记 N） |
| 🟠 橙色 | Coord Event（标记 C） |
| 🟢 绿色箭头 | 地图连接（显示目标地图名和偏移量） |
| ⬜ 白/黄闪烁边框 | 当前选中的实体 |

### 小地图

| 颜色 | 含义 |
|------|------|
| 🟡 黄色圆点 | 城市（City/Town） |
| 🟢 绿色圆点 | 道路（Route） |
| 🔴 红色圆点（白边） | 当前选中的地图 |

## TMX 导入/导出

Map 编辑模式下，工具栏右侧提供 **📥 Import TMX** 和 **📤 Export TMX** 按钮，支持在 pokered-editor 与 [Tiled Map Editor](https://www.mapeditor.org/) 之间交换地图数据。

### 格式说明

- **TMX 格式**：使用 Tiled 的 **JSON 导出格式**（非 XML），文件后缀通常为 `.json` 或 `.tmx.json`
- **方块结构**：游戏使用 4×4 tile 的 **block** 作为最小编辑单位（16 tiles/block）。导入时自动将 tile 归组为 block 并匹配 blockset；导出时将 block 展开为 4×4 tile 网格

### 导入 TMX

1. 在 Tiled 中设计地图（使用与游戏一致的 tileset 和 8×8 tile 尺寸）
2. 另存为 JSON 格式（File → Export As → JSON map files）
3. 在 pokered-editor 中点击 **📥 Import TMX**，选择导出的 JSON 文件
4. 导入的地图自动添加到地图列表末尾，block 数据匹配当前 tileset 的 blockset

**Tiled 对象层支持**：
- `warps` 层或 `type=warp` 的对象 → 导入为地图传送点
- `npcs`/`trainer` 层或对应 type 的对象 → 导入为 NPC
- `signs` 层或 `type=sign` 的对象 → 导入为路牌

**自定义属性**：
- Tiled 地图级属性 `tileset` 和 `music` 会映射到地图头的 tileset 和 BGM
- 对象属性如 `spriteName`、`movement`、`facing`、`isTrainer` 等会映射到 NPC 字段

### 导出 TMX

1. 选择要导出的地图
2. 点击 **📤 Export TMX**，自动下载 `{MapName}.tmx.json` 文件
3. 在 Tiled 中打开（File → Open）

**导出内容**：
- **ground 层**：block 展开为 4×4 tile 网格的 tilelayer
- **warps 层**：包含所有传送点的 objectgroup（含 destMap、destWarpId 属性）
- **signs 层**：包含所有路牌的 objectgroup（含 textId 属性）
- **npcs 层**：包含所有 NPC 的 objectgroup（含 spriteId、movement、facing、textId、isTrainer 等属性）
- 地图属性：`mapName`、`music`、`tileset`

### 导入注意事项

- TMX 的 tile 尺寸必须是 8×8（与游戏 tileset 一致的像素尺寸）
- 导入使用当前已加载的 blockset 进行 block 匹配；不匹配的 tile 排列会用 borderBlock 填充
- tile 层尺寸如果不是 4 的倍数，会自动补齐
- Tiled 的 GID flip 标志（水平/垂直翻转）在导入时会被忽略（游戏不支持 block 内 tile 翻转）

