**本文档描述已实现的 GUI DSL 语法。** 通用 Flex 布局、RTL、动画等高级特性仍为 proposal。（双语文本 `@t("en", "中文")` 已实现，见 §1.3。）

# Game UI DSL 语法规范

## 一、语法概述

**Game UI DSL** 是一种声明式 UI 描述语言，专为游戏界面设计。当前实现支持两种使用模式：

1. **独立 `.gui` 文件** — 纯 UI 布局定义，编译为 schema v2 JSON
2. **`.scene` 文件中的 `ui` 块** — 与游戏脚本共存的内联 UI 布局

### 1.1 已实现的核心特性

- **声明式语法** — 描述"是什么"而非"怎么做"
- **瓦片坐标定位** — 基于 20×18 瓦片网格的绝对定位（`rect`）
- **内置组件类型** — panel, container, text, button, tile, divider, list, flex_list, cursor, bracket, pixel_rect
- **自定义组件** — `component` 声明（构建期 prop schema）+ 游戏侧注册的 `custom:*` 元素（见 §5.13）
- **对象字面量** — `{key: value, ...}` 语法用于复杂属性
- **模板变量** — `{variable}` 运行时数据绑定
- **双语文本（i18n）** — `@t("en", "中文")` 内联本地化字符串（见 §1.3）
- **Schema v2 输出** — 编译为 pokered 渲染器期望的 JSON 格式

### 1.2 未实现的高级特性（Proposal）

- Flex 布局系统
- RTL（从右到左）布局支持
- 响应式设计和断点系统
- 动画和过渡效果
- 九宫格图片资源系统
- 主题和样式复用

### 1.3 双语文本（i18n）— `@t`

任何 `text(...)` 或 `button(...)` 的文本参数都可以用 `@t("英文", "中文")`
包裹，使其成为双语文本（第一个参数为英文 `en`，第二个为中文 `zh`）：

```
text(@t("TEXT SPEED", "文字速度")) { rect = {tx: 1, ty: 1, tw: 16, th: 1} }
button(@t("CANCEL", "取消"))      { rect = {tx: 2, ty: 16, tw: 8, th: 1} }
```

编译为 schema v2 JSON 时，`value` 字段会输出为按语言索引的对象：

```json
{ "type": "text", "value": { "en": "TEXT SPEED", "zh": "文字速度" } }
```

渲染器按当前语言（`DataContext` 的 `__lang`，默认 `en`）选择对应文本；
未提供的语言回退到 `en`、再到任意已有语言。普通字符串（无 `@t`）行为不变，
仍编译为单个字符串。`@t` 也可与模板绑定混用：
`@t("MONEY ${balance}", "金钱 ${balance}")`。

> 在 `.scene` 脚本 DSL 中，`@t(...)` 同样可用于 `@speaker` 文本与 `@option`
> 标签，编译为运行时的 `game.t("en", "zh")` 调用（见 `docs/JS_SCRIPT_I18N.md`）。

---

## 二、基础语法规则

### 2.1 语法结构

```ebnf
document            ::= (screen_declaration | component_declaration | global_definition)*
screen_declaration  ::= "screen" identifier ("{" block_content "}" | "=" screen_extension)
component_declaration ::= "component" identifier "{" (prop_schema)* "}"
prop_schema         ::= identifier ":" prop_kind ["required"]
prop_kind           ::= "int" | "string" | "bool" | "color" | "expr"
global_definition   ::= "@variables" | "@theme" | "@style" | "@resources" | "@atlas" | "@handlers" "{" block_content "}"
block_content       ::= (component | property_declaration | conditional_block | loop_block | i18n_declaration)*
component           ::= identifier? "=" component_type ("(" expression ")")? "{" block_content "}"
component_type      ::= "panel" | "container" | "text" | "list" | "button" | "image" | "input" | "dropdown"
                      | "tile" | "divider" | "flex_list" | "cursor" | "bracket" | "pixel_rect"
                      | declared_component_name        /* 编译为 custom:<name> */
property_declaration ::= identifier "=" value
value               ::= string | number | boolean | array | object | object_literal | binding | rtl_aware_value
object_literal      ::= "{" (identifier ":" value ("," identifier ":" value)*)? "}"
binding             ::= "{" expression "}"
expression          ::= variable | function_call | binary_op | ternary_op
```

### 2.2 缩进规则

- 使用**空格**缩进（推荐 2 或 4 空格）
- 同一层级的语句必须缩进一致
- `{` 后的内容增加一级缩进，`}` 恢复原缩进

### 2.3 注释

```
// 单行注释

/*
 * 多行注释
 * 可以跨行
 */
```

---

## 三、数据类型

### 3.1 基础类型

| 类型   | 示例                                   | 说明                           |
| ------ | -------------------------------------- | ------------------------------ |
| 字符串 | `"hello"`, `'world'`, `` `template` `` | 支持双引号、单引号、模板字符串 |
| 数字   | `42`, `3.14`, `-10`, `1.2e5`           | 整数或浮点数                   |
| 布尔值 | `true`, `false`                        | 小写                           |
| 空值   | `null`                                 | 表示无值                       |

### 3.2 复合类型

```
// 数组
colors = ["red", "green", "blue"]
margins = [10, 20, 10, 20]

// 对象
style = {
    color = "red"
    size = 14
}

// 多行对象（推荐）
item = {
    name = "Sword"
    price = 120
    tags = ["weapon", "melee"]
}
```

