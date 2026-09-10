//! DSL AST interpreter — executes `.scene` storylines without a JavaScript
//! engine.
//!
//! The DSL compiler emits a deliberately trivial JavaScript subset (sequential
//! `await game.x(...)` calls, `if/else` over primitive comparisons, at most
//! `let`-bindings of awaited results — see `docs/low-end-hardware-optimization.md`
//! §四). This module interprets the *source* AST (`crate::ast::GameScene`)
//! directly, mirroring the `dotzuki-engine-script` Boa runtime protocol 1:1:
//!
//! - a statement that calls an async `game.*` function produces a
//!   [`ScriptCommand`]; the driver dispatches it and later resumes via
//!   [`Interpreter::signal_done`] with the [`CommandResult`];
//! - a statement that calls a sync query (`getFlag`, `hasItem`, `getMoney`,
//!   `setFlag`, …) resolves immediately through the [`ScriptHost`];
//! - `@t("en", "中文")` i18n is resolved from the host's current language.
//!
//! The interpreter is the **canonical** scene semantics; the Boa path is a
//! legacy/dev fallback (see §四, "AST 解释器定为规范语义").
//!
//! Supported statements: `@speaker` / `@say` / `@choice` / `@option` / `@if` /
//! `@each` / bare commands / assignments. `@run` raw-JS blocks are rejected
//! with an `Unsupported` error — the only such block in pokered
//! (VermilionGym's trash-can puzzle) is ported as a native function module in
//! `pokered-core::overworld::native_script`.

use crate::hash::HashMap;

use dotzuki_engine_script::{CommandResult, ScriptCommand};

use crate::ast::{BinOp, Expression, LocalizedText, StoryStmt};

/// Runtime value of a DSL expression. Deliberately small — the compiled JS
/// subset only produces primitives and arrays (objects only appear inside
/// `@run`, which the interpreter rejects).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Undefined,
    Bool(bool),
    Number(f64),
    Text(String),
    Array(Vec<Value>),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Undefined => "undefined",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::Text(_) => "string",
            Value::Array(_) => "array",
        }
    }

    /// JS `ToNumber` for the value kinds the DSL produces.
    fn to_number(&self) -> f64 {
        match self {
            Value::Undefined | Value::Array(_) => f64::NAN,
            Value::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Number(n) => *n,
            Value::Text(s) => {
                // JS ToNumber trims whitespace before parsing (" 42 " → 42).
                let t = s.trim();
                if t.is_empty() {
                    0.0
                } else {
                    t.parse::<f64>().unwrap_or(f64::NAN)
                }
            }
        }
    }

    /// JS `String()` for the value kinds the DSL produces.
    fn to_text(&self) -> String {
        match self {
            Value::Undefined => "undefined".to_string(),
            Value::Bool(b) => {
                if *b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Value::Number(n) => format!("{}", n),
            Value::Text(s) => s.clone(),
            Value::Array(_) => "[object Array]".to_string(),
        }
    }

    /// JS truthiness: `undefined`, `false`, `0`/`-0`/`NaN` and `""` are
    /// falsy; everything else (including empty arrays) is truthy.
    fn is_truthy(&self) -> bool {
        match self {
            Value::Undefined => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0 && !n.is_nan(),
            Value::Text(s) => !s.is_empty(),
            Value::Array(_) => true,
        }
    }

    /// JS strict equality (`===`): same-kind comparison, never across kinds.
    /// Arrays compare by identity in JS; two separately-built arrays are
    /// never `===`, so we always answer false.
    fn strict_eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Undefined, Value::Undefined) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Text(a), Value::Text(b)) => a == b,
            _ => false,
        }
    }

    /// JS `ToInt32` (wrapping, `NaN`/inf → 0) for bitwise operators.
    fn to_i32(&self) -> i32 {
        let n = self.to_number();
        if n.is_nan() || n.is_infinite() {
            0
        } else {
            (n as i64).wrapping_rem_euclid(1 << 32) as u32 as i32
        }
    }
}

/// Outcome of a `game.*` call routed through the host.
#[derive(Debug, Clone, PartialEq)]
pub enum HostCall {
    /// The call issued an **async command**: the host must dispatch this
    /// [`ScriptCommand`] to the game and later resume the interpreter via
    /// [`Interpreter::signal_done`] with the command's result.
    Command(ScriptCommand),
    /// The call completed **synchronously** (a query like `getFlag` /
    /// `hasItem`, or a sync mutation like `setFlag` that the host already
    /// applied).
    Value(Value),
}

/// Game-side service the interpreter drives. Mirrors the `game` global of
/// the Boa path: async effects become [`HostCall::Command`] (→ the driver's
/// `ScriptCommand` dispatch), sync queries answer immediately.
pub trait ScriptHost {
    /// Invoke `game.<name>(args)`.
    ///
    /// Async commands return [`HostCall::Command`] — the interpreter suspends
    /// the storyline until [`Interpreter::signal_done`] delivers the outcome.
    /// Sync queries/mutations return [`HostCall::Value`].
    fn call(&mut self, name: &str, args: &[Value]) -> Result<HostCall, String>;

    /// Current language code (e.g. `"en"`, `"zh"`), used to resolve
    /// `@t("en", "中文")` literals. Defaults to `"en"`.
    fn lang(&self) -> &str {
        "en"
    }
}

