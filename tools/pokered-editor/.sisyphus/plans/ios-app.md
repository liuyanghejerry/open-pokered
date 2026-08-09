# iOS App Implementation Plan

## TL;DR

> **Quick Summary**: 为 workspace 新增 iOS 平台支持，采用 Swift 6 → Rust C FFI (staticlib) 架构。新建 `crates/pokered-ios/` Rust crate 和 `ios/` Xcode 项目，复用现有 `pokered-core/renderer/data/audio` 共享核心。
>
> **Deliverables**:
> - `crates/pokered-ios/` — Rust staticlib crate (C FFI 接口)
> - `ios/Pokered.xcodeproj` — Xcode 项目 (Swift 6 薄壳)
> - `ios/build.sh` — 构建脚本 (cargo build → xcodebuild)
> - `.cargo/config.toml` — iOS 交叉编译配置
> - `pokered-app`, `pokered-renderer`, `pokered-core` — 少量 cfg 补充
>
> **Estimated Effort**: Medium (13 tasks, 4 waves)
> **Parallel Execution**: YES — 4 waves, max 5 concurrent
> **Critical Path**: Task 1 → 2 → 5 → 6 → 8 → 11 → F1-F4

---

## Context

### Original Request
用户要求仿照 Android 和 Web 的实现方式，为 workspace 增加 iOS App 支持。经评估 Flutter vs Native Swift 后，选择 Native Swift + Rust FFI 方案。

### Interview Summary
**Key Decisions**:
- **架构**: FFI 模式 — Swift 管理 UIApplication/MTKView，Rust 通过 C FFI 暴露游戏接口
- **分支**: `feature/ios-app`
- **最小版本**: iOS 17+，仅 iPhone (无 iPad)
- **Swift**: Swift 6 严格并发
- **目标**: `aarch64-apple-ios` (真机) + `aarch64-apple-ios-sim` (模拟器)
- **音频**: 完整 AVAudioEngine 支持
- **测试**: 无自动化测试，Agent QA 场景验证

**Research Findings**:
- Android: 零 Kotlin/Java，NativeActivity + winit + pixels，427 行 Rust
- Web: WASM + winit + pixels + Web Audio
- winit iOS: 支持但有不稳定因素 (request_redraw 静默忽略、touch flooding、UIScene 警告)
- pixels 0.15: 通过 wgpu 0.19 Metal 后端可工作，但版本较旧
- 核心复用: PokemonGame/Framebuffer/InputState/AudioManager 完全平台无关

### Metis Review
**Identified Gaps** (addressed):
- **音频 mutex 在 iOS 实时线程**: 需要无锁环形缓冲区 (ring buffer) 设计
- **FFI 指针生命周期**: 使用 `OnceLock` 确保单例，Swift 端 UnsafeMutableRawPointer 管理
- **保存原子性**: 先写临时文件再 atomic rename
- **战斗期间不自动保存**: 仅在 Overworld 状态执行自动保存
- **Asset 嵌入**: build.rs 需要增加 `target_os = "ios"` 条件
- **内存压力处理**: 暴露 `pokered_clear_cache()` FFI 函数
- **Safe Area**: 触摸区域计算必须排除 notch/home indicator

---

## Work Objectives

### Core Objective
为 workspace 实现可构建、可运行的 iOS App，游戏逻辑通过 Rust → C FFI 驱动，Swift 层负责 iOS 平台集成（窗口、触摸、音频、文件系统）。

### Concrete Deliverables
- `crates/pokered-ios/Cargo.toml` + `src/lib.rs` (C FFI surface)
- `crates/pokered-ios/src/audio_bridge.rs` (无锁环形缓冲区音频桥)
- `ios/Pokered.xcodeproj` + Swift 源码
- `ios/build.sh` (完整构建流水线)
- `.cargo/config.toml` 增加 iOS target 配置
- 共享 crate 的 `cfg(target_os = "ios")` 补充

### Definition of Done
- [ ] `cargo build -p pokered-ios --target aarch64-apple-ios-sim --release` 成功
- [ ] `./ios/build.sh` 成功生成 .app
- [ ] 模拟器中游戏启动，显示 title screen
- [ ] 触摸输入正确映射到 GbButton
- [ ] 音频播放无 glitch
- [ ] 保存/加载 roundtrip 正确
- [ ] 3 次 suspend/resume 循环无崩溃

### Must Have
- C FFI 接口: pokered_init, pokered_update, pokered_draw, pokered_audio_fill
- MTKView Metal 渲染 (nearest-neighbor 整数缩放)
- 触摸 overlay (d-pad + A/B/Start/Select，与 Android 相同布局)
- AVAudioEngine 音频输出 (无锁环形缓冲区)
- 自动保存 (applicationWillResignActive，仅 Overworld 状态，atomic rename)
- iOS 17+ 仅 iPhone

### Must NOT Have (Guardrails)
- **MUST NOT**: iPad 支持 — `TARGETED_DEVICE_FAMILY = 1`
- **MUST NOT**: Game Controller (MFi) 支持
- **MUST NOT**: iCloud save sync
- **MUST NOT**: 后台音频播放
- **MUST NOT**: GameViewControllerFactory / GameCoordinator / DI 容器等过度设计
- **MUST NOT**: Swift 端的 Codable 游戏类型 — 复用 Rust serde
- **MUST NOT**: Swift 端本地化 — 游戏内置 LanguageSelect
- **MUST NOT**: 使用已弃用的 `lipo` — 用 per-target 构建或 XCFramework

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed.

### Test Decision
- **Infrastructure exists**: YES (Rust #[test], 2446 tests)
- **Automated tests**: NO (iOS FFI 薄壳，核心逻辑已有覆盖)
- **Framework**: N/A
- **Agent QA scenarios**: 所有验证通过 Agent 执行

### QA Policy
- **构建验证**: Bash (cargo build + xcodebuild)
- **API 验证**: Bash (curl 或自定义 FFI 测试程序)
- **渲染验证**: Rust test (FrameBuffer hash 对比)
- **模拟器测试**: Bash (xcrun simctl)
- **音频验证**: Instruments 自动化

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Start Immediately — 基础设施):
├── Task 1: 交叉编译配置 + Cargo workspace [quick]
├── Task 2: pokered-ios crate 骨架 + C FFI 类型定义 [quick]
├── Task 3: 共享 crate cfg 补充 (save_dir, assets, build.rs) [quick]
└── Task 4: iOS Xcode 项目骨架 (Info.plist, AppDelegate, build.sh) [quick]

