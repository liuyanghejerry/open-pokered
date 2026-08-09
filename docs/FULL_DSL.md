**本文档描述 DSL 的完整愿景。** 已实现部分见 `GAME_UI_DSL.md`。

# Game UI DSL 语法规则总结

## 实现状态

| 功能         | 状态     | 说明                                     |
| ------------ | -------- | ---------------------------------------- |
| `game_scene` | ✅ 已实现 | 场景定义（脚本 + UI）                    |
| `screen`     | ✅ 已实现 | 纯 UI 布局                               |
| `@variables` | ✅ 已实现 | 场景变量                                 |
| `@storylines`| ✅ 已实现 | 剧情流程                                 |
| `@theme`     | ✅ 已实现 | 颜色主题                                 |
| `@style`     | ✅ 已实现 | 样式复用（含继承链）                     |
| `@atlas`     | ✅ 已实现 | 纹理图集                                 |
| `ui { }`     | ✅ 已实现 | 内联 UI 布局                             |
| `@trigger`   | ✅ 已实现 | 故事线路由                               |
| `@speaker`   | ✅ 已实现 | 对话                                     |
| `@choice`    | ✅ 已实现 | 选择分支                                 |
| `@if/@else`  | ✅ 已实现 | 条件分支                                 |
| `@command`   | ✅ 已实现 | 游戏命令                                 |
| `panel`      | ✅ 已实现 | 边框容器                                 |
| `container`  | ✅ 已实现 | 无边框容器                               |
| `text`       | ✅ 已实现 | 文本组件                                 |
| `tile`       | ✅ 已实现 | 瓦片渲染（Pokered 扩展）                 |
| `divider`    | ✅ 已实现 | 分隔线（Pokered 扩展）                   |
| `list`       | ✅ 已实现 | 滚动列表                                 |
| `flex_list`  | ✅ 已实现 | 弹性列表（Pokered 扩展）                 |
| `button`     | ✅ 已实现 | 按钮                                     |
| `image`      | ✅ 已实现 | 图像                                     |
| `input`      | ✅ 已实现 | 输入框                                   |
| `dropdown`   | ✅ 已实现 | 下拉选择器                               |
| `rect`       | ✅ 已实现 | 瓦片坐标定位（Pokered 扩展）             |
| `@keyframes` | ❌ 未实现 | 动画定义                                 |
| `@audio`     | ❌ 未实现 | 音效                                     |
| `@resources` | ❌ 未实现 | 资源定义                                 |
| i18n (`@t`)  | ✅ 已实现 | 国际化：`@t("en", "中文")` 双语文本      |
| RTL          | ❌ 未实现 | 从右到左布局                             |
| 响应式       | ❌ 未实现 | 断点系统                                 |

## 一、核心语法速览

```
// 一个完整的游戏场景定义
game_scene 场景名 {
    @variables { ... }      // 数据
    @characters { ... }     // 角色
    @theme 主题名 { ... }   // 主题
    @style 样式名 { ... }   // 样式复用
    @atlas 图集名 { ... }   // 图片图集
    ui { ... }              // UI 布局
    @storylines { ... }     // 剧情流程
    @keyframes { ... }      // 动画
    @audio { ... }          // 音效
    map_layout { ... }      // 地图
}
```

---

## 二、基础规则

### 2.1 缩进规则

- 使用 **2 或 4 空格**缩进
- 同一层级缩进必须一致
- `{` 后内容增加一级缩进，`}` 后恢复

### 2.2 注释

```
// 单行注释
/* 多行注释 */
```

### 2.3 数据类型

| 类型   | 示例                        |
| ------ | --------------------------- |
| 字符串 | `"hello"`, `'world'`        |
| 数字   | `42`, `3.14`, `-10`         |
| 布尔   | `true`, `false`             |
| 数组   | `["a", "b", "c"]`           |
| 对象   | `{name="小红", age=18}`     |
| 绑定   | `"{gold}"`, `"{item.name}"` |

---

## 三、数据定义

### 3.1 @variables - 变量

```
@variables {
    变量名 = 初始值
    gold = 500
    player = { name = "RED", level = 5 }
    inventory = ["sword", "shield"]
}
```

### 3.2 @characters - 角色

```
@characters {
    角色名 = {
        name = "显示名称"
        sprite = "图片路径"
        avatar = "头像路径"
    }
}
```

### 3.3 @theme - 主题颜色

```
@theme 主题名 {
    颜色名 = 颜色值
    primary = "#ff0000"
    background = "#1a1a1a"
}
```

引用方式：`color = "@theme.primary"`

### 3.4 @style - 样式复用

```
@style 样式名 {
    属性 = 值
}
@style 子样式 : 父样式 { ... }

使用：@style 样式名
```

### 3.5 @atlas - 图集