### 3.3 数据绑定与表达式

```
// 简单绑定
text = "{username}"
text = "你好，{username}！"

// 表达式
text = "{price + tax}"
visible = "{count > 0}"
text = "{items.length > 0 ? items[0].name : '无'}"

// 嵌套访问
text = "{user.profile.name}"

// 格式化
text = "{price:0.2f}"
text = "{date:YYYY-MM-DD}"
```

---

## 四、瓦片坐标系统（Pokered 特性）

### 4.1 坐标系

游戏画面使用 **20×18 瓦片网格**：
- `tx` — 瓦片列（0=左）
- `ty` — 瓦片行（0=上）
- `tw` — 宽度（瓦片数）
- `th` — 高度（瓦片数）

### 4.2 rect 属性

所有组件支持 `rect` 属性进行绝对定位：

```
panel {
    rect = {tx: 0, ty: 12, tw: 20, th: 6}
}

text("Hello") {
    rect = {tx: 1, ty: 13, tw: 18, th: 4}
}
```

### 4.3 模板变量

`rect` 的值可以是模板变量：

```
tile(223) {
    rect = {tx: "{cursor_x}", ty: 3, tw: 1, th: 1}
}
```

---

## 五、组件系统（已实现）

### 5.1 Panel — 边框容器

```
panel {
    rect = {tx: 0, ty: 12, tw: 20, th: 6}
    style = "default"              // "default" | "single" | "double" | 自定义对象
    text("内容") { rect = {tx: 1, ty: 13, tw: 18, th: 4} }
}
```

**style 属性：**
- `"default"` — 默认边框样式
- `"single"` / `"double"` — 单线/双线边框
- 自定义对象：
  ```
  style = {corner_tl: 99, edge_top: 100, corner_tr: 101, edge_left: 102, edge_right: 103, corner_bl: 108, edge_bottom: 111, corner_br: 110}
  ```

### 5.2 Container — 无边框容器

```
container {
    rect = {tx: 0, ty: 0, tw: 20, th: 18}
    layout = {gap: 0}
    clip = false
    visible = "{show_entry1}"
    text("子元素") { rect = {tx: 4, ty: 0, tw: 10, th: 1} }
}
```

### 5.3 Text — 文本组件

```
text("显示内容") {
    rect = {tx: 1, ty: 1, tw: 6, th: 1}
    color = "Black"                // "Black" | "DarkGray" | "LightGray" | "White" | "#rrggbb"
    align = "left"                 // "left" | "center" | "right"
    font = "pk_glyph"              // 字体名称
    wrap = "word"                  // "word" 启用换行
    line_spacing = 1               // 行间距（瓦片数）
}
```

**模板变量：**
```
text("{player_name}") {
    rect = {tx: 5, ty: 2, tw: 7, th: 1}
}
```

**value 别名：**
```
// 以下两种写法等价
text("Hello") { rect = {...} }
text { value = "Hello"; rect = {...} }
```

### 5.4 Tile — 瓦片渲染

```
tile(31) {
    rect = {tx: 18, ty: 16, tw: 1, th: 1}
}

tile("{sprite_index}") {
    rect = {tx: 15, ty: 4, tw: 2, th: 2}
    visible = "{has_selected}"
    flip_x = false
    flip_y = false
    palette = "name"
    repeat = 1                     // 水平重复次数
}
```

### 5.5 Divider — 分隔线

```
divider {
    rect = {tx: 1, ty: 9, tw: 18, th: 1}
    tiles = [122]                  // 瓦片 ID 数组
    repeat = 17                    // 重复次数
    orientation = "horizontal"     // "horizontal" | "vertical"
}
```

### 5.6 List — 滚动列表

```
list {
    rect = {tx: 1, ty: 1, tw: 11, th: 3}
    source = "{items}"             // 数据源模板变量
    item_template = {height: 1, gap: 1}
    cursor = {tile: 223, position: "left"}
    max_visible = 3
    selected = 0
    footer = "text"
}
```

**cursor 属性：**
- 简写：`cursor = {tile: 223}`（仅指定瓦片 ID）
- 完整：`cursor = {tile: 223, position: "left"}`

### 5.7 FlexList — 弹性列表

```
flex_list("{bag_items}") {
    rect = {tx: 1, ty: 4, tw: 18, th: 13}
    item_layout = [
        {field: "name", width: 14, align: "left"},
        {field: "qty", width: 3, align: "right", prefix: "x"}
    ]
    padding = {top: 1, left: 1}
    gap = 1
    cursor = {tile: 223, position: "left"}
    selected = 0
}
```

**item_layout 列定义：**
- `field` — 数据字段名
- `width` — 列宽（瓦片数）
- `align` — 对齐方式
- `prefix` — 值前缀（如 `"x"`, `"$"`）

### 5.8 Button — 按钮

```
button("确定") {
    rect = {tx: 10, ty: 15, tw: 5, th: 1}
    on_click = "handler"
}
```

### 5.9 Image — 图像

```
image("sprite.png") {
    rect = {tx: 0, ty: 0, tw: 7, th: 7}
    slice = "[8,8,8,8]"            // 九宫格切片
}
```

### 5.10 Input / Dropdown — 输入组件

```
input {
    rect = {tx: 0, ty: 0, tw: 20, th: 1}
    placeholder = "请输入..."
}

dropdown {
    rect = {tx: 0, ty: 0, tw: 10, th: 1}
}
```