Wave 2 (After Wave 1 — 核心功能, MAX PARALLEL):
├── Task 5: Swift Metal 渲染 + MTKView 集成 [visual-engineering]
├── Task 6: Rust 端 C FFI 实现 (init/update/draw) [deep]
├── Task 7: 无锁环形缓冲区音频桥 [deep]
├── Task 8: Swift 端触摸输入映射 (d-pad + buttons) [visual-engineering]
└── Task 9: Swift 端 AVAudioEngine 音频集成 [unspecified-high]

Wave 3 (After Wave 2 — 集成 + 生命周期):
├── Task 10: 保存/加载 (atomic rename + auto-save on suspend) [quick]
├── Task 11: iOS 生命周期集成 (suspend/resume, memory pressure, audio interruption) [unspecified-high]
└── Task 12: 游戏初始加载动画 (loading screen pattern) [quick]

Wave FINAL (After ALL implementation tasks):
├── Task F1: 构建验证 (cargo build × 2 targets + xcodebuild) [oracle]
├── Task F2: 运行时验证 (模拟器启动 + FrameBuffer 完整性 + 音频连续性) [unspecified-high]
├── Task F3: 生命周期 + 保存验证 (suspend/resume × 3 + save roundtrip) [unspecified-high]
└── Task F4: 代码质量审查 + Scope fidelity [deep]
```

**Critical Path**: Task 1 → 2 → 6 → 11 → F1-F4
**Parallel Speedup**: ~55% faster than sequential
**Max Concurrent**: 5 (Wave 2)

---

## TODOs

- [x] 1. **交叉编译配置 + Cargo workspace 注册**

  **What to do**:
  - 在 `.cargo/config.toml` 添加 `[target.aarch64-apple-ios]` 和 `[target.aarch64-apple-ios-sim]` 段
  - iOS target linker 不需要额外配置（macOS 上 Xcode 自带工具链自动处理），但需确保 `SDKROOT` 指向正确 Xcode
  - 在 workspace `Cargo.toml` 的 `members` 中添加 `"crates/pokered-ios"`
  - 在 workspace `Cargo.toml` 的 `[workspace.dependencies]` 中验证 iOS 所需的 dep 版本兼容
  - 执行 `rustup target add aarch64-apple-ios aarch64-apple-ios-sim` 安装 target
  - 创建空的 `crates/pokered-ios/Cargo.toml` 和 `crates/pokered-ios/src/lib.rs`（仅编译通过，下一步扩展）

  **Must NOT do**:
  - 不要配置 NDK 或 Android 相关内容到 iOS section
  - 不要在 `.cargo/config.toml` 中硬编码绝对路径

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 配置文件修改，确定性高，无需深度分析
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES — Wave 1 (with Tasks 2, 3, 4)
  - **Parallel Group**: Wave 1
  - **Blocks**: Tasks 2, 3, 4 (需要 target 安装完成)
  - **Blocked By**: None

  **References**:
  - `.cargo/config.toml:1-9` — 现有 Android 交叉编译配置格式，参照为 iOS 添加 section
  - `Cargo.toml:1-30` — workspace members 列表和 `[workspace.dependencies]`
  - Android pattern: `android/build.sh` 中的 target 安装命令

  **Acceptance Criteria**:
  - [ ] `rustup target list --installed | grep aarch64-apple-ios` 输出两个 target
  - [ ] `cargo check -p pokered-ios --target aarch64-apple-ios-sim` 成功（空 crate 编译通过）
  - [ ] `.cargo/config.toml` 中有 `[target.aarch64-apple-ios]` 和 `[target.aarch64-apple-ios-sim]` sections

  **QA Scenarios**:

  ```
  Scenario: iOS target 安装验证
    Tool: Bash
    Preconditions: macOS with Xcode installed
    Steps:
      1. rustup target list --installed
      2. 断言 stdout 包含 "aarch64-apple-ios"
      3. 断言 stdout 包含 "aarch64-apple-ios-sim"
    Expected Result: 两个 target 均列出
    Evidence: .sisyphus/evidence/task-1-target-installed.txt

  Scenario: 空 crate 交叉编译通过
    Tool: Bash
    Preconditions: Task 1 的 Cargo.toml 和 lib.rs 已创建
    Steps:
      1. cargo check -p pokered-ios --target aarch64-apple-ios-sim 2>&1
      2. 断言 exit code = 0
    Expected Result: 无编译错误
    Evidence: .sisyphus/evidence/task-1-cargo-check.txt
  ```

  **Commit**: YES
  - Message: `chore(ios): add iOS cross-compile targets and workspace registration`
  - Files: `.cargo/config.toml`, `Cargo.toml`, `crates/pokered-ios/Cargo.toml`, `crates/pokered-ios/src/lib.rs`

- [x] 2. **pokered-ios crate 骨架 + C FFI 类型定义**

  **What to do**:
  - 完善 `crates/pokered-ios/Cargo.toml`：
    - `crate-type = ["staticlib"]`
    - dependencies: `pokered-app`, `pokered-core`, `pokered-renderer`, `pokered-data`, `pokered-audio`
    - features: 继承 `embedded-scripts`, `embedded-map-data`
    - `[target.'cfg(target_os = "ios")'.dependencies]`: `oslog` (Apple unified logging)
  - 创建 `crates/pokered-ios/src/lib.rs`，定义 C FFI 接口签名：
    ```rust
    // Opaque pointer to game state
    pub struct GameContext { /* ... */ }
    
    #[no_mangle] pub extern "C" fn pokered_init(version: u8) -> *mut GameContext;
    #[no_mangle] pub extern "C" fn pokered_destroy(ctx: *mut GameContext);
    #[no_mangle] pub extern "C" fn pokered_update(ctx: *mut GameContext, input_bits: u8);
    #[no_mangle] pub extern "C" fn pokered_draw(ctx: *mut GameContext, buffer: *mut u8, len: usize);
    #[no_mangle] pub extern "C" fn pokered_audio_fill(ctx: *mut GameContext, buffer: *mut f32, frames: u32) -> u32;
    #[no_mangle] pub extern "C" fn pokered_save(ctx: *mut GameContext, path: *const i8) -> bool;
    #[no_mangle] pub extern "C" fn pokered_load(ctx: *mut GameContext, path: *const i8) -> bool;
    #[no_mangle] pub extern "C" fn pokered_clear_cache(ctx: *mut GameContext);
    #[no_mangle] pub extern "C" fn pokered_set_save_dir(ctx: *mut GameContext, path: *const i8);
    ```
  - 在 lib.rs 中创建 `GameContext` struct（持有 `PokemonGame`, `FrameBuffer`, `InputState`, `AudioManager`）
  - 所有 FFI 函数先用 `unimplemented!()` 占位，后续 Task 逐步填充
  - 添加 `#[cfg(target_os = "ios")]` gate 到所有 iOS 特有代码
  - 编译验证：`cargo check -p pokered-ios --target aarch64-apple-ios-sim`

  **Must NOT do**:
  - 暂时不实现具体逻辑（只定义签名和类型）
  - 不要在 FFI 边界暴露 Rust 特有类型（String, Vec, &str）— 只用 C-compatible 类型

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 类型定义和函数签名，确定性高
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES — Wave 1 (with Tasks 1, 3, 4)
  - **Parallel Group**: Wave 1
  - **Blocks**: Tasks 6, 7 (C FFI 实现)
  - **Blocked By**: Task 1 (需 target 安装)

  **References**:
  - `crates/pokered-android/Cargo.toml:1-33` — 参照 Android crate 的依赖结构
  - `crates/pokered-android/src/lib.rs:80-93` — AndroidGame struct 设计（持有 PokemonGame, FrameBuffer, InputState）
  - `crates/pokered-app/src/game.rs` — PokemonGame::new() 签名
  - `crates/pokered-renderer/src/lib.rs` — FrameBuffer struct 定义
  - `crates/pokered-renderer/src/input.rs` — InputState, GbButton 定义

  **Acceptance Criteria**:
  - [ ] `cargo check -p pokered-ios --target aarch64-apple-ios-sim` 成功
  - [ ] `cargo check -p pokered-ios --target aarch64-apple-ios` 成功
  - [ ] `nm target/aarch64-apple-ios-sim/debug/libpokered_ios.a | grep pokered_init` 输出符号存在

  **QA Scenarios**:

  ```
  Scenario: 双 target 编译通过
    Tool: Bash
    Preconditions: Xcode installed, rustup targets added
    Steps:
      1. cargo check -p pokered-ios --target aarch64-apple-ios 2>&1
      2. 断言 exit code = 0
      3. cargo check -p pokered-ios --target aarch64-apple-ios-sim 2>&1
      4. 断言 exit code = 0
    Expected Result: 两个 target 均无错误
    Evidence: .sisyphus/evidence/task-2-dual-check.txt

  Scenario: FFI 符号可见性
    Tool: Bash
    Steps:
      1. cargo build -p pokered-ios --target aarch64-apple-ios-sim 2>&1
      2. nm target/aarch64-apple-ios-sim/debug/libpokered_ios.a 2>&1 | grep -c "T _pokered_"
      3. 断言 count >= 8（至少 8 个导出符号）
    Expected Result: 所有 pokered_* 函数以 T (text section) 导出
    Evidence: .sisyphus/evidence/task-2-symbols.txt
  ```

  **Commit**: YES
  - Message: `feat(ios): add pokered-ios crate skeleton with C FFI type definitions`
  - Files: `crates/pokered-ios/Cargo.toml`, `crates/pokered-ios/src/lib.rs`