```
@atlas 图集名 {
    source = "图片路径"
    regions = {
        区域名 = [x, y, 宽, 高, slice=数值]
    }
}
```

引用方式：`"@atlas.图集名.区域名"`

---

## 四、UI 组件

### 4.1 组件类型

| 组件          | 用途       |
| ------------- | ---------- |
| `panel`       | 带边框面板 |
| `container`   | 通用容器   |
| `text`        | 文字显示   |
| `button`      | 按钮       |
| `image`       | 图片       |
| `list`        | 列表       |
| `input`       | 输入框     |
| `dropdown`    | 下拉菜单   |
| `progressbar` | 进度条     |
| `slider`      | 滑块       |

### 4.2 通用属性

```
组件 {
    width/height = 数值 | "auto" | "100%"
    padding = 数值 | [上, 右, 下, 左]
    margin = 数值 | [上, 右, 下, 左]
    visible = true/false/"{绑定}"
    position = "absolute" | "relative"
    left/right/top/bottom = 数值
}
```

### 4.3 图片/九宫格

```
image("路径") {
    slice = 16                    // 统一切片
    slice = [上, 右, 下, 左]     // 分别指定
    scale = "stretch" | "repeat" | "fit"
    tint = "#ffffff"
}
```

### 4.4 按钮状态

```
button("文字") {
    @hover { scale = 1.02 }
    @pressed { scale = 0.98 }
    @disabled { opacity = 0.5 }
    on_click = "处理函数"
}
```

---

## 五、Flex 布局

### 5.1 容器属性

| 属性              | 值                                 | 默认      |
| ----------------- | ---------------------------------- | --------- |
| `display`         | `flex`, `block`                    | `block`   |
| `flex_direction`  | `row`, `column`                    | `row`     |
| `flex_wrap`       | `wrap`, `nowrap`                   | `nowrap`  |
| `justify_content` | `start`, `center`, `space_between` | `start`   |
| `align_items`     | `start`, `center`, `stretch`       | `stretch` |
| `gap`             | 数值                               | `0`       |

### 5.2 子项属性

| 属性          | 说明               |
| ------------- | ------------------ |
| `flex_grow`   | 放大比例           |
| `flex_shrink` | 缩小比例           |
| `flex_basis`  | 基础大小           |
| `flex = 1`    | 简写，等于 `1 1 0` |
| `order`       | 排列顺序           |

---

## 六、布局与样式属性

### 6.1 文本

```
text("内容") {
    align = "left" | "center" | "right" | "start" | "end"
    color = 颜色
    font_size = "small" | "normal" | "large" | "xl"
    weight = "normal" | "bold"
    dir = "ltr" | "rtl" | "auto"
}
```

### 6.2 边框

```
border = "single" | "double" | "rounded" | "none"
border_color = 颜色
border_width = 数值
```

### 6.3 逻辑属性（RTL 支持）

| 物理属性             | 逻辑属性              |
| -------------------- | --------------------- |
| `margin-left`        | `margin-inline-start` |
| `margin-right`       | `margin-inline-end`   |
| `padding-left/right` | `padding-inline`      |
| `text-align: left`   | `text-align: start`   |

---

## 七、剧情流程

### 7.1 对话

```
@speaker("角色名") {
    @mood("情绪")
    @avatar("头像路径")

    "对话内容"

    @pause(秒数)
    @play_sound("音效")
}
```

> **旁白形式 `@speaker("")`**：角色名为空字符串时，编译为不带任何前缀的
> `game.showText("…")`。原文中没有 "角色名：" 前缀的台词（旁白、系统提示、
> 名字已写进正文的对话）应使用此形式，而不是占位角色名——否则会把多余的
> `System: ` / `: ` 前缀泄漏到文本框里。（见 `js_storyline.rs` 的
> `compile_speaker` 与回归测试 `test_speaker_empty_name_no_prefix`。）

**语义固定**：`@speaker` 的语义是**玩家主动对话**——玩家走到 NPC 面前
按 A 触发（绑定在 `@trigger` 的 `npc` 上）并逐页按 A 推进。它**不**用于
剧情中 NPC 自动连续说话。

**剧情台词 `@say("角色名")`**：剧情（由 `@load` / `coord` / `on_enter`
自动触发的 storyline）中 NPC 连续、交错说话使用 `@say`——语义与
`@speaker` 明确分开，编译产物相同（都是 `game.showText`，按 A 推进）：

```
@say("OAK") { "Hey! Wait!" }
@say("") { "<RIVAL>: What's up?" }
@speaker("NPC") { "需要玩家主动搭话的台词" }
```

规则：**主动对话用 `@speaker`，剧情台词用 `@say`**。多个说话人轮番说话时
各自一个 `@say` 块。

### 7.2 选项

```
@choice {
    @option("选项文字") {
        // 选择后的逻辑
    }
    @option("另一个选项") { ... }
}
```