### 5.11 Cursor — 选择光标

在 `rect.tx/ty` 基准点上按"基准 + 网格偏移"绘制选择符（▶）。最终位置为
`base_tx + col*col_step` / `base_ty + row*row_step`，`col`/`row` 为数据绑定：

```
cursor {
    rect = {tx: 5, ty: 14, tw: 1, th: 1}
    row = "{cursor}"          // 1-D 列表光标：仅设 row_step
    row_step = 2
}
```

- 1-D 列表光标：设 `row_step`，`row = "{cursor}"`
- 2-D 网格（战斗 FIGHT/PKMN/ITEM/RUN）：同时设 `col_step` + `row_step`
- 枚举偏移选择器（选项界面）：`col_step = 1`，`col = "{opt_index}"`
- 多光标界面（如 party 的 ▶ + ◆）：放多个 `cursor` 元素，各自带 `visible` 条件

### 5.12 Bracket / PixelRect — 像素图元

复刻 pokered-ui `Frame` 图元（括号框、原始矩形）的声明式版本，基于
painter 的 `draw_pixel_rect` 合成：

```
bracket {
    rect = {tx: 0, ty: 8, tw: 10, th: 4}
}

pixel_rect {
    rect = {tx: 2, ty: 2, tw: 4, th: 1}
}
```

### 5.13 自定义组件 — `component` 声明 + `custom:*` 元素

游戏特有的图元（如 Gen-I 血条）**不**内置在引擎里，而是由游戏侧注册为
`custom:*` 元素（`ElementRegistry`；pokered 见 pokered-ui 的
`custom_elements` 模块），并在 `.gui` 中用 `component` 声明其构建期 schema。
编译器会按声明校验每个使用点（缺必填 prop、prop 类型不符、未声明的 prop
都报编译错误）；运行时加载布局后还会按实现侧的 `schema()` 复验。

声明（通常集中在共享的 `components.gui`）：

```
// Gen-I HP bar：4px 高，按原版 GetHealthBarColor 阈值三色填充
component hp_bar {
  current: expr required
  max: expr required
}
```

prop 类型可取 `int` / `string` / `bool` / `color` / `expr`，可选 `required`
标记。使用点直接以声明名作为元素类型：

```
hp_bar {
    rect = {tx: 13, ty: 3, tw: 6, th: 1}
    current = "{hp}"
    max = "{max_hp}"
}
```

编译输出的 JSON 元素 `type` 为 `custom:hp_bar`，由游戏注册的实现负责渲染。

---

## 六、完整示例

### 6.1 对话框

```
screen Dialog {
    panel {
        rect = {tx: 0, ty: 12, tw: 20, th: 6}
        style = "default"
        text("{text}") {
            rect = {tx: 1, ty: 13, tw: 18, th: 4}
            wrap = "word"
            line_spacing = 1
        }
        tile(31) {
            rect = {tx: 18, ty: 16, tw: 1, th: 1}
        }
    }
}
```

编译输出：
```json
{
    "schema_version": 2,
    "screen": "Dialog",
    "elements": [
        {"type": "border", "rect": {"tx": 0, "ty": 12, "tw": 20, "th": 6}, "style": "default", "children": [
            {"type": "text", "rect": {"tx": 1, "ty": 13, "tw": 18, "th": 4}, "value": "{text}", "wrap": "word", "line_spacing": 1},
            {"type": "tile", "rect": {"tx": 18, "ty": 16, "tw": 1, "th": 1}, "tile_id": 31}
        ]}
    ]
}
```

### 6.2 背包界面

```
screen Bag {
    panel {
        rect = {tx: 6, ty: 0, tw: 8, th: 3}
        style = "default"
    }
    text("ITEM") {
        rect = {tx: 7, ty: 1, tw: 6, th: 1}
    }
    panel {
        rect = {tx: 0, ty: 3, tw: 20, th: 15}
        style = "default"
    }
    flex_list("{bag_items}") {
        rect = {tx: 1, ty: 4, tw: 18, th: 13}
        item_layout = [
            {field: "name", width: 14, align: "left"},
            {field: "qty", width: 3, align: "right", prefix: "x"}
        ]
        padding = {top: 1, left: 1}
        gap = 1
        cursor = {tile: 223, position: "left"}
    }
    text("CANCEL") {
        rect = {tx: 2, ty: 16, tw: 16, th: 1}
        color = "DarkGray"
    }
}
```

### 6.3 宝可梦列表

```
screen Party {
    text("No POKEMON!") {
        rect = {tx: 3, ty: 8, tw: 10, th: 1}
        visible = "{show_empty}"
    }
    container {
        rect = {tx: 0, ty: 0, tw: 20, th: 18}
        layout = {gap: 0}
        clip = false
        visible = "{show_entry1}"
        text("{mon1_name}") {
            rect = {tx: 4, ty: 0, tw: 10, th: 1}
        }
        text("L{mon1_level}") {
            rect = {tx: 14, ty: 0, tw: 3, th: 1}
        }
        text("{mon1_status}") {
            rect = {tx: 17, ty: 0, tw: 3, th: 1}
            color = "DarkGray"
        }
        text("{mon1_hp}") {
            rect = {tx: 14, ty: 1, tw: 6, th: 1}
        }
    }
    // ... mon2 到 mon6 结构相同
}
```

---

## 七、全局定义

### 4.1 变量定义

