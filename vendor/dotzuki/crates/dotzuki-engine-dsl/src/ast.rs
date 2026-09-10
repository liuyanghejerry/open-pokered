use serde::{Deserialize, Serialize};
use crate::hash::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Document {
    Scene(GameScene),
    Screen(ScreenLayout),
    /// A declarations-only `.gui` file (e.g. a `components.gui` prelude):
    /// `component` schemas with no `screen` block.
    Components(Vec<ComponentDecl>),
}

/// Value kind a declared component prop accepts.
///
/// `Expr` admits anything a data binding can carry — a number literal or a
/// `"{var}"` template string — and is the right kind for runtime-bound props
/// like `current`/`max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropKind {
    Int,
    String,
    Bool,
    Color,
    Expr,
}

impl PropKind {
    pub fn name(self) -> &'static str {
        match self {
            PropKind::Int => "int",
            PropKind::String => "string",
            PropKind::Bool => "bool",
            PropKind::Color => "color",
            PropKind::Expr => "expr",
        }
    }
}

/// One prop schema row of a [`ComponentDecl`]: `current: expr required`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropDecl {
    pub name: String,
    pub kind: PropKind,
    pub required: bool,
    pub span: SourceSpan,
}

/// A `component` declaration — the build-time schema for a game-registered
/// custom element:
///
/// ```text
/// component hp_bar {
///     current: expr required
///     max: expr required
/// }
/// ```
///
/// Declared components are valid component types in `screen` blocks; uses are
/// validated against the schema (unknown/missing/mistyped props) and compile
/// to `{"type": "custom:<name>"}` JSON dispatched to the game's registered
/// `CustomElement` at runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentDecl {
    pub name: String,
    pub props: Vec<PropDecl>,
    pub span: SourceSpan,
}

/// Author-facing text that may carry per-locale variants for i18n.
///
/// `Plain` text is identical in every language (the common case). `Localized`
/// holds ordered `(locale, text)` pairs produced by the `@t("en", "中文")`
/// DSL literal — e.g. `[("en", "YES"), ("zh", "是")]`. Locale codes are
/// assigned positionally (first arg → `en`, second → `zh`).
///
/// In the Scene DSL a `Localized` value compiles to `game.t("en", "zh")`; in
/// the GUI DSL it compiles to a `{"en": …, "zh": …}` JSON object that the
/// renderer resolves against the current language.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LocalizedText {
    Plain(String),
    /// Ordered locale→text pairs, e.g. `[("en", "…"), ("zh", "…")]`.
    Localized(Vec<(String, String)>),
}

impl LocalizedText {
    /// The base/default text: the `en` variant, else the first available, else "".
    pub fn default_text(&self) -> &str {
        match self {
            LocalizedText::Plain(s) => s,
            LocalizedText::Localized(pairs) => pairs
                .iter()
                .find(|(l, _)| l == "en")
                .or_else(|| pairs.first())
                .map(|(_, t)| t.as_str())
                .unwrap_or(""),
        }
    }

    /// Text for `locale`, falling back to the `en`/first variant, else "".
    pub fn get(&self, locale: &str) -> &str {
        match self {
            LocalizedText::Plain(s) => s,
            LocalizedText::Localized(pairs) => pairs
                .iter()
                .find(|(l, _)| l == locale)
                .map(|(_, t)| t.as_str())
                .unwrap_or_else(|| self.default_text()),
        }
    }

    /// Whether this text carries explicit per-locale variants.
    pub fn is_localized(&self) -> bool {
        matches!(self, LocalizedText::Localized(_))
    }
}

impl From<&str> for LocalizedText {
    fn from(s: &str) -> Self {
        LocalizedText::Plain(s.to_string())
    }
}

impl From<String> for LocalizedText {
    fn from(s: String) -> Self {
        LocalizedText::Plain(s)
    }
}

/// Ergonomic equality against plain strings (compares the default/`en` text);
/// keeps existing `assert_eq!(text, "…")` call sites concise. Both `str` and
/// `&str` are covered so the comparison works for owned and borrowed
/// `LocalizedText` alike.
impl PartialEq<str> for LocalizedText {
    fn eq(&self, other: &str) -> bool {
        self.default_text() == other
    }
}