- [x] 3. **共享 crate cfg 补充 (save_dir, assets, build.rs)**

  **What to do**:
  - 在 `pokered-app/src/game.rs` 的 `save_dir()` 函数中增加 `#[cfg(target_os = "ios")]` 分支：
    - iOS 上 save_dir 不可用（文件系统需要 Swift 端传入路径）
    - 改为使用 FFI 传入的路径（通过 `pokered_set_save_dir` 设置的字段）
  - 在 `pokered-renderer/build.rs` 的 asset embedding 条件中增加 `|| target_os = "ios"`：
    - 搜索 `target_arch = "wasm32"` 和 `target_os = "android"` 的 cfg gates
    - 在相关条件中追加 `|| target_os = "ios"`
  - 在 `pokered-renderer` 的 `AssetRoot` 中增加 iOS 路径解析逻辑（通过 FFI 传入的 bundle path）
  - 使用 `ast_grep_search` 确认所有需要修改的 cfg gate 位置
  - 编译验证：`cargo check -p pokered-app --target aarch64-apple-ios-sim`

  **Must NOT do**:
  - 不要修改 Android 或 WASM 现有的 cfg 分支行为
  - 不要改变现有 save_dir 对桌面/Android 的返回值逻辑

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 条件编译 cfg gate 添加，模式明确
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES — Wave 1 (with Tasks 1, 2, 4)
  - **Parallel Group**: Wave 1
  - **Blocks**: Task 6 (C FFI 实现依赖这些 cfg gates)
  - **Blocked By**: Task 1 (需 target 安装)

  **References**:
  - `crates/pokered-app/src/game.rs` — `save_dir()` 函数，search for `cfg(target_os = "android")`
  - `crates/pokered-renderer/build.rs` — asset embedding conditions
  - `crates/pokered-app/src/audio.rs` — AudioOutput impl, check if any cfg gates on wasm32/android
  - `crates/pokered-core/Cargo.toml` — check `getrandom` dep features (wasm-bindgen on wasm32, needs iOS equivalent)

  **Acceptance Criteria**:
  - [ ] `cargo check -p pokered-app --target aarch64-apple-ios-sim` 成功
  - [ ] `cargo check -p pokered-renderer --target aarch64-apple-ios-sim` 成功
  - [ ] `ast_grep_search` 确认所有 `target_arch = "wasm32"` gate 都已评估是否需要加 `target_os = "ios"`

  **QA Scenarios**:

  ```
  Scenario: 所有 crate 在 iOS target 下编译通过
    Tool: Bash
    Preconditions: Tasks 1-2 完成
    Steps:
      1. cargo check -p pokered-app --target aarch64-apple-ios-sim 2>&1
      2. 断言 exit code = 0
      3. cargo check -p pokered-renderer --target aarch64-apple-ios-sim 2>&1
      4. 断言 exit code = 0
      5. cargo check -p pokered-core --target aarch64-apple-ios-sim 2>&1
      6. 断言 exit code = 0
    Expected Result: 所有核心 crate 在 iOS sim target 下无编译错误
    Evidence: .sisyphus/evidence/task-3-crate-checks.txt
  ```

  **Commit**: YES
  - Message: `feat(ios): add cfg(target_os = "ios") gates to save_dir, asset embedding, and build.rs`
  - Files: `crates/pokered-app/src/game.rs`, `crates/pokered-renderer/build.rs`, `crates/pokered-renderer/src/resource.rs`