```
@variables {
    shop_items = [
        {icon="⚔️", name="铁剑", price=120, description="攻击力+10"},
        {icon="🛡️", name="木盾", price=80, description="防御力+5"}
    ]
    gold = 500
    config = {
        max_items = 99
        discount_rate = 0.1
    }
}
```

### 4.2 主题定义

```
@theme dark {
    primary = "#c9a03d"
    background = "#1a1a1e"
    surface = "#2a2a2e"
    text = "#ffffff"
    text_muted = "#888888"
    border = "#3a3a3e"
}

@theme light {
    primary = "#0066cc"
    background = "#f5f5f5"
    surface = "#ffffff"
    text = "#333333"
    text_muted = "#888888"
    border = "#dddddd"
}

screen Main {
    theme = "dark"
    // ...
}
```

### 4.3 样式复用

```
@style card {
    border = "rounded"
    padding = 12
    background = "@theme.surface"
    display = "flex"
    gap = 8
}

@style card_hover : card {
    background = "@theme.primary"
    scale = 1.02
}

product_card = container {
    @style card

    @hover {
        @style card_hover
    }
}
```

### 4.4 资源定义

```
@resources {
    // 普通图片
    logo = image("assets/logo.png")

    // 九宫格图片
    panel_default = image("ui/panel_default.9.png") {
        slice = [24, 24, 24, 24]
        scale_mode = "stretch"
    }

    panel_hover = image("ui/panel_hover.9.png") {
        slice = 24
    }

    // 平铺纹理
    wood_texture = image("ui/wood_tex.jpg") {
        repeat = "tile"
        tile_size = [64, 64]
    }
}
```

### 4.5 图集定义

```
@atlas "game_ui" {
    source = "assets/ui/atlas.png"
    layout = "grid"           // grid | packed
    cell_size = [128, 128]    // 网格模式

    // 定义图集中的区域
    regions = {
        panel_bg = [0, 0, 128, 128, slice=16]
        panel_bg_hover = [128, 0, 128, 128, slice=16]
        button = [0, 128, 64, 64, slice=[8,8,8,8]]
        button_hover = [64, 128, 64, 64, slice=[8,8,8,8]]
        icon_gold = [0, 192, 32, 32]
        icon_sword = [32, 192, 32, 32]
    }
}
```

---

## 五、组件系统

### 5.1 Panel - 面板组件

```
panel {
    // 字符边框（轻量级）
    border = "single"          // single | double | rounded | none

    // 图片背景（九宫格）
    background_image = "ui/panel.9.png" {
        slice = [20, 20, 20, 20]   // [top, right, bottom, left]
        scale = "stretch"           // stretch | repeat | repeat_x | repeat_y | fit | fill
        tint = "#ffffff"            // 着色
        opacity = 1.0
        auto_slice = true           // .9.png 自动读取切片
    }

    // 多层背景
    backgrounds = [
        { type = "gradient", colors = ["#1a1a2e", "#16213e"] },
        { type = "image", source = "ui/noise.png", blend = "multiply", opacity = 0.3 },
        { type = "nine_slice", source = "ui/panel_frame.png", slice = 16 }
    ]

    // 边框图片
    border_image = "ui/border.png" {
        slice = [10, 10, 10, 10]
        inset = 2
    }

    // 内边距（确保内容在安全区内）
    padding = 24
    margin = [4, 8]
    background = "@theme.background"
    width = "auto"
    height = "auto"
    title = text("标题")
}
```

### 5.2 Container - 通用容器

```
container {
    // 无默认边框，支持所有布局属性
    display = "flex"           // block | flex | inline
    // 继承所有布局和样式属性
}
```

### 5.3 Text - 文本组件

```
text("显示内容") {
    align = "left"             // left | center | right | justify | start | end
    color = "@theme.text"
    background = "transparent"
    font_size = "normal"       // small | normal | large | xl
    weight = "normal"          // normal | bold
    italic = false
    underline = false
    margin = [0, 0, 4, 0]
    dir = "auto"               // ltr | rtl | auto
    unicode_bidi = "normal"    // normal | embed | bidi-override
}
```

### 5.4 List - 列表组件

```
list(source=items, max_visible=10) {
    each = "{icon} {name} - ${price}"
    spacing = 2
    cursor = "arrow"           // arrow | hand | none
    selected_bg = "@theme.primary"
    on_select = "handler"
    scrollbar_position = "end" // LTR=右侧，RTL=左侧
}
```

### 5.5 Button - 按钮组件

```
button("确定") {
    // 样式变体
    style = "primary"          // primary | secondary | danger | success | ghost

    // 图片背景
    background_image = "@atlas.game_ui.button" {
        slice = 8
    }

    // 状态样式
    @hover {
        background_image = "@atlas.game_ui.button_hover"
        scale = 1.02
    }

    @pressed {
        scale = 0.98
        offset_y = 2
    }

    @disabled {
        grayscale = true
        opacity = 0.6
    }

    // 基础属性
    width = 20
    height = 3
    action = "submit"
    disabled = false
    icon = "✓"
    icon_position = "start"    // start | end | top | bottom
    on_click = "handler"
}
```

### 5.6 Image - 图像组件

```
image("{url}") {
    width = 40
    height = 10
    object_fit = "cover"       // cover | contain | fill
    slice = [10, 10, 10, 10]   // 九宫格切片
    scale_mode = "stretch"
    repeat = "none"            // none | tile | repeat_x | repeat_y
}
```

