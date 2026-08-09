# Game DSL Mapping Contract

This document defines the compilation contract for the Game DSL. Each entry
specifies a DSL construct and its compiled output: JavaScript for runtime logic
or JSON for UI layout data.

Source documents:
- FULL_DSL.md -- scene/script DSL syntax
- GAME_UI_DSL.md -- UI component DSL syntax
- game.d.ts -- game.* API type definitions
- command.rs -- ScriptCommand enum for engine commands

---

## Conflict Resolution: FULL_DSL vs GAME_UI_DSL

Two DSL specifications exist. This contract reconciles them by file type:

| File Extension | Top-Level Construct | Source Document |
|---|---|---|
| `.scene` | `game_scene { ... }` | FULL_DSL.md |
| `.gui` | `screen { ... }` | GAME_UI_DSL.md |
| `.theme` | `@theme` directives | FULL_DSL.md SS3.3 |
| `.style` | `@style` directives | FULL_DSL.md SS3.4 |

**Conflict rules:**

1. `.scene` files use FULL_DSL's `game_scene` as the top-level wrapper. Inside
   `ui { ... }` blocks, use GAME_UI_DSL's component syntax (panel, text,
   button, image, list, etc.).

2. `.gui` files follow GAME_UI_DSL's `screen` structure end-to-end.

3. `.theme` and `.style` files follow FULL_DSL's `@` directive approach
   (unquoted identifiers, block-based).

4. Storyline logic (inside `@storylines { }` or at the scene level) compiles
   to JS.

5. UI layout (inside `ui { }` or the `screen` body) compiles to JSON.

6. Each `.scene` file compiles to a module that exports an async function named
   `storyline_main` by default.

---

## Mapping Fragments

### 1. Single @speaker (Dialogue)

Source: FULL_DSL.md SS7.1, game.d.ts `showText`

**DSL Input:**
```
@speaker("Oak") {
    "Hello! Welcome to the world of Pokemon!"
}
```

**Compiled Output (JS):**
```js
await game.showText("Oak: Hello! Welcome to the world of Pokemon!");
```

The speaker name is prepended as `"Name: "` to each dialogue line. Multiple
lines inside a single `@speaker` block are concatenated with newlines. The
full string is passed to `game.showText()`, which blocks until the player
dismisses the text box.

**Narrator form — `@speaker("")`.** An *empty* speaker name compiles to a bare
`game.showText("…")` with **no prefix**:

```
@speaker("") { "OAK: Hello there!" }
```
```js
await game.showText("OAK: Hello there!");   // emitted verbatim, no "Name: "
```

This is the canonical form for prefix-less original dialogue (narration, system
messages, dialogue whose name is baked into the text). Using it instead of a
placeholder speaker is what prevents a spurious `System: ` / `: ` prefix from
leaking into the textbox. See `compile_speaker` in `js_storyline.rs` and the
`test_speaker_empty_name_no_prefix` regression test.

**Semantic split — `@speaker` vs `@say`.** `@speaker` is fixed to
*player-initiated dialogue*: a textbox the player opens by talking to an NPC
(`@trigger` `npc` binding) and pages with A. **Cutscene speech uses `@say`** —
an auto-triggered storyline (`@load` / coord) where NPCs talk in sequence.
Same name/prefix rules and the same compiled API; the two directives differ
only in *meaning* (which storyline kind each belongs to):

```
@say("Oak") { "Follow me!" }
```
```js
await game.showText("Oak: Follow me!");   // identical output to @speaker
```

---

### 2. @choice Two Options

Source: FULL_DSL.md SS7.2, game.d.ts `showChoice`

**DSL Input:**
```
@choice {
    @option("Yes") {
        @speaker("Oak") { "Great!" }
    }
    @option("No") {
        @speaker("Oak") { "Too bad." }
    }
}
```

**Compiled Output (JS):**
```js
const choice = await game.showChoice(["Yes", "No"]);
if (choice === 0) {
    await game.showText("Oak: Great!");
} else {
    await game.showText("Oak: Too bad.");
}
```

Each `@option` produces a label in the `showChoice` array. The 0-based index
returned by `showChoice` drives an if/else chain. The last option is the else
branch (no index check needed). For 3+ options, each non-final option gets an
`else if` guard.