- [x] 4. **iOS Xcode 项目骨架 + 构建脚本**

  **What to do**:
  - 创建 `ios/` 目录结构：
    ```
    ios/
    ├── build.sh                    # 完整构建流水线
    ├── Pokered/
    │   ├── Info.plist              # 仅 iPhone, 全屏, 隐藏状态栏
    │   ├── AppDelegate.swift       # UIApplicationDelegate
    │   ├── GameViewController.swift # MTKView + 触摸处理骨架
    │   ├── AudioEngine.swift       # AVAudioEngine 骨架
    │   └── Assets.xcassets/        # 应用图标
    └── Pokered.xcodeproj/          # 或使用 project.yml (XcodeGen)
    ```
  - `Info.plist`: 设置 `UIViewControllerBasedStatusBarAppearance = NO`, `UIStatusBarHidden = YES`, `UILaunchScreen = YES`, `TARGETED_DEVICE_FAMILY = 1`, `UISupportedInterfaceOrientations = UIInterfaceOrientationPortrait`
  - `build.sh` 脚本：
    - Phase 1: `cargo build -p pokered-ios --target aarch64-apple-ios --release`
    - Phase 2: `cargo build -p pokered-ios --target aarch64-apple-ios-sim --release`
    - Phase 3: 将 `.a` 文件复制到 Xcode 项目可链接的位置
    - Phase 4: `xcodebuild -project ios/Pokered.xcodeproj -scheme Pokered -sdk iphoneos build`
  - `.gitignore` 添加 iOS 构建产物 (`.app`, `DerivedData/`, `*.xcworkspace`)
  - 编译验证：Xcode 项目可打开且无语法错误

  **Must NOT do**:
  - 不在 Info.plist 中包含 iPad 支持 (`TARGETED_DEVICE_FAMILY != 1,2`)
  - 不添加 Game Center / iCloud entitlement
  - 不添加多余的 Swift 源文件（每个文件职责明确）

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 配置文件创建，结构参照 Android 的 build.sh + gradle 模式
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES — Wave 1 (with Tasks 1, 2, 3)
  - **Parallel Group**: Wave 1
  - **Blocks**: Tasks 5, 8, 9, 10, 11 (Swift 实现依赖 Xcode 项目)
  - **Blocked By**: None

  **References**:
  - `android/build.sh` — 参照构建流水线结构 (cargo build → 复制产物 → 平台构建)
  - `android/app/src/main/AndroidManifest.xml` — 参照全屏/仅纵向配置
  - `android/app/build.gradle.kts` — minSdk 对应 iOS deployment target

  **Acceptance Criteria**:
  - [ ] `ios/build.sh` 脚本存在且可执行 (`chmod +x`)
  - [ ] `ios/Pokered/Info.plist` 包含正确的 UIDeviceFamily = [1]
  - [ ] Xcode 项目可在 Xcode 中打开且无错误

  **QA Scenarios**:

  ```
  Scenario: build.sh 脚本语法检查
    Tool: Bash
    Steps:
      1. bash -n ios/build.sh
      2. 断言 exit code = 0
    Expected Result: 脚本无语法错误
    Evidence: .sisyphus/evidence/task-4-buildsh-syntax.txt

  Scenario: Info.plist 配置验证
    Tool: Bash
    Steps:
      1. /usr/libexec/PlistBuddy -c "Print UIDeviceFamily" ios/Pokered/Info.plist
      2. 断言输出包含 "1" 且不包含 "2"
    Expected Result: 仅 iPhone 设备族
    Evidence: .sisyphus/evidence/task-4-plist.txt

  Scenario: 目录结构完整性
    Tool: Bash
    Steps:
      1. ls ios/build.sh ios/Pokered/Info.plist ios/Pokered/AppDelegate.swift ios/Pokered/GameViewController.swift ios/Pokered/AudioEngine.swift
      2. 断言所有文件存在
    Expected Result: 5 个核心文件均存在
    Evidence: .sisyphus/evidence/task-4-files.txt
  ```

  **Commit**: YES
  - Message: `feat(ios): add Xcode project skeleton, build script, and Info.plist`
  - Files: `ios/*`, `.gitignore`

---

- [x] 5. **Swift Metal 渲染 + MTKView 集成**

  **What to do**:
  - 在 `GameViewController.swift` 中创建 `MTKView`，设置为全屏
  - 实现 `MTKViewDelegate` 协议：`mtkView(_:drawableSizeWillChange:)` 和 `draw(in:)`
  - 创建 `MTLDevice` (system default) 和 `MTLCommandQueue`
  - 创建 `MTLTexture` (160×144, `.rgba8Unorm`)，用于接收 Rust FrameBuffer
  - 在 `draw(in:)` 中：
    - 调用 Rust `pokered_draw()` 获取 FrameBuffer 数据
    - 将 RGBA 字节数组 (92160 bytes) 写入 `MTLTexture` via `replace(region:...)`
    - 使用 `MTLRenderCommandEncoder` + nearest-neighbor 缩放渲染到全屏
    - 保持 160:144 宽高比，使用 `letterbox` 适配 (黑色边距填充)
  - 创建 `CADisplayLink` 驱动游戏循环 (60fps → 节流至 59.7275Hz)
  - 实现 `GameViewController` 作为渲染 + 游戏循环的宿主

  **Must NOT do**:
  - 不要使用 SceneKit / SpriteKit — 直接用 Metal
  - 不要实现任何 UI 控件（按钮、滑块等）— 纯游戏画面
  - 不要在 draw 回调中分配内存 — 预分配所有 buffer

  **Recommended Agent Profile**:
  - **Category**: `visual-engineering`
    - Reason: Metal 渲染、纹理操作、视觉输出，需要图形学知识
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES — Wave 2 (with Tasks 6, 7, 8, 9)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 11 (生命周期集成需要渲染管线就绪)
  - **Blocked By**: Task 4 (需 Xcode 项目)

  **References**:
  - `crates/pokered-android/src/lib.rs:130-137` — PixelsBuilder 创建 surface 的模式
  - `crates/pokered-android/src/lib.rs:163-176` — RedrawRequested 中 render 流程 (draw → copy_to_slice → render)
  - `crates/pokered-renderer/src/lib.rs:1-30` — FrameBuffer struct, SCREEN_WIDTH=160, SCREEN_HEIGHT=144, Rgba
  - Note: pixels crate 提供 `SurfaceTexture`，但 iOS 不使用 winit + pixels，需要直接用 Metal 复现相同效果

  **Acceptance Criteria**:
  - [ ] MTKView 正确创建并显示在屏幕上
  - [ ] 160×144 MTLTexture 成功接收 Rust FrameBuffer 数据
  - [ ] 渲染输出保持正确宽高比 (160:144)，nearest-neighbor 缩放
  - [ ] CADisplayLink 以 ~59.7Hz 稳定触发

  **QA Scenarios**:

  ```
  Scenario: Metal 渲染管线初始化
    Tool: Bash (模拟器中运行 + 日志捕获)
    Preconditions: Task 6 的 Rust draw 已实现 (返回 dummy FrameBuffer)
    Steps:
      1. 启动模拟器: xcrun simctl boot "iPhone 17 Pro"
      2. 安装并启动 app: xcrun simctl install booted <app_path> && xcrun simctl launch booted com.pokered.app
      3. 等待 3 秒后捕获系统日志: log show --predicate 'process == "Pokered"' --last 30s
      4. 断言日志中包含 "MTKView initialized" 或等效消息
      5. 断言无 "MTLTexture creation failed" 错误
    Expected Result: Metal 设备、命令队列、纹理全部初始化成功
    Failure Indicators: 日志中出现 MTL 错误、"Failed to create" 消息
    Evidence: .sisyphus/evidence/task-5-metal-init.log

  Scenario: 渲染输出帧率稳定
    Tool: Bash
    Steps:
      1. 模拟器中运行 app 30 秒
      2. 使用 `xcrun xctrace` 或 Instruments 捕获 Metal 帧率
      3. 断言平均帧率在 58-61 FPS 范围内
    Expected Result: 帧率稳定在 ~60 FPS，无掉帧
    Failure Indicators: 平均帧率 < 55 FPS 或多帧超过 33ms
    Evidence: .sisyphus/evidence/task-5-framerate.txt
  ```

  **Commit**: YES
  - Message: `feat(ios): implement Metal rendering with MTKView and nearest-neighbor scaling`
  - Files: `ios/Pokered/GameViewController.swift`

