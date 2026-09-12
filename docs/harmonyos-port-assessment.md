# HarmonyOS 支持成本与改动范围调研

> 以下保留实施前的调研快照。共享宿主及首轮鸿蒙接入现已实现，构建方式和验证结果见 [开发说明](harmonyos-development.md)。文中的“只读”“未构建”和人日预算描述的是调研阶段。

更新日期：2026-09-12。open-pokered 首轮评估基线：`746f7dd`，引擎依赖为 `dotzuki v0.6.0 / 88f1fccd`。本次只读检查 `/Users/liuyanghe02/develop/dotzuki-2`，当前为 `master`、HEAD `341ca857612cf77fef90e1b2c23e7b1e98f2b97b`，工作区干净；[PR #64](https://github.com/liuyanghejerry/dotzuki/pull/64) 的鸿蒙适配已进入本地主线。

本次 HEAD 与上次审查的 PR head `1ef87642618d41bbd84167330a66dae98b92808d` 的 Git tree 均为 `f9a954de6f8801e88eeba8b4ce3aac6c71afd351`，文件内容完全一致。因此更新“已合并”的状态，保留上次源码结论与 10–18 人日接入估算；该预算已经扣除了鸿蒙宿主开发，不应因同一实现合并再次扣减。本地 HEAD 没有对应 tag。

## 当前结论：复用 PR #64 后的增量成本

**可行性明显提高，推荐直接复用原生鸿蒙宿主。预计新增工作由原来的 20–34 人日下调至 10–18 人日（约 2–4 周），首个 open-pokered 可操作画面约 3–5 人日。** 两个数字是累计里程碑，不相加。估算假设熟悉本项目和 Rust FFI、可复用现有 SDK/模拟器环境，不同步迁移 Android/iOS，也不把 PokemonGame 改写成通用零 Rust 游戏。

PR 已提供工程模板、跨语言桥接、GLES3 呈现、OHAudio 输出以及移动 ABI，原先从零完成平台外壳的预算不再适用。剩余核心工作是把 **PokemonGame 接到已有宿主协议**，并完成游戏特有的音频、存档和时钟适配。

这是源码审查后的工程估算。本次没有执行构建、测试或修改 dotzuki-2；PR 作者报告的模拟器验证与本项目真机验证应分开看待。

## PR 已完成什么，哪些仍需接入

| 能力 | PR #64 的实现 | open-pokered 剩余工作 |
| --- | --- | --- |
| 鸿蒙工程 | Stage/ArkTS、Hvigor、CMake、Node-API、XComponent 模板 | 复用模板，替换应用标识、启动数据和链接的 Rust 库 |
| 移动 ABI | 生命周期、八键输入、RGBA、PCM、外部存档；有 ABI 版本与错误接口 | 实现承载 PokemonGame 的适配器，不能直接使用原封不动的 RunnerGame 库 |
| 设备依赖拆分 | dotzuki-runner 新增 device-audio，gpu 控制 dotzuki-app，dotzuki-ui 可关闭默认 GPU | 将同样的 feature 边界应用到 pokered-app/audio/ui/renderer；升级依赖本身不会自动完成 |
| 画面 | GLES3 最近邻纹理、黑边、Surface 创建/销毁 | 去掉 320×240 常量，使用 160×144 和 10:9 比例；indexed framebuffer 需 to_rgba |
| 音频 | 游戏线程产 PCM、SPSC 队列、OHAudio 消费并做 f32→S16LE 与欠载补零 | 给实际接收 BGM/SFX/叫声命令的 pokered AudioManager 增加无设备 PCM 输出 |
| 存档 | external_saves、JSON 导入导出、ArkTS Preferences 持久化 | 对齐 Pokemon 保存数据、剧情 flags、选项和菜单 SAVE；保留既有保存语义 |
| 生命周期与输入 | pause/resume、页面隐藏保存、八键触控 | 覆盖 Ability 前后台、取消触摸时清键、Surface 重建、音频中断与恢复 |

关键代码位于 PR head：

- [移动运行时及具体 RunnerGame 绑定](https://github.com/liuyanghejerry/dotzuki/blob/1ef87642618d41bbd84167330a66dae98b92808d/workspace/crates/dotzuki-runner-mobile/src/lib.rs#L73)。
- [鸿蒙 Native 宿主](https://github.com/liuyanghejerry/dotzuki/blob/1ef87642618d41bbd84167330a66dae98b92808d/workspace/crates/dotzuki-cli/templates/harmony/entry/src/main/cpp/dotzuki_host.cpp)。
- [ArkTS 页面与存储](https://github.com/liuyanghejerry/dotzuki/blob/1ef87642618d41bbd84167330a66dae98b92808d/workspace/crates/dotzuki-cli/templates/harmony/entry/src/main/ets/pages/Index.ets)。
- [运行时 feature 拆分](https://github.com/liuyanghejerry/dotzuki/blob/1ef87642618d41bbd84167330a66dae98b92808d/workspace/crates/dotzuki-runner/Cargo.toml)。

## 不能直接执行 export 的原因与建议路线

`MobileRunner::from_pack` 固定执行 `PackFiles → LoadedProject → RunnerGame::new`，没有注入自定义游戏运行时的接口。`dotzuki export --harmony` 也先验证零 Rust 项目、生成 `game.dzpk`，并校验库名为 `libdotzuki_runner_mobile.a`。open-pokered 的主入口是 `PokemonGame`，包含专有内容和运行逻辑；换库名或把本仓库打进 dzpk 都不能自动完成适配。

建议新增 `pokered-mobile` 静态库，保留 PR 的接口组织与线程契约，内部驱动 PokemonGame。平台模板复用 PR 实现，显式替换 create 参数、头文件和链接配置；资源继续沿用 pokered 的编译嵌入方式。可以保留兼容的函数形状，但不应把接受 pokered 配置的新 create 伪装为完全兼容原有 dzpk 契约。

长期可将 dotzuki-runner-mobile 的队列/ABI 支撑代码与 RunnerGame 绑定分离，提供供两种游戏运行时实现的公共接口。该通用化是可选的上游工程工作，不是鸿蒙首版的前置条件；若与首版一起完成，额外预留 2–4 人日。完全迁移到 RunnerGame 不在本次移植范围内。

## 接入时必须处理的具体差异

1. **分辨率**：头文件和宿主的纹理分配、上传尺寸、宽高比均使用 320×240。仅修改 Rust 返回的 frame_len 会让宿主继续按旧尺寸取数据，必须成套修改，最好从 width/height 接口查询。
2. **节拍**：`Frame` 忽略 timestamp，每次回调进入 `RenderFrame` 都调用一次 tick，源码中没有固定步长调度。Rust 的 `59.7275` 常量只用于计算每 tick 的 PCM 数量，不会限制游戏推进速度。应使用时间累加器按既有 GB 节拍推进，并限制恢复时补帧；否则刷新回调频率变化可能同时影响玩法与音频队列。
3. **音源**：PR 的 `RunnerGame::render_audio` 与 pokered 的 AudioOutput 是两套运行层。可借鉴 PR 的 device-audio/PCM 分离，但必须连接 pokered 自己的 AudioManager，避免新建一份没有接收游戏音效命令的音源。旧 iOS 外壳的空音频桥接无需作为新实现基础。
4. **保存语义**：PR 的页面隐藏导出稳定状态不等同于 Pokemon 的菜单 SAVE。菜单保存应产生可持久化的已提交存档，并及时写入宿主；切后台可补刷，不能只依赖异步 onPageHide。需要保存完整剧情 flags 和选项，并验证启动 CONTINUE。若要增加自动存档，应另行明确产品行为。
5. **生命周期**：当前有页面回调和 Surface 清理，但还要验证系统切后台的实际覆盖、按键释放、音频队列恢复及回调停止后再销毁实例。PR 中的队列和线程契约可参考，尚不能把冒烟验证当成长时间并发验证。
6. **依赖**：dotzuki-app 本身仍默认依赖 renderer 和 notify；PR 是让 runner 可以不依赖它。pokered-app 当前直接依赖它，必须采用相同的绕开/按需启用方式。OHOS 的 target_env 识别和 Boa 依赖审查仍然适用，但不需要从零研究窗口/音频后端。

## 修订后的工作量

| 增量工作包 | 人日 | 完成标准 |
| --- | ---: | --- |
| 引擎版本对接与 pokered feature 拆分 | 2–4 | OHOS 游戏库构建不带入桌面窗口/ALSA 路径 |
| PokemonGame 移动 ABI 与 PCM 输出 | 2–3 | 输入、RGBA、实际 BGM/SFX、实例生命周期接通 |
| 模板接入、160×144 与固定步长 | 1–2 | 鸿蒙显示可操作游戏；刷新率不改变逻辑速度 |
| 存档与生命周期补全 | 1–2 | 菜单 SAVE、杀进程 CONTINUE、剧情 flags 完整 |
| 资源、HAP 与构建脚本 | 1–2 | 离线包和可重复构建 |
| 游戏闭环、已有平台回归、真机修整 | 3–5 | 新游戏/移动/对话/战斗/保存与恢复、音频稳定 |
| **合计** | **10–18** | **单机可维护测试版本** |

首个可操作画面预计累计 3–5 人日；不包含完整音频、存档和真机质量结论。设备与签名准备、商店接入、联机、编辑器发布按钮、Android/iOS 全面迁移不计入上述预算。若引擎升级存在 PR 之外的 API 变化，按实际差异追加估算。

适配已进入 dotzuki 本地主线：试验可在 pokered 自己的隔离配置中固定合并 commit `341ca857612cf77fef90e1b2c23e7b1e98f2b97b`，正式接入统一更新所有 dotzuki 依赖到包含这些修改的新 tag。合并不会改变已固定旧 tag 的下游依赖。只读分析 dotzuki-2 不需要改它或在其中构建。

## 验证证据与下一步

PR 描述报告了 ARM64 OHOS 静态库构建、HAP 构建安装、ArkUI→Node-API→Rust→EGL 显示和触控推进对话。运行环境为 DevEco 6.1.1.300、SDK API 24、Pura 90 API 24 **模拟器**。源码中另有非零 PCM、存档往返与 ABI 帧复制测试；本次未重跑这些测试。[PR 验证说明](https://github.com/liuyanghejerry/dotzuki/pull/64)

因此，平台基础链路已有作者报告的验证；尚不能推断 open-pokered 已可运行，也不能推断真实手机的音频时延、后台恢复与持久化都已验证。模板声明 API 17，但报告的环境为 API 24，最低支持版本仍应单独验证。

建议直接沿原生宿主路线做 PokemonGame 接入 PoC。ArkWeb 仍可作为快速试玩备选，但原先支持它的“节省整套原生宿主研发”理由已经明显减弱。

---

## 历史评估：2026-09-10，未纳入 PR #64

以下保留首轮代码证据和估算来源。**其 20–34 人日和“从零新增宿主”范围已被上文替代，不是当前排期。**

### 当时的结论与范围

支持原生 HarmonyOS 手机应用可行，属于中等规模的平台移植。游戏规则、地图、剧情、UI 布局、像素绘制与 APU 可以大部分复用；主要工作是拆开共享运行层的桌面依赖，新增鸿蒙窗口、输入、音频、存储和生命周期适配。

建议采用 **ArkUI 外壳 + C/C++ Native 桥接 + Rust 运行时 + XComponent 显示 framebuffer + OHAudio**。一个熟悉仓库、Rust FFI 和移动端开发的工程师，预计 **20–34 人日**完成可持续维护的单机测试版本，约 **4–7 个全职工作周**。这是基于代码审查的工程估算，尚未经过鸿蒙交叉编译和真机验证。

本估算覆盖：ARM64 手机、离线单机、现有中英文内容、触控、声音、存读档、前后台切换、构建与回归。暂不包括编辑器、PC/手表形态、跨设备联机、账号云存档、商店服务接入和上架审核等待时间。最低 HarmonyOS/API 版本在 PoC 时按目标真机与所用 API 冻结。

如果目标只是仍兼容现有 APK 的设备，应先验证当前 Android 包，不应套用原生 HAP 移植预算。本文主要评估需要原生 HarmonyOS 应用的场景。

## 代码证据

| 位置 | 现状 | 对移植的影响 |
| --- | --- | --- |
| `crates/pokered-core/`、`pokered-data/` | 游戏逻辑与静态数据独立；默认使用原生剧情 AST | 玩法和内容原则上复用，不重写 ArkTS 游戏逻辑 |
| `crates/pokered-renderer/Cargo.toml` | 有 `framebuffer` 与 `gpu` feature，默认启用 `gpu` | CPU 像素绘制可复用，但须关闭所有依赖路径上的 GPU 默认 feature |
| `crates/pokered-ui/Cargo.toml` | 默认 `gpu`，并直接依赖默认配置的 `dotzuki-renderer` | 仅在新 shell 写 `default-features=false` 不够，Cargo feature 会合并 |
| `crates/pokered-app/Cargo.toml` | 非 WASM 目标自动启用 `cpal`，并引入 `clap`、`notify`、`dotzuki-app` | 要把通用运行时与桌面启动/工具功能分开 |
| `crates/pokered-app/src/game.rs` | `PokemonGame::new` 区分桌面与 Android/iOS/WASM；默认保存路径也按平台硬编码 | 新平台不能直接复用桌面构造函数和可执行文件目录存档 |
| `crates/pokered-audio/src/output.rs` | `AudioOutput` 在非 WASM 下直接持有 `CpalOutput` | 需分离音源/游戏音频控制与设备输出，并增加外部 PCM 消费接口 |
| `crates/pokered-ios/src/lib.rs`（迁移前） | 旧版 init/update/draw/save/load/audio_fill C ABI，输出 160×144 RGBA | 可参考 ABI 形状；不应直接复制并假定实现完整 |
| `crates/pokered-android/`、`android/` | NativeActivity、JNI、Kotlin 虚拟按键、winit/pixels | 可借鉴交互设计，平台代码需要重做 |
| `crates/pokered-web/` | 已有完整 WASM 游戏 shell，包含首个用户手势恢复音频逻辑 | 可作为低成本 ArkWeb 方案的起点 |

### 已证实的依赖问题

本机执行 `rustc --print cfg --target aarch64-unknown-linux-ohos` 得到：

```text
target_arch="aarch64"
target_env="ohos"
target_os="linux"
```

因此平台条件应识别 `target_env = "ohos"`，不能写成 `target_os = "ohos"`，也不能把所有 Linux 目标都当成桌面 Linux。

迁移前曾用 `cargo tree --offline --locked -p pokered-ios --target aarch64-unknown-linux-ohos` 检查旧移动端依赖，可见 `pixels 0.15.0 → wgpu 0.19.4`、`winit 0.30.13`、`cpal 0.15.3 → alsa/alsa-sys`、`notify 6.1.1 → inotify`，以及 `boa_engine 0.20.0`。这说明直接沿用当时的 shell 会携带桌面平台依赖。读取已锁定版本的源码也确认，winit 的 Unix 平台条件未排除 OHOS，cpal 的 Linux 条件会启用 ALSA。

上述命令同时输出了引擎缓存中模板 manifest 的 `{{project-name}}` 包名诊断。依赖树有输出，但不能把它当成交叉编译通过；该诊断也应在构建 PoC 中澄清。

`script-boa` 默认关闭只代表运行时不选择旧解释器，不代表 Boa 从依赖图消失：当前 `dotzuki-engine-script` 仍无条件依赖 Boa。移植时先确认其兼容性；若要裁剪，再单独拆分共享类型/脚本功能，不能把彻底去 Boa 当作零成本前提。

### iOS C ABI 的复用边界

- `GameContext.audio` 初始化为 `None`，当前导出实现没有给它接入音源的路径；`pokered_update` 只在它为 `Some` 时向 ring buffer 推送 PCM。与此同时 `PokemonGame` 自己持有另一套音频输出。因此现有 `audio_fill` 不能直接视为已接通的游戏音频源。
- 初始化使用进程级 `OnceLock`，销毁后仍无法再次初始化，需要明确鸿蒙页面/Ability 重建时的实例管理。
- 更新与音频回调均从同一裸指针建立可变上下文引用；ring buffer 溢出时生产者也修改读指针。共享到新平台前，应重新审查并发所有权、溢出行为及销毁顺序。
- `set_save_dir` 只修改外壳字段；游戏内部默认保存路径另有逻辑。需要统一菜单保存、加载、剧情标志文件和外部存档操作的路径。

这些是静态代码发现，未据此声称现有 iOS 真机一定出现对应故障。

## 推荐实现

```text
ArkUI：页面、虚拟按键、安全区、生命周期、沙箱路径
  │ Node-API：初始化、输入与控制
  ▼
C/C++ Native 模块 ── XComponent / EGL / OpenGL ES：显示纹理
  │ C ABI          └─ OHAudio：消费 PCM
  ▼
Rust 通用运行时：update / draw / 游戏音频控制 / save / load
  ▼
现有 core + data + ui + framebuffer renderer + audio/APU
```

Rust 已提供 `aarch64-unknown-linux-ohos` 等目标和预编译标准库，但仍需配置 SDK Clang、sysroot 与 linker；该支持面向 OpenHarmony，商业 HarmonyOS 的 SDK、ABI 与具体设备组合仍须实测。[Rust 官方平台文档](https://doc.rust-lang.org/rustc/platform-support/openharmony.html)

用 DevEco Native C++ 工程装配外壳，把 Rust 静态库链接进提供 Node-API 的 `.so`，是一条可控的接入路线；ArkTS 与 C/C++ 的注册/调用机制有官方文档支持。[Node-API 开发流程](https://developer.huawei.com/consumer/en/doc/harmonyos-guides-V5/use-napi-process-V5)

画面通过 XComponent 的 NativeWindow 和 EGL/OpenGL ES 显示。已有 framebuffer 每帧只需展开为 92,160 字节，按 60 帧计算约 5.5 MB/s 原始像素数据；这一计算说明无需为该游戏先移植整套 GPU 抽象，但不是整机性能测试。用最近邻采样保持像素清晰，避免把每帧图像编码成 PNG 或在 ArkTS 中逐像素更新。[华为游戏渲染及窗口指南](https://developer.huawei.com/consumer/cn/doc/doccenter-games/games-universal-napi-xcomponent-0000002299097400)

音频保留 APU、曲目与音效控制，设备层改为 OHAudio。应让实际接收游戏音乐/SFX 命令的同一音源产生 PCM，处理采样率、欠载补零、暂停恢复和音频中断。官方游戏适配指南推荐 OHAudio，并建议游戏使用 `STREAM_USAGE_GAME`。[华为游戏音视频适配指南](https://developer.huawei.com/consumer/cn/doc/doccenter-games/games-universal-video-0000002630264564)

游戏时钟延续现有节拍，显示回调只负责调度；不能随 90/120 Hz 屏幕刷新次数推进逻辑。后台暂停后应重置计时基准与按键状态，避免恢复时大量补帧或持续走路。

## 改动边界

| 范围 | 建议改动 |
| --- | --- |
| 新增 `harmonyos/` | DevEco/Hvigor 工程、ArkUI 页面和按键、Native 桥接、窗口、音频、签名配置模板、资源打包 |
| 新增 `crates/pokered-harmonyos/` 或通用 `pokered-mobile/` | Rust C ABI、实例生命周期、输入状态、RGBA/PCM 接口、存档路径注入；PoC 可以先用鸿蒙专用小壳 |
| `pokered-app` | 增加可嵌入运行时入口和 feature 边界；桌面 CLI、热重载、设备初始化不进入鸿蒙构建 |
| `pokered-audio` | 拆分游戏音源/控制与设备输出；保留现有桌面与 Web 后端 |
| `pokered-renderer`、`pokered-ui` | 修正默认 feature 的传递，保留 framebuffer 和所需 resource 能力 |
| `dotzuki` 独立仓库 | `dotzuki-app` 无条件依赖 notify/默认 renderer，相关依赖需按需裁剪；有源码调整时发布新 tag 后在本仓库更新 |
| 根 workspace、脚本与 CI | 注册新 crate、交叉编译配置、fetch-gfx、HAP 构建、文档和验证任务 |
| 游戏内容 | 不计划重写战斗、地图、剧情、UI 布局、字体与音乐数据 |

预计涉及本仓库和引擎仓库两个边界。文件数量受“先独立 shell”还是“同步抽取所有移动端公共层”影响很大，不用代码复用百分比或精确行数作为报价依据。

## 工作量估算

以下各项是增量工作量，可相加；PoC 包含在总量内。

| 工作包 | 人日 | 可验收结果 |
| --- | ---: | --- |
| 工具链与最小 PoC | 3–5 | Rust OHOS 库链接进 HAP；真机启动、显示游戏一帧并响应输入 |
| 运行时及依赖拆分 | 4–7 | 鸿蒙依赖树排除桌面 GPU/设备后端；构造与资源加载可嵌入 |
| 画面、触控与生命周期 | 4–6 | 正确缩放、多点按键、前后台与 Surface 重建、稳定逻辑时钟 |
| OHAudio 接入 | 3–5 | BGM/SFX/叫声正确；切后台、音频中断、恢复无持续爆音 |
| 存档、资源与打包 | 2–4 | 冷启动 CONTINUE、选项/剧情标志持久化、离线运行、可重复构建 |
| 回归与真机修整 | 4–7 | 关键流程、画面对照、长时间音频和帧率检查、已有平台回归 |
| **合计** | **20–34** | **单机可维护测试版本** |

假设已有可用于开发的真机、签名条件和工具安装权限；只有一名实施者，不计并行压缩。若工程师不熟悉 ArkTS/NDK/FFI，建议额外预留 5–10 人日学习与集成时间。人工费用可按 `20–34 × 团队人日单价`计算，设备费用按已有资源另计；SDK ABI 或依赖出现重大兼容问题时需重估。

## 低成本替代：ArkWeb 承载现有 WASM

如果优先目标是尽快在鸿蒙设备上试玩，可先用 `pokered-web` 构建产物放进 ArkWeb。预估 **2–4 人日得到演示版本，5–10 人日得到经过基本离线、存档与生命周期验证的测试包**；两者是累计档位，不相加。

需验证实际 ArkWeb 内核的 WASM/WebGL/Web Audio 能力、触控映射、用户手势解锁音频、本地资源 URL/跨域与 MIME、存储持久化、恢复前台后的节拍。华为文档明确需根据 ArkWeb 内核版本核对 Web 标准支持，本地资源也存在跨域约束，所以这只是候选路线，不能承诺直接打包即用。[ArkWeb 简介](https://developer.huawei.com/consumer/cn/doc/doccenter-capabilities/web-component-overview)、[本地资源跨域说明](https://developer.huawei.com/consumer/cn/doc/doccenter-dev-faq/faqs-arkweb-174)

优点是避免 OHOS 原生 Cargo 依赖拆分，能快速验证设备覆盖。代价是表现受 Web 内核影响，需要单独测启动、耗电和输入/音频延迟。现有 Web BroadcastChannel 联机只适用于对应浏览上下文，不能算作跨设备联机支持。

不优先选择“升级/替换 winit、wgpu、cpal 后直接移植 Android shell”：当前锁定依赖未形成可直接复用的鸿蒙路径，而且会扩大已有桌面/Web/移动端回归范围。只有项目明确要求统一 GPU 窗口栈时，才值得单独做该路线验证。

## 建议实施顺序与验证限制

先投入 3–5 人日 PoC，依次验证工具链链接、运行时实例化、真机显示/按键和实际游戏音源 PCM 接出。PoC 必须从一开始避免依赖 ALSA/X11/Wayland；全平台公共层重构可在路线验证后完善。

随后完成标题→新游戏→移动/对话→战斗→菜单保存→杀进程→CONTINUE 的闭环，并核对剧情标志、选项与存档旁文件。测试前后台反复切换、Surface 重建、长按/多点触控、长时间音频及不同刷新率设备。共享 Rust 修改需跑相关模块回归；视觉 PR 按仓库规定提交同场景同帧的 master/分支前后截图，同时保存鸿蒙设备显示证据。

本次完成的是代码/依赖静态调查、Rust target cfg 检查及官方文档核验。本机未安装 OHOS Rust target，没有完成 HarmonyOS SDK 链接、HAP 构建或真机测试；未对游戏源代码做实现修改。华为部分文档正文直连超时，相关能力说明由搜索返回的官方页面内容核对，不能替代 SDK 实测。