---

### 3. @if / @else Conditional (⚠️ Deprecated — use @run)

Source: FULL_DSL.md SS7.3

**DSL Input:**
```
@if (gold > 100) {
    @speaker("Shopkeeper") { "You have enough gold!" }
} @else {
    @speaker("Shopkeeper") { "Not enough gold." }
}
```

**Compiled Output (JS):**
```js
if (gold > 100) {
    await game.showText("Shopkeeper: You have enough gold!");
} else {
    await game.showText("Shopkeeper: Not enough gold.");
}
```

DSL conditions are emitted verbatim as JS expressions. `@else if (cond)` chains
map to `else if (cond)` in JS. Single `@if` without `@else` maps to a standalone
`if` block. Variables in conditions resolve from the scene's `@variables` scope
or from game state via `game.getFlag()`.

> **⚠️ Deprecation note:** As of the `@run` block addition (Entry 14), `@if`/`@else`
> is deprecated for new code. Use `@run { if (...) { ... } else { ... } }` instead.
> The `@if` parser remains functional for backward compatibility with existing
> `.scene` files but will not receive new features.
>
> **Exception:** `@if`/`@else` is still required when you need to wrap DSL-native
> blocks like `@speaker` or `@choice` inside a conditional, since these blocks
> cannot appear inside raw JS `@run { ... }` blocks. In this case, keep the
> outer `@if`/`@else` as the conditional wrapper and use `@run` for the
> imperative command sequences inside each branch.

---

### 4. Nested @choice Inside @if

Source: FULL_DSL.md SS7.2, SS7.3

**DSL Input:**
```
@if (hasStarter == true) {
    @speaker("Oak") { "You already have a Pokemon." }
} @else {
    @choice {
        @option("Charmander") {
            @speaker("Oak") { "A fiery choice!" }
        }
        @option("Squirtle") {
            @speaker("Oak") { "A water type!" }
        }
    }
}
```

**Compiled Output (JS):**
```js
if (game.getFlag("HAS_STARTER")) {
    await game.showText("Oak: You already have a Pokemon.");
} else {
    const choice = await game.showChoice(["Charmander", "Squirtle"]);
    if (choice === 0) {
        await game.showText("Oak: A fiery choice!");
    } else {
        await game.showText("Oak: A water type!");
    }
}
```

Nesting is preserved exactly. Undefined variable names in conditions that match
known game flags compile to `game.getFlag()`. Each nested `@choice` produces
its own independent if/else chain at the correct indentation depth.

---

### 5. @variables Declarations

Source: FULL_DSL.md SS3.1, GAME_UI_DSL.md SS4.1

**DSL Input:**
```
@variables {
    gold = 500
    player = { name = "RED", level = 5 }
    inventory = ["sword", "shield"]
}
```

**Compiled Output (JS):**
```js
let gold = 500;
let player = { name: "RED", level: 5 };
let inventory = ["sword", "shield"];
```

DSL object literals `{ key = value }` become JS objects `{ key: value }` (equals
becomes colon). DSL array literals map directly to JS arrays. Scene-level
variables use `let` to allow mutation. Variables that are never reassigned
in the DSL body may use `const`.

---

### 6. Expression Binding (Inline Interpolation)

Source: FULL_DSL.md SS3.3, GAME_UI_DSL.md SS3.3

**DSL Input (bare expression):**
```
text("{gold - price}")
```

**Extracted Expression:**
```js
(gold - price)
```

**Full Compiled Output (JS):**
```js
`${gold - price}`
```

**DSL Input (embedded in text):**
```
text("You need {price - gold} more gold!")
```

**Compiled Output (JS):**
```js
`You need ${price - gold} more gold!`
```

Any `{expression}` inside a DSL string is extracted and embedded into a JS
template literal via `${expression}`. Expressions pass through verbatim:
arithmetic operators, property access (`user.name`), ternary expressions,
and function calls all compile as-is.

---

### 7. Simple UI Panel

Source: GAME_UI_DSL.md SS5.1

**DSL Input:**
```
ui {
    panel {
        border = "single"
        padding = 24
        title = text("Shop")
    }
}
```

