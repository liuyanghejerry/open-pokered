// Storyline → JavaScript codegen.
//
// Transforms AST `StorylineBlock` + `StoryStmt` nodes into an executable
// ES module JavaScript function `export async function storyline_main()`.
//
// Mapping rules are defined in `docs/DSL_MAPPING.md` entries 1-4.

use crate::ast::*;
use crate::sourcemap::SourceMapBuilder;

/// Convert an AST `SourceSpan` into the sourcemap-layer `SourceSpan`.
fn to_sm_span(ast_span: &SourceSpan) -> crate::sourcemap::SourceSpan {
    crate::sourcemap::SourceSpan {
        file: ast_span.file.clone(),
        line_start: ast_span.line_start as u32,
        col_start: ast_span.col_start as u32,
        line_end: ast_span.line_end as u32,
        col_end: ast_span.col_end as u32,
    }
}

/// Return the bare string value of a `StringLit` expression, or `None`.
fn as_string_lit(expr: &Expression) -> Option<&str> {
    match expr {
        Expression::StringLit(s) => Some(s.as_str()),
        _ => None,
    }
}

/// Map a `BinOp` to its JavaScript operator string.
fn binop_to_js(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Eq => "===",
        BinOp::Neq => "!==",
        BinOp::Gt => ">",
        BinOp::Lt => "<",
        BinOp::Gte => ">=",
        BinOp::Lte => "<=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::BitOr => "|",
        BinOp::BitAnd => "&",
    }
}

/// Build an indentation string of `depth * 2` spaces.
fn indent(depth: usize) -> String {
    "  ".repeat(depth)
}

/// Compile `@run { ... }` raw JavaScript block.
///
/// Emits each non-empty line indented by `depth * 2` spaces.
/// Empty lines (whitespace-only) are preserved as blank lines.
/// No sourcemap entries are produced since @run contains raw JS.
fn compile_run(js: &str, depth: usize) -> String {
    if js.is_empty() {
        return String::new();
    }
    let pad = indent(depth);
    js.lines()
        .map(|line| {
            if line.trim().is_empty() {
                "\n".to_string()
            } else {
                format!("{}{}\n", pad, line)
            }
        })
        .collect()
}