/// Interpreter execution state (mirrors `dotzuki_engine_script::EngineState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpState {
    Idle,
    Running,
    WaitingForCommand,
    Finished,
}

#[derive(Debug, Clone)]
enum Suspended {
    None,
    /// An awaited command whose result is discarded; advance the frame index
    /// on resume.
    AwaitPlain,
    /// `name = await game.x(...)`: bind the result into the frame's locals.
    AwaitAssign {
        frame: usize,
        name: String,
    },
    /// `await game.showChoice(...)`: the result picks an option body frame.
    AwaitChoice {
        frame: usize,
    },
}

/// One execution frame: a statement list with a cursor and its own `let`
/// scope (JS block scoping — reads walk outward, writes stay innermost).
#[derive(Debug, Clone)]
struct Frame {
    stmts: Vec<StoryStmt>,
    index: usize,
    locals: HashMap<String, Value>,
    /// For a frame whose `stmts` ARE an `@each` body: remaining iterations.
    each: Option<EachState>,
}

#[derive(Debug, Clone)]
struct EachState {
    item_var: String,
    source: Vec<Value>,
    next: usize,
}

/// Result of evaluating an expression: either a plain value or an async
/// command that the caller must suspend on.
enum Eval {
    Value(Value),
    Command(ScriptCommand),
}

/// Statement-level outcome of one interpreter step.
enum StepOutcome {
    /// A command was issued; the storyline is suspended until `signal_done`.
    Command(ScriptCommand),
    /// The step consumed no frames; keep running.
    Continue,
}

/// A suspended-statement interpreter for one storyline function.
///
/// `load_function` starts a fresh execution (fresh locals — same as calling
/// an exported JS async function); `tick` runs it until the next `await`
/// (or completion); `signal_done` delivers the awaited result and resumes.
pub struct Interpreter<H: ScriptHost> {
    host: H,
    state: InterpState,
    stack: Vec<Frame>,
    suspended: Suspended,
    pending_command: Option<ScriptCommand>,
}

impl<H: ScriptHost> Interpreter<H> {
    pub fn new(host: H) -> Self {
        Self {
            host,
            state: InterpState::Idle,
            stack: Vec::new(),
            suspended: Suspended::None,
            pending_command: None,
        }
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }

    pub fn state(&self) -> InterpState {
        self.state
    }

    pub fn is_idle(&self) -> bool {
        self.state == InterpState::Idle
    }

    pub fn is_waiting(&self) -> bool {
        self.state == InterpState::WaitingForCommand
    }

    /// Start executing a storyline function. Resets all execution state;
    /// plain (non-call) top-level assignments are hoisted to the function
    /// top, exactly like the JS codegen (`compile_named_block`).
    pub fn load_function(&mut self, stmts: &[StoryStmt]) {
        self.stack.clear();
        self.suspended = Suspended::None;
        self.pending_command = None;

        let is_plain_assign = |stmt: &StoryStmt| {
            matches!(stmt, StoryStmt::Assign { .. })
                && !matches!(
                    stmt,
                    StoryStmt::Assign {
                        value: Expression::Call { .. },
                        ..
                    }
                )
        };
        let mut body: Vec<StoryStmt> = Vec::with_capacity(stmts.len());
        // Hoist plain assignments (mirrors the codegen's `decls` partition).
        for stmt in stmts {
            if is_plain_assign(stmt) {
                body.push(stmt.clone());
            }
        }
        // Then the remaining statements in order (call-assigns stay put).
        for stmt in stmts {
            if !is_plain_assign(stmt) {
                body.push(stmt.clone());
            }
        }

        self.stack.push(Frame {
            stmts: body,
            index: 0,
            locals: HashMap::default(),
            each: None,
        });
        self.state = InterpState::Running;
    }

    /// Advance the interpreter: returns the next pending command, or `None`
    /// when the storyline is idle/finished. Called every frame by the driver
    /// (mirrors `ScriptEngine::tick`).
    pub fn tick(&mut self) -> Result<Option<ScriptCommand>, String> {
        match self.state {
            InterpState::WaitingForCommand => Ok(self.pending_command.clone()),
            InterpState::Idle | InterpState::Finished => Ok(None),
            InterpState::Running => self.run_until_blocked(),
        }
    }

    /// Deliver the result of the last dispatched command and resume the
    /// storyline, returning the next command (if the storyline immediately
    /// awaits another one). No-op (returns `Ok(None)`) when not waiting.
    pub fn signal_done(&mut self, result: CommandResult) -> Result<Option<ScriptCommand>, String> {
        if self.state != InterpState::WaitingForCommand {
            return Ok(None);
        }
        self.pending_command = None;
        let value = match result {
            CommandResult::Void => Value::Undefined,
            CommandResult::Bool(b) => Value::Bool(b),
            CommandResult::Number(n) => Value::Number(n),
            CommandResult::Text(s) => Value::Text(s),
        };
        let suspended = core::mem::replace(&mut self.suspended, Suspended::None);
        if let Err(e) = self.resume(suspended, &value) {
            self.state = InterpState::Finished;
            return Err(e);
        }
        self.state = InterpState::Running;
        self.run_until_blocked()
    }

    // ── execution core ────────────────────────────────────────────────────