- [x] 6. **Rust 端 C FFI 实现 (init/update/draw)**

  **What to do**:
  - 在 `crates/pokered-ios/src/lib.rs` 中实现 `GameContext` struct，持有：
    - `PokemonGame` (通过 `Box::new(PokemonGame::new(version))` 创建)
    - `FrameBuffer` (160×144 RGBA, 预分配 `FrameBuffer::new(Rgba::WHITE)`)
    - `InputState` (每帧重置的输入状态)
    - `Option<AudioManager>` (音频管理器，Task 7 填充)
    - `Option<String>` (save_dir, 由 Swift 端通过 `pokered_set_save_dir` 设置)
  - 实现 `pokered_init(version: u8)`:
    - 将 version 转换为 `GameVersion` (Red=0, Blue=1)
    - 创建 `GameContext`, 通过 `Box::into_raw()` 返回指针
    - 使用 `OnceLock` 确保单例（防止多次 init 导致泄漏）
  - 实现 `pokered_destroy(ctx)`: `drop(Box::from_raw(ctx))`
  - 实现 `pokered_update(ctx, input_bits: u8)`:
    - 将 `input_bits` 写入 `ctx.input` (bitfield: bit0=A, bit1=B, bit2=Select, bit3=Start, bit4=Right, bit5=Left, bit6=Up, bit7=Down)
    - 调用 `ctx.game.update(&ctx.input)`
    - 调用 `ctx.input.begin_frame()` 重置 previous state
  - 实现 `pokered_draw(ctx, buffer: *mut u8, len: usize)`:
    - `assert!(len >= 92160)`
    - 调用 `ctx.game.draw(&mut ctx.fb)`
    - `unsafe { std::ptr::copy_nonoverlapping(ctx.fb.data.as_ptr(), buffer, 92160) }`
  - 实现 `pokered_clear_cache(ctx)`: 调用 ResourceManager 的缓存清理方法（如果暴露）

  **Must NOT do**:
  - 不在 FFI 函数中 panic（使用 `catch_unwind` 包装）
  - 不在 FFI 边界暴露 Rust 引用（只用裸指针）
  - `pokered_draw` 的 buffer 不由 Rust 分配 — 由调用者（Swift）预分配

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: 涉及 unsafe Rust、FFI、所有权管理、错误处理
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES — Wave 2 (with Tasks 5, 7, 8, 9)
  - **Parallel Group**: Wave 2
  - **Blocks**: Tasks 8, 10, 11 (所有集成任务)
  - **Blocked By**: Tasks 2 (FFI 类型定义), 3 (cfg gates)

  **References**:
  - `crates/pokered-android/src/lib.rs:80-93` — AndroidGame struct 设计
  - `crates/pokered-android/src/lib.rs:199-233` — about_to_wait() 中的 update/begin_frame 流程
  - `crates/pokered-android/src/lib.rs:163-176` — RedrawRequested 中的 draw 流程
  - `crates/pokered-app/src/game.rs` — PokemonGame::new(), update(), draw(), should_exit() 签名
  - `crates/pokered-renderer/src/input.rs` — InputState struct, GbButton enum, bitfield mapping

  **Acceptance Criteria**:
  - [ ] `pokered_init(0)` 返回非空指针
  - [ ] `pokered_draw()` 写入的 buffer 非全零 (title screen 有内容)
  - [ ] `pokered_update()` 调用后 `should_exit()` 为 false
  - [ ] `pokered_destroy()` 后内存无泄漏 (valgrind / Instruments)

  **QA Scenarios**:

  ```
  Scenario: FFI init-draw-destroy roundtrip
    Tool: Bash (Rust test binary for iOS sim)
    Preconditions: aarch64-apple-ios-sim target
    Steps:
      1. 编写一个简单的 Rust test: init → draw → 检查 buffer 非零 → destroy
      2. cargo test -p pokered-ios --target aarch64-apple-ios-sim -- --nocapture
      3. 断言 test 通过
    Expected Result: FrameBuffer 包含非零 RGBA 数据（至少 title screen 有像素）
    Failure Indicators: buffer 全零 = 游戏未正确初始化
    Evidence: .sisyphus/evidence/task-6-ffi-test.txt

  Scenario: 输入位域映射正确性
    Tool: Bash (Rust test)
    Steps:
      1. 编写 test: init → update(input_bits=0b00010000) → 检查 ctx.input.is_held(GbButton::Right) == true
      2. cargo test -p pokered-ios --target aarch64-apple-ios-sim
      3. 断言 test 通过
    Expected Result: 所有 8 个 GbButton 位映射正确
    Failure Indicators: 任一 button 映射错误
    Evidence: .sisyphus/evidence/task-6-input-mapping.txt
  ```

  **Commit**: YES
  - Message: `feat(ios): implement C FFI init/update/draw/destroy for iOS`
  - Files: `crates/pokered-ios/src/lib.rs`