fn compile_expression(expr: &Expression) -> String {
    match expr {
        Expression::StringLit(s) => serde_json::to_string(s).unwrap(),
        Expression::Localized(pairs) => crate::codegen::i18n::localized_pairs_to_js_t(pairs),
        Expression::NumberLit(n) => {
            if *n == n.trunc() && n.is_finite() {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        Expression::BoolLit(b) => {
            if *b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Expression::Variable(name) => name.clone(),
        Expression::ArrayLit(elements) => {
            let parts: Vec<String> = elements.iter().map(compile_expression).collect();
            format!("[{}]", parts.join(", "))
        }
        Expression::Call { callee, args } => {
            let args_js: Vec<String> = args.iter().map(compile_expression).collect();
            // All callable APIs (getFlag, hasItem, …) live on the `game` object;
            // there are no bare globals (see ScriptEngine::new). The DSL has no
            // member access, so a condition/argument call is written bare
            // (`getFlag(...)`) and MUST be namespaced to `game.` here — otherwise
            // it compiles to a bare reference that throws ReferenceError at
            // runtime (and the engine silently swallows it). Statements already
            // namespace via `game["name"](...)` in compile_command.
            format!("game.{}({})", callee, args_js.join(", "))
        }
        Expression::UnaryOp { op, operand } => {
            let inner = compile_expression(operand);
            match op {
                UnaryOp::Not => format!("(!{inner})"),
                UnaryOp::Neg => format!("(-{inner})"),
            }
        }
        Expression::BinaryOp { op, left, right } => {
            let lhs = compile_expression(left);
            let rhs = compile_expression(right);
            format!("({} {} {})", lhs, binop_to_js(*op), rhs)
        }
        Expression::TernaryOp {
            condition,
            then_expr,
            else_expr,
        } => {
            let cond = compile_expression(condition);
            let then = compile_expression(then_expr);
            let els = compile_expression(else_expr);
            format!("({} ? {} : {})", cond, then, els)
        }
        Expression::ObjectLit(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{}: {}", k, compile_expression(v)))
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
    }
}

fn compile_story_stmt(
    stmt: &StoryStmt,
    sourcemap: &mut SourceMapBuilder,
    line: &mut u32,
    depth: usize,
) -> String {
    match stmt {
        StoryStmt::Speaker { name, texts, span } => {
            compile_speaker(name, texts, span, sourcemap, line, depth)
        }
        StoryStmt::Say { name, texts, span } => {
            compile_speaker(name, texts, span, sourcemap, line, depth)
        }
        StoryStmt::Choice { options, span } => {
            compile_choice(options, span, sourcemap, line, depth)
        }
        StoryStmt::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => compile_if(
            condition,
            then_branch,
            else_branch,
            span,
            sourcemap,
            line,
            depth,
        ),
        StoryStmt::Each {
            item_var,
            source,
            body,
            span,
        } => compile_each(item_var, source, body, span, sourcemap, line, depth),
        StoryStmt::Assign { name, value, span } => {
            compile_assign(name, value, span, sourcemap, line, depth)
        }
        StoryStmt::Command { name, args, span } => {
            compile_command(name, args, span, sourcemap, line, depth)
        }
        StoryStmt::Run { js, .. } => compile_run(js, depth),
    }
}

/// Compile `@speaker("Name") { "text" }` / `@say("Name") { "text" }`
/// → `await game.showText("Name: text");`
///
/// Both render identically (`game.showText`, A-advanced); they differ in
/// *meaning*: `@speaker` is player-initiated dialogue (talking to an NPC),
/// `@say` is a cutscene line inside an auto-triggered storyline.
///
/// Multiple text lines are joined with newlines. An **empty** string-literal
/// name (`@speaker("")`) is the narrator/no-speaker form and emits the body
/// verbatim with no prefix — this is how prefix-less original dialogue is
/// represented faithfully (avoids a spurious `": "` / `"System: "` prefix).
/// A non-empty string-literal name is prepended as `"Name: "`; for variable
/// names a template literal `` `${name}: text` `` is emitted.
fn compile_speaker(
    name: &Expression,
    texts: &[LocalizedText],
    span: &SourceSpan,
    sourcemap: &mut SourceMapBuilder,
    line: &mut u32,
    depth: usize,
) -> String {
    sourcemap.record_span(&to_sm_span(span), *line, 0);

    let pad = indent(depth);

    // Join each line's text per-language. Plain lines read identically for
    // every locale; `@t(...)` lines contribute their per-locale variant.
    let has_localized = texts.iter().any(|t| t.is_localized());
    let en_body = texts
        .iter()
        .map(|t| t.get("en"))
        .collect::<Vec<_>>()
        .join("\n");

    let text_arg = if !has_localized {
        // Monolingual fast path — byte-for-byte the historical output.
        let body = en_body;
        if let Some(name_str) = as_string_lit(name) {
            let full = if name_str.is_empty() {
                body
            } else {
                format!("{}: {}", name_str, body)
            };
            serde_json::to_string(&full).unwrap()
        } else {
            let name_js = compile_expression(name);
            let escaped_body = body
                .replace('\\', "\\\\")
                .replace('`', "\\`")
                .replace("${", "\\${");
            format!("`${{{}}}: {}`", name_js, escaped_body)
        }
    } else {
        // Bilingual — emit `game.t("en", "zh")`. For a string-literal speaker
        // name the "Name: " prefix is baked into each language string; for a
        // variable name it is applied via a template literal.
        let zh_body = texts
            .iter()
            .map(|t| t.get("zh"))
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(name_str) = as_string_lit(name) {
            let (en_full, zh_full) = if name_str.is_empty() {
                (en_body, zh_body)
            } else {
                (
                    format!("{}: {}", name_str, en_body),
                    format!("{}: {}", name_str, zh_body),
                )
            };
            format!(
                "game.t({}, {})",
                serde_json::to_string(&en_full).unwrap(),
                serde_json::to_string(&zh_full).unwrap()
            )
        } else {
            let name_js = compile_expression(name);
            format!(
                "`${{{}}}: ${{game.t({}, {})}}`",
                name_js,
                serde_json::to_string(&en_body).unwrap(),
                serde_json::to_string(&zh_body).unwrap()
            )
        }
    };

    let js = format!("{}await game.showText({});\n", pad, text_arg);
    *line += 1;
    js
}

/// Compile `@choice { @option("A") { ... } @option("B") { ... } }` into
/// a `game.showChoice()` call + if/else chain.
///
/// The last option uses an `else` branch; all preceding options use
/// `if (choice === N)` / `else if (choice === N)`.
fn compile_choice(
    options: &[ChoiceOption],
    span: &SourceSpan,
    sourcemap: &mut SourceMapBuilder,
    line: &mut u32,
    depth: usize,
) -> String {
    sourcemap.record_span(&to_sm_span(span), *line, 0);

    let pad = indent(depth);
    let mut out = String::new();

    let labels: Vec<String> = options
        .iter()
        .map(|o| match &o.label {
            LocalizedText::Plain(s) => serde_json::to_string(s).unwrap(),
            LocalizedText::Localized(pairs) => crate::codegen::i18n::localized_pairs_to_js_t(pairs),
        })
        .collect();
    let labels_js = format!("[{}]", labels.join(", "));

    out.push_str(&format!(
        "{}const choice = await game.showChoice({});\n",
        pad, labels_js
    ));
    *line += 1;

    if options.is_empty() {
        return out;
    }

    let last = options.len() - 1;
    for (i, opt) in options.iter().enumerate() {
        let guard = if i == 0 {
            format!("{}if (choice === {}) {{\n", pad, i)
        } else if i == last {
            format!("{}}} else {{\n", pad)
        } else {
            format!("{}}} else if (choice === {}) {{\n", pad, i)
        };
        out.push_str(&guard);
        *line += 1;

        for body_stmt in &opt.body {
            let body_js = compile_story_stmt(body_stmt, sourcemap, line, depth + 1);
            out.push_str(&body_js);
        }
    }

    out.push_str(&format!("{}}}\n", pad));
    *line += 1;

    out
}

/// Compile `@if (cond) { ... } @else { ... }` → `if (cond) { ... } else { ... }`
fn compile_if(
    condition: &Expression,
    then_branch: &[StoryStmt],
    else_branch: &[StoryStmt],
    span: &SourceSpan,
    sourcemap: &mut SourceMapBuilder,
    line: &mut u32,
    depth: usize,
) -> String {
    sourcemap.record_span(&to_sm_span(span), *line, 0);

    let pad = indent(depth);
    let cond_js = compile_expression(condition);
    let mut out = String::new();

    // `if (condition) {`
    out.push_str(&format!("{}if ({}) {{\n", pad, cond_js));
    *line += 1;

    for stmt in then_branch {
        let js = compile_story_stmt(stmt, sourcemap, line, depth + 1);
        out.push_str(&js);
    }

    if else_branch.is_empty() {
        out.push_str(&format!("{}}}\n", pad));
        *line += 1;
    } else {
        out.push_str(&format!("{}}} else {{\n", pad));
        *line += 1;

        for stmt in else_branch {
            let js = compile_story_stmt(stmt, sourcemap, line, depth + 1);
            out.push_str(&js);
        }

        out.push_str(&format!("{}}}\n", pad));
        *line += 1;
    }

    out
}

/// Compile `@each item in items { ... }` → `for (const item of items) { ... }`
fn compile_each(
    item_var: &str,
    source: &Expression,
    body: &[StoryStmt],
    span: &SourceSpan,
    sourcemap: &mut SourceMapBuilder,
    line: &mut u32,
    depth: usize,
) -> String {
    sourcemap.record_span(&to_sm_span(span), *line, 0);

    let pad = indent(depth);
    let source_js = compile_expression(source);
    let mut out = String::new();

    out.push_str(&format!(
        "{}for (const {} of {}) {{\n",
        pad, item_var, source_js
    ));
    *line += 1;

    for stmt in body {
        let js = compile_story_stmt(stmt, sourcemap, line, depth + 1);
        out.push_str(&js);
    }

    out.push_str(&format!("{}}}\n", pad));
    *line += 1;

    out
}

/// Compile `name = value` → `let name = value;`
fn compile_assign(
    name: &str,
    value: &Expression,
    _span: &SourceSpan,
    _sourcemap: &mut SourceMapBuilder,
    line: &mut u32,
    depth: usize,
) -> String {
    let pad = indent(depth);
    let val_js = compile_expression(value);
    // A game-API call returns a Promise (async effects like startBattle/giveItem)
    // or a plain value (sync queries like getMoney/hasItem); either way the
    // assignment must `await` it so `let r = startBattle(...)` captures the
    // resolved outcome rather than the pending Promise. Awaiting a non-Promise is
    // a JS no-op, so this is safe for sync queries too.
    let val_js = if matches!(value, Expression::Call { .. }) {
        format!("await {}", val_js)
    } else {
        val_js
    };
    // Bare assignment (no `let`): every Assign variable is declared once at the
    // top of the storyline function (see compile_named_block). A `let` here
    // would be block-scoped, so an Assign inside an @if/@choice branch (e.g.
    // `result = startBattle(...)` picking the rival's party) was invisible to
    // later statements (`if (result === "win")` threw ReferenceError and the
    // storyline died silently after the battle).
    let js = format!("{}{} = {};\n", pad, name, val_js);
    *line += 1;
    js
}

/// Collect every Assign variable name in a statement list, recursing into
/// @if/@each/@choice bodies (they can hold call-valued assignments like
/// `result = startBattle(...)` that later statements reference).
fn collect_assign_names(stmts: &[StoryStmt], out: &mut Vec<String>) {
    for stmt in stmts {
        match stmt {
            StoryStmt::Assign { name, .. } => {
                if !out.contains(name) {
                    out.push(name.clone());
                }
            }
            StoryStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_assign_names(then_branch, out);
                collect_assign_names(else_branch, out);
            }
            StoryStmt::Each { body, .. } => collect_assign_names(body, out),
            StoryStmt::Choice { options, .. } => {
                for opt in options {
                    collect_assign_names(&opt.body, out);
                }
            }
            _ => {}
        }
    }
}

/// Compile `@command("name", args...)` → `await game["name"](args...);`
fn compile_command(
    name: &str,
    args: &[Expression],
    span: &SourceSpan,
    sourcemap: &mut SourceMapBuilder,
    line: &mut u32,
    depth: usize,
) -> String {
    sourcemap.record_span(&to_sm_span(span), *line, 0);

    let pad = indent(depth);
    let args_js: Vec<String> = args.iter().map(compile_expression).collect();
    let js = format!("{}await game[\"{}\"]({});\n", pad, name, args_js.join(", "));
    *line += 1;
    js
}

pub fn compile_storyline(
    name: &str,
    storyline: &StorylineBlock,
    sourcemap: &mut SourceMapBuilder,
) -> String {
    compile_storyline_with_vars(name, storyline, sourcemap, &[])
}

/// Compile a storyline, treating `module_vars` (the scene's @variables names)
/// as module-scoped: assignments to them are bare mutations with no local
/// `let` redeclaration (which would shadow the module variable).
pub fn compile_storyline_with_vars(
    name: &str,
    storyline: &StorylineBlock,
    sourcemap: &mut SourceMapBuilder,
    module_vars: &[String],
) -> String {
    let func_name = format!("storyline_{}", name);
    compile_named_block(&func_name, storyline, sourcemap, module_vars)
}

pub fn compile_onload(
    scene_name: &str,
    block: &StorylineBlock,
    sourcemap: &mut SourceMapBuilder,
) -> String {
    let func_name = format!("{}OnLoad", scene_name);
    compile_named_block(&func_name, block, sourcemap, &[])
}

fn compile_named_block(
    func_name: &str,
    block: &StorylineBlock,
    sourcemap: &mut SourceMapBuilder,
    module_vars: &[String],
) -> String {
    let mut line: u32 = 0;
    let mut out = String::new();

    out.push_str(&format!("export async function {}() {{\n", func_name));
    line += 1;

    // Hoist plain variable declarations (non-call Assign) to the top of the
    // block so the `let` bindings exist before any story logic references them.
    // Assignments that CALL a game API (e.g. `result = startBattle(...)`,
    // `floor = elevatorMenu([...])`) must stay in their original position:
    // hoisting them would execute the effect (battle, menu, item) before the
    // preceding dialogue/cutscene lines that set the scene.
    let (decls, rest): (Vec<&StoryStmt>, Vec<&StoryStmt>) =
        block.statements.iter().partition(|s| match s {
            StoryStmt::Assign { value, .. } => !matches!(value, Expression::Call { .. }),
            _ => false,
        });

    let mut declared: Vec<String> = Vec::new();
    for stmt in &decls {
        // Emit the declaration with its initializer (`let gold = 500;`) —
        // compile_assign emits bare `name = ...` (no `let`) by design.
        // A name from the scene's @variables block is NOT redeclared: the
        // assignment mutates the module-scoped variable (bare, no `let`).
        if let StoryStmt::Assign { name, value, .. } = stmt {
            let val_js = compile_expression(value);
            if module_vars.contains(name) {
                out.push_str(&format!("{}{} = {};\n", indent(1), name, val_js));
            } else {
                out.push_str(&format!("{}let {} = {};\n", indent(1), name, val_js));
                declared.push(name.clone());
            }
            line += 1;
        }
    }

    // Declare every remaining Assign variable (call-valued or nested in
    // @if/@each/@choice branches) as `let name;` so in-place bare assignments
    // write to a function-scoped binding rather than an implicit global.
    // Names declared in the scene's @variables block are SKIPPED — they live
    // at module scope and a `let` here would shadow them (the old in-place
    // `let gold = ...` had exactly that TDZ/shadowing bug).
    let mut extra = Vec::new();
    collect_assign_names(&block.statements, &mut extra);
    for name in extra {
        if !declared.contains(&name) && !module_vars.contains(&name) {
            out.push_str(&format!("{}let {};\n", indent(1), name));
            line += 1;
            declared.push(name);
        }
    }

    if !declared.is_empty() && !rest.is_empty() {
        out.push('\n');
        line += 1;
    }

    for stmt in &rest {
        let js = compile_story_stmt(stmt, sourcemap, &mut line, 1);
        out.push_str(&js);
    }

    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn span(line: usize, col: usize) -> SourceSpan {
        SourceSpan::point("test.scene", line, col)
    }

    fn s(val: &str) -> Expression {
        Expression::StringLit(val.to_string())
    }

    fn n(val: f64) -> Expression {
        Expression::NumberLit(val)
    }

    fn v(name: &str) -> Expression {
        Expression::Variable(name.to_string())
    }

    fn binop(op: BinOp, left: Expression, right: Expression) -> Expression {
        Expression::BinaryOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    fn compile_stmt(stmt: &StoryStmt) -> (String, SourceMapBuilder) {
        let mut sm = SourceMapBuilder::new("test.scene", "test.scene.js");
        let mut line = 0;
        let js = compile_story_stmt(stmt, &mut sm, &mut line, 0);
        (js, sm)
    }

    fn compile_block(block: &StorylineBlock) -> (String, SourceMapBuilder) {
        let mut sm = SourceMapBuilder::new("test.scene", "test.scene.js");
        let js = compile_storyline("main", block, &mut sm);
        (js, sm)
    }

    #[test]
    fn test_speaker_empty_name_no_prefix() {
        // @speaker("") is the narrator/no-speaker form: body verbatim, no "X: " prefix.
        let stmt = StoryStmt::Speaker {
            name: s(""),
            texts: vec!["I'm raising".into(), "monsters too!".into()],
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(js.contains("await game.showText("));
        assert!(
            js.contains("\"I'm raising\\nmonsters too!\""),
            "empty speaker should emit body verbatim, got: {}",
            js
        );
        assert!(
            !js.contains(": I'm raising"),
            "empty speaker must not add a prefix, got: {}",
            js
        );
    }

    // i18n: @t localized text/labels

    fn lt(en: &str, zh: &str) -> LocalizedText {
        LocalizedText::Localized(vec![("en".into(), en.into()), ("zh".into(), zh.into())])
    }

    #[test]
    fn test_speaker_localized_narrator_emits_game_t() {
        // @speaker("") with a localized line → game.showText(game.t("en", "zh")).
        let stmt = StoryStmt::Speaker {
            name: s(""),
            texts: vec![lt("Hello!", "你好！")],
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(
            js.contains("await game.showText(game.t(\"Hello!\", \"你好！\"))"),
            "got: {}",
            js
        );
    }

    #[test]
    fn test_speaker_localized_bakes_name_prefix_per_language() {
        // A string-literal speaker name prefixes BOTH language strings.
        let stmt = StoryStmt::Speaker {
            name: s("PROF"),
            texts: vec![lt("Hi", "你好")],
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(
            js.contains("game.t(\"PROF: Hi\", \"PROF: 你好\")"),
            "got: {}",
            js
        );
    }

    #[test]
    fn test_speaker_mixed_plain_and_localized_lines() {
        // A plain line + a localized line: plain text is shared across both
        // languages; only the localized line diverges.
        let stmt = StoryStmt::Speaker {
            name: s(""),
            texts: vec!["Shared".into(), lt("EN", "中")],
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(
            js.contains("game.t(\"Shared\\nEN\", \"Shared\\n中\")"),
            "got: {}",
            js
        );
    }

    #[test]
    fn test_choice_localized_label() {
        let stmt = StoryStmt::Choice {
            options: vec![
                ChoiceOption {
                    label: lt("YES", "是"),
                    body: vec![],
                    span: span(1, 0),
                },
                ChoiceOption {
                    label: "NO".into(),
                    body: vec![],
                    span: span(1, 0),
                },
            ],
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(
            js.contains("game.showChoice([game.t(\"YES\", \"是\"), \"NO\"])"),
            "got: {}",
            js
        );
    }

    // Expression tests

    #[test]
    fn test_expression_string_lit() {
        assert_eq!(compile_expression(&s("Hello")), "\"Hello\"");
        assert_eq!(
            compile_expression(&s("He said \"hi\"")),
            "\"He said \\\"hi\\\"\""
        );
        assert_eq!(compile_expression(&s("Line1\nLine2")), "\"Line1\\nLine2\"");
    }

    #[test]
    fn test_expression_number_lit() {
        assert_eq!(compile_expression(&n(42.0)), "42");
        assert_eq!(compile_expression(&n(3.14)), "3.14");
        assert_eq!(compile_expression(&n(0.0)), "0");
        assert_eq!(compile_expression(&n(-5.0)), "-5");
    }

    #[test]
    fn test_expression_bool_lit() {
        assert_eq!(compile_expression(&Expression::BoolLit(true)), "true");
        assert_eq!(compile_expression(&Expression::BoolLit(false)), "false");
    }

    #[test]
    fn test_expression_variable() {
        assert_eq!(compile_expression(&v("gold")), "gold");
        assert_eq!(compile_expression(&v("player_name")), "player_name");
    }

    #[test]
    fn test_expression_binary_ops() {
        assert_eq!(
            compile_expression(&binop(BinOp::Add, n(1.0), n(2.0))),
            "(1 + 2)"
        );
        assert_eq!(
            compile_expression(&binop(BinOp::Gt, v("gold"), n(100.0))),
            "(gold > 100)"
        );
        assert_eq!(
            compile_expression(&binop(BinOp::Eq, v("x"), Expression::BoolLit(true))),
            "(x === true)"
        );
        assert_eq!(
            compile_expression(&binop(
                BinOp::And,
                binop(BinOp::Gt, v("a"), n(0.0)),
                binop(BinOp::Lt, v("a"), n(10.0))
            )),
            "((a > 0) && (a < 10))"
        );
    }

    #[test]
    fn test_expression_ternary() {
        assert_eq!(
            compile_expression(&Expression::TernaryOp {
                condition: Box::new(binop(BinOp::Gt, v("x"), n(0.0))),
                then_expr: Box::new(s("positive")),
                else_expr: Box::new(s("negative")),
            }),
            "((x > 0) ? \"positive\" : \"negative\")"
        );
    }

    // Speaker tests

    #[test]
    fn test_speaker_single_text() {
        let stmt = StoryStmt::Speaker {
            name: s("Prof"),
            texts: vec!["Hello!".into()],
            span: span(1, 0),
        };
        let (js, sm) = compile_stmt(&stmt);
        assert!(js.contains("await game.showText("));
        assert!(js.contains("\"Prof: Hello!\""));
        assert_eq!(sm.mappings().len(), 1);
    }

    #[test]
    fn test_speaker_multiple_texts() {
        let stmt = StoryStmt::Speaker {
            name: s("Prof"),
            texts: vec!["Hello!".into(), "Welcome!".into()],
            span: span(1, 0),
        };
        let (js, sm) = compile_stmt(&stmt);
        assert!(js.contains("await game.showText("));
        assert!(js.contains("\\n"));
        assert_eq!(sm.mappings().len(), 1);
    }

    #[test]
    fn test_speaker_variable_name() {
        let stmt = StoryStmt::Speaker {
            name: v("npcName"),
            texts: vec!["Hello!".into()],
            span: span(1, 0),
        };
        let (js, sm) = compile_stmt(&stmt);
        assert!(js.contains("${npcName}"));
        assert!(js.contains("await game.showText("));
        assert_eq!(sm.mappings().len(), 1);
    }

    #[test]
    fn test_say_cutscene_line() {
        // `@say("Prof")` is cutscene speech; it renders through the same
        // showText path as @speaker (A-advanced), differing only in meaning.
        let stmt = StoryStmt::Say {
            name: s("Prof"),
            texts: vec!["Hello!".into(), "Welcome!".into()],
            span: span(1, 0),
        };
        let (js, sm) = compile_stmt(&stmt);
        assert!(js.contains("await game.showText("));
        assert!(js.contains("\"Prof: Hello!\\nWelcome!\""));
        assert!(
            !js.contains("showTextAuto"),
            "@say must not emit showTextAuto"
        );
        assert_eq!(sm.mappings().len(), 1);

        // Bilingual @say routes through game.t, like @speaker.
        let stmt = StoryStmt::Say {
            name: s(""),
            texts: vec![LocalizedText::Localized(vec![
                ("en".into(), "Hi".into()),
                ("zh".into(), "你好".into()),
            ])],
            span: span(1, 0),
        };
        let (js, _) = compile_stmt(&stmt);
        assert!(js.contains("await game.showText(game.t("));
        assert!(js.contains("\"Hi\""));
        assert!(js.contains("\"你好\""));
    }

    // Choice tests

    #[test]
    fn test_choice_two_options() {
        let stmt = StoryStmt::Choice {
            options: vec![
                ChoiceOption {
                    label: "Yes".into(),
                    body: vec![StoryStmt::Speaker {
                        name: s("Prof"),
                        texts: vec!["Great!".into()],
                        span: span(3, 0),
                    }],
                    span: span(2, 0),
                },
                ChoiceOption {
                    label: "No".into(),
                    body: vec![StoryStmt::Speaker {
                        name: s("Prof"),
                        texts: vec!["Too bad.".into()],
                        span: span(5, 0),
                    }],
                    span: span(4, 0),
                },
            ],
            span: span(1, 0),
        };
        let (js, sm) = compile_stmt(&stmt);
        assert!(js.contains("const choice = await game.showChoice([\"Yes\", \"No\"]);"));
        assert!(js.contains("if (choice === 0) {"));
        assert!(js.contains("} else {"));
        assert!(sm.mappings().len() >= 1);
    }

    #[test]
    fn test_choice_three_options() {
        let stmt = StoryStmt::Choice {
            options: vec![
                ChoiceOption {
                    label: "A".into(),
                    body: vec![],
                    span: span(1, 0),
                },
                ChoiceOption {
                    label: "B".into(),
                    body: vec![],
                    span: span(1, 0),
                },
                ChoiceOption {
                    label: "C".into(),
                    body: vec![],
                    span: span(1, 0),
                },
            ],
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(js.contains("if (choice === 0) {"));
        assert!(js.contains("else if (choice === 1) {"));
        assert!(js.contains("} else {"));
    }

    // If / Else tests

    #[test]
    fn test_if_else() {
        let stmt = StoryStmt::If {
            condition: binop(BinOp::Gt, v("gold"), n(100.0)),
            then_branch: vec![StoryStmt::Speaker {
                name: s("Shopkeeper"),
                texts: vec!["You have enough!".into()],
                span: span(2, 0),
            }],
            else_branch: vec![StoryStmt::Speaker {
                name: s("Shopkeeper"),
                texts: vec!["Not enough.".into()],
                span: span(4, 0),
            }],
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(js.contains("if ((gold > 100)) {"));
        assert!(js.contains("Shopkeeper: You have enough!"));
        assert!(js.contains("} else {"));
        assert!(js.contains("Shopkeeper: Not enough."));
    }

    #[test]
    fn test_if_no_else() {
        let stmt = StoryStmt::If {
            condition: binop(BinOp::Eq, v("hasKey"), Expression::BoolLit(true)),
            then_branch: vec![StoryStmt::Speaker {
                name: s("Guard"),
                texts: vec!["Door opens.".into()],
                span: span(2, 0),
            }],
            else_branch: vec![],
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(js.contains("if ((hasKey === true)) {"));
        assert!(js.contains("Guard: Door opens."));
        assert!(!js.contains("else"));
    }

    // Nested control flow

    #[test]
    fn test_nested_choice_in_if() {
        let stmt = StoryStmt::If {
            condition: v("hasStarter"),
            then_branch: vec![StoryStmt::Speaker {
                name: s("Prof"),
                texts: vec!["You have a monster.".into()],
                span: span(2, 0),
            }],
            else_branch: vec![StoryStmt::Choice {
                options: vec![
                    ChoiceOption {
                        label: "Flambit".into(),
                        body: vec![StoryStmt::Speaker {
                            name: s("Prof"),
                            texts: vec!["Fiery!".into()],
                            span: span(5, 0),
                        }],
                        span: span(4, 0),
                    },
                    ChoiceOption {
                        label: "Aquakit".into(),
                        body: vec![StoryStmt::Speaker {
                            name: s("Prof"),
                            texts: vec!["Water!".into()],
                            span: span(7, 0),
                        }],
                        span: span(6, 0),
                    },
                ],
                span: span(3, 0),
            }],
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(js.contains("if (hasStarter) {"));
        assert!(js.contains("Prof: You have a monster."));
        assert!(js.contains("const choice = await game.showChoice"));
        assert!(js.contains("\"Flambit\""));
        assert!(js.contains("\"Aquakit\""));
        assert!(js.contains("Prof: Fiery!"));
        assert!(js.contains("Prof: Water!"));
    }

    // Each loop

    #[test]
    fn test_each_loop() {
        let stmt = StoryStmt::Each {
            item_var: "item".to_string(),
            source: v("inventory"),
            body: vec![StoryStmt::Speaker {
                name: s("Shopkeeper"),
                texts: vec!["Here you go.".into()],
                span: span(2, 0),
            }],
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(js.contains("for (const item of inventory) {"));
        assert!(js.contains("Shopkeeper: Here you go."));
        assert!(js.contains("}"));
    }

    // Assign

    #[test]
    fn test_assign_statement() {
        let stmt = StoryStmt::Assign {
            name: "gold".to_string(),
            value: n(500.0),
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        // Bare assignment — the `let gold;` declaration is hoisted to the top
        // of the storyline function by compile_named_block.
        assert_eq!(js.trim(), "gold = 500;");
    }

    #[test]
    fn test_assign_string() {
        let stmt = StoryStmt::Assign {
            name: "playerName".to_string(),
            value: s("RED"),
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert_eq!(js.trim(), "playerName = \"RED\";");
    }

    // Command

    #[test]
    fn test_command_statement() {
        let stmt = StoryStmt::Command {
            name: "giveItem".to_string(),
            args: vec![s("POTION"), n(1.0)],
            span: span(1, 0),
        };
        let (js, sm) = compile_stmt(&stmt);
        assert!(js.contains("await game[\"giveItem\"](\"POTION\", 1);"));
        assert_eq!(sm.mappings().len(), 1);
    }

    // Full storyline

    #[test]
    fn test_full_storyline() {
        let block = StorylineBlock {
            statements: vec![
                // Variable declarations
                StoryStmt::Assign {
                    name: "gold".to_string(),
                    value: n(500.0),
                    span: span(1, 0),
                },
                StoryStmt::Assign {
                    name: "player".to_string(),
                    value: s("RED"),
                    span: span(2, 0),
                },
                // Speaker
                StoryStmt::Speaker {
                    name: s("Prof"),
                    texts: vec!["Hello!".into(), "Welcome!".into()],
                    span: span(4, 0),
                },
                // Choice
                StoryStmt::Choice {
                    options: vec![
                        ChoiceOption {
                            label: "Yes".into(),
                            body: vec![StoryStmt::Speaker {
                                name: s("Prof"),
                                texts: vec!["Great!".into()],
                                span: span(7, 0),
                            }],
                            span: span(5, 0),
                        },
                        ChoiceOption {
                            label: "No".into(),
                            body: vec![],
                            span: span(8, 0),
                        },
                    ],
                    span: span(4, 0),
                },
            ],
            span: span(1, 0),
        };

        let (js, sm) = compile_block(&block);

        assert!(js.starts_with("export async function storyline_main() {"));
        assert!(js.ends_with("}\n"));

        let gold_pos = js.find("let gold = 500;").unwrap();
        let player_pos = js.find("let player = \"RED\";").unwrap();
        let speaker_pos = js.find("await game.showText").unwrap();
        assert!(
            gold_pos < speaker_pos,
            "let declarations should precede story logic"
        );
        assert!(
            player_pos < speaker_pos,
            "let declarations should precede story logic"
        );

        // Content checks
        assert!(js.contains("Prof: Hello!\\nWelcome!"));
        assert!(js.contains("const choice = await game.showChoice"));
        assert!(sm.mappings().len() >= 1);
    }

    /// Call-assignments (e.g. `result = startBattle(...)`, `floor =
    /// elevatorMenu([...])`) must stay in their original position — hoisting
    /// them to the top would run the effect before the preceding dialogue.
    #[test]
    fn test_call_assign_stays_in_place() {
        let block = StorylineBlock {
            statements: vec![
                StoryStmt::Speaker {
                    name: s(""),
                    texts: vec!["Ready?".into()],
                    span: span(1, 0),
                },
                StoryStmt::Assign {
                    name: "floor".to_string(),
                    value: Expression::Call {
                        callee: "elevatorMenu".to_string(),
                        args: vec![s("1F")],
                    },
                    span: span(2, 0),
                },
                StoryStmt::If {
                    condition: Expression::BinaryOp {
                        op: BinOp::Eq,
                        left: Box::new(Expression::Variable("floor".into())),
                        right: Box::new(Expression::NumberLit(0.0)),
                    },
                    then_branch: vec![StoryStmt::Command {
                        name: "warpTo".to_string(),
                        args: vec![s("CityMart1F"), n(12.0), n(1.0)],
                        span: span(3, 0),
                    }],
                    else_branch: vec![],
                    span: span(3, 0),
                },
            ],
            span: span(1, 0),
        };

        let (js, _sm) = compile_block(&block);

        let speaker_pos = js.find("await game.showText").unwrap();
        let menu_pos = js.find("game.elevatorMenu").unwrap();
        let warp_pos = js.find("game[\"warpTo\"]").unwrap();
        assert!(
            speaker_pos < menu_pos,
            "elevatorMenu call-assign must run AFTER the preceding showText"
        );
        assert!(
            menu_pos < warp_pos,
            "warpTo must run after the elevatorMenu result is available"
        );
        assert!(js.contains("let floor;"));
        assert!(js.contains("floor = await game.elevatorMenu"));
        assert!(js.contains("if ((floor === 0))"));
    }

    /// Regression: a call-valued Assign nested in an @if branch (rival party
    /// selection) used to compile to a block-scoped `let result = ...`, so the
    /// later `if (result === "win")` threw ReferenceError and the storyline
    /// died silently right after the battle. The declaration must be hoisted
    /// to function scope and the in-place assignment must be bare.
    #[test]
    fn test_branch_assign_is_function_scoped() {
        let battle = |opp: &str| StoryStmt::Assign {
            name: "result".to_string(),
            value: Expression::Call {
                callee: "startBattle".to_string(),
                args: vec![s(opp)],
            },
            span: span(2, 0),
        };
        let block = StorylineBlock {
            statements: vec![StoryStmt::If {
                condition: Expression::Variable("flagA".into()),
                then_branch: vec![battle("OPP_RIVAL1")],
                else_branch: vec![battle("OPP_RIVAL2")],
                span: span(1, 0),
            }],
            span: span(1, 0),
        };
        let (js, _sm) = compile_block(&block);
        assert!(js.contains("let result;"), "hoisted declaration: {}", js);
        assert!(js.contains("result = await game.startBattle(\"OPP_RIVAL1\");"));
        assert!(js.contains("result = await game.startBattle(\"OPP_RIVAL2\");"));
        assert!(
            !js.contains("let result ="),
            "no block-scoped shadowing: {}",
            js
        );
        // Declaration precedes both branches.
        let decl = js.find("let result;").unwrap();
        let use1 = js
            .find("result = await game.startBattle(\"OPP_RIVAL1\")")
            .unwrap();
        assert!(decl < use1);
    }

    // Edge cases

    #[test]
    fn test_empty_storyline() {
        let block = StorylineBlock {
            statements: vec![],
            span: span(1, 0),
        };
        let (js, _sm) = compile_block(&block);
        assert!(js.contains("export async function storyline_main() {"));
        assert!(js.contains("}\n"));
    }

    #[test]
    fn test_empty_texts() {
        let stmt = StoryStmt::Speaker {
            name: s("Ghost"),
            texts: vec![],
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(js.contains("await game.showText(\"Ghost: \");"));
    }

    #[test]
    fn test_empty_choice() {
        let stmt = StoryStmt::Choice {
            options: vec![],
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(js.contains("game.showChoice([])"));
    }

    // @run block tests

    #[test]
    fn test_run_block_basic() {
        let stmt = StoryStmt::Run {
            js: "game.setFlag(\"EVENT_PROF_INTRO\");".to_string(),
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(
            js.contains("game.setFlag"),
            "JS should contain the run content"
        );
        assert!(!js.contains("todo"), "Should NOT have todo macro anymore");
    }

    #[test]
    fn test_run_block_multi_line() {
        let js_text = "let x = 1;\nlet y = 2;\ngame.doSomething(x, y);";
        let stmt = StoryStmt::Run {
            js: js_text.to_string(),
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(js.contains("let x = 1;"));
        assert!(js.contains("let y = 2;"));
        assert!(js.contains("game.doSomething(x, y);"));
        // Check indentation (depth=0, so no indent)
        assert!(js
            .lines()
            .all(|l| l.trim().is_empty() || !l.starts_with(' ')));
    }

    #[test]
    fn test_run_block_indentation() {
        let js_text = "game.speak(\"Hello\");";
        let mut sm = SourceMapBuilder::new("test.scene", "test.scene.js");
        let mut line = 0;
        let js = compile_run(js_text, 2); // depth=2 → 4 spaces indent
        assert!(
            js.contains("    game.speak(\"Hello\");"),
            "Should have 4-space indent, got: {:?}",
            js
        );
    }

    #[test]
    fn test_run_block_empty() {
        let stmt = StoryStmt::Run {
            js: String::new(),
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert_eq!(js, "", "empty @run should produce no output");
    }

    #[test]
    fn test_run_block_preserves_empty_lines() {
        let js_text = "line1\n\nline3";
        let stmt = StoryStmt::Run {
            js: js_text.to_string(),
            span: span(1, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        let lines: Vec<&str> = js.lines().collect();
        // At depth 0, should have: "line1", "", "line3"
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[1], "");
        assert_eq!(lines[2], "line3");
    }

    #[test]
    fn test_run_block_in_choice() {
        // @run inside @choice option body (depth=2)
        let stmt = StoryStmt::Choice {
            options: vec![ChoiceOption {
                label: "Give".into(),
                body: vec![StoryStmt::Run {
                    js: "game.giveItem(\"POTION\");".to_string(),
                    span: span(2, 0),
                }],
                span: span(1, 0),
            }],
            span: span(0, 0),
        };
        let (js, _sm) = compile_stmt(&stmt);
        assert!(js.contains("game.giveItem(\"POTION\");"));
        assert!(!js.contains("todo"), "Should NOT have todo macro");
    }
}