    fn resume(&mut self, suspended: Suspended, value: &Value) -> Result<(), String> {
        match suspended {
            Suspended::None => Ok(()),
            Suspended::AwaitPlain => {
                let frame = self.stack.last_mut().ok_or("resume: no active frame")?;
                frame.index += 1;
                Ok(())
            }
            Suspended::AwaitAssign { frame, name } => {
                let f = self.stack.get_mut(frame).ok_or("resume: frame vanished")?;
                f.locals.insert(name, value.clone());
                f.index += 1;
                Ok(())
            }
            Suspended::AwaitChoice { frame } => {
                let options = {
                    let f = self.stack.get_mut(frame).ok_or("resume: frame vanished")?;
                    f.index += 1;
                    match f.stmts.get(f.index - 1) {
                        Some(StoryStmt::Choice { options, .. }) => options.clone(),
                        _ => Vec::new(),
                    }
                };
                if options.is_empty() {
                    return Ok(());
                }
                // The codegen emits `if (choice === 0) … else if … else { last }`:
                // any out-of-range result falls through to the LAST option.
                let idx = match value {
                    Value::Number(n) if *n >= 0.0 => *n as usize,
                    _ => options.len() - 1,
                };
                let chosen = idx.min(options.len() - 1);
                self.stack.push(Frame {
                    stmts: options[chosen].body.clone(),
                    index: 0,
                    locals: HashMap::default(),
                    each: None,
                });
                Ok(())
            }
        }
    }

    /// Run statement-by-statement until a command is emitted or the story
    /// completes. On a command: state → WaitingForCommand, returns it.
    fn run_until_blocked(&mut self) -> Result<Option<ScriptCommand>, String> {
        loop {
            // Frame completion: iterate the next @each element, pop, or stop.
            match self.stack.last() {
                None => {
                    self.state = InterpState::Idle;
                    self.suspended = Suspended::None;
                    self.pending_command = None;
                    return Ok(None);
                }
                Some(top) if top.index < top.stmts.len() => {}
                Some(top) if top.each.as_ref().is_some_and(|e| e.next < e.source.len()) => {
                    // Next loop iteration: re-run the body with the item var
                    // rebound in a fresh scope.
                    let (item_var, value) = {
                        let top = self.stack.last().unwrap();
                        let each = top.each.as_ref().unwrap();
                        (each.item_var.clone(), each.source[each.next].clone())
                    };
                    let frame = self.stack.last_mut().unwrap();
                    frame.locals.clear();
                    frame.locals.insert(item_var, value);
                    frame.index = 0;
                    frame.each.as_mut().unwrap().next += 1;
                    continue;
                }
                Some(_) => {
                    self.stack.pop();
                    continue;
                }
            }

            let stmt = {
                let top = self.stack.last().unwrap();
                top.stmts[top.index].clone()
            };
            match self.step(stmt) {
                Ok(StepOutcome::Command(cmd)) => {
                    self.pending_command = Some(cmd.clone());
                    self.state = InterpState::WaitingForCommand;
                    return Ok(Some(cmd));
                }
                Ok(StepOutcome::Continue) => {}
                Err(e) => {
                    self.state = InterpState::Finished;
                    return Err(e);
                }
            }
        }
    }