**Compiled Output (JSON):**
```json
{
    "type": "panel",
    "border": "single",
    "padding": 24,
    "children": [
        {
            "id": "title",
            "type": "text",
            "value": "Shop"
        }
    ]
}
```

A `panel` becomes a JSON object with `type: "panel"`. Named children
(`title = text(...)`) carry an `id` field matching the assigned name.
Anonymous children (`text(...)`) omit `id`. The `children` array preserves
the declaration order of child components.

---

### 8. UI with Button

Source: GAME_UI_DSL.md SS5.5, SS12.1

**DSL Input:**
```
ui {
    panel {
        btn = button("Buy") {
            on_click = "buy"
        }
    }
}
```

**Compiled Output (JSON):**
```json
{
    "type": "panel",
    "children": [
        {
            "id": "btn",
            "type": "button",
            "label": "Buy",
            "onClick": "buy"
        }
    ]
}
```

A `button` becomes a JSON element with `type: "button"` and a `label` for its
text content. The `on_click` property compiles to `onClick` (camelCase), whose
value is a handler function name. The runtime resolves the handler from the
script context. Button state blocks (`@hover`, `@pressed`, `@disabled`) compile
to a nested `states` object.

---

### 9. @theme Definition

Source: FULL_DSL.md SS3.3, GAME_UI_DSL.md SS4.2

**DSL Input:**
```
@theme dark {
    primary = "#c9a03d"
    background = "#1a1a2e"
    text = "#ffffff"
}
```

**Compiled Output (JSON):**
```json
{
    "dark": {
        "primary": "#c9a03d",
        "background": "#1a1a2e",
        "text": "#ffffff"
    }
}
```

Each `@theme` compiles to a named entry in the theme tokens map. Values are
emitted as their literal types (color strings, numeric sizes, boolean flags).
A `.theme` file with multiple `@theme` blocks produces a single JSON map with
one key per theme name.

---

### 10. @style Inheritance

Source: FULL_DSL.md SS3.4, GAME_UI_DSL.md SS4.3

**DSL Input:**
```
@style card {
    border = "rounded"
    padding = 12
    background = "@theme.surface"
}

@style card_hover : card {
    background = "@theme.primary"
    scale = 1.02
}
```

**Compiled Output (JSON):**
```json
{
    "card": {
        "border": "rounded",
        "padding": 12,
        "background": "@theme.surface"
    },
    "card_hover": {
        "__extends": "card",
        "background": "@theme.primary",
        "scale": 1.02
    }
}
```

A base `@style` (no parent) compiles to a standalone style object. An inherited
`@style child : parent` compiles to an object with `__extends` referencing the
parent style ID. The runtime performs a shallow merge of child properties onto
the parent at resolution time. Circular inheritance is a compile-time error.

---

### 11. @atlas with Nine-Slice

Source: FULL_DSL.md SS3.5, GAME_UI_DSL.md SS4.5

**DSL Input:**
```
@atlas "ui" {
    source = "assets/ui/atlas.png"
    regions = {
        btn = [0, 0, 64, 64, slice=8]
    }
}
```

**Compiled Output (JSON):**
```json
{
    "ui": {
        "source": "assets/ui/atlas.png",
        "regions": {
            "btn": {
                "x": 0,
                "y": 0,
                "width": 64,
                "height": 64,
                "nineSlice": {
                    "top": 8,
                    "right": 8,
                    "bottom": 8,
                    "left": 8
                }
            }
        }
    }
}
```

A region coordinate tuple `[x, y, w, h]` maps to `x`, `y`, `width`, `height`.
The `slice=N` shorthand expands to a uniform four-side `nineSlice` object.
The slice array form `slice=[top, right, bottom, left]` maps each side
individually. Regions without a `slice` parameter omit the `nineSlice` field
entirely.

---

## Command Syntax — Calling Game API Functions (Entry 12) (⚠️ Deprecated — use @run)

> **⚠️ Deprecation note:** As of the `@run` block addition (Entry 14), bare
> command statements (`command(args)`) and `@command(...)` are deprecated for
> new code. Use `@run { await game.command(args); }` instead. The old syntax
> remains supported for backward compatibility with existing `.scene` files.