impl PartialEq<&str> for LocalizedText {
    fn eq(&self, other: &&str) -> bool {
        self.default_text() == *other
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameScene {
    pub name: String,
    pub variables: Option<VariablesBlock>,
    pub storylines: Vec<Storyline>,
    pub on_load: Option<StorylineBlock>,
    pub ui: Option<UiBlock>,
    pub themes: Vec<Theme>,
    pub styles: Vec<Style>,
    pub atlases: Vec<Atlas>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenLayout {
    pub name: String,
    pub theme: Option<String>,
    pub components: Vec<UiComponent>,
    /// When set, output schema v2 JSON: `{"schema_version": 2, "screen": "...", "elements": [...]}`
    pub schema_version: Option<u8>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerCondition {
    pub map: String,
    /// Legacy/advisory NPC key (string). Conflict detection groups on this.
    pub npc: Option<String>,
    pub on_enter: bool,
    pub after: Option<String>,
    pub priority: Option<i32>,
    // ── Binding fields (DSL-as-source for `script_config.json`) ───────────
    // These let `config_gen` regenerate the map's `script_config.json` from
    // the `.scene` so the DSL is the single source of truth.
    /// Numeric NPC object/text id this storyline's `talk` handler binds to.
    pub npc_id: Option<u8>,
    /// Numeric sign id this storyline's `talk` handler binds to.
    pub sign_id: Option<u8>,
    /// Coordinate tile(s) that fire this storyline (`coord`/`coords`).
    pub coords: Vec<(u16, u16)>,
    /// CamelCase name identifier for this trigger condition.
    pub name: String,
    /// Named show/hide toggle for the bound NPC object.
    pub toggle_id: Option<String>,
    /// Script-facing NPC id (for moveNpc) for the bound NPC object.
    pub script_id: Option<String>,
    /// Whether the bound NPC object starts hidden.
    pub default_hidden: bool,
    /// Object-only binding: emit the NPC entry without a `talk` fn (used for
    /// the rare toggled objects that have no dialogue handler).
    pub no_talk: bool,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Storyline {
    pub name: String,
    /// Routing/binding conditions. A storyline may carry MORE than one
    /// `@trigger` when several map objects route to the same handler (e.g.
    /// ProfLab's two DEX balls both calling `talkDex`), each with its
    /// own npc id / toggle. Empty for the legacy unnamed `main` storyline.
    pub triggers: Vec<TriggerCondition>,
    pub statements: Vec<StoryStmt>,
    pub span: SourceSpan,
}

/// Legacy unnamed storyline block (backward compat).
/// Parser converts unnamed `@storylines { }` into `Storyline { name: "main", ... }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorylineBlock {
    pub statements: Vec<StoryStmt>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StoryStmt {
    /// A dialogue line the **player initiates** — talking to an NPC (a
    /// storyline bound by a `@trigger` `npc`/`talk`). Meaning is fixed to
    /// interactive talk; it does NOT express cutscene speech.
    Speaker {
        name: Expression,
        texts: Vec<LocalizedText>,
        span: SourceSpan,
    },
    /// A cutscene line (`@say("Name")`): speech inside an auto-triggered
    /// storyline (on-enter / coord), where NPCs talk in sequence. Same
    /// rendering as a speaker line, but semantically scripted dialogue —
    /// the author marks it explicitly so `@speaker` stays unambiguous.
    Say {
        name: Expression,
        texts: Vec<LocalizedText>,
        span: SourceSpan,
    },
    Choice {
        options: Vec<ChoiceOption>,
        span: SourceSpan,
    },
    If {
        condition: Expression,
        then_branch: Vec<StoryStmt>,
        else_branch: Vec<StoryStmt>,
        span: SourceSpan,
    },
    Each {
        item_var: String,
        source: Expression,
        body: Vec<StoryStmt>,
        span: SourceSpan,
    },
    Run {
        js: String,
        span: SourceSpan,
    },
    Assign {
        name: String,
        value: Expression,
        span: SourceSpan,
    },
    Command {
        name: String,
        args: Vec<Expression>,
        span: SourceSpan,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub label: LocalizedText,
    pub body: Vec<StoryStmt>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiBlock {
    pub components: Vec<UiComponent>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UiComponent {
    Panel {
        props: ComponentProps,
        children: Vec<UiComponent>,
        span: SourceSpan,
    },
    Container {
        props: ComponentProps,
        children: Vec<UiComponent>,
        span: SourceSpan,
    },
    Text {
        content: LocalizedText,
        props: ComponentProps,
        span: SourceSpan,
    },
    Button {
        label: LocalizedText,
        props: ComponentProps,
        span: SourceSpan,
    },
    List {
        source: Expression,
        format: Option<String>,
        props: ComponentProps,
        span: SourceSpan,
    },
    Image {
        src: String,
        props: ComponentProps,
        span: SourceSpan,
    },
    Input {
        props: ComponentProps,
        span: SourceSpan,
    },
    Dropdown {
        props: ComponentProps,
        span: SourceSpan,
    },
    Tile {
        tile_id: Expression,
        props: ComponentProps,
        span: SourceSpan,
    },
    Divider {
        tiles: Vec<Expression>,
        props: ComponentProps,
        span: SourceSpan,
    },
    FlexList {
        source: Expression,
        format: Option<String>,
        props: ComponentProps,
        span: SourceSpan,
    },
    /// A selection cursor (▶) positioned by base (rect) + col/row grid offset.
    /// Grid step / index bindings (`col_step`, `row_step`, `col`, `row`,
    /// `glyph`) are carried as generic props and emitted to the JSON for the
    /// renderer's `CursorParams`.
    Cursor {
        props: ComponentProps,
        span: SourceSpan,
    },
    /// Partial box border (`left`/`right`/`top`/`bottom`/`with_arrow`).
    Bracket {
        props: ComponentProps,
        span: SourceSpan,
    },
    /// Raw pixel rectangle (`px`/`py`/`pw`/`ph`).
    PixelRect {
        props: ComponentProps,
        span: SourceSpan,
    },
    /// A use of a `component`-declared custom element. Compiles to
    /// `{"type": "custom:<name>"}`; rendered by the game's registered
    /// [`CustomElement`] implementation.
    Custom {
        name: String,
        props: ComponentProps,
        span: SourceSpan,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentProps {
    pub id: Option<String>,
    pub width: Option<Expression>,
    pub height: Option<Expression>,
    pub padding: Option<Vec<Expression>>,
    pub margin: Option<Vec<Expression>>,
    pub align: Option<String>,
    pub on_click: Option<String>,
    pub flex_grow: Option<u32>,
    pub visible: Option<bool>,
    pub custom: HashMap<String, Expression>,
    pub span: SourceSpan,
    // Pokered-specific layout properties
    pub rect: Option<RectDef>,
    pub style: Option<String>,
    pub value: Option<String>,
    pub color: Option<String>,
    pub font: Option<String>,
    pub wrap: Option<String>,
    pub line_spacing: Option<u32>,
    /// Integer text-scale factor (proportional path) — big title/heading text.
    pub scale: Option<u32>,
    pub tile_id: Option<Expression>,
    pub tiles: Option<Vec<Expression>>,
    pub repeat: Option<u32>,
    pub orientation: Option<String>,
    pub cursor: Option<Expression>,
    pub selected: Option<Expression>,
    pub max_visible: Option<u32>,
    pub footer: Option<String>,
    pub item_template: Option<Expression>,
    pub item_layout: Option<Vec<Expression>>,
    pub gap: Option<u32>,
    pub clip: Option<bool>,
    pub flip_x: Option<bool>,
    pub flip_y: Option<bool>,
    pub palette: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RectDef {
    pub tx: Expression,
    pub ty: Expression,
    pub tw: Expression,
    pub th: Expression,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariablesBlock {
    pub decls: Vec<VariableDecl>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableDecl {
    pub name: String,
    pub value: Expression,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub tokens: HashMap<String, String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Style {
    pub name: String,
    pub extends: Option<String>,
    pub properties: HashMap<String, Expression>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Atlas {
    pub name: String,
    pub source: String,
    pub regions: Vec<AtlasRegion>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtlasRegion {
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub nine_slice: Option<[u32; 4]>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    StringLit(String),
    /// A localized string literal `@t("en", "中文")`, carrying ordered
    /// `(locale, text)` pairs. Appears where text may be authored bilingually
    /// (GUI `text()`/`button()` arguments and Scene `@speaker`/`@option`).
    Localized(Vec<(String, String)>),
    NumberLit(f64),
    BoolLit(bool),
    Variable(String),
    ArrayLit(Vec<Expression>),
    ObjectLit(Vec<(String, Expression)>),
    Call {
        callee: String,
        args: Vec<Expression>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expression>,
    },
    BinaryOp {
        op: BinOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    TernaryOp {
        condition: Box<Expression>,
        then_expr: Box<Expression>,
        else_expr: Box<Expression>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Neq,
    Gt,
    Lt,
    Gte,
    Lte,
    And,
    Or,
    BitOr,
    BitAnd,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file: String,
    pub line_start: usize,
    pub col_start: usize,
    pub line_end: usize,
    pub col_end: usize,
    pub byte_start: usize,
    pub byte_end: usize,
}

impl SourceSpan {
    pub fn new(
        file: impl Into<String>,
        line_start: usize,
        col_start: usize,
        line_end: usize,
        col_end: usize,
        byte_start: usize,
        byte_end: usize,
    ) -> Self {
        Self {
            file: file.into(),
            line_start,
            col_start,
            line_end,
            col_end,
            byte_start,
            byte_end,
        }
    }

    pub fn point(file: impl Into<String>, line: usize, col: usize) -> Self {
        Self {
            file: file.into(),
            line_start: line,
            col_start: col,
            line_end: line,
            col_end: col,
            byte_start: 0,
            byte_end: 0,
        }
    }
}