    fn step(&mut self, stmt: StoryStmt) -> Result<StepOutcome, String> {
        match stmt {
            StoryStmt::Speaker { name, texts, .. } | StoryStmt::Say { name, texts, .. } => {
                let text = self.speaker_text(&name, &texts)?;
                self.suspended = Suspended::AwaitPlain;
                Ok(StepOutcome::Command(ScriptCommand::ShowText { text }))
            }
            StoryStmt::Choice { options, .. } => {
                let labels: Vec<String> = options
                    .iter()
                    .map(|o| self.localized_text(&o.label))
                    .collect();
                self.suspended = Suspended::AwaitChoice {
                    frame: self.stack.len() - 1,
                };
                Ok(StepOutcome::Command(ScriptCommand::ShowChoice {
                    options: labels,
                }))
            }
            StoryStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond = match self.eval(&condition)? {
                    Eval::Value(v) => v,
                    Eval::Command(cmd) => {
                        return Err(format!(
                            "async command in @if condition ({:?}); only sync queries (getFlag, hasItem, …) are allowed in conditions",
                            core::mem::discriminant(&cmd)
                        ))
                    }
                };
                self.advance_top_index();
                if cond.is_truthy() {
                    if !then_branch.is_empty() {
                        self.push_block(then_branch);
                    }
                } else if !else_branch.is_empty() {
                    self.push_block(else_branch);
                }
                Ok(StepOutcome::Continue)
            }
            StoryStmt::Each {
                item_var,
                source,
                body,
                ..
            } => {
                let arr = match self.eval(&source)? {
                    Eval::Value(Value::Array(items)) => items,
                    Eval::Value(other) => {
                        return Err(format!(
                            "@each source must be an array, got {}",
                            other.type_name()
                        ))
                    }
                    Eval::Command(cmd) => {
                        return Err(format!(
                            "async command in @each source ({:?}); only sync queries are allowed",
                            core::mem::discriminant(&cmd)
                        ))
                    }
                };
                self.advance_top_index();
                if arr.is_empty() {
                    return Ok(StepOutcome::Continue);
                }
                let first = arr[0].clone();
                let mut locals = HashMap::default();
                locals.insert(item_var.clone(), first);
                self.stack.push(Frame {
                    stmts: body.clone(),
                    index: 0,
                    locals,
                    each: Some(EachState {
                        item_var: item_var.clone(),
                        source: arr,
                        next: 1,
                    }),
                });
                Ok(StepOutcome::Continue)
            }
            StoryStmt::Run { js, .. } => Err(format!(
                "unsupported statement: @run raw-JS blocks cannot run in the native interpreter \
                 (this scene has `@run {{ … }}` starting {:?}); port it to DSL or a native function",
                js.trim().chars().take(40).collect::<String>()
            )),
            StoryStmt::Assign { name, value, .. } => match self.eval(&value)? {
                Eval::Command(cmd) => {
                    self.suspended = Suspended::AwaitAssign {
                        frame: self.stack.len() - 1,
                        name,
                    };
                    Ok(StepOutcome::Command(cmd))
                }
                Eval::Value(v) => {
                    self.bind_top(name, v);
                    self.advance_top_index();
                    Ok(StepOutcome::Continue)
                }
            },
            StoryStmt::Command { name, args, .. } => {
                let mut values = Vec::with_capacity(args.len());
                for arg in &args {
                    match self.eval(arg)? {
                        Eval::Value(v) => values.push(v),
                        Eval::Command(cmd) => {
                            return Err(format!(
                                "async command in call arguments ({:?}); only sync queries are allowed in expressions",
                                core::mem::discriminant(&cmd)
                            ))
                        }
                    }
                }
                match self.host.call(&name, &values)? {
                    HostCall::Command(cmd) => {
                        self.suspended = Suspended::AwaitPlain;
                        Ok(StepOutcome::Command(cmd))
                    }
                    HostCall::Value(_) => {
                        self.advance_top_index();
                        Ok(StepOutcome::Continue)
                    }
                }
            }
        }
    }

    /// Resolve `@speaker`/`@say` to the exact text the JS codegen produces:
    /// a non-empty string-literal name is prefixed `"Name: "`, an empty name
    /// is the narrator form (verbatim), per-language lines joined with `\n`,
    /// localized lines resolved through `game.t` (host lang).
    fn speaker_text(
        &mut self,
        name: &Expression,
        texts: &[LocalizedText],
    ) -> Result<String, String> {
        let name_str = match name {
            Expression::StringLit(s) => Some(s.clone()),
            _ => None,
        };
        let body = |lang: &str| -> String {
            texts
                .iter()
                .map(|t| match t {
                    LocalizedText::Plain(s) => s.clone(),
                    LocalizedText::Localized(pairs) => pairs
                        .iter()
                        .find(|(l, _)| *l == lang)
                        .map(|(_, s)| s.clone())
                        .unwrap_or_else(|| t.default_text().to_string()),
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let lang = self.host.lang().to_string();
        let rendered = body(&lang);
        Ok(match name_str {
            Some(n) if n.is_empty() => rendered,
            Some(n) => format!("{}: {}", n, rendered),
            None => {
                let rendered_name = match self.eval(name)? {
                    Eval::Value(v) => v,
                    Eval::Command(cmd) => {
                        return Err(format!(
                            "async command in speaker name ({:?})",
                            core::mem::discriminant(&cmd)
                        ))
                    }
                };
                format!("{}: {}", rendered_name.to_text(), rendered)
            }
        })
    }

    fn localized_text(&mut self, text: &LocalizedText) -> String {
        match text {
            LocalizedText::Plain(s) => s.clone(),
            LocalizedText::Localized(pairs) => {
                let lang = self.host.lang().to_string();
                pairs
                    .iter()
                    .find(|(l, _)| *l == lang)
                    .map(|(_, s)| s.clone())
                    .unwrap_or_else(|| text.default_text().to_string())
            }
        }
    }

    /// Single-pass expression evaluation. Async command calls surface as
    /// [`Eval::Command`] so the caller can suspend (JS: the generated code
    /// `await`s calls in assignments and bare statements).
    fn eval(&mut self, expr: &Expression) -> Result<Eval, String> {
        match expr {
            Expression::StringLit(s) => Ok(Eval::Value(Value::Text(s.clone()))),
            Expression::Localized(pairs) => {
                let lang = self.host.lang().to_string();
                let text = pairs
                    .iter()
                    .find(|(l, _)| *l == lang)
                    .map(|(_, s)| s.clone())
                    .unwrap_or_else(|| {
                        pairs
                            .iter()
                            .find(|(l, _)| l == "en")
                            .or_else(|| pairs.first())
                            .map(|(_, s)| s.clone())
                            .unwrap_or_default()
                    });
                Ok(Eval::Value(Value::Text(text)))
            }
            Expression::NumberLit(n) => Ok(Eval::Value(Value::Number(*n))),
            Expression::BoolLit(b) => Ok(Eval::Value(Value::Bool(*b))),
            Expression::Variable(name) => self
                .lookup(name)
                .map(Eval::Value)
                .ok_or_else(|| format!("variable '{}' is not defined", name)),
            Expression::ArrayLit(elements) => {
                let mut items = Vec::with_capacity(elements.len());
                for e in elements {
                    match self.eval(e)? {
                        Eval::Value(v) => items.push(v),
                        Eval::Command(cmd) => {
                            return Err(format!(
                                "async command in array literal ({:?}); only sync queries are allowed in expressions",
                                core::mem::discriminant(&cmd)
                            ))
                        }
                    }
                }
                Ok(Eval::Value(Value::Array(items)))
            }
            Expression::ObjectLit(_) => Err(
                "unsupported expression: object literals only appear in @run blocks, \
                 which the native interpreter rejects"
                    .to_string(),
            ),
            Expression::Call { callee, args } => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    match self.eval(arg)? {
                        Eval::Value(v) => values.push(v),
                        Eval::Command(cmd) => {
                            return Err(format!(
                                "async command inside call arguments ({:?}); only sync queries are allowed in expressions",
                                core::mem::discriminant(&cmd)
                            ))
                        }
                    }
                }
                match self.host.call(callee, &values)? {
                    HostCall::Command(cmd) => Ok(Eval::Command(cmd)),
                    HostCall::Value(v) => Ok(Eval::Value(v)),
                }
            }
            Expression::UnaryOp { op, operand } => {
                let v = match self.eval(operand)? {
                    Eval::Value(v) => v,
                    Eval::Command(cmd) => {
                        return Err(format!(
                            "async command inside unary expression ({:?}); only sync queries are allowed",
                            core::mem::discriminant(&cmd)
                        ))
                    }
                };
                Ok(Eval::Value(match op {
                    crate::ast::UnaryOp::Not => Value::Bool(!v.is_truthy()),
                    crate::ast::UnaryOp::Neg => Value::Number(-v.to_number()),
                }))
            }
            Expression::BinaryOp { op, left, right } => {
                let l = match self.eval(left)? {
                    Eval::Value(v) => v,
                    Eval::Command(cmd) => {
                        return Err(format!(
                            "async command inside binary expression ({:?}); only sync queries are allowed",
                            core::mem::discriminant(&cmd)
                        ))
                    }
                };
                // Short-circuit: JS && / || return one of the operands.
                match op {
                    BinOp::And => {
                        if !l.is_truthy() {
                            return Ok(Eval::Value(l));
                        }
                        return self.eval(right);
                    }
                    BinOp::Or => {
                        if l.is_truthy() {
                            return Ok(Eval::Value(l));
                        }
                        return self.eval(right);
                    }
                    _ => {}
                }
                let r = match self.eval(right)? {
                    Eval::Value(v) => v,
                    Eval::Command(cmd) => {
                        return Err(format!(
                            "async command inside binary expression ({:?}); only sync queries are allowed",
                            core::mem::discriminant(&cmd)
                        ))
                    }
                };
                Ok(Eval::Value(eval_binop(*op, &l, &r)))
            }
            Expression::TernaryOp {
                condition,
                then_expr,
                else_expr,
            } => {
                let c = match self.eval(condition)? {
                    Eval::Value(v) => v,
                    Eval::Command(cmd) => {
                        return Err(format!(
                            "async command inside ternary condition ({:?}); only sync queries are allowed",
                            core::mem::discriminant(&cmd)
                        ))
                    }
                };
                if c.is_truthy() {
                    self.eval(then_expr)
                } else {
                    self.eval(else_expr)
                }
            }
        }
    }

    fn lookup(&self, name: &str) -> Option<Value> {
        self.stack
            .iter()
            .rev()
            .find_map(|f| f.locals.get(name).cloned())
    }

    fn bind_top(&mut self, name: String, value: Value) {
        if let Some(frame) = self.stack.last_mut() {
            frame.locals.insert(name, value);
        }
    }

    fn advance_top_index(&mut self) {
        if let Some(frame) = self.stack.last_mut() {
            frame.index += 1;
        }
    }

    fn push_block(&mut self, stmts: Vec<StoryStmt>) {
        self.stack.push(Frame {
            stmts,
            index: 0,
            locals: HashMap::default(),
            each: None,
        });
    }
}