### 5.7 Input - 输入框组件

```
input {
    placeholder = "请输入..."
    value = "{input_text}"
    max_length = 50
    password = false
    dir = "auto"
    on_change = "handler"
    on_enter = "submit"
}
```

### 5.8 Dropdown - 下拉选择器

```
dropdown(source=languages) {
    value = "{selected_lang}"
    placeholder = "选择语言"
    on_change = "switch_language"
}
```

### 5.9 ProgressBar - 进度条组件

```
progressbar(value=75, max=100) {
    width = 200
    height = 16
    background_color = "@theme.surface"
    fill_color = "@theme.primary"
    border = "single"
    show_percent = true
}
```

### 5.10 Slider - 滑块组件

```
slider(value=50, min=0, max=100) {
    width = 200
    track_image = "ui/slider_track.png" {
        slice = [4, 4, 4, 4]
    }
    handle_image = "ui/slider_handle.png"
    on_change = "on_volume_change"
}
```

---

## 六、Flex 布局系统

### 6.1 容器属性

| 属性              | 可选值                                                                    | 默认值    | 说明               |
| ----------------- | ------------------------------------------------------------------------- | --------- | ------------------ |
| `display`         | `block`, `flex`, `inline`                                                 | `block`   | 布局模式           |
| `flex_direction`  | `row`, `column`, `row_reverse`, `column_reverse`, `inline`, `block`       | `row`     | 主轴方向           |
| `flex_wrap`       | `nowrap`, `wrap`, `wrap_reverse`                                          | `nowrap`  | 换行行为           |
| `justify_content` | `start`, `end`, `center`, `space_between`, `space_around`, `space_evenly` | `start`   | 主轴对齐（逻辑值） |
| `align_items`     | `start`, `end`, `center`, `stretch`, `baseline`                           | `stretch` | 交叉轴对齐         |
| `align_content`   | `start`, `end`, `center`, `space_between`, `space_around`, `stretch`      | `stretch` | 多行对齐           |
| `gap`             | 数值                                                                      | `0`       | 统一间距           |
| `row_gap`         | 数值                                                                      | `0`       | 行间距             |
| `column_gap`      | 数值                                                                      | `0`       | 列间距             |

### 6.2 子项属性

| 属性          | 类型                                        | 默认值 | 说明                         |
| ------------- | ------------------------------------------- | ------ | ---------------------------- |
| `flex_grow`   | 数字                                        | `0`    | 放大比例                     |
| `flex_shrink` | 数字                                        | `1`    | 缩小比例                     |
| `flex_basis`  | 数值/`auto`/`content`                       | `auto` | 基础大小                     |
| `flex`        | 简写                                        | -      | `grow shrink basis` 或单数字 |
| `align_self`  | `auto`, `start`, `end`, `center`, `stretch` | `auto` | 覆盖对齐                     |
| `order`       | 整数                                        | `0`    | 排列顺序                     |

### 6.3 简写示例

```
container {
    flex = 1                // flex-grow: 1, flex-shrink: 1, flex-basis: 0
    flex = "0 0 200px"      // 固定宽度
    flex = "1 1 auto"       // 完整形式
}
```

---

## 七、国际化（i18n）系统

### 7.1 资源声明

```
screen Shop {
    i18n {
        resources = ["locales/{lang}.i18n"]
        fallback = "en-US"
        default_lang = "zh-CN"
        auto_detect = true
        rtl_languages = ["ar", "he", "fa", "ur"]
        auto_rtl = true
    }
    // ...
}
```

### 7.2 翻译函数

```
// 基础翻译
text("{t('shop.title')}")

// 带参数
text("{t('shop.gold', {amount=gold})}")

// 复数（ICU 格式）
text("{count, plural, =0{无商品} one{1件商品} other{{count}件商品}}")

// 选择（性别等）
text("{gender, select, male{他} female{她} other{他们}}购买了{item}")

// 上下文区分
text("{t('sword', {context='weapon'})}")
```

### 7.3 格式化

```
// 日期时间
text("{date, date, long}")
text("{time, time, medium}")

// 数字
text("{price, number, currency}")
text("{rate, number, percent}")

// 自定义
text("{date, date, ::yyyy年MM月dd日}")
```

### 7.4 组件内嵌翻译

```
@trans key="welcome_with_link" {
    link = button("这里") {
        action = "open_link"
    }
}
```

---

## 八、RTL（Right-to-Left）支持

### 8.1 方向声明

```
screen MyScreen {
    dir = "rtl"              // ltr | rtl | auto

    panel {
        // 继承屏幕方向
    }
}
```

### 8.2 逻辑属性

| 物理属性             | 逻辑属性              | 说明             |
| -------------------- | --------------------- | ---------------- |
| `margin-left`        | `margin-inline-start` | 内联方向起始边距 |
| `margin-right`       | `margin-inline-end`   | 内联方向结束边距 |
| `margin-left/right`  | `margin-inline`       | 内联方向双边距   |
| `margin-top/bottom`  | `margin-block`        | 块方向边距       |
| `padding-left/right` | `padding-inline`      | 内联方向内边距   |
| `text-align: left`   | `text-align: start`   | 逻辑对齐         |
| `inset-left`         | `inset-inline-start`  | 逻辑定位         |

### 8.3 RTL 条件块