Two equivalent syntaxes exist for calling game API functions from within
`@storylines` blocks.

### Syntax A: Bare Identifier (primary)

```
command_name(arg1, arg2, ...)
```

**DSL Input:**
```
heal()
giveItem("Potion", 1)
fadeScreen("out")
```

**Compiled JS:**
```js
await game["heal"]();
await game["giveItem"]("Potion", 1);
await game["fadeScreen"]("out");
```

### Syntax B: @command Directive (explicit escape hatch)

Useful when the command name might conflict with future DSL keywords, or
when you want to visually distinguish game API calls from DSL directives.

```
@command("command_name", arg1, arg2, ...)
```

**DSL Input:**
```
@command("heal")
@command("giveItem", "Potion", 1)
@command("fadeScreen", "out")
```

**Compiled JS:** (identical output as Syntax A)
```js
await game["heal"]();
await game["giveItem"]("Potion", 1);
await game["fadeScreen"]("out");
```

### Argument Types

Arguments can be:
- **String literals**: `"potion"` → `"potion"`
- **Number literals**: `5` → `5`
- **Boolean literals**: `true` / `false` → `true` / `false`
- **Variables**: `gold` → `gold`

---

## Summary Table

| # | Construct | Output | Key API / Schema | Source |
|---|---|---|---|---|
| 1 | `@speaker` | JS | `game.showText()` — player-initiated talk (fixed meaning) | FULL_DSL SS7.1 |
| 1a | `@say` | JS | `game.showText()` — cutscene speech in auto-triggered storylines | FULL_DSL SS7.1 |
| 2 | `@choice @option` | JS | `game.showChoice()` + if/else | FULL_DSL SS7.2 |
| 3 | `@if/@else` (deprecated) | JS | `if`/`else` statement | FULL_DSL SS7.3 |
| 4 | Nested control flow | JS | Preserved nesting | FULL_DSL SS7.2, SS7.3 |
| 5 | `@variables` | JS | `let`/`const` declarations | FULL_DSL SS3.1, GAME_UI_DSL SS4.1 |
| 6 | `{expression}` binding | JS | Template literal `${}` | FULL_DSL SS3.3, GAME_UI_DSL SS3.3 |
| 7 | `panel` | JSON | `{ type: "panel", children: [...] }` | GAME_UI_DSL SS5.1 |
| 8 | `button` | JSON | `{ type: "button", onClick: "name" }` | GAME_UI_DSL SS5.5 |
| 9 | `@theme` | JSON | Tokens map by name | FULL_DSL SS3.3, GAME_UI_DSL SS4.2 |
| 10 | `@style inheritance` | JSON | `__extends` merge reference | FULL_DSL SS3.4, GAME_UI_DSL SS4.3 |
| 11 | `@atlas` with slice | JSON | `nineSlice` expansion | FULL_DSL SS3.5, GAME_UI_DSL SS4.5 |
| 12 | `@command` / bare call (deprecated) | JS | `game["name"]()` | FULL_DSL SS7.4 |
| 13 | `@storyline` + `@trigger` | JS + JSON | Named storylines with routing table | FULL_DSL SS6 |
| 14 | `@run { ... }` | JS | Raw JS passthrough (Boa) | — |

---

## V2: Named Storylines with Trigger Routing (Entry 13)

### Syntax

Each `.scene` file now contains named `@storyline("name")` blocks instead of
a single unnamed `@storylines` block. Each storyline declares its trigger
conditions with `@trigger(...)`.

### @storyline directive

```
@storyline("unique_name") {
  @trigger(...)
  @speaker(...)
  @choice(...)
  command(args)
}
```

The `name` parameter must be unique within the `.scene` file.
It determines the generated JS function name: `storyline_{name}()`.

### @trigger directive

> **Unified design (current).** In the fused migration the `.scene` is the
> single source of truth for routing/binding data: each storyline declares its
> binding inline, and `script_config.json` (the runtime `MapScriptConfig`
> contract) is **regenerated from** these triggers. See
> [`DSL_UNIFIED_DESIGN.md`](./DSL_UNIFIED_DESIGN.md). The string-NPC /
> `onEnter` / `storyline_routes.json` form below this note is the older
> `feat/dsl-scene-migration` shape, kept for historical reference.