fn eval_binop(op: BinOp, l: &Value, r: &Value) -> Value {
    match op {
        BinOp::Add => {
            if matches!(l, Value::Text(_)) || matches!(r, Value::Text(_)) {
                Value::Text(format!("{}{}", l.to_text(), r.to_text()))
            } else {
                Value::Number(l.to_number() + r.to_number())
            }
        }
        BinOp::Sub => Value::Number(l.to_number() - r.to_number()),
        BinOp::Mul => Value::Number(l.to_number() * r.to_number()),
        BinOp::Div => Value::Number(l.to_number() / r.to_number()),
        BinOp::Eq => Value::Bool(l.strict_eq(r)),
        BinOp::Neq => Value::Bool(!l.strict_eq(r)),
        BinOp::Gt => Value::Bool(l.to_number() > r.to_number()),
        BinOp::Lt => Value::Bool(l.to_number() < r.to_number()),
        BinOp::Gte => Value::Bool(l.to_number() >= r.to_number()),
        BinOp::Lte => Value::Bool(l.to_number() <= r.to_number()),
        // Handled with short-circuiting in `eval`; unreachable here.
        BinOp::And | BinOp::Or => Value::Bool(false),
        BinOp::BitOr => Value::Number((l.to_i32() | r.to_i32()) as f64),
        BinOp::BitAnd => Value::Number((l.to_i32() & r.to_i32()) as f64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeHost {
        lang: String,
        calls: Vec<(String, Vec<Value>)>,
        commands: HashMap<String, Vec<ScriptCommand>>,
        sync: HashMap<String, Value>,
    }

    impl FakeHost {
        fn new() -> Self {
            Self {
                lang: "en".into(),
                calls: Vec::new(),
                commands: HashMap::default(),
                sync: HashMap::default(),
            }
        }
        fn with_sync(name: &str, v: Value) -> Self {
            let mut s = Self::new();
            s.sync.insert(name.to_string(), v);
            s
        }
        fn enqueue(&mut self, name: &str, cmd: ScriptCommand) {
            self.commands.entry(name.to_string()).or_default().push(cmd);
        }
    }

    impl ScriptHost for FakeHost {
        fn call(&mut self, name: &str, args: &[Value]) -> Result<HostCall, String> {
            self.calls.push((name.to_string(), args.to_vec()));
            if let Some(v) = self.sync.get(name) {
                return Ok(HostCall::Value(v.clone()));
            }
            if let Some(cmds) = self.commands.get_mut(name) {
                if !cmds.is_empty() {
                    return Ok(HostCall::Command(cmds.remove(0)));
                }
            }
            Err(format!("unknown fn {}", name))
        }
        fn lang(&self) -> &str {
            &self.lang
        }
    }

    fn span() -> crate::ast::SourceSpan {
        crate::ast::SourceSpan::point("test.scene", 1, 0)
    }

    fn text_stmt(s: &str) -> StoryStmt {
        StoryStmt::Speaker {
            name: Expression::StringLit("".into()),
            texts: vec![s.into()],
            span: span(),
        }
    }

    fn cmd_stmt(name: &str, args: Vec<Expression>) -> StoryStmt {
        StoryStmt::Command {
            name: name.into(),
            args,
            span: span(),
        }
    }

    #[test]
    fn sequential_commands_flow() {
        let mut host = FakeHost::new();
        host.enqueue("showText", ScriptCommand::ShowText { text: "hi".into() });
        host.enqueue("delay", ScriptCommand::Delay { frames: 5 });
        let mut interp = Interpreter::new(host);
        interp.load_function(&[
            text_stmt("hi"),
            cmd_stmt("delay", vec![Expression::NumberLit(5.0)]),
        ]);
        let cmd = interp.tick().unwrap().expect("first command");
        assert_eq!(cmd, ScriptCommand::ShowText { text: "hi".into() });
        assert!(interp.is_waiting());
        // tick while waiting returns the same command (mirrors Boa).
        assert_eq!(interp.tick().unwrap(), Some(cmd.clone()));
        let next = interp
            .signal_done(CommandResult::Void)
            .unwrap()
            .expect("second command");
        assert_eq!(next, ScriptCommand::Delay { frames: 5 });
        assert!(interp.is_waiting());
        assert!(interp.signal_done(CommandResult::Void).unwrap().is_none());
        assert!(interp.is_idle());
        // signal_done when idle is a no-op.
        assert!(interp.signal_done(CommandResult::Void).unwrap().is_none());
    }

    #[test]
    fn if_condition_on_sync_query() {
        let mut host = FakeHost::with_sync("getFlag", Value::Bool(true));
        host.enqueue("showText", ScriptCommand::ShowText { text: "A".into() });
        let mut interp = Interpreter::new(host);
        interp.load_function(&[StoryStmt::If {
            condition: Expression::Call {
                callee: "getFlag".into(),
                args: vec![Expression::StringLit("X".into())],
            },
            then_branch: vec![text_stmt("A")],
            else_branch: vec![text_stmt("B")],
            span: span(),
        }]);
        let cmd = interp.tick().unwrap().expect("then branch text");
        assert_eq!(cmd, ScriptCommand::ShowText { text: "A".into() });
    }

    #[test]
    fn if_else_taken_on_false_query() {
        let mut host = FakeHost::with_sync("hasItem", Value::Bool(false));
        host.enqueue("showText", ScriptCommand::ShowText { text: "no".into() });
        let mut interp = Interpreter::new(host);
        interp.load_function(&[StoryStmt::If {
            condition: Expression::Call {
                callee: "hasItem".into(),
                args: vec![Expression::StringLit("POTION".into())],
            },
            then_branch: vec![text_stmt("yes")],
            else_branch: vec![text_stmt("no")],
            span: span(),
        }]);
        let cmd = interp.tick().unwrap().expect("else branch text");
        assert_eq!(cmd, ScriptCommand::ShowText { text: "no".into() });
    }

    #[test]
    fn choice_routes_option_body() {
        let mut host = FakeHost::new();
        host.enqueue(
            "showChoice",
            ScriptCommand::ShowChoice {
                options: vec!["A".into(), "B".into()],
            },
        );
        host.enqueue(
            "showText",
            ScriptCommand::ShowText {
                text: "picked B".into(),
            },
        );
        let mut interp = Interpreter::new(host);
        interp.load_function(&[StoryStmt::Choice {
            options: vec![
                crate::ast::ChoiceOption {
                    label: "A".into(),
                    body: vec![text_stmt("picked A")],
                    span: span(),
                },
                crate::ast::ChoiceOption {
                    label: "B".into(),
                    body: vec![text_stmt("picked B")],
                    span: span(),
                },
            ],
            span: span(),
        }]);
        let cmd = interp.tick().unwrap().expect("choice command");
        assert_eq!(
            cmd,
            ScriptCommand::ShowChoice {
                options: vec!["A".into(), "B".into()]
            }
        );
        let next = interp
            .signal_done(CommandResult::Number(1.0))
            .unwrap()
            .expect("option body");
        assert_eq!(
            next,
            ScriptCommand::ShowText {
                text: "picked B".into()
            }
        );
        assert!(interp.signal_done(CommandResult::Void).unwrap().is_none());
    }

    #[test]
    fn out_of_range_choice_hits_last_option() {
        let mut host = FakeHost::new();
        host.enqueue(
            "showChoice",
            ScriptCommand::ShowChoice {
                options: vec!["A".into(), "B".into(), "C".into()],
            },
        );
        host.enqueue(
            "showText",
            ScriptCommand::ShowText {
                text: "body2".into(),
            },
        );
        let mut interp = Interpreter::new(host);
        interp.load_function(&[StoryStmt::Choice {
            options: (0..3)
                .map(|i| crate::ast::ChoiceOption {
                    label: format!("{}", i).into(),
                    body: vec![text_stmt(&format!("body{}", i))],
                    span: span(),
                })
                .collect(),
            span: span(),
        }]);
        interp.tick().unwrap();
        let next = interp
            .signal_done(CommandResult::Number(7.0))
            .unwrap()
            .expect("fallback");
        assert_eq!(
            next,
            ScriptCommand::ShowText {
                text: "body2".into()
            }
        );
    }

    #[test]
    fn assignment_binds_awaited_result() {
        let mut host = FakeHost::new();
        host.enqueue(
            "startBattle",
            ScriptCommand::StartBattle {
                trainer_id: "RIVAL".into(),
            },
        );
        host.enqueue(
            "showText",
            ScriptCommand::ShowText {
                text: "result: win".into(),
            },
        );
        let mut interp = Interpreter::new(host);
        interp.load_function(&[
            StoryStmt::Assign {
                name: "r".into(),
                value: Expression::Call {
                    callee: "startBattle".into(),
                    args: vec![Expression::StringLit("RIVAL".into())],
                },
                span: span(),
            },
            StoryStmt::If {
                condition: Expression::BinaryOp {
                    op: BinOp::Eq,
                    left: Box::new(Expression::Variable("r".into())),
                    right: Box::new(Expression::StringLit("win".into())),
                },
                then_branch: vec![text_stmt("result: win")],
                else_branch: vec![],
                span: span(),
            },
        ]);
        let cmd = interp.tick().unwrap().expect("battle command");
        assert_eq!(
            cmd,
            ScriptCommand::StartBattle {
                trainer_id: "RIVAL".into()
            }
        );
        let next = interp
            .signal_done(CommandResult::Text("win".into()))
            .unwrap()
            .expect("branch");
        assert_eq!(
            next,
            ScriptCommand::ShowText {
                text: "result: win".into()
            }
        );
    }

    #[test]
    fn sync_assignment_binds_immediately() {
        let mut host = FakeHost::with_sync("getMoney", Value::Number(5000.0));
        host.enqueue(
            "showText",
            ScriptCommand::ShowText {
                text: "rich".into(),
            },
        );
        let mut interp = Interpreter::new(host);
        interp.load_function(&[
            StoryStmt::Assign {
                name: "m".into(),
                value: Expression::Call {
                    callee: "getMoney".into(),
                    args: vec![],
                },
                span: span(),
            },
            StoryStmt::If {
                condition: Expression::BinaryOp {
                    op: BinOp::Gte,
                    left: Box::new(Expression::Variable("m".into())),
                    right: Box::new(Expression::NumberLit(1000.0)),
                },
                then_branch: vec![text_stmt("rich")],
                else_branch: vec![],
                span: span(),
            },
        ]);
        let cmd = interp.tick().unwrap().expect("no suspension");
        assert_eq!(
            cmd,
            ScriptCommand::ShowText {
                text: "rich".into()
            }
        );
    }

    #[test]
    fn hoisted_plain_assignments_visible_in_branches() {
        let mut host = FakeHost::new();
        host.enqueue("placeholder", ScriptCommand::Delay { frames: 0 });
        host.enqueue("showText", ScriptCommand::ShowText { text: "500".into() });
        let mut interp = Interpreter::new(host);
        interp.load_function(&[
            cmd_stmt("placeholder", vec![Expression::NumberLit(1.0)]),
            StoryStmt::Assign {
                name: "gold".into(),
                value: Expression::NumberLit(500.0),
                span: span(),
            },
            StoryStmt::If {
                condition: Expression::Variable("gold".into()),
                then_branch: vec![text_stmt("500")],
                else_branch: vec![],
                span: span(),
            },
        ]);
        // Hoisting puts `gold = 500` first; the placeholder command runs
        // next, then the branch text.
        let cmd = interp.tick().unwrap().expect("placeholder");
        assert_eq!(cmd, ScriptCommand::Delay { frames: 0 });
        let next = interp
            .signal_done(CommandResult::Void)
            .unwrap()
            .expect("branch");
        assert_eq!(next, ScriptCommand::ShowText { text: "500".into() });
    }

    #[test]
    fn speaker_name_prefix_and_localized() {
        let mut host = FakeHost::new();
        host.lang = "zh".into();
        host.enqueue(
            "showText",
            ScriptCommand::ShowText {
                text: String::new(),
            },
        );
        let mut interp = Interpreter::new(host);
        interp.load_function(&[StoryStmt::Speaker {
            name: Expression::StringLit("PROF".into()),
            texts: vec![crate::ast::LocalizedText::Localized(vec![
                ("en".into(), "Hi".into()),
                ("zh".into(), "你好".into()),
            ])],
            span: span(),
        }]);
        let cmd = interp.tick().unwrap().expect("speaker");
        assert_eq!(
            cmd,
            ScriptCommand::ShowText {
                text: "PROF: 你好".into()
            }
        );
    }

    #[test]
    fn each_iterates_and_rebinds() {
        let host = FakeHost::new();
        let mut interp = Interpreter::new(host);
        interp.load_function(&[StoryStmt::Each {
            item_var: "item".into(),
            source: Expression::ArrayLit(vec![
                Expression::StringLit("a".into()),
                Expression::StringLit("b".into()),
            ]),
            body: vec![StoryStmt::Speaker {
                name: Expression::Variable("item".into()),
                texts: vec!["x".into()],
                span: span(),
            }],
            span: span(),
        }]);
        let c1 = interp.tick().unwrap().expect("first iteration");
        assert_eq!(
            c1,
            ScriptCommand::ShowText {
                text: "a: x".into()
            }
        );
        let c2 = interp
            .signal_done(CommandResult::Void)
            .unwrap()
            .expect("second iteration");
        assert_eq!(
            c2,
            ScriptCommand::ShowText {
                text: "b: x".into()
            }
        );
        assert!(interp.signal_done(CommandResult::Void).unwrap().is_none());
        assert!(interp.is_idle());
    }

    #[test]
    fn each_with_statements_after_loop() {
        let host = FakeHost::new();
        let mut interp = Interpreter::new(host);
        interp.load_function(&[
            StoryStmt::Each {
                item_var: "item".into(),
                source: Expression::ArrayLit(vec![Expression::StringLit("a".into())]),
                body: vec![text_stmt("in loop")],
                span: span(),
            },
            text_stmt("after"),
        ]);
        let c1 = interp.tick().unwrap().expect("loop body");
        assert_eq!(
            c1,
            ScriptCommand::ShowText {
                text: "in loop".into()
            }
        );
        let c2 = interp
            .signal_done(CommandResult::Void)
            .unwrap()
            .expect("after loop");
        assert_eq!(
            c2,
            ScriptCommand::ShowText {
                text: "after".into()
            }
        );
    }

    #[test]
    fn run_block_is_rejected() {
        let mut host = FakeHost::new();
        let mut interp = Interpreter::new(host);
        interp.load_function(&[StoryStmt::Run {
            js: "let x = 1;".into(),
            span: span(),
        }]);
        let err = interp.tick().unwrap_err();
        assert!(err.contains("unsupported"), "got: {}", err);
        assert_eq!(interp.state(), InterpState::Finished);
    }

    #[test]
    fn js_truthiness_zero_is_falsy() {
        let mut host = FakeHost::with_sync("getMoney", Value::Number(0.0));
        host.enqueue(
            "showText",
            ScriptCommand::ShowText {
                text: "poor".into(),
            },
        );
        let mut interp = Interpreter::new(host);
        interp.load_function(&[StoryStmt::If {
            condition: Expression::Call {
                callee: "getMoney".into(),
                args: vec![],
            },
            then_branch: vec![text_stmt("rich")],
            else_branch: vec![text_stmt("poor")],
            span: span(),
        }]);
        let cmd = interp.tick().unwrap().expect("else branch");
        assert_eq!(
            cmd,
            ScriptCommand::ShowText {
                text: "poor".into()
            }
        );
    }

    #[test]
    fn strict_equality_across_kinds_is_false() {
        assert!(!Value::Number(1.0).strict_eq(&Value::Bool(true)));
        assert!(Value::Number(1.0).strict_eq(&Value::Number(1.0)));
        assert!(!Value::Text("1".into()).strict_eq(&Value::Number(1.0)));
    }

    #[test]
    fn short_circuit_or_returns_operand() {
        let mut host = FakeHost::with_sync("getMoney", Value::Number(0.0));
        let mut interp = Interpreter::new(host);
        interp.load_function(&[StoryStmt::Assign {
            name: "m".into(),
            value: Expression::BinaryOp {
                op: BinOp::Or,
                left: Box::new(Expression::Call {
                    callee: "getMoney".into(),
                    args: vec![],
                }),
                right: Box::new(Expression::NumberLit(100.0)),
            },
            span: span(),
        }]);
        // The assignment is a plain (non-call) assign → hoisted; the sync
        // call is evaluated during execution. Verify no error and idle.
        assert!(interp.tick().unwrap().is_none());
        assert!(interp.is_idle());
        let calls: Vec<String> = interp.host().calls.iter().map(|(n, _)| n.clone()).collect();
        assert_eq!(calls, vec!["getMoney"]);
    }
}