```
container {
    margin_inline_start = 10

    @rtl {
        // 仅在 RTL 模式下生效
        justify_content = "end"
    }

    @ltr {
        // 仅在 LTR 模式下生效
        justify_content = "start"
    }
}

// 属性级覆盖
margin_left = 10 {
    rtl = "margin_right"
}
```

### 8.4 图标自动翻转

```
icon("arrow-right") {
    auto_flip = true         // RTL 下自动翻转
    flip_in_rtl = true
}

icon("hand-pointer") {
    auto_flip = false        // 某些图标不翻转
}
```

---

## 九、条件渲染与循环

### 9.1 条件渲染

```
@if (user.is_logged_in) {
    welcome = text("欢迎回来，{user.name}！")
} @else if (user.is_guest) {
    guest = text("访客模式")
} @else {
    login_button = button("登录")
}
```

### 9.2 循环

```
// 简单循环
@each item in items {
    card = container {
        text(item.name)
        text(item.price)
    }
}

// 带索引
@each (item, index) in items {
    item = text("{index + 1}. {item.name}")
}

// 内置列表（高性能）
list(source=items, max_visible=10) {
    each = "{name} - ${price}"
}
```

---

## 十、响应式设计

### 10.1 断点语法

```
panel {
    display = "flex"
    flex_direction = "column"

    @media (min_width = 768px) {
        flex_direction = "row"
        gap = 20
    }

    @media (max_width = 480px) {
        padding = 8
    }

    // 命名断点
    @mobile { flex_direction = "column" }
    @tablet { flex_direction = "row" }
    @desktop { justify_content = "center" }
    @wide { max_width = 1200 }

    // 横竖屏
    @orientation portrait { /* ... */ }
    @orientation landscape { /* ... */ }
}
```

### 10.2 内置断点

| 断点       | 最小宽度 | 适用设备 |
| ---------- | -------- | -------- |
| `@mobile`  | 0        | 手机     |
| `@tablet`  | 768px    | 平板     |
| `@desktop` | 1024px   | 桌面     |
| `@wide`    | 1440px   | 宽屏     |

---

## 十一、动画与过渡

### 11.1 状态过渡

```
panel {
    background_image = "ui/panel_default.png"

    transitions = {
        hover = {
            duration = 0.15
            easing = "ease_out"
            properties = ["scale", "opacity", "background_image"]
        }

        press = {
            duration = 0.05
            easing = "linear"
        }
    }
}
```

### 11.2 内置动画

```
panel {
    background_image = "ui/panel.png" {
        animation = "pulse" {
            duration = 2.0
            from_opacity = 0.8
            to_opacity = 1.0
            repeat = "infinite"
        }
    }
}

// 自定义动画
@keyframes float {
    0% { transform = "translateY(0px)" }
    50% { transform = "translateY(-10px)" }
    100% { transform = "translateY(0px)" }
}

floating_icon = image("icon.png") {
    animation = "float" {
        duration = 2.0
        repeat = "infinite"
        easing = "ease_in_out"
    }
}
```

---

## 十二、事件与交互

### 12.1 事件绑定

```
button("点击") {
    on_click = "handle_click"
    on_hover = "show_tooltip"
    on_focus = "highlight"
}

input {
    on_change = "validate"
    on_enter = "submit"
}

list {
    on_select = "load_details"
    on_double_click = "open"
}
```

### 12.2 动作定义

```
button("关闭") {
    action = "close"                  // 内置动作
    action = "execute:buy_item"       // 带参数
    confirm = "确定吗？"               // 确认对话框
    on_click = "save_and_close"       // 自定义处理
}
```

---

## 十三、完整示例：游戏商店界面

