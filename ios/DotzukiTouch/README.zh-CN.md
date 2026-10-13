# 原生触控覆盖层

> - **Audience**: rust developers, engine contributors
> - **Type**: how-to
> - **Status**: active
> - **Last verified**: v0.9.0
>
> 本文是 `README.md` 的中文翻译，同步至引擎版本 v0.9.0（源文档 commit 843113598fd46471c136faac0b5b4145b4104bde）。
> 内容以英文源为准；发现不一致请更新英文源再同步翻译。

将共享移动触控覆盖层接入 iOS 游戏，或修改 Android 和 HarmonyOS 导出器使用的控制器。

开始前，请阅读[移动架构](../../../docs/explanation/mobile-architecture.zh-CN.md)。
控制器输出[引擎](../../../docs/reference/glossary.md#engine)的八位输入，
不依赖具体游戏、渲染器或 Rust 运行时实例。

## iOS 接入

在 Xcode 中把本目录添加为本地 Swift package，链接 **DotzukiTouch** product。
组件要求 iOS 15 及以上，以 C++17 编译 Objective-C++ 源文件。
Swift 导入 `DotzukiTouch`，Objective-C 导入 `DotzukiTouchOverlay.h`。

1. 创建 `DTZTouchOverlay`，放在游戏渲染视图上方，并约束到宿主视图的完整边界。
   组件内部处理 `safeAreaInsets`；不要再次约束到安全区，否则会重复避让。
2. 设置 `onInputChanged`。回调提供触控位掩码，A、B、Select、Start、右、左、上、下
   分别占第 0–7 位。每次游戏 tick 调用一次 `consumeInput()`，将结果与实体手柄输入合并。
   此方法保留两帧之间的短按；输入回调本身只报告持续按住的状态。
3. 设置 `onSettingsVisibilityChanged`。设置编辑器显示时暂停游戏更新；
   只有编辑器关闭且应用 scene 处于活动状态时才恢复。
4. 宿主菜单接管输入、scene 失活或移除游戏视图前，调用 `releaseAll()`。
   组件也会在应用失活、窗口失焦、布局变化、触摸取消和离开窗口时释放输入。
5. 宿主模态窗口接管输入或实体手柄激活时，设置 `controlsEnabled = false`。
   触控恢复可用时重新启用。此操作会释放按键并隐藏覆盖层，不修改实体手柄输入。
6. 宿主支持正反两个横屏方向。游戏保持画面比例；背景可延伸到安全区之外，
   操作控件留在安全区内。

齿轮打开原生设置编辑器。设置以 `dotzuki-touch-v1` 为键保存在应用 `UserDefaults` 中，
提供大小、向内/垂直偏移、不透明度、左右交换、浮动方向盘、可选触觉反馈和恢复默认。
触觉反馈默认关闭。状态栏、Home 指示条和系统手势仍由宿主管理，组件不会禁用系统导航。

此 package 提供控制器，不包含游戏循环、音频、存档或 iOS 导出器。
SwiftUI 宿主可通过 `UIViewRepresentable` 嵌入视图，把两个回调转交给输入和暂停状态，
并确保存在能呈现设置编辑器的宿主视图控制器。

## Android 和 HarmonyOS

导出器把 `include/dotzuki_touch.hpp` 安装在移动 C ABI 头文件旁。
Android 通过 JNI 调用，HarmonyOS 通过 Node-API 调用。
控件叠加在完整游戏画面上：左下四向移动、右下 A/B、右上 Start/Select、左上设置。
左右交换会镜像整个布局，游戏 framebuffer 保持原有宽高比。

共享控制器使用 pt/dp/vp 逻辑坐标，平台层负责转换设备避让区与触点坐标。
每根手指持续拥有最初触碰的控件，直到释放。方向盘使用半径 18% 的中心死区、
斜向附近 1.22 的换轴比例和有限的跟踪半径。从 A 滑到 Start 不会触发 Start。
两指同时按住 A 时，必须全部释放才松开 A。窗口变化和取消事件会丢弃触点归属，
旧触点的 Move 事件不会重新触发输入。帧采样会保留 tick 之间的短按，
不会自动连发或排队输出互相冲突的方向。

A/B 的可见圆形小于触摸矩形。独立热区互不重叠，菜单热区为 64×48 逻辑单位。
控件可适配到 320×220 的安全区域；窗口更小时禁用触控，恢复可用尺寸后重新显示。
浮动方向盘只在移动侧的拇指区域内开始。

使用默认垂直位置时，固定拇指控件下方为 runner 底部对话预留视口高度的 22%，
展开的折叠屏和平板也按此比例预留。该空间只计算一次底部系统避让区。
小窗口会按需缩减预留空间，确保拇指控件位于顶部按钮下方。
向下移动控件或使用浮动方向盘时，控件可能进入这一区域；
对话面板更大的自定义游戏需要检查自己的布局。

## 验证

已完成共享控制器 sanitizer 测试、Android 模拟器原生测试、HarmonyOS 模拟器交互
和 UIKit 模拟器测试。HarmonyOS 与 iOS 真机上的运行行为仍需实机验证。

对话避让回归在 HarmonyOS 6.1.1 API 24 arm64 模拟器的 2584×1828 窗口中验证，
覆盖默认布局和大小为 1.2 的镜像布局：
[修复前](../../../docs/assets/harmony-touch/harmony-dialogue-before.png)、
[修复后](../../../docs/assets/harmony-touch/harmony-dialogue-after.png)、
[镜像布局](../../../docs/assets/harmony-touch/harmony-dialogue-mirrored.png)。
截图使用同一段 NPC 对话。底部对话区上方的选项菜单仍可能与 A/B 重叠；
通用移动 ABI 不提供这些菜单的边界。

从仓库根目录运行共享控制器测试：

```bash
workspace/crates/dotzuki-mobile/controls/tests/run.sh
```

测试使用 C++17 及地址/未定义行为检查，覆盖多指归属、连续换向、死区、
斜向切换缓冲、取消、布局变化、设置激活、安全区、镜像布局和浮动模式。
仓库 Linux CI 执行此测试。

原生验收与几何测试分开：构建导出的宿主、操作设置编辑器、同时按住方向和 A/B、
切换正反横屏、用系统界面打断游戏，确认返回后所有按键均已释放。
覆盖 16:9、超宽屏、挖孔设备，以及平板/折叠屏窗口变化。
iOS 接入还需验证宿主的 scene 暂停处理和手柄切换。

Android 原生视图测试位于 `tests/android/TouchInstrumentation.java`。
`scripts/test-android-touch.py --help` 描述如何针对已导出项目与 adb 设备执行测试。
脚本构建独立副本，使用应用标识 `com.dotzuki.touchregression`，不会修改输入项目。
测试检查全窗口布局、抬起一指后的触点 ID、失焦释放和尺寸变化释放。
安装的测试应用使用独立的存档与设置空间。

iOS package 的 `tests/ios` 包含 XCTest 接入测试；**iOS touch controls** 工作流
在 macOS runner 的 iPhone 模拟器中构建并测试组件。
UIKit 测试验证 Swift API 导入、布局命中、镜像偏好和释放回调，
并将覆盖层渲染图附加到测试结果。