### 7.3 条件判断

```
@if (条件) {
    // 满足时执行
} @else if (另一条件) {
    // 另一条件满足
} @else {
    // 都不满足
}
```

### 7.4 循环

```
@each item in items {
    // 遍历 items 数组
}

@each (item, index) in items {
    // 带索引遍历
}
```

### 7.5 变量操作

```
变量名 = 新值                    // 直接赋值
@add_item("物品名", 数量)        // 添加物品
@remove_item("物品名", 数量)     // 移除物品
@give_item("角色", "物品")       // 给予角色
@give_item_to("角色", "物品")    // 同上
```

### 7.6 场景控制

```
@change_scene("场景名")           // 切换场景
@move_player_to(x, y)            // 移动玩家
@start_battle { ... }            // 开始战斗
@show_menu("菜单名")              // 显示菜单
@show_pokemon("宝可梦名")         // 显示宝可梦
@show_starter_menu               // 显示初始选择
```

### 7.7 音效与动画

```
@play_bgm("音乐路径")
@play_sound("音效路径")
@play_sound("音效") { loop = true }

@show_effect("特效名")
@animation("动画名")
```

---

## 八、响应式

### 8.1 断点

```
@media (min_width = 768px) { ... }
@mobile { ... }      // 手机
@tablet { ... }      // 平板
@desktop { ... }     // 桌面
@wide { ... }        // 宽屏
@rtl { ... }         // RTL 模式
@ltr { ... }         // LTR 模式
@orientation portrait { ... }
@orientation landscape { ... }
```

---

## 九、动画

### 9.1 关键帧

```
@keyframes 动画名 {
    0% { opacity = 0 }
    100% { opacity = 1 }
}
```

### 9.2 使用动画

```
组件 {
    animation = "动画名" {
        duration = 1.0
        repeat = "infinite" | 次数
        easing = "ease_in_out"
    }
}
```

### 9.3 内置动画

| 动画             | 效果   |
| ---------------- | ------ |
| `blink`          | 闪烁   |
| `pulse`          | 脉冲   |
| `shake`          | 抖动   |
| `slide_in_right` | 右滑入 |
| `bounce`         | 弹跳   |

---

## 十、国际化

### 10.1 声明

```
i18n {
    resources = ["locales/{lang}.i18n"]
    fallback = "en-US"
    default_lang = "zh-CN"
    rtl_languages = ["ar", "he"]
}
```

### 10.2 使用

```
text("{t('翻译键')}")
text("{t('翻译键', {参数=值})}")

// 复数
"{count, plural, =0{无} one{1个} other{{count}个}}"

// 格式化
"{date, date, long}"
"{price, number, currency}"
```

---

## 十一、事件

### 11.1 触发时机

```
@on_scene_enter { ... }      // 进入场景时
@on_interact_with("角色") { ... }  // 与角色交互
@on_map_encounter(...) { ... }     // 地图遭遇
```

### 11.2 UI 事件

```
on_click = "函数名"
on_change = "处理函数"
on_select = "选择处理"
```

---

## 十二、完整示例（精简版）

```
game_scene ShopScene {
    @variables { gold = 500 }

    @theme fantasy {
        primary = "#c9a03d"
        background = "#1a1a2e"
    }

    ui {
        panel {
            background = "@theme.background"
            padding = 24

            title = text("商店") {
                align = "center"
                color = "@theme.primary"
            }

            buy_button = button("购买") {
                on_click = """
                    @if (gold >= 100) {
                        gold -= 100
                        @speaker("商人") "谢谢惠顾！"
                    } @else {
                        @speaker("商人") "钱不够"
                    }
                """
            }
        }
    }
}
```

---

## 十三、关键词速查表

| 关键词                 | 用途         |
| ---------------------- | ------------ |
| `game_scene`           | 定义游戏场景 |
| `@variables`           | 定义变量     |
| `@characters`          | 定义角色     |
| `@theme`               | 定义主题     |
| `@style`               | 定义样式     |
| `@atlas`               | 定义图集     |
| `ui { }`               | UI 布局块    |
| `@storylines`          | 剧情块       |
| `@keyframes`           | 动画关键帧   |
| `@audio`               | 音频定义     |
| `map_layout`           | 地图布局     |
| `@speaker`             | 说话         |
| `@choice`              | 选项         |
| `@if/@else`            | 条件         |
| `@each`                | 循环         |
| `@hover/@pressed`      | 状态         |
| `@media`               | 响应式       |
| `@rtl/@ltr`            | 方向         |
| `t()`                  | 翻译         |
| `@add_item/@give_item` | 物品操作     |

---

**核心记忆点**：

- `@` 开头的是**指令/定义**
- `{}` 包裹**代码块**
- `=` 用于**赋值**
- `"{表达式}"` 是**数据绑定**
- 缩进表达**层级关系**
