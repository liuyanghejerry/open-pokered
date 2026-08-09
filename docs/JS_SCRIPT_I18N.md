# JS Script i18n Guide

## Overview

Map scripts in `crates/pokered-data/maps/*/script.js` use the `game` global object to interact with the game engine. The following i18n APIs are available for supporting multiple languages.

## API

### `game.lang()`

Returns the current language as a string.

| Return value | Language |
|-------------|----------|
| `"en"`      | English  |
| `"zh"`      | Chinese  |

```js
if (game.lang() === "zh") {
    await game.showText("你好！");
} else {
    await game.showText("Hello!");
}
```

### `game.t(en, zh)`

Selects the appropriate string based on `game.lang()`. Returns `en` when language is English, `zh` when language is Chinese.

```js
await game.showText(game.t(
    "Hello! Welcome to the world of POKéMON!",
    "你好！欢迎来到宝可梦的世界！"
));
```

### DSL `@t("en", "zh")` (preferred for `.scene` / `.gui`)

When authoring in the DSL rather than hand-written JS, wrap text in
`@t("English", "中文")`. In `.scene` files it works in `@speaker` lines and
`@option` labels and compiles to the `game.t(en, zh)` call above; in `.gui`
files it works in `text(...)` / `button(...)` and compiles to a per-locale
`{"en": …, "zh": …}` value the renderer resolves by language.

```
@speaker("") {
    @t("Hello! Welcome to the world of POKéMON!", "你好！欢迎来到宝可梦的世界！")
}
@choice {
    @option(@t("YES", "是")) { ... }
    @option(@t("NO", "否"))  { ... }
}
```

## Examples

### Simple dialogue

```js
export async function talkCooltrainerF() {
    await game.showText(game.t(
        "It's rumored that\nCLEFAIRYs came\nfrom the moon!",
        "据说皮皮是从\n月亮来的！"
    ));
}
```

### Choice dialogue

```js
export async function talkSuperNerd() {
    const choice = await game.showTextChoice(
        game.t("Did you check out the MUSEUM?", "你去博物馆看过了吗？"),
        [game.t("YES", "是"), game.t("NO", "否")]
    );

    if (choice === 0) {
        await game.showText(game.t(
            "Weren't those fossils from MT.MOON amazing?",
            "月见山的化石是不是很棒？"
        ));
    }
}
```

### Conditional logic

```js
export async function talkSign() {
    if (game.lang() === "zh") {
        await game.showText("常青森林\n前方请小心！");
    } else {
        await game.showText("VIRIDIAN FOREST\nBe careful!");
    }
}
```

## Important Notes

- All new scripts should use `game.t()` for bilingual text.
- `\n` is used for line breaks within a single `showText` call.
- Text is not clipped by the game engine — wrap lines to ~15 CJK chars or ~30 ASCII chars per line for optimal display.