A storyline declares its binding with one or more `@trigger` lines:

```
@trigger(
  map     = "MapName",           // which map (scoping/documentation)
  npc     = 1,                   // NPC object id (NUMERIC) → npcs[].talk
  sign    = 2,                   // sign id               → signs[].talk
  coord   = [10, 1],             // one coord tile        → coordEvents[]
  coords  = [[10, 1], [11, 1]],  // several coord tiles   → coordEvents[]
  name    = "northExit1",        // coord event name      → coordEvents[].name (camelCase, unique per map)
  toggle  = "PALLET_TOWN_OBJ_1", //                       → npcs[].toggleId
  script  = "PALLETTOWN_OAK",    //                       → npcs[].scriptId
  hidden  = true,                //                       → npcs[].defaultHidden
  no_talk = true                 // object-only binding (emit no `talk` fn)
)
```

- NPC ids are **numeric** (`npc = 1`), not object-name strings.
- A storyline may carry **several** `@trigger` lines when multiple objects
  route to one handler (e.g. OaksLab's two POKéDEX balls → `talkPokedex`); each
  line emits its own `npcs[]` entry but they share the one `talk` function.
- `no_talk = true` produces an `npcs[]` entry with `toggleId`/`scriptId`/
  `defaultHidden` but **no** `talk` field — for toggled objects that have no
  dialogue handler.

### Generating & verifying the binding contract

`script_config.json` is **derived from** the `.scene`, not hand-maintained:

- `config_gen::compile_scene_to_config` / `bin/gen_map_config` regenerate each
  map's `script_config.json` from its `script.scene`.
- `tests/config_roundtrip.rs` regenerates every map's config from its `.scene`
  and asserts the bindings (npcs / signs / coordEvents) match the committed
  config — the **no-drift guarantee** between DSL source and runtime contract
  (0 mismatches across 248 maps). `onLoad` is excluded for now (see
  DSL_UNIFIED_DESIGN.md "Known follow-ups").

Runtime routing is then 100% driven by `script_config.json`, matched by
**function name** (storyline name == compiled fn name == config
`talk`/`trigger`/`onLoad`).

### after chain & `storyline_routes.json` (historical / advisory)

The earlier `@trigger(map, npc = "Name", onEnter, after)` form fed a compiler
pass that grouped storylines by `(map, npc)`, followed the `after` chain, and
generated a `storyline_routes.json`:

```json
{
  "routes": [
    { "map": "OaksLab", "npc": "Oak", "storyline": "oak_ask", "after": null },
    { "map": "ViridianMart", "onEnter": true, "storyline": "mart_pickup", "after": "oak_ask" },
    { "map": "OaksLab", "npc": "Oak", "storyline": "oak_delivery", "after": "mart_pickup" }
  ]
}
```

In the unified design `storyline_routes.json` is **advisory only** — it powers
the build-time CONFLICT detector (two storylines on the same `(map, npc)` with
no `after` link → warning); it does **not** drive runtime routing.

### Generated JavaScript

Named storylines generate separate JS functions:

```js
export async function storyline_oak_ask() { ... }
export async function storyline_mart_pickup() { ... }
export async function storyline_oak_delivery() { ... }
```

Unnamed `@storylines` (backward compat) generates `storyline_main()`.

### Backward Compatibility

Existing unnamed `@storylines { ... }` syntax continues to work unchanged.
It is treated as `@storyline("main")` with `@trigger(onEnter = true)`.

### Conflict Detection

The compiler checks for conflicting storylines at build time:
- Same `(map, npc)` + multiple storylines with no `after` chain → CONFLICT warning
- Different NPCs → no conflict
- Different maps → no conflict
- Connected by `after` chain → no conflict

### Example: Cross-Map Quest (Oak's Parcel)

See `assets/scenes/oak_parcel.scene` for a complete example spanning
OaksLab → ViridianMart → OaksLab with three sequential storylines.

---

## Entry 14: `@run { ... }` — Raw JavaScript Blocks

### Design Rationale

The DSL previously grew expression syntax (Call, UnaryOp, BinaryOp, TernaryOp,
BitOr/BitAnd, hex literals, `!` negation) to handle increasingly complex game
logic. This created a "JS in DSL" problem — a second-class JavaScript dialect
inside the DSL that was syntactically similar but different enough to cause
confusion.

The solution is a layered design:

| Layer | What | Why |
|---|---|---|
| **DSL** | `@speaker`, `@choice`, `@option`, `@load`, `@storyline`, `@trigger`, `@variables`, `@theme`, `@style`, `@atlas` | Domain-specific constructs with compiler verification |
| **JS** (`@run`) | Control flow, expressions, imperative commands | Use the full JavaScript language (Boa engine) without DSL constraints |

`@run { ... }` is the bridge: its content is raw JavaScript emitted verbatim,
bypassing the DSL parser entirely.

### Syntax

```
@run {
    // Any valid JavaScript — Boa executes this directly
    await game.facePlayer("down");
    game.setFlag("EVENT_SOMETHING");
    
    if (game.getFlag("EVENT_X") && amount > 3) {
        await game.giveItem("potion", 3);
    }
}
```

### Where @run Can Appear

`@run` blocks are valid inside:
- `@storyline("name") { ... }` — storyline bodies
- `@load { ... }` — on-load blocks
- `@if { ... }` / `@else { ... }` — conditional branches (for migrating commands within)
- `@choice { @option("label") { ... } }` — option bodies

They are **not** valid at the top level of `game_scene { ... }` (only directives
like `@variables`, `@storylines`, `@ui`, etc. are allowed there).

### Compiled Output

The raw JS content is emitted **as-is**, with each non-empty line indented to
match the surrounding code block depth:

**DSL Input:**
```
@storyline("start") {
  @trigger(map = "PalletTown", npc = "Oak")
  @run {
    if (!game.getFlag("EVENT_MET")) {
      await game.facePlayer("down");
      game.setFlag("EVENT_MET");
    }
  }
}
```

**Compiled Output (JS):**
```js
export async function storyline_start() {
  if (!game.getFlag("EVENT_MET")) {
    await game.facePlayer("down");
    game.setFlag("EVENT_MET");
  }
}
```

### Mapping: Old DSL → `@run` JS

When migrating existing `.scene` files, use these mappings:

| Old DSL Syntax | `@run` JS Equivalent |
|---|---|
| `commandName(arg1, arg2)` | `await game.commandName(arg1, arg2);` |
| `@command("name", args...)` | `await game.name(args...);` |
| `getFlag("EVENT_X")` (in condition) | `game.getFlag("EVENT_X")` |
| `setFlag("EVENT_X")` | `game.setFlag("EVENT_X");` (sync) |
| `playSound("SFX_...")` | `await game.playSound("SFX_...");` |
| `playMusic("MUSIC_...")` | `await game.playMusic("MUSIC_...");` |
| `setJoyIgnore(flags)` | `await game.setJoyIgnore(flags);` |
| `clearJoyIgnore()` | `await game.clearJoyIgnore();` |
| `facePlayer("down")` | `await game.facePlayer("down");` |
| `faceNpc("name", "dir")` | `await game.faceNpc("name", "dir");` |
| `delay(n)` | `await game.delay(n);` |
| `showObject("name")` | `await game.showObject("name");` |
| `hideObject("name")` | `await game.hideObject("name");` |
| `moveNpc("name", ...)` | `await game.moveNpc("name", ...);` |
| `followNpc("name", x, y)` | `await game.followNpc("name", x, y);` |
| `let x = expr` (assign) | `let x = expr;` (direct JS) |
| `@each item in items { ... }` | `for (const item of items) { ... }` (direct JS) |

### What @run Does NOT Do

- **No parsing or validation** of the JS content — if Boa rejects it at runtime,
  that's between you and the JavaScript engine
- **No sourcemap entries** — @run blocks don't produce sourcemap mappings since
  they contain raw JS, not DSL constructs
- **No variable checking** — the semantic validator skips @run content entirely
  (no undefined-variable errors for JS identifiers)

### Example: Real-World Migration

See `crates/pokered-data/maps/PalletTown/script.scene` for a
complete migration from old `@if`/bare-command syntax to `@run` blocks.