```
@variables {
    shop_items = [
        {icon="⚔️", name_key="items.sword", price=120, description_key="desc.sword", type="weapon"},
        {icon="🛡️", name_key="items.shield", price=80, description_key="desc.shield", type="armor"},
        {icon="💊", name_key="items.potion", price=30, description_key="desc.potion", type="consumable"},
        {icon="📜", name_key="items.scroll", price=200, description_key="desc.scroll", type="consumable"},
        {icon="👢", name_key="items.boots", price=60, description_key="desc.boots", type="armor"}
    ]
    gold = 500
    selected_item = null
    item_filter = "all"
}

@atlas "game_ui" {
    source = "assets/ui/atlas.png"
    layout = "packed"

    regions = {
        panel_bg = [0, 0, 256, 256, slice=32]
        panel_bg_hover = [256, 0, 256, 256, slice=32]
        panel_dark = [512, 0, 256, 256, slice=32]
        button_primary = [0, 256, 128, 64, slice=[12,12,12,12]]
        button_primary_hover = [128, 256, 128, 64, slice=[12,12,12,12]]
        button_secondary = [256, 256, 128, 64, slice=[12,12,12,12]]
        icon_gold = [0, 320, 32, 32]
        icon_sword = [32, 320, 32, 32]
        icon_shield = [64, 320, 32, 32]
        icon_potion = [96, 320, 32, 32]
        icon_scroll = [128, 320, 32, 32]
    }
}

@theme fantasy {
    primary = "#c9a03d"
    primary_dark = "#a07828"
    background = "#1a1a2e"
    surface = "#16213e"
    text = "#eeeeee"
    text_muted = "#888888"
    success = "#4caf50"
    error = "#f44336"
    warning = "#ff9800"
}

@style card {
    background_image = "@atlas.game_ui.panel_bg"
    slice = 16
    padding = 12
    border = "none"
    display = "flex"
    flex_direction = "column"
    gap = 8

    @hover {
        background_image = "@atlas.game_ui.panel_bg_hover"
        scale = 1.02
        transition_duration = 0.1
    }
}

@style filter_button {
    background_image = "@atlas.game_ui.button_secondary"
    slice = 12
    padding = [4, 12]

    @pressed {
        scale = 0.95
    }
}

@style filter_button_active : filter_button {
    background_image = "@atlas.game_ui.button_primary"
    style = "primary"
}

screen FantasyShop {
    i18n {
        resources = ["locales/{lang}.i18n"]
        fallback = "en-US"
        default_lang = "zh-CN"
        rtl_languages = ["ar", "he"]
        auto_rtl = true
    }

    theme = "fantasy"

    panel {
        background_image = "@atlas.game_ui.panel_dark"
        slice = 32
        padding = 24
        width = 900
        height = 600

        display = "flex"
        flex_direction = "column"
        gap = 16

        // 标题栏
        header = container {
            display = "flex"
            justify_content = "space_between"
            align_items = "center"

            title = text("{t('shop.title')}") {
                font_size = "xl"
                weight = "bold"
                color = "@theme.primary"
            }

            gold_display = container {
                display = "flex"
                align_items = "center"
                gap = 4

                gold_icon = image("@atlas.game_ui.icon_gold") {
                    width = 24
                    height = 24
                }

                gold_text = text("{t('shop.gold', {amount=gold})}") {
                    color = "@theme.primary"
                    weight = "bold"
                }
            }
        }

        // 欢迎语
        welcome = text("{t('shop.welcome')}") {
            align = "start"
            dir = "auto"
            color = "@theme.text_muted"
            margin_bottom = 8
        }

        // 筛选栏
        filters = container {
            display = "flex"
            gap = 8
            margin_bottom = 8

            @each filter in [
                {key="all", label="{t('shop.filter.all')}"},
                {key="weapon", label="{t('shop.filter.weapon')}"},
                {key="armor", label="{t('shop.filter.armor')}"},
                {key="consumable", label="{t('shop.filter.consumable')}"}
            ] {
                filter_btn = button("{filter.label}") {
                    @style filter_button

                    @if (item_filter == filter.key) {
                        @style filter_button_active
                    }

                    on_click = "set_filter('{filter.key}')"
                }
            }
        }

        // 主体内容区
        main_area = container {
            flex_grow = 1

            display = "flex"
            flex_direction = "row"
            gap = 20

            @mobile {
                flex_direction = "column"
            }

            // 左侧商品列表
            item_list = list(
                source="{filtered_items}",
                max_visible=5
            ) {
                each = "{icon} {t(name_key)}  ${price}"
                spacing = 3
                cursor = "hand"
                selected_bg = "@theme.primary_dark"
                on_select = "update_selected"
            }

            // 右侧详情卡片
            details = container {
                flex_grow = 1
                @style card

                @if (selected_item != null) {
                    detail_content = container {
                        display = "flex"
                        flex_direction = "column"
                        gap = 12

                        // 商品图标和名称
                        header = container {
                            display = "flex"
                            gap = 12
                            align_items = "center"

                            icon = image("{selected_item.icon}") {
                                width = 48
                                height = 48
                            }

                            name = text("{t(selected_item.name_key)}") {
                                font_size = "large"
                                weight = "bold"
                            }
                        }

                        // 描述
                        description = text("{t(selected_item.description_key)}") {
                            color = "@theme.text_muted"
                            dir = "auto"
                        }

                        // 类型标签
                        type_badge = container {
                            background = "@theme.primary_dark"
                            padding = [2, 8]
                            border_radius = 4
                            width = "fit"

                            type_text = text("{t('shop.type.' + selected_item.type)}") {
                                font_size = "small"
                            }
                        }

                        // 价格行
                        price_row = container {
                            display = "flex"
                            justify_content = "space_between"
                            align_items = "center"
                            margin_top = 8

                            price_label = text("{t('shop.price')}:")
                            price_value = text("${selected_item.price}") {
                                color = "@theme.primary"
                                weight = "bold"
                                font_size = "large"
                            }
                        }

                        // 购买按钮
                        buy_button = button("{t('shop.buy')}") {
                            background_image = "@atlas.game_ui.button_primary"
                            slice = 12
                            width = "100%"
                            padding = [8, 16]
                            margin_top = 8
                            on_click = "buy_item"

                            @hover {
                                scale = 1.02
                            }

                            @pressed {
                                scale = 0.98
                            }
                        }
                    }
                } @else {
                    empty_state = container {
                        align_items = "center"
                        padding = 32

                        empty_text = text("{t('shop.select_hint')}") {
                            align = "center"
                            color = "@theme.text_muted"
                        }
                    }
                }
            }
        }

        // 底部操作栏
        footer = container {
            display = "flex"
            justify_content = "space_between"
            align_items = "center"
            margin_top = 8
            padding_top = 8
            border_top = "single"
            border_color = "@theme.border"

            info_text = text("{t('shop.items_count', {count=filtered_items.length})}") {
                font_size = "small"
                color = "@theme.text_muted"
            }

            close_button = button("{t('shop.close')}") {
                style = "secondary"
                background_image = "@atlas.game_ui.button_secondary"
                slice = 12
                action = "close"
            }
        }

        // 动画效果
        @keyframes fade_in {
            0% { opacity = 0 }
            100% { opacity = 1 }
        }

        details {
            animation = "fade_in" {
                duration = 0.2
            }
        }

        // RTL 调整
        @rtl {
            details {
                margin_inline_start = 8
            }

            filters {
                justify_content = "end"
            }
        }
    }
}
```

