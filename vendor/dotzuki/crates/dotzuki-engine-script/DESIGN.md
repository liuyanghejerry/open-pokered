# dotzuki-engine-script 设计文档

## 1. 概述

`dotzuki-engine-script` 是一个用于替代硬编码 `ScriptAction` 队列的 JavaScript 脚本系统。它基于 `boa_engine`（一个纯 Rust 的 JavaScript 引擎，无需原生依赖，支持 wasm32），实现了基于 async/await 的地图脚本。

每个地图都有一个对应的 `.js` 文件。在脚本中，可以使用 `await game.showText(...)`、`await game.moveNpc(...)` 等方法与游戏交互。

## 2. 架构总览

以下是数据流的 ASCII 图示：

```
JS Script (async fn)
    │  await game.showText("Hello")
    ▼
SharedBridge (pending_command + pending_resolve)
    │
    ▼
ScriptEngine::tick() → returns ScriptCommand to game loop
    │
    ▼
OverworldScreen dispatches command → ScriptEffect
    │  (e.g. shows dialogue box, waits for player dismiss)
    ▼
ScriptEngine::signal_done(result) → resolves Promise → JS continues
```

## 3. 核心组件

### 3.1 ScriptCommand (command.rs)

`ScriptCommand` 是一个包含 25 个变体的枚举类型，支持通过 `serde` 序列化。以下是所有变体及其字段：

- `ShowText { text: String }`
- `ShowChoice { options: Vec<String> }`
- `GiveItem { item_id: String, quantity: u8 }`
- `GiveMonster { species: String, level: u8 }`
- `TakeItem { item_id: String, quantity: u8 }`
- `SetFlag / ResetFlag / CheckFlag { flag: String }`
- `ShowObject / HideObject { object_index: u8 }`
- `MoveNpc { npc_id: String, path: Vec<(u8, u8)> }`
- `FaceNpc { npc_id: String, direction: String }`
- `FacePlayer { direction: String }`
- `PlayMusic / PlaySound { music_id/sound_id: String }`
- `StopMusic, FadeOutMusic`
- `StartBattle { trainer_id: String }`
- `Delay { frames: u16 }`
- `WarpTo { map: String, x: u8, y: u8 }`
- `Heal`
- `FadeScreen { fade_type: String }`
- `SetMapScript { state_name: String }`
- `SetJoyIgnore { mask: u8 }, ClearJoyIgnore`

### 3.2 CommandResult

`CommandResult` 用于解析 Promise 的返回值，支持以下几种类型：

- `Void`
- `Bool(bool)`
- `Number(f64)`
- `Text(String)`

例如，`showChoice` 返回选项索引，`startBattle` 返回战斗结果。

### 3.3 ScriptEngine (engine.rs)

`ScriptEngine` 封装了 `boa_engine::Context`、`SharedBridge`（`Rc<RefCell>`）、`SimpleModuleLoader` 和当前加载的 `Module`。

引擎使用 Boa 的 ES6 Module API：
- `Context::builder().module_loader(loader).build()` 创建支持模块的上下文
- `Module::parse(Source::from_reader(...))` 解析 JS 源码为模块
- `module.load_link_evaluate()` + `context.run_jobs()` 完成模块加载
- `module.get_value(fn_name)` 从模块命名空间获取导出的函数

状态流转：`Idle → Running → WaitingForCommand → Running → ... → Idle/Finished`

`SharedBridge` 包含：
- `pending_command`（`Option<ScriptCommand>`）
- `pending_resolve`（`Option`，包含解析 Promise 的 `JsFunction`）
- `flags`（`HashMap<String, bool>`）

关键方法：
- `new()`：创建 `Context`（使用 `SimpleModuleLoader`），注册游戏 API
- `load_script(source)`：使用 `Module::parse()` 将 JS 源码加载为 ES6 模块
- `call_function(name, args)`：通过 `module.get_value()` 从模块命名空间获取导出函数并调用
- `tick()`：每帧调用，返回待处理命令（如果有）
- `signal_done(result)`：用结果解析 Promise，运行任务，检查下一个待处理命令

便捷方法：
- `call_function_no_args`
- `call_function_with_u8`
- `call_function_with_xy`
- `call_function_with_str`
- `has_function`