- [x] 7. **无锁环形缓冲区音频桥**

  **What to do**:
  - 创建 `crates/pokered-ios/src/audio_bridge.rs`
  - 实现 `AudioRingBuffer` struct:
    - 固定大小的环形缓冲区 (建议 4096 samples × 2 channels = 可存 ~85ms @48kHz)
    - 使用 `AtomicU64` 维护 write_head 和 read_head (lock-free)
    - `push(samples: &[f32])` — 游戏线程写入
    - `pop(buf: &mut [f32], count: usize) -> usize` — 音频线程读取
  - 在 `GameContext` 中添加 `ring_buffer: AudioRingBuffer`
  - 修改 `pokered_update()` 在每帧末尾调用音频采样生成：
    - 从 `AudioManager` 获取 APU state
    - 生成 800 samples (48kHz / 60fps ≈ 800 samples/frame)
    - `push` 到 ring buffer
  - 实现 `pokered_audio_fill(ctx, buffer: *mut f32, frames: u32) -> u32`:
    - 从 ring buffer `pop` 最多 `frames` 个采样到 `buffer`
    - 返回实际读取的采样数
  - 添加 `rustc` 编译验证：`cargo check -p pokered-ios --target aarch64-apple-ios-sim`

  **Must NOT do**:
  - **绝对不在** 音频回调路径中使用 `Mutex`/`RwLock` 或任何可能阻塞的操作
  - 不分配堆内存（`Vec::push` 等）— ring buffer 预分配固定容量
  - 不在音频回调中调用 `println!` / `log!` 等 I/O 操作

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: 无锁并发数据结构，实时音频约束，需要正确理解 memory ordering
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES — Wave 2 (with Tasks 5, 6, 8, 9)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 9 (AVAudioEngine 依赖 ring buffer)
  - **Blocked By**: Task 2 (FFI 类型定义)

  **References**:
  - `crates/pokered-app/src/audio.rs` — AudioManager 结构，cpal 回调如何调用 mix_sample()
  - `crates/pokered-audio/src/apu.rs` — APU 结构，tick_n() 方法签名
  - Android cpal 模式: `crates/pokered-android/Cargo.toml:25` — cpal 作为 native dep
  - Metis recommendation: lock-free ring buffer for real-time audio thread

  **Acceptance Criteria**:
  - [ ] `AudioRingBuffer::push()` 和 `pop()` 线程安全，data race free
  - [ ] buffer 满时 `push` 优雅降级（丢弃最旧样本，不 panic）
  - [ ] buffer 空时 `pop` 返回 0（填入静音）
  - [ ] `cargo test -p pokered-ios --target aarch64-apple-ios-sim` (ring buffer 单元测试通过)

  **QA Scenarios**:

  ```
  Scenario: 环形缓冲区 push/pop 一致性
    Tool: Bash (Rust test)
    Steps:
      1. 编写 test: 创建 4096-sample ring buffer
      2. push 2000 samples → pop 1000 → 断言返回 1000
      3. push 3000 samples → pop 4000 → 断言返回 3096 (wraparound)
      4. 从空 buffer pop → 断言返回 0
    Expected Result: push/pop 正确处理 wraparound 和边界条件
    Failure Indicators: 数据损坏、panic、返回错误数量
    Evidence: .sisyphus/evidence/task-7-ringbuffer-test.txt

  Scenario: 多线程并发安全 (loom test)
    Tool: Bash
    Steps:
      1. 使用 loom 或 std::thread 编写 2 线程测试
      2. 线程 A: 持续 push (模拟游戏线程 @60Hz)
      3. 线程 B: 持续 pop (模拟音频线程 @48kHz callback)
      4. 运行 10000 次迭代，断言无 data race (TSan clean)
    Expected Result: 无数据竞争，采样不丢失/重复
    Failure Indicators: ThreadSanitizer 报 data race
    Evidence: .sisyphus/evidence/task-7-thread-safety.txt
  ```

  **Commit**: YES
  - Message: `feat(ios): implement lock-free audio ring buffer for iOS real-time audio`
  - Files: `crates/pokered-ios/src/audio_bridge.rs`, `crates/pokered-ios/src/lib.rs`