---

## 十四、语法总览表

### 已实现（Pokered UI 布局）

| 类别         | 语法元素                  | 示例                                  |
| ------------ | ------------------------- | ------------------------------------- |
| **屏幕**     | `screen`                  | `screen Dialog { }`                   |
| **组件**     | `type`, `id = type`       | `text("hello")`, `tile(31)`           |
| **定位**     | `rect`                    | `rect = {tx: 0, ty: 12, tw: 20, th: 6}` |
| **属性**     | `key = value`             | `align = "center"`                    |
| **绑定**     | `{expression}`            | `"{user.name}"`                       |
| **对象字面量** | `{key: value}`          | `cursor = {tile: 223, position: "left"}` |
| **边框**     | `panel`                   | `panel { style = "default" }`         |
| **容器**     | `container`               | `container { layout = {gap: 0} }`     |
| **文本**     | `text`                    | `text("Hello") { color = "Black" }`   |
| **瓦片**     | `tile`                    | `tile(31) { rect = {...} }`           |
| **分隔线**   | `divider`                 | `divider { tiles = [122]; repeat = 17 }` |
| **列表**     | `list`                    | `list { source = "{items}" }`         |
| **弹性列表** | `flex_list`               | `flex_list("{items}") { item_layout = [...] }` |
| **按钮**     | `button`                  | `button("OK") { on_click = "handler" }` |
| **图像**     | `image`                   | `image("sprite.png") { slice = "..." }` |
| **输入**     | `input`                   | `input { placeholder = "..." }`       |
| **下拉**     | `dropdown`                | `dropdown { }`                        |
| **光标**     | `cursor`                  | `cursor { row = "{cursor}"; row_step = 2 }` |
| **括号框**   | `bracket`                 | `bracket { rect = {...} }`            |
| **像素矩形** | `pixel_rect`              | `pixel_rect { rect = {...} }`         |
| **自定义组件** | `component` 声明 + 使用 | `component hp_bar { current: expr required }` → `hp_bar { current = "{hp}" }` |

### 未实现（Proposal）

| 类别         | 语法元素                  | 示例                                  |
| ------------ | ------------------------- | ------------------------------------- |
| **条件**     | `@if/@else`               | `@if (logged_in) { ... }`             |
| **循环**     | `@each`                   | `@each item in items { ... }`         |
| **翻译**     | `t()`                     | `"{t('key')}"`                        |
| **方向**     | `dir`, `@rtl/@ltr`        | `dir="rtl"`, `@rtl { ... }`           |
| **响应式**   | `@media`                  | `@media (min_width=768) { }`          |
| **主题**     | `@theme`                  | `@theme dark { ... }`                 |
| **样式**     | `@style`                  | `@style card { ... }`                 |
| **变量**     | `@variables`              | `@variables { gold = 500 }`           |
| **资源**     | `@resources`, `@atlas`    | `@atlas "name" { ... }`               |
| **事件**     | `on_*`                    | `on_click = "handler"`                |
| **动画**     | `@keyframes`, `animation` | `animation = "pulse" { ... }`         |

---

## 十五、设计原则总结

### 已实现

1. **声明式** — 描述"是什么"而非"怎么做"
2. **瓦片坐标** — 基于 20×18 网格的绝对定位
3. **组件化** — 内置组件 + `component` 声明的游戏自定义组件（`custom:*`）
4. **数据绑定** — 模板变量 `{var}` 运行时解析
5. **对象字面量** — `{key: value}` 语法用于复杂属性
6. **Schema v2 输出** — 编译为渲染器期望的 JSON 格式
7. **构建期校验** — 自定义组件按 `component` schema 校验使用点，运行时加载后复验

### 未实现（Proposal）

8. **国际化优先** — i18n 和 RTL 作为一等公民
9. **游戏友好** — 九宫格切片、图集、动画效果
10. **响应式** — 内置断点系统，适配多种屏幕

---

## 十六、文件扩展名与关联

| 文件类型 | 扩展名        | 说明                         | 状态     |
| -------- | ------------- | ---------------------------- | -------- |
| 场景文件 | `.scene`      | 游戏场景（脚本 + 可选 UI）   | ✅ 已实现 |
| UI 布局  | `.gui`        | 纯 UI 布局定义               | ✅ 已实现 |
| 主题文件 | `.theme`      | 颜色主题定义                 | ✅ 已实现 |
| 样式文件 | `.style`      | 可复用样式集合               | ✅ 已实现 |
| 资源定义 | `.res`        | 资源清单                     | ❌ 未实现 |
| 动画定义 | `.anim`       | 关键帧动画定义               | ❌ 未实现 |

### 编译输出

| 输入      | 输出                    | 说明                         |
| --------- | ----------------------- | ---------------------------- |
| `.scene`  | `name.js` + `name_ui.json` | 脚本 + 可选 UI 布局         |
| `.gui`    | `name.json`（schema v2） | 纯 UI 布局                   |
| `.theme`  | `name.json`             | 主题 token                   |
| `.style`  | `name_styles.json`      | 已解析的样式（含继承链）     |

这套语法现已具备**生产级**的 UI 描述能力，可广泛应用于游戏界面、终端应用、原型设计等场景。