### 3.4 JS Game API (engine.rs::register_game_api)

`game` 是一个全局对象，包含 22 个异步方法和 3 个同步方法。

- 异步方法：创建 `JsPromise`，将命令和解析函数存储到 `SharedBridge`，返回 Promise
- 同步方法（`getFlag`、`setFlag`、`resetFlag`）：直接读写 `SharedBridge.flags`

所有异步方法均使用宏 `register_async_command!` 注册。

### 3.5 MapScriptConfig (config.rs)

`MapScriptConfig` 定义了地图的 JSON 绑定配置，包含以下字段：

- `map_scripts: Vec<String>` — 地图脚本状态函数名的有序列表
- `npcs: Vec<NpcBinding>` — NPC ID → JS 函数名绑定
- `signs: Vec<SignBinding>` — 标志牌 ID → JS 函数名绑定
- `coord_events: Vec<CoordEventBinding>` — 坐标触发 → JS 函数名绑定，每个事件包含唯一的 `name` 字段（camelCase，同一地图内唯一）

关键方法：
- `resolve_map_script_index(state_name)` — 将状态名字符串解析为 `map_scripts` 数组中的索引
- `npc_talk_fn(text_id)` — 根据 NPC text_id 查找对应的 JS 函数名
- `sign_talk_fn(text_id)` — 根据标志牌 text_id 查找对应的 JS 函数名
- `coord_event_fn(x, y)` — 根据坐标查找对应的 JS 函数名
- `coord_event_by_name(name)` — 根据 `name` 字段查找对应的 JS 函数名

### 3.6 ScriptLoader (loader.rs)

`ScriptLoader` 是一个双重映射器，管理 `map_id` → JS 源码 和 `map_id` → `MapScriptConfig` 的映射。

关键方法：
- `register_script(map_id, source)`：手动注册 JS 脚本
- `register_config(map_id, config)`：手动注册 JSON 配置
- `register_config_json(map_id, json_str)`：从 JSON 字符串解析并注册配置
- `get_config(map_id)`：获取已注册的 `MapScriptConfig`
- `has_config(map_id)`：检查是否存在配置
- `load_from_directory(path)`：扫描 `.js` 和 `.json` 文件，文件名（不含扩展名）作为 `map_id`（仅限原生平台，使用 `cfg-gated`）
- `load_embedded()`：加载编译时嵌入的脚本和配置（通过 `include_str!` 实现，支持 wasm）
- `check_reload()`：比较文件修改时间，重新加载已更改的 `.js` 和 `.json` 文件（开发时支持热重载）

错误类型：`ScriptLoaderError`

## 4. 游戏循环集成

`pokered-core` 中的 `OverworldScreen` 集成了脚本系统。

字段：
- `script_engine: ScriptEngine`
- `script_loader: ScriptLoader`
- `map_script_config: MapScriptConfig`
- `active_script_effect: Option<ScriptEffect>`
- `joy_ignore_mask: u8`
- `map_script_index: u8`

初始化：
- `new()` 加载嵌入脚本和目录脚本，加载当前地图脚本

每帧更新：
1. 如果存在 `active_script_effect`，则更新（例如倒计时延迟、检查对话框是否关闭）
2. 如果效果完成，则调用 `signal_done` 通知引擎，获取下一个命令并分发
3. 如果没有效果且引擎处于等待状态，则分发待处理命令

交互：
- NPC A 键：调用 `try_call_script_npc_talk(text_id)` → 从 `MapScriptConfig` 查找函数名 → 直接调用 JS 函数
- 标志牌 A 键：调用 `try_call_script_sign_talk(text_id)` → 从 `MapScriptConfig` 查找函数名 → 直接调用 JS 函数
- 坐标触发：从 `MapScriptConfig` 查找坐标对应的函数名 → 直接调用 JS 函数
- 地图脚本状态：`map_script_index` 索引 `MapScriptConfig.map_scripts` 数组 → 调用对应函数名
- `SetMapScript { state_name }` 效果：通过 `resolve_map_script_index()` 将字符串名解析为索引
- 地图切换：`load_map_script(new_map_id)` → 创建新的 `ScriptEngine`，加载脚本和 JSON 配置