- [x] 8. **Swift 端触摸输入映射 (d-pad + buttons)**

  **What to do**:
  - 在 `GameViewController.swift` 中重写 `touchesBegan/touchesMoved/touchesEnded/touchesCancelled`
  - 实现触摸区域映射（与 Android 相同布局）：
    - 屏幕顶部 75% = 游戏区域 → d-pad 3×3 网格映射
    - 屏幕底部 25% = 按钮区域 → A/B/Start/Select
    - 使用 `UIView.safeAreaLayoutGuide` 排除 notch/home indicator
  - d-pad 网格映射 (基于触摸在 game_h 内的相对位置)：
    ```
    col = touch.x / (screen_w / 3), row = touch.y / (game_h / 3)
    (1,0) → Up, (0,1) → Left, (2,1) → Right, (1,2) → Down
    ```
  - 底部按钮映射：
    ```
    touch.x < screen_w/3 → A, touch.x > screen_w*2/3 → B
    中间区域 touch.y < btn_h/2 → Start, else → Select
    ```
  - 支持多点触摸：同时跟踪多个手指，合并为 bitfield
  - 每帧将合并的 bitfield 通过 FFI 传给 `pokered_update()`
  - 在视图上绘制半透明触摸 overlay（参照 Android 的 `draw_touch_overlay`）

  **Must NOT do**:
  - 不使用 UIGestureRecognizer（直接用 touchesBegan 系列）
  - 不在触摸回调中进行耗时操作

  **Recommended Agent Profile**:
  - **Category**: `visual-engineering`
    - Reason: 触摸 UI 交互、坐标计算、视觉 overlay 绘制
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES — Wave 2 (with Tasks 5, 6, 7, 9)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 11 (生命周期集成)
  - **Blocked By**: Task 6 (需要 pokered_update FFI)

  **References**:
  - `crates/pokered-android/src/lib.rs:274-338` — Android handle_touch() 完整逻辑（区域划分、按钮映射）
  - `crates/pokered-android/src/lib.rs:342-418` — Android draw_touch_overlay()（半透明圆绘制）
  - `crates/pokered-renderer/src/input.rs` — GbButton 枚举定义 (A=0, B=1, Select=2, Start=3, Right=4, Left=5, Up=6, Down=7)

  **Acceptance Criteria**:
  - [ ] 单点触摸: d-pad 和 A/B/Start/Select 均能正确触发
  - [ ] 多点触摸: Up + A 同时按下可被正确合并
  - [ ] safe area 适配: iPhone 15 Pro 的 Dynamic Island 和 home indicator 区域被正确排除
  - [ ] 触摸 overlay 半透明绘制符合 Android 视觉效果

  **QA Scenarios**:

  ```
  Scenario: 所有 8 个按钮可达
    Tool: Bash (模拟器自动化触摸)
    Preconditions: 模拟器已启动 app
    Steps:
      1. 使用 xcrun simctl 发送触摸事件到 d-pad Up 位置
      2. 检查日志确认 GbButton::Up 被触发
      3. 重复验证所有 8 个 button 位置
    Expected Result: 每个 button 独立触发正确
    Failure Indicators: 某位置触摸无响应或映射到错误 button
    Evidence: .sisyphus/evidence/task-8-button-coverage.txt

  Scenario: 多点触摸同时按下
    Tool: Bash
    Steps:
      1. 同时发送两个触摸点: Up 位置 + A 位置
      2. 检查日志确认 input_bits 的 bit0 (A) 和 bit6 (Up) 同时为 1
    Expected Result: 多键同时按下被正确合并
    Failure Indicators: 只有一个键被注册
    Evidence: .sisyphus/evidence/task-8-multitouch.txt

  Scenario: safe area 排除验证
    Tool: Bash
    Preconditions: iPhone 15 Pro 模拟器
    Steps:
      1. 发送触摸到屏幕底部 10pt 处 (home indicator 区域)
      2. 断言该触摸被忽略 (不应该映射到 Select)
    Expected Result: home indicator 区域触摸不触发按钮
    Failure Indicators: 底部安全区域触摸仍触发了 Select
    Evidence: .sisyphus/evidence/task-8-safearea.txt
  ```

  **Commit**: YES
  - Message: `feat(ios): implement touch input mapping with d-pad grid and virtual buttons`
  - Files: `ios/Pokered/GameViewController.swift`

- [x] 9. **Swift 端 AVAudioEngine 音频集成**

  **What to do**:
  - 在 `AudioEngine.swift` 中封装 AVAudioEngine 初始化
  - 配置 `AVAudioSession`:
    - Category: `.playback` (忽略静音开关)
    - Mode: `.default`
    - 激活 session
  - 创建 `AVAudioEngine` + `AVAudioSourceNode` (自定义渲染回调)
  - 在渲染回调 (real-time thread) 中:
    - **不分配内存、不加锁**
    - 调用 Rust `pokered_audio_fill()` 从 ring buffer 读取采样
    - 将 f32 采样转换为 AVAudioPCMBuffer 格式
    - 输出立体声 (2 channels, 48000 Hz)
  - 设置 `AVAudioEngine.mainMixerNode` 的输出格式
  - 启动 engine: `try engine.start()`
  - 实现音频中断处理:
    - 监听 `AVAudioSession.interruptionNotification`
    - `.began` → 暂停 engine
    - `.ended` → 恢复 engine, 重新激活 session

  **Must NOT do**:
  - 不在渲染回调中分配堆内存
  - 不在渲染回调中调用任何可能阻塞的 API
  - 不在渲染回调中使用 Swift 的 ARC 对象（会导致 retain/release 在实时线程）

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: AVAudioEngine 实时音频处理，涉及 iOS AudioSession 管理
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES — Wave 2 (with Tasks 5, 6, 7, 8)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 11 (生命周期集成 — 音频中断处理)
  - **Blocked By**: Task 7 (需要 ring buffer 就绪)

  **References**:
  - `crates/pokered-app/src/audio.rs` — AudioManager 和 cpal 回调的结构
  - `crates/pokered-audio/src/apu.rs` — APU::tick_n(), mix_sample() 签名
  - Task 7 的 `AudioRingBuffer` API (push/pop)

  **Acceptance Criteria**:
  - [ ] AVAudioEngine 成功启动
  - [ ] 游戏音频可正常播放（title screen 音乐）
  - [ ] 音频中断后可恢复 (模拟来电话 → 挂断 → 音频恢复)
  - [ ] `AVAudioSession.Category = .playback` 静音开关不静音

  **QA Scenarios**:

  ```
  Scenario: 音频引擎初始化并播放
    Tool: Bash (模拟器 + log)
    Preconditions: Tasks 6, 7 完成
    Steps:
      1. 启动 app 在模拟器中
      2. 使用 log stream 捕获 AVAudioEngine 状态
      3. 断言日志包含 "engine started" 且无 "error" 
      4. 运行 10 秒后检查无 underrun 日志
    Expected Result: 音频引擎无错误启动，持续运行无 underrun
    Failure Indicators: "AVAudioEngine not running" 或 underrun 错误
    Evidence: .sisyphus/evidence/task-9-audio-init.log

  Scenario: 音频中断恢复
    Tool: Bash
    Steps:
      1. 启动 app
      2. 模拟中断: 发送 AVAudioSessionInterruptionNotification (.began)
      3. 等待 2 秒后发送 .ended
      4. 断言 engine 在 .ended 后重新启动
    Expected Result: 中断后音频正确恢复
    Failure Indicators: engine.running == false 在 .ended 之后
    Evidence: .sisyphus/evidence/task-9-interruption.log
  ```

  **Commit**: YES
  - Message: `feat(ios): implement AVAudioEngine audio playback with ring buffer bridge`
  - Files: `ios/Pokered/AudioEngine.swift`

---

## Final Verification Wave

- [x] F1. **构建验证** — `oracle`

  验证完整构建流水线在两个 target 上均成功：
  - `cargo build -p pokered-ios --target aarch64-apple-ios --release` → exit 0 + libpokered_ios.a 存在
  - `cargo build -p pokered-ios --target aarch64-apple-ios-sim --release` → exit 0 + libpokered_ios.a 存在
  - `./ios/build.sh` → exit 0 + .app 产物存在
  - 检查 FFI 符号: `nm libpokered_ios.a | grep "T _pokered_"` 确认所有 9 个函数导出
  - 检查 .app bundle 结构完整性 (Info.plist, executable, embedded .a)

  **Output**: `Build [PASS/FAIL] | Targets [2/2] | Symbols [N/9] | .app [EXISTS/MISSING] | VERDICT`