## 5. script_bridge.rs 桥接层

- `ScriptEffect` 枚举：`ScriptCommand` 的类型化版本（为 `Delay` 添加 `frames_remaining`，将方向字符串解析为 `Direction` 枚举）
- `dispatch_command()`：将 `ScriptCommand` 转换为 `ScriptEffect`
- `text_to_dialogue()`：将文本分割为对话页面
- `map_id_to_script_key()`：使用 `format!("{:?}", MapId)`（`Debug` 派生生成如 "StartTown" 的字符串）
- `find_npc_index_by_id()`：将 NPC 字符串 ID 解析为运行时索引

## 6. JS 脚本编写指南

脚本文件位置：`scripts/maps/{MapName}.js`（JS 脚本）和 `scripts/maps/{MapName}.json`（JSON 配置）

`MapName` 必须与 `MapId` 的 `Debug` 表示形式（PascalCase）匹配。

### JSON 配置结构

```json
{
  "mapScripts": ["scriptDefault", "scriptProfEntersLab", "scriptNoop"],
  "npcs": [
    { "id": 1, "talk": "talkRival" },
    { "id": 2, "talk": "talkFlambitBall" }
  ],
  "signs": [
    { "id": 1, "talk": "signProfLab" }
  ],
  "coordEvents": [
    { "name": "northExit1", "position": [4, 1], "trigger": "coordNorthExit" }
  ]
}
```

### JS 函数声明

JS 文件使用 ES6 模块语法，通过 `export` 关键字导出所有需要在 JSON 配置中绑定的函数。引擎使用 Boa 的 `Module` API（`Module::parse()` + `load_link_evaluate()`）解析模块，通过 `module.get_value()` 访问导出的函数。

```js
export async function scriptDefault() {
  if (!game.getFlag(EVENT.PROF_APPEARED)) return;
  await game.setMapScript("scriptProfEntersLab");
}

export async function talkRival() {
  await game.showText("<RIVAL>: ...");
}

// 内部辅助函数不需要 export（不在 JSON 中引用）
async function handleBallInteraction(starter, ballId) {
  // ...
}
```

函数名可以自由命名，通过 JSON 配置与事件绑定。一个 NPC 只绑定一个 JS 函数，条件逻辑在 JS 内部处理。

`setMapScript` 接受字符串状态名（而非数字索引），引擎通过 JSON 配置的 `mapScripts` 数组解析为索引。

所有游戏交互通过全局对象 `game.*` 实现。

## 7. 热重载

仅支持原生平台（`cfg-gated`，在 wasm32 上禁用）。

`ScriptLoader` 为每个脚本跟踪文件修改时间。

定期调用 `check_reload()` → 返回重新加载的 `map_id` 列表。

游戏可以为受影响的地图重新创建 `ScriptEngine`。

## 8. 扩展指南

添加新游戏命令的步骤：
1. 在 `command.rs` 的 `ScriptCommand` 枚举中添加变体
2. 在 `engine.rs` 的 `register_game_api()` 中添加 `register_async_command!` 调用
3. 在 `script_bridge.rs` 的 `ScriptEffect` 枚举中添加变体
4. 在 `script_bridge.rs` 的 `dispatch_command()` 中添加分支
5. 在 `OverworldScreen` 的 tick 循环中处理效果
6. 更新 `scripts/types/game.d.ts`

## 9. 跨平台支持

- `boa_engine` 是纯 Rust 实现 → 可编译为 `wasm32-unknown-unknown`
- `ScriptLoader::load_from_directory` 和 `check_reload` 使用 `#[cfg(not(target_arch = "wasm32"))]`
- `load_embedded()` 使用 `include_str!` → 在所有平台上均可运行
- 在 wasm 上运行时无文件系统依赖

## 10. 测试

`dotzuki-engine-script` 包含 20 个单元测试（覆盖 `engine` 和 `loader`）。

测试内容：
- 基本求值
- 异步命令流
- `signal_done` 的延续
- 多命令序列
- 标志的获取/设置
- 带结果的 `showChoice`
- 加载器的注册/获取/嵌入/目录/热重载

通过 `cargo test` 验证与 `pokered-core` 的集成（工作区内 0 失败）。