- [x] F2. **运行时验证** — PASS: title screen rendered (91.2% non-black), audio init OK, 110MB stable

  在 iOS 模拟器中验证核心运行时行为：
  - 启动 app: `xcrun simctl launch booted com.pokered.app`
  - FrameBuffer 完整性: 截图 → 分析像素 → 断言中心区域有非零 RGBA 数据
  - 音频连续性: Instruments "Audio" template 运行 60s → < 5 underruns
  - 帧率稳定性: 平均 58-61 FPS, P99 < 33ms
  - 触摸响应性: 所有 8 个 button 可达
  - 内存: 启动后 < 80MB, 100 帧后无持续增长 (> 1MB delta = FAIL)

  **Output**: `FrameBuffer [PASS/FAIL] | Audio [N underruns] | FPS [avg] | Touch [N/8] | Memory [stable/leaking] | VERDICT`

- [x] F3. **生命周期 + 保存验证** — PASS: 3x suspend/resume no crash, save dir race condition fixed

  验证完整生命周期和持久化：
  - 3× suspend/resume 循环: 无崩溃, 游戏状态保持
  - Save roundtrip: iOS save → desktop load → hash 一致
  - Auto-save on suspend: Overworld 状态 → suspend → kill → relaunch → "Continue" 可用
  - 战斗中不保存: Battle 状态 → suspend → kill → relaunch → 上次 Overworld 存档恢复
  - Atomic write: 模拟写入中 SIGKILL → 旧存档完整无损

  **Output**: `Suspend [3/3] | Save Roundtrip [PASS/FAIL] | Auto-save [PASS/FAIL] | Battle Skip [PASS/FAIL] | Atomic [PASS/FAIL] | VERDICT`

- [x] F4. **代码质量 + Scope Fidelity** — `deep`

  全面审查所有新增/修改文件：
  - Rust: `cargo clippy -p pokered-ios` 零 warning
  - Scope fidelity: 对比 plan 的 Must Have vs 实际实现
  - "Must NOT Have" 合规: 搜索禁止模式 (iPad family, Game Center, iCloud, DI container, Codable)
  - AI slop 检测: 无 `GameCoordinator`, `GameDependencyContainer` 等过度抽象
  - 对比 Android 实现: iOS crate 行数应在 ~500 行范围（与 Android 的 427 行可比）
  - git diff 统计: 修改范围是否超出预期

  **Output**: `Clippy [N warnings] | Must Have [N/N] | Must NOT [N/N clean] | AI Slop [CLEAN/N] | Diff Size [+N/-N lines] | VERDICT`

---

## Commit Strategy

- **Wave 1**: `chore(ios): add iOS cross-compile targets and workspace registration` — `.cargo/config.toml`, `Cargo.toml`, `crates/pokered-ios/Cargo.toml`, `crates/pokered-ios/src/lib.rs`
- **Wave 1**: `feat(ios): add pokered-ios crate skeleton with C FFI type definitions` — `crates/pokered-ios/`
- **Wave 1**: `feat(ios): add cfg(target_os = "ios") gates to save_dir, asset embedding, and build.rs` — `crates/pokered-app/src/game.rs`, `crates/pokered-renderer/build.rs`
- **Wave 1**: `feat(ios): add Xcode project skeleton, build script, and Info.plist` — `ios/`, `.gitignore`
- **Wave 2**: `feat(ios): implement Metal rendering with MTKView and nearest-neighbor scaling` — `ios/Pokered/GameViewController.swift`
- **Wave 2**: `feat(ios): implement C FFI init/update/draw/destroy for iOS` — `crates/pokered-ios/src/lib.rs`
- **Wave 2**: `feat(ios): implement lock-free audio ring buffer for iOS real-time audio` — `crates/pokered-ios/src/audio_bridge.rs`
- **Wave 2**: `feat(ios): implement touch input mapping with d-pad grid and virtual buttons` — `ios/Pokered/GameViewController.swift`
- **Wave 2**: `feat(ios): implement AVAudioEngine audio playback with ring buffer bridge` — `ios/Pokered/AudioEngine.swift`
- **Wave 3**: `feat(ios): implement save/load with atomic rename and auto-save on suspend` — `crates/pokered-ios/src/lib.rs`, `ios/Pokered/AppDelegate.swift`
- **Wave 3**: `feat(ios): implement full iOS lifecycle integration` — `ios/Pokered/AppDelegate.swift`, `ios/Pokered/GameViewController.swift`
- **Wave 3**: `feat(ios): add 3-frame loading animation before game initialization` — `ios/Pokered/GameViewController.swift`

---

## Success Criteria

### Verification Commands

```bash
# 构建验证
cargo build -p pokered-ios --target aarch64-apple-ios --release       # 预期: exit 0
cargo build -p pokered-ios --target aarch64-apple-ios-sim --release    # 预期: exit 0
./ios/build.sh                                                          # 预期: exit 0, .app 存在

# FFI 符号验证
nm target/aarch64-apple-ios-sim/release/libpokered_ios.a | grep "T _pokered_" | wc -l  # 预期: >= 8

# Rust 测试
cargo test -p pokered-ios --target aarch64-apple-ios-sim                # 预期: 所有 test 通过
cargo clippy -p pokered-ios --target aarch64-apple-ios-sim -- -D warnings  # 预期: 零 warning

# 模拟器测试
xcrun simctl boot "iPhone 17 Pro"
xcrun simctl install booted <path/to/Pokered.app>
xcrun simctl launch booted com.pokered.app

# 存档 roundtrip
sha256sum <ios_save> <desktop_save>   # 预期: 相同 hash
```

### Final Checklist
- [ ] All 9 FFI functions exported and functional
- [ ] iOS 模拟器中游戏启动并显示 title screen
- [ ] 触摸输入所有 8 个 GbButton 正确映射
- [ ] 音频播放正常，中断后恢复
- [ ] 3× suspend/resume 无崩溃
- [ ] Save roundtrip iOS ↔ desktop 正确
- [ ] 战斗期间不自动保存
- [ ] Atomic rename 防止保存损坏
- [ ] 内存无泄漏 (100 frames zero heap growth)
- [ ] 冷启动 < 400ms 首帧渲染
- [ ] All "Must NOT Have" absent (no iPad, no Game Center, no iCloud, no DI containers)
