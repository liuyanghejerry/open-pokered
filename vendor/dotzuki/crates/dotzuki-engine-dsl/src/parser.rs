use crate::ast::*;
use crate::hash::{HashMap, HashSet};
use crate::lexer::{SpannedToken, Token};

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnexpectedToken {
        expected: Vec<String>,
        found: Token,
        span: SourceSpan,
    },
    MissingBlock {
        keyword: String,
        span: SourceSpan,
    },
    InvalidComponentType {
        found: String,
        valid: Vec<String>,
        span: SourceSpan,
    },
    UnclosedBlock {
        expected_close: String,
        span: SourceSpan,
    },
    IndentationError {
        msg: String,
        span: SourceSpan,
    },
    UnterminatedString {
        span: SourceSpan,
    },
    UnexpectedEof {
        expected: Vec<String>,
        span: SourceSpan,
    },
    /// A required prop of a declared custom component is missing at a use site.
    MissingRequiredProp {
        component: String,
        prop: String,
        span: SourceSpan,
    },
    /// A prop value's kind does not match the component declaration.
    PropTypeMismatch {
        component: String,
        prop: String,
        expected: String,
        span: SourceSpan,
    },
    /// A prop not present in the component declaration (and not a standard
    /// layout prop) was passed to a declared custom component.
    UnknownProp {
        component: String,
        prop: String,
        valid: Vec<String>,
        span: SourceSpan,
    },
    /// A `component` declaration with the same name appears twice.
    DuplicateComponentDecl {
        name: String,
        span: SourceSpan,
    },
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnexpectedToken {
                expected, found, ..
            } => write!(
                f,
                "expected one of [{}], found {:?}",
                expected.join(", "),
                found
            ),
            Self::MissingBlock { keyword, .. } => write!(f, "missing '{{' after {}", keyword),
            Self::InvalidComponentType { found, valid, .. } => write!(
                f,
                "'{}' is not a valid component type (valid: {})",
                found,
                valid.join(", ")
            ),
            Self::UnclosedBlock { expected_close, .. } => {
                write!(f, "unclosed block, expected {}", expected_close)
            }
            Self::IndentationError { msg, .. } => write!(f, "indentation error: {}", msg),
            Self::UnterminatedString { .. } => write!(f, "unterminated string literal"),
            Self::UnexpectedEof { expected, .. } => write!(
                f,
                "unexpected end of input, expected {}",
                expected.join(", ")
            ),
            Self::MissingRequiredProp {
                component, prop, ..
            } => write!(f, "component '{}' requires prop '{}'", component, prop),
            Self::PropTypeMismatch {
                component,
                prop,
                expected,
                ..
            } => write!(
                f,
                "prop '{}' of component '{}' expects a {} value",
                prop, component, expected
            ),
            Self::UnknownProp {
                component,
                prop,
                valid,
                ..
            } => write!(
                f,
                "component '{}' has no prop '{}' (declared: {})",
                component,
                prop,
                valid.join(", ")
            ),
            Self::DuplicateComponentDecl { name, .. } => {
                write!(f, "component '{}' is declared twice", name)
            }
        }
    }
}

impl core::error::Error for ParseError {}

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticError {
    UndefinedVariable {
        name: String,
        defined_vars: Vec<String>,
        span: SourceSpan,
    },
    CircularStyleInheritance {
        chain: Vec<String>,
        span: SourceSpan,
    },
    MissingStyleParent {
        parent: String,
        span: SourceSpan,
    },
    EmptyChoice {
        span: SourceSpan,
    },
    DuplicateName {
        name: String,
        kind: String,
        span: SourceSpan,
    },
}

impl core::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UndefinedVariable { name, .. } => write!(f, "undefined variable '{}'", name),
            Self::CircularStyleInheritance { chain, .. } => {
                write!(f, "circular style inheritance: {}", chain.join(" -> "))
            }
            Self::MissingStyleParent { parent, .. } => {
                write!(f, "style parent '{}' not found", parent)
            }
            Self::EmptyChoice { .. } => write!(f, "@choice must have at least one @option"),
            Self::DuplicateName { name, kind, .. } => {
                write!(f, "duplicate {} name '{}'", kind, name)
            }
        }
    }
}

impl core::error::Error for SemanticError {}

const VALID_COMPONENT_TYPES: &[&str] = &[
    "panel",
    "container",
    "text",
    "button",
    "list",
    "image",
    "input",
    "dropdown",
    "tile",
    "divider",
    "flex_list",
    "cursor",
    "bracket",
    "pixel_rect",
];

pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
    errors: Vec<ParseError>,
    current_scope: Vec<String>,
    /// Kept for future error-context reporting; currently unused.
    _source: String,
    /// Custom component schemas usable as component types in `screen` blocks
    /// — from `component` declarations in this file plus any pre-registered
    /// set (e.g. a `components.gui` prelude) via [`Parser::with_components`].
    component_decls: HashMap<String, ComponentDecl>,
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>, source: &str) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: Vec::new(),
            current_scope: Vec::new(),
            _source: source.to_string(),
            component_decls: HashMap::default(),
        }
    }

    /// Pre-register component declarations (a shared prelude) so screens in
    /// this file may use them without declaring them locally.
    pub fn with_components(mut self, decls: &[ComponentDecl]) -> Self {
        for d in decls {
            self.component_decls.insert(d.name.clone(), d.clone());
        }
        self
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|st| &st.token)
    }
    fn peek_st(&self) -> Option<&SpannedToken> {
        self.tokens.get(self.pos)
    }
    fn peek_n(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n).map(|st| &st.token)
    }

    fn current_span(&self) -> SourceSpan {
        self.peek_st()
            .map(|st| st.span.clone())
            .unwrap_or_else(|| SourceSpan::point("", 0, 0))
    }

    fn advance(&mut self) -> Option<SpannedToken> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn expect_peek(&mut self, expected: &Token) -> Result<(), ParseError> {
        match self.peek() {
            Some(t) if t == expected => {
                self.advance();
                Ok(())
            }
            Some(found) => Err(ParseError::UnexpectedToken {
                expected: vec![format!("{:?}", expected)],
                found: found.clone(),
                span: self.current_span(),
            }),
            None => Err(ParseError::UnexpectedEof {
                expected: vec![format!("{:?}", expected)],
                span: self.current_span(),
            }),
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.advance() {
            Some(SpannedToken {
                token: Token::Identifier(name),
                ..
            }) => Ok(name),
            Some(st) => Err(ParseError::UnexpectedToken {
                expected: vec!["identifier".into()],
                found: st.token,
                span: st.span,
            }),
            None => Err(ParseError::UnexpectedEof {
                expected: vec!["identifier".into()],
                span: self.current_span(),
            }),
        }
    }

    fn expect_string(&mut self) -> Result<String, ParseError> {
        match self.advance() {
            Some(SpannedToken {
                token: Token::StringLit(s),
                ..
            }) => Ok(s),
            Some(st) => Err(ParseError::UnexpectedToken {
                expected: vec!["string".into()],
                found: st.token,
                span: st.span,
            }),
            None => Err(ParseError::UnexpectedEof {
                expected: vec!["string".into()],
                span: self.current_span(),
            }),
        }
    }

    fn skip_noise(&mut self) {
        while let Some(tok) = self.peek() {
            match tok {
                Token::Newline | Token::Comment(_) | Token::Indent(_) | Token::Dedent(_) => {
                    self.advance();
                }
                _ => break,
            }
        }
    }

    fn skip_to_safe_point(&mut self) {
        let mut brace_depth = 0;
        loop {
            match self.peek() {
                Some(Token::LBrace) => {
                    brace_depth += 1;
                    self.advance();
                }
                Some(Token::RBrace) => {
                    if brace_depth == 0 {
                        self.advance();
                        return;
                    }
                    brace_depth -= 1;
                    self.advance();
                }
                Some(Token::Eof) | None => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn record_error(&mut self, err: ParseError) {
        self.errors.push(err);
    }

    // ── expression parser ──

    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Result<Expression, ParseError> {
        let cond = self.parse_or()?;
        if self.peek() == Some(&Token::Question) {
            self.advance();
            let then_expr = self.parse_ternary()?;
            self.expect_peek(&Token::Colon)?;
            let else_expr = self.parse_ternary()?;
            return Ok(Expression::TernaryOp {
                condition: Box::new(cond),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            });
        }
        Ok(cond)
    }

    fn parse_or(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_bitor()?;
        while self.peek() == Some(&Token::OrOr) {
            self.advance();
            let right = self.parse_bitor()?;
            left = Expression::BinaryOp {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_bitor(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_and()?;
        while self.peek() == Some(&Token::BitOr) {
            self.advance();
            let right = self.parse_and()?;
            left = Expression::BinaryOp {
                op: BinOp::BitOr,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_bitand()?;
        while self.peek() == Some(&Token::AndAnd) {
            self.advance();
            let right = self.parse_bitand()?;
            left = Expression::BinaryOp {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_bitand(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_equality()?;
        while self.peek() == Some(&Token::BitAnd) {
            self.advance();
            let right = self.parse_equality()?;
            left = Expression::BinaryOp {
                op: BinOp::BitAnd,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_comparison()?;
        loop {
            match self.peek() {
                Some(&Token::EqEq) => {
                    self.advance();
                    let right = self.parse_comparison()?;
                    left = Expression::BinaryOp {
                        op: BinOp::Eq,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Some(&Token::NotEq) => {
                    self.advance();
                    let right = self.parse_comparison()?;
                    left = Expression::BinaryOp {
                        op: BinOp::Neq,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_term()?;
        loop {
            match self.peek() {
                Some(&Token::Gt) => {
                    self.advance();
                    let right = self.parse_term()?;
                    left = Expression::BinaryOp {
                        op: BinOp::Gt,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Some(&Token::Lt) => {
                    self.advance();
                    let right = self.parse_term()?;
                    left = Expression::BinaryOp {
                        op: BinOp::Lt,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Some(&Token::GtEq) => {
                    self.advance();
                    let right = self.parse_term()?;
                    left = Expression::BinaryOp {
                        op: BinOp::Gte,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Some(&Token::LtEq) => {
                    self.advance();
                    let right = self.parse_term()?;
                    left = Expression::BinaryOp {
                        op: BinOp::Lte,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_factor()?;
        loop {
            match self.peek() {
                Some(&Token::Plus) => {
                    self.advance();
                    let right = self.parse_factor()?;
                    left = Expression::BinaryOp {
                        op: BinOp::Add,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Some(&Token::Minus) => {
                    self.advance();
                    let right = self.parse_factor()?;
                    left = Expression::BinaryOp {
                        op: BinOp::Sub,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_unary()?;
        loop {
            match self.peek() {
                Some(&Token::Star) => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expression::BinaryOp {
                        op: BinOp::Mul,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Some(&Token::Slash) => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expression::BinaryOp {
                        op: BinOp::Div,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expression, ParseError> {
        if self.peek() == Some(&Token::Plus) {
            self.advance();
            return self.parse_unary();
        }
        self.parse_unary_inner()
    }

    fn parse_unary_inner(&mut self) -> Result<Expression, ParseError> {
        if self.peek() == Some(&Token::Not) {
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expression::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(operand),
            });
        }
        if self.peek() == Some(&Token::Minus) {
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expression::UnaryOp {
                op: UnaryOp::Neg,
                operand: Box::new(operand),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expression, ParseError> {
        match self.advance() {
            Some(SpannedToken {
                token: Token::NumberLit(n),
                ..
            }) => Ok(Expression::NumberLit(n)),
            Some(SpannedToken {
                token: Token::StringLit(s),
                ..
            }) => Ok(Expression::StringLit(s)),
            Some(SpannedToken {
                token: Token::DirectiveT,
                ..
            }) => Ok(Expression::Localized(self.parse_t_pairs()?)),
            Some(SpannedToken {
                token: Token::BoolLit(b),
                ..
            }) => Ok(Expression::BoolLit(b)),
            Some(SpannedToken {
                token: Token::Identifier(name),
                ..
            }) => {
                if self.peek() == Some(&Token::LParen) {
                    self.advance();
                    let args = self.parse_call_args()?;
                    Ok(Expression::Call { callee: name, args })
                } else {
                    Ok(Expression::Variable(name))
                }
            }
            Some(SpannedToken {
                token: Token::LParen,
                ..
            }) => {
                let expr = self.parse_expression()?;
                self.expect_peek(&Token::RParen)?;
                Ok(expr)
            }
            Some(SpannedToken {
                token: Token::LBracket,
                ..
            }) => {
                let mut elements = Vec::new();
                self.skip_noise();
                if self.peek() != Some(&Token::RBracket) {
                    loop {
                        elements.push(self.parse_expression()?);
                        self.skip_noise();
                        match self.peek() {
                            Some(Token::Comma) => {
                                self.advance();
                                self.skip_noise();
                            }
                            Some(Token::RBracket) => break,
                            Some(t) => {
                                return Err(ParseError::UnexpectedToken {
                                    expected: vec![", or ]".into()],
                                    found: t.clone(),
                                    span: self.current_span(),
                                })
                            }
                            None => {
                                return Err(ParseError::UnexpectedEof {
                                    expected: vec![", or ]".into()],
                                    span: self.current_span(),
                                })
                            }
                        }
                    }
                }
                self.expect_peek(&Token::RBracket)?;
                Ok(Expression::ArrayLit(elements))
            }
            Some(SpannedToken {
                token: Token::LBrace,
                ..
            }) => {
                let mut fields = Vec::new();
                self.skip_noise();
                if self.peek() != Some(&Token::RBrace) {
                    loop {
                        let key = self.expect_ident()?;
                        self.expect_peek(&Token::Colon)?;
                        let value = self.parse_expression()?;
                        fields.push((key, value));
                        self.skip_noise();
                        match self.peek() {
                            Some(Token::Comma) => {
                                self.advance();
                                self.skip_noise();
                            }
                            Some(Token::RBrace) => break,
                            Some(t) => {
                                return Err(ParseError::UnexpectedToken {
                                    expected: vec![", or }".into()],
                                    found: t.clone(),
                                    span: self.current_span(),
                                })
                            }
                            None => {
                                return Err(ParseError::UnexpectedEof {
                                    expected: vec![", or }".into()],
                                    span: self.current_span(),
                                })
                            }
                        }
                    }
                }
                self.expect_peek(&Token::RBrace)?;
                Ok(Expression::ObjectLit(fields))
            }
            Some(st) => Err(ParseError::UnexpectedToken {
                expected: vec!["number, string, bool, or identifier".into()],
                found: st.token,
                span: st.span,
            }),
            None => Err(ParseError::UnexpectedEof {
                expected: vec!["expression".into()],
                span: self.current_span(),
            }),
        }
    }

    fn parse_call_args(&mut self) -> Result<Vec<Expression>, ParseError> {
        let mut args = Vec::new();
        self.skip_noise();
        if self.peek() == Some(&Token::RParen) {
            self.advance();
            return Ok(args);
        }
        loop {
            args.push(self.parse_expression()?);
            self.skip_noise();
            match self.peek() {
                Some(Token::Comma) => {
                    self.advance();
                    self.skip_noise();
                }
                Some(Token::RParen) => {
                    self.advance();
                    return Ok(args);
                }
                Some(t) => {
                    return Err(ParseError::UnexpectedToken {
                        expected: vec![", or )".into()],
                        found: t.clone(),
                        span: self.current_span(),
                    })
                }
                None => {
                    return Err(ParseError::UnexpectedEof {
                        expected: vec![", or )".into()],
                        span: self.current_span(),
                    })
                }
            }
        }
    }

    // ── i18n: `@t("en", "中文")` localized string literals ──

    /// Locale codes assigned positionally to `@t(...)` arguments.
    /// First argument → `en`, second → `zh`. Extra arguments get an
    /// `und` (undetermined) tag and are ignored by codegen.
    const LOCALE_ORDER: [&'static str; 2] = ["en", "zh"];

    /// Parse the `(...)` arguments of an `@t` localized literal, assuming the
    /// `@t` directive token has already been consumed. Each argument is a
    /// string literal; locale codes are assigned positionally via
    /// [`Self::LOCALE_ORDER`]. Returns ordered `(locale, text)` pairs.
    fn parse_t_pairs(&mut self) -> Result<Vec<(String, String)>, ParseError> {
        self.expect_peek(&Token::LParen)?;
        let mut texts: Vec<String> = Vec::new();
        self.skip_noise();
        if self.peek() != Some(&Token::RParen) {
            loop {
                self.skip_noise();
                texts.push(self.expect_string()?);
                self.skip_noise();
                match self.peek() {
                    Some(Token::Comma) => {
                        self.advance();
                        self.skip_noise();
                    }
                    _ => break,
                }
            }
        }
        self.expect_peek(&Token::RParen)?;
        let pairs = texts
            .into_iter()
            .enumerate()
            .map(|(i, t)| {
                let locale = Self::LOCALE_ORDER.get(i).copied().unwrap_or("und");
                (locale.to_string(), t)
            })
            .collect();
        Ok(pairs)
    }

    /// Parse a piece of author-facing text that is either a plain string
    /// literal (`"…"`) or a localized `@t("en", "中文")` literal.
    fn parse_localized_text(&mut self) -> Result<LocalizedText, ParseError> {
        self.skip_noise();
        match self.peek() {
            Some(Token::DirectiveT) => {
                self.advance(); // consume `@t`
                Ok(LocalizedText::Localized(self.parse_t_pairs()?))
            }
            _ => Ok(LocalizedText::Plain(self.expect_string()?)),
        }
    }

    // ── top-level ──

    pub fn parse(mut self) -> (Option<Document>, Vec<ParseError>) {
        self.skip_noise();

        // ── `component` declarations (before any screen block) ────────────
        let mut local_decls: Vec<ComponentDecl> = Vec::new();
        while matches!(self.peek(), Some(Token::Identifier(id)) if id == "component") {
            match self.parse_component_decl() {
                Ok(decl) => {
                    if self.component_decls.contains_key(&decl.name) {
                        self.record_error(ParseError::DuplicateComponentDecl {
                            name: decl.name.clone(),
                            span: decl.span.clone(),
                        });
                    }
                    self.component_decls.insert(decl.name.clone(), decl.clone());
                    local_decls.push(decl);
                }
                Err(e) => {
                    self.record_error(e);
                    break;
                }
            }
            self.skip_noise();
        }

        let doc = match self.peek() {
            Some(Token::KeywordGameScene) => match self.parse_game_scene() {
                Ok(scene) => Some(Document::Scene(scene)),
                Err(e) => {
                    self.record_error(e);
                    None
                }
            },
            Some(Token::KeywordScreen) => match self.parse_screen() {
                Ok(screen) => Some(Document::Screen(screen)),
                Err(e) => {
                    self.record_error(e);
                    None
                }
            },
            // Declarations-only file (a `components.gui` prelude).
            Some(Token::Eof) | None if !local_decls.is_empty() => {
                Some(Document::Components(local_decls))
            }
            Some(tok) => {
                self.record_error(ParseError::UnexpectedToken {
                    expected: vec!["game_scene, screen or component".into()],
                    found: tok.clone(),
                    span: self.current_span(),
                });
                None
            }
            None => {
                self.record_error(ParseError::UnexpectedEof {
                    expected: vec!["game_scene, screen or component".into()],
                    span: self.current_span(),
                });
                None
            }
        };
        (doc, self.errors)
    }

    /// Parse one `component <name> { prop: kind [required] ... }` declaration.
    fn parse_component_decl(&mut self) -> Result<ComponentDecl, ParseError> {
        let start_span = self.current_span();
        self.advance(); // consume `component`
        let name = self.expect_ident()?;
        self.expect_peek(&Token::LBrace)?;

        let mut props = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => {
                    let prop_span = self.current_span();
                    let prop_name = self.expect_ident()?;
                    self.expect_peek(&Token::Colon)?;
                    let kind_name = self.expect_ident()?;
                    let kind = match kind_name.as_str() {
                        "int" => PropKind::Int,
                        "string" => PropKind::String,
                        "bool" => PropKind::Bool,
                        "color" => PropKind::Color,
                        "expr" => PropKind::Expr,
                        other => {
                            return Err(ParseError::UnexpectedToken {
                                expected: vec![
                                    "int".into(),
                                    "string".into(),
                                    "bool".into(),
                                    "color".into(),
                                    "expr".into(),
                                ],
                                found: Token::Identifier(other.to_string()),
                                span: self.current_span(),
                            });
                        }
                    };
                    let required = if matches!(self.peek(), Some(Token::Identifier(id)) if id == "required")
                    {
                        self.advance();
                        true
                    } else {
                        false
                    };
                    props.push(PropDecl {
                        name: prop_name,
                        kind,
                        required,
                        span: prop_span,
                    });
                }
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        Ok(ComponentDecl {
            name,
            props,
            span: merge_span(&start_span, &self.current_span()),
        })
    }

    fn expect_keyword(&mut self, kw: Token) -> Result<(), ParseError> {
        match self.peek() {
            Some(t) if t == &kw => {
                self.advance();
                Ok(())
            }
            Some(found) => Err(ParseError::UnexpectedToken {
                expected: vec![format!("{:?}", kw)],
                found: found.clone(),
                span: self.current_span(),
            }),
            None => Err(ParseError::UnexpectedEof {
                expected: vec![format!("{:?}", kw)],
                span: self.current_span(),
            }),
        }
    }

    fn parse_game_scene(&mut self) -> Result<GameScene, ParseError> {
        let start_span = self.current_span();
        self.expect_keyword(Token::KeywordGameScene)?;
        let name = self.expect_ident()?;
        self.expect_peek(&Token::LBrace)?;

        let mut variables = None;
        let mut storylines: Vec<Storyline> = Vec::new();
        let mut on_load: Option<StorylineBlock> = None;
        let mut ui = None;
        let mut themes = Vec::new();
        let mut styles = Vec::new();
        let mut atlases = Vec::new();

        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                Some(Token::DirectiveVariables) => variables = Some(self.parse_variables_block()?),
                Some(Token::DirectiveStorylines) => {
                    let block = self.parse_storylines_block()?;
                    storylines.push(Storyline {
                        name: "main".into(),
                        triggers: Vec::new(),
                        statements: block.statements,
                        span: block.span,
                    });
                }
                Some(Token::DirectiveStoryline) => {
                    let storyline = self.parse_named_storyline()?;
                    storylines.push(storyline);
                }
                Some(Token::DirectiveOnLoad) => {
                    if on_load.is_some() {
                        self.record_error(ParseError::UnexpectedToken {
                            expected: vec!["a single @load per scene".into()],
                            found: Token::DirectiveOnLoad,
                            span: self.current_span(),
                        });
                    } else {
                        on_load = Some(self.parse_onload_block()?);
                    }
                }
                Some(Token::DirectiveTheme) => themes.push(self.parse_theme()?),
                Some(Token::DirectiveStyle) => styles.push(self.parse_style()?),
                Some(Token::DirectiveAtlas) => atlases.push(self.parse_atlas()?),
                Some(Token::KeywordUi) => ui = Some(self.parse_ui_block()?),
                Some(tok) => {
                    self.record_error(ParseError::UnexpectedToken {
                        expected: vec!["a directive or ui block".into()],
                        found: tok.clone(),
                        span: self.current_span(),
                    });
                    self.skip_to_safe_point();
                }
            }
        }

        self.expect_peek(&Token::RBrace).ok();
        Ok(GameScene {
            name,
            variables,
            storylines,
            on_load,
            ui,
            themes,
            styles,
            atlases,
            span: SourceSpan::new(
                &start_span.file,
                start_span.line_start,
                start_span.col_start,
                self.current_span().line_end,
                self.current_span().col_end,
                0,
                0,
            ),
        })
    }

    fn parse_screen(&mut self) -> Result<ScreenLayout, ParseError> {
        let start_span = self.current_span();
        self.expect_keyword(Token::KeywordScreen)?;
        let name = self.expect_ident()?;
        let theme = if self.peek() == Some(&Token::Colon) {
            self.advance();
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect_peek(&Token::LBrace)?;
        let mut components = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => components.push(self.parse_component()?),
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        Ok(ScreenLayout {
            name,
            theme,
            components,
            schema_version: None,
            span: SourceSpan::new(
                &start_span.file,
                start_span.line_start,
                start_span.col_start,
                self.current_span().line_end,
                self.current_span().col_end,
                0,
                0,
            ),
        })
    }

    fn parse_variables_block(&mut self) -> Result<VariablesBlock, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveVariables)?;
        self.expect_peek(&Token::LBrace)?;
        let mut decls = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => {
                    let name = self.expect_ident()?;
                    self.expect_peek(&Token::Equals)?;
                    let value = self.parse_expression()?;
                    let span = value_span(&value);
                    decls.push(VariableDecl {
                        name: name.clone(),
                        value,
                        span,
                    });
                    self.current_scope.push(name);
                }
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        Ok(VariablesBlock {
            decls,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_storylines_block(&mut self) -> Result<StorylineBlock, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveStorylines)?;
        self.expect_peek(&Token::LBrace)?;
        self.parse_stmt_block_body(span)
    }

    fn parse_onload_block(&mut self) -> Result<StorylineBlock, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveOnLoad)?;
        self.expect_peek(&Token::LBrace)?;
        self.parse_stmt_block_body(span)
    }

    fn parse_stmt_block_body(&mut self, span: SourceSpan) -> Result<StorylineBlock, ParseError> {
        let mut statements = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => statements.push(self.parse_story_stmt()?),
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        Ok(StorylineBlock {
            statements,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_named_storyline(&mut self) -> Result<Storyline, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveStoryline)?;
        self.expect_peek(&Token::LParen)?;
        let name = self.expect_string()?;
        self.expect_peek(&Token::RParen)?;
        self.expect_peek(&Token::LBrace)?;

        let mut triggers = Vec::new();
        loop {
            self.skip_noise();
            if self.peek() == Some(&Token::DirectiveTrigger) {
                triggers.push(self.parse_trigger_condition()?);
            } else {
                break;
            }
        }

        let mut statements = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => statements.push(self.parse_story_stmt()?),
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        Ok(Storyline {
            name,
            triggers,
            statements,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_trigger_condition(&mut self) -> Result<TriggerCondition, ParseError> {
        let span = self.current_span();
        self.advance();
        self.expect_peek(&Token::LParen)?;

        fn coord_from_expr(e: &Expression) -> Option<(u16, u16)> {
            if let Expression::ArrayLit(items) = e {
                if items.len() == 2 {
                    if let (Expression::NumberLit(x), Expression::NumberLit(y)) =
                        (&items[0], &items[1])
                    {
                        return Some((*x as u16, *y as u16));
                    }
                }
            }
            None
        }

        let mut map = String::new();
        let mut npc = None;
        let mut on_enter = false;
        let mut after = None;
        let mut priority = None;
        let mut npc_id = None;
        let mut sign_id = None;
        let mut coords: Vec<(u16, u16)> = Vec::new();
        let mut toggle_id = None;
        let mut script_id = None;
        let mut default_hidden = false;
        let mut no_talk = false;
        let mut name = String::new();

        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::RParen) | None | Some(Token::Eof) => break,
                _ => {
                    let key = self.expect_ident()?;
                    self.expect_peek(&Token::Equals)?;
                    let value = self.parse_expression()?;
                    match key.as_str() {
                        "map" => {
                            if let Expression::StringLit(s) = &value {
                                map = s.clone();
                            }
                        }
                        "name" => {
                            if let Expression::StringLit(s) = &value {
                                name = s.clone();
                            }
                        }
                        "npc" => match &value {
                            Expression::NumberLit(n) => npc_id = Some(*n as u8),
                            Expression::StringLit(s) => match s.parse::<u8>() {
                                Ok(n) => npc_id = Some(n),
                                Err(_) => npc = Some(s.clone()),
                            },
                            _ => {}
                        },
                        "sign" => match &value {
                            Expression::NumberLit(n) => sign_id = Some(*n as u8),
                            Expression::StringLit(s) => {
                                if let Ok(n) = s.parse::<u8>() {
                                    sign_id = Some(n);
                                }
                            }
                            _ => {}
                        },
                        "coord" => {
                            if let Some(c) = coord_from_expr(&value) {
                                coords.push(c);
                            }
                        }
                        "coords" => {
                            if let Expression::ArrayLit(items) = &value {
                                for it in items {
                                    if let Some(c) = coord_from_expr(it) {
                                        coords.push(c);
                                    }
                                }
                            }
                        }
                        "toggle" | "toggleId" => {
                            if let Expression::StringLit(s) = &value {
                                toggle_id = Some(s.clone());
                            }
                        }
                        "script" | "scriptId" => {
                            if let Expression::StringLit(s) = &value {
                                script_id = Some(s.clone());
                            }
                        }
                        "hidden" | "defaultHidden" => {
                            if let Expression::BoolLit(b) = &value {
                                default_hidden = *b;
                            }
                        }
                        "no_talk" | "noTalk" => {
                            if let Expression::BoolLit(b) = &value {
                                no_talk = *b;
                            }
                        }
                        "on_enter" | "onEnter" => {
                            if let Expression::BoolLit(b) = &value {
                                on_enter = *b;
                            }
                        }
                        "after" => {
                            if let Expression::StringLit(s) = &value {
                                after = Some(s.clone());
                            }
                        }
                        "priority" => {
                            if let Expression::NumberLit(n) = &value {
                                priority = Some(*n as i32);
                            }
                        }
                        _ => {}
                    }
                    self.skip_noise();
                    if self.peek() == Some(&Token::Comma) {
                        self.advance();
                    }
                }
            }
        }
        self.expect_peek(&Token::RParen).ok();

        Ok(TriggerCondition {
            map,
            npc,
            on_enter,
            after,
            priority,
            npc_id,
            sign_id,
            coords,
            name,
            toggle_id,
            script_id,
            default_hidden,
            no_talk,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_run_block(&mut self) -> Result<StoryStmt, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveRun)?;
        self.skip_noise();

        match self.peek() {
            Some(Token::RawBlock(js)) => {
                let js = js.clone();
                self.advance();
                let end_span = self.current_span();
                Ok(StoryStmt::Run {
                    js,
                    span: merge_span(&span, &end_span),
                })
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: vec!["@run { ... }".into()],
                found: self.peek().cloned().unwrap_or(Token::Eof),
                span: self.current_span(),
            }),
        }
    }

    fn parse_story_stmt(&mut self) -> Result<StoryStmt, ParseError> {
        match self.peek() {
            Some(Token::DirectiveRun) => self.parse_run_block(),
            Some(Token::DirectiveSpeaker) => self.parse_speaker_stmt(),
            Some(Token::DirectiveSay) => self.parse_say_stmt(),
            Some(Token::DirectiveChoice) => self.parse_choice_stmt(),
            Some(Token::DirectiveIf) => self.parse_if_stmt(),
            Some(Token::DirectiveEach) => self.parse_each_stmt(),
            Some(Token::DirectiveCommand) => self.parse_directive_command_stmt(),
            Some(Token::Identifier(_)) => {
                if self.peek_n(1) == Some(&Token::Equals) {
                    self.parse_assign_stmt()
                } else {
                    self.parse_command_stmt()
                }
            }
            Some(tok) => Err(ParseError::UnexpectedToken {
                expected: vec!["@speaker, @say, @choice, @if, @each, or assignment".into()],
                found: tok.clone(),
                span: self.current_span(),
            }),
            None => Err(ParseError::UnexpectedEof {
                expected: vec!["story statement".into()],
                span: self.current_span(),
            }),
        }
    }

    fn parse_speaker_stmt(&mut self) -> Result<StoryStmt, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveSpeaker)?;
        let (name, texts, end_span) = self.parse_speech_body(span.clone())?;
        Ok(StoryStmt::Speaker {
            name,
            texts,
            span: end_span,
        })
    }

    fn parse_say_stmt(&mut self) -> Result<StoryStmt, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveSay)?;
        let (name, texts, end_span) = self.parse_speech_body(span.clone())?;
        Ok(StoryStmt::Say {
            name,
            texts,
            span: end_span,
        })
    }

    /// Parse the `(name) { "text" ... }` body shared by `@speaker` and `@say`.
    fn parse_speech_body(
        &mut self,
        span: SourceSpan,
    ) -> Result<(Expression, Vec<LocalizedText>, SourceSpan), ParseError> {
        self.expect_peek(&Token::LParen)?;
        let name = self.parse_expression()?;
        self.expect_peek(&Token::RParen)?;
        self.expect_peek(&Token::LBrace)?;
        let mut texts: Vec<LocalizedText> = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::StringLit(_)) => {
                    texts.push(LocalizedText::Plain(self.expect_string()?));
                }
                Some(Token::DirectiveT) => {
                    self.advance(); // consume `@t`
                    texts.push(LocalizedText::Localized(self.parse_t_pairs()?));
                }
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => break,
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        let end_span = merge_span(&span, &self.current_span());
        Ok((name, texts, end_span))
    }

    fn parse_choice_stmt(&mut self) -> Result<StoryStmt, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveChoice)?;
        self.expect_peek(&Token::LBrace)?;
        let mut options = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::DirectiveOption) => options.push(self.parse_option()?),
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => break,
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        Ok(StoryStmt::Choice {
            options,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_option(&mut self) -> Result<ChoiceOption, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveOption)?;
        self.expect_peek(&Token::LParen)?;
        let label = self.parse_localized_text()?;
        self.expect_peek(&Token::RParen)?;
        self.expect_peek(&Token::LBrace)?;
        let mut body = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => body.push(self.parse_story_stmt()?),
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        Ok(ChoiceOption {
            label,
            body,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_if_stmt(&mut self) -> Result<StoryStmt, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveIf)?;
        self.expect_peek(&Token::LParen)?;
        let condition = self.parse_expression()?;
        self.expect_peek(&Token::RParen)?;
        self.expect_peek(&Token::LBrace)?;
        let mut then_branch = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => then_branch.push(self.parse_story_stmt()?),
            }
        }
        self.expect_peek(&Token::RBrace).ok();

        let mut else_branch = Vec::new();
        self.skip_noise();
        if self.peek() == Some(&Token::DirectiveElse) {
            self.advance();
            self.skip_noise();
            if self.peek() == Some(&Token::DirectiveIf) {
                self.expect_keyword(Token::DirectiveIf)?;
                self.expect_peek(&Token::LParen)?;
                let elif_cond = self.parse_expression()?;
                self.expect_peek(&Token::RParen)?;
                self.expect_peek(&Token::LBrace)?;
                let mut elif_then = Vec::new();
                loop {
                    self.skip_noise();
                    match self.peek() {
                        Some(Token::RBrace) | Some(Token::Eof) | None => break,
                        _ => elif_then.push(self.parse_story_stmt()?),
                    }
                }
                self.expect_peek(&Token::RBrace).ok();
                let mut elif_else = Vec::new();
                self.skip_noise();
                if self.peek() == Some(&Token::DirectiveElse) {
                    self.advance();
                    self.skip_noise();
                    if self.peek() == Some(&Token::LBrace) {
                        self.expect_peek(&Token::LBrace)?;
                        loop {
                            self.skip_noise();
                            match self.peek() {
                                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                                _ => elif_else.push(self.parse_story_stmt()?),
                            }
                        }
                        self.expect_peek(&Token::RBrace).ok();
                    }
                }
                else_branch = vec![StoryStmt::If {
                    condition: elif_cond,
                    then_branch: elif_then,
                    else_branch: elif_else,
                    span: self.current_span(),
                }];
            } else {
                self.expect_peek(&Token::LBrace)?;
                loop {
                    self.skip_noise();
                    match self.peek() {
                        Some(Token::RBrace) | Some(Token::Eof) | None => break,
                        _ => else_branch.push(self.parse_story_stmt()?),
                    }
                }
                self.expect_peek(&Token::RBrace).ok();
            }
        }

        Ok(StoryStmt::If {
            condition,
            then_branch,
            else_branch,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_each_stmt(&mut self) -> Result<StoryStmt, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveEach)?;
        let item_var = self.expect_ident()?;
        if let Some(Token::Identifier(s)) = self.peek() {
            if s == "in" {
                self.advance();
            }
        }
        let source = self.parse_expression()?;
        self.expect_peek(&Token::LBrace)?;
        let mut body = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => body.push(self.parse_story_stmt()?),
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        Ok(StoryStmt::Each {
            item_var,
            source,
            body,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_assign_stmt(&mut self) -> Result<StoryStmt, ParseError> {
        let span = self.current_span();
        let name = self.expect_ident()?;
        self.expect_peek(&Token::Equals)?;
        let value = self.parse_expression()?;
        Ok(StoryStmt::Assign {
            name,
            value,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_command_stmt(&mut self) -> Result<StoryStmt, ParseError> {
        let span = self.current_span();
        let name = match self.advance() {
            Some(SpannedToken {
                token: Token::Identifier(s),
                ..
            }) => s,
            Some(st) => format!("{:?}", st.token),
            None => {
                return Err(ParseError::UnexpectedEof {
                    expected: vec!["command name".into()],
                    span: self.current_span(),
                })
            }
        };
        let mut args = Vec::new();
        self.skip_noise();
        if self.peek() == Some(&Token::LParen) {
            self.advance();
            loop {
                self.skip_noise();
                match self.peek() {
                    Some(Token::RParen) | None | Some(Token::Eof) => break,
                    _ => {
                        args.push(self.parse_expression()?);
                        if self.peek() == Some(&Token::Comma) {
                            self.advance();
                        }
                    }
                }
            }
            self.expect_peek(&Token::RParen).ok();
        }
        Ok(StoryStmt::Command {
            name,
            args,
            span: merge_span(&span, &self.current_span()),
        })
    }

    /// Parse `@Command("api_func", arg1, arg2, ...)` — directive escape hatch
    /// for calling arbitrary game.* API functions.
    ///
    /// The first string argument becomes the command name, and the remaining
    /// arguments are passed as-is.
    fn parse_directive_command_stmt(&mut self) -> Result<StoryStmt, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveCommand)?;

        if self.peek() != Some(&Token::LParen) {
            return Err(ParseError::UnexpectedToken {
                expected: vec!["\"(\" after @command".into()],
                found: self.peek().cloned().unwrap_or(Token::Eof),
                span: self.current_span(),
            });
        }
        self.advance();

        let mut args = Vec::new();
        self.skip_noise();
        loop {
            match self.peek() {
                Some(Token::RParen) | None | Some(Token::Eof) => break,
                _ => {
                    args.push(self.parse_expression()?);
                    self.skip_noise();
                    if self.peek() == Some(&Token::Comma) {
                        self.advance();
                        self.skip_noise();
                    }
                }
            }
        }
        self.expect_peek(&Token::RParen).ok();

        // The first argument (must be a string) is the command name.
        // Remaining args are the command arguments.
        if args.is_empty() {
            return Err(ParseError::UnexpectedToken {
                expected: vec!["command name (string) as first argument".into()],
                found: Token::RParen,
                span: self.current_span(),
            });
        }
        let name = match &args[0] {
            Expression::StringLit(s) => s.clone(),
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: vec!["string literal as @command name".into()],
                    found: Token::Identifier(format!("{:?}", args[0])),
                    span: span,
                });
            }
        };
        let cmd_args = args.into_iter().skip(1).collect();

        Ok(StoryStmt::Command {
            name,
            args: cmd_args,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_ui_block(&mut self) -> Result<UiBlock, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::KeywordUi)?;
        self.expect_peek(&Token::LBrace)?;
        let mut components = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => components.push(self.parse_component()?),
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        Ok(UiBlock {
            components,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_component(&mut self) -> Result<UiComponent, ParseError> {
        let span = self.current_span();
        let id: Option<String> = if matches!(self.peek(), Some(Token::Identifier(_)))
            && self.peek_n(1) == Some(&Token::Equals)
        {
            let name = self.expect_ident()?;
            self.advance();
            Some(name)
        } else {
            None
        };

        let comp_type = self.expect_ident()?;
        let custom_decl = self.component_decls.get(&comp_type).cloned();
        if !VALID_COMPONENT_TYPES.contains(&comp_type.as_str()) && custom_decl.is_none() {
            let mut valid: Vec<String> = VALID_COMPONENT_TYPES
                .iter()
                .map(|s| s.to_string())
                .collect();
            valid.extend(self.component_decls.keys().cloned());
            return Err(ParseError::InvalidComponentType {
                found: comp_type.clone(),
                valid,
                span: self.current_span(),
            });
        }

        let expr_arg: Option<Expression> = if self.peek() == Some(&Token::LParen) {
            self.advance();
            let expr = self.parse_expression()?;
            self.expect_peek(&Token::RParen)?;
            Some(expr)
        } else {
            None
        };

        self.expect_peek(&Token::LBrace)?;
        let mut props = self.parse_component_props()?;
        if let Some(ref id_str) = id {
            props.id = Some(id_str.clone());
        }

        let mut children = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => children.push(self.parse_component()?),
            }
        }
        self.expect_peek(&Token::RBrace).ok();

        let end_span = merge_span(&span, &self.current_span());
        match comp_type.as_str() {
            "panel" => Ok(UiComponent::Panel {
                props,
                children,
                span: end_span,
            }),
            "container" => Ok(UiComponent::Container {
                props,
                children,
                span: end_span,
            }),
            "text" => {
                let content = localized_from_expr(expr_arg);
                Ok(UiComponent::Text {
                    content,
                    props,
                    span: end_span,
                })
            }
            "button" => {
                let label = localized_from_expr(expr_arg);
                Ok(UiComponent::Button {
                    label,
                    props,
                    span: end_span,
                })
            }
            "list" => {
                // `source = …` is parsed as a generic property (into props.custom)
                // by parse_component_props above. Lift it into the dedicated
                // `source` field and drop it from custom so it does not leak into
                // the compiled JSON as an unknown field (which would otherwise
                // make the renderer reject the element via deny_unknown_fields).
                let source = props
                    .custom
                    .remove("source")
                    .unwrap_or_else(|| Expression::Variable("items".into()));
                Ok(UiComponent::List {
                    source,
                    format: None,
                    props,
                    span: end_span,
                })
            }
            "image" => {
                let src = match expr_arg {
                    Some(Expression::StringLit(s)) => s,
                    _ => String::new(),
                };
                Ok(UiComponent::Image {
                    src,
                    props,
                    span: end_span,
                })
            }
            "input" => Ok(UiComponent::Input {
                props,
                span: end_span,
            }),
            "dropdown" => Ok(UiComponent::Dropdown {
                props,
                span: end_span,
            }),
            "tile" => {
                let tile_id = expr_arg.unwrap_or(Expression::NumberLit(0.0));
                Ok(UiComponent::Tile {
                    tile_id,
                    props,
                    span: end_span,
                })
            }
            "divider" => {
                let tiles = props.tiles.clone().unwrap_or_default();
                Ok(UiComponent::Divider {
                    tiles,
                    props,
                    span: end_span,
                })
            }
            "flex_list" => {
                let source = match expr_arg {
                    Some(expr) => expr,
                    None => Expression::Variable("items".into()),
                };
                Ok(UiComponent::FlexList {
                    source,
                    format: None,
                    props,
                    span: end_span,
                })
            }
            "cursor" => Ok(UiComponent::Cursor {
                props,
                span: end_span,
            }),
            "bracket" => Ok(UiComponent::Bracket {
                props,
                span: end_span,
            }),
            "pixel_rect" => Ok(UiComponent::PixelRect {
                props,
                span: end_span,
            }),
            _ => {
                // Checked against `component_decls` at the top of this fn.
                let decl = custom_decl.expect("custom component decl looked up above");
                validate_custom_props(&decl, &props, &end_span)?;
                Ok(UiComponent::Custom {
                    name: comp_type,
                    props,
                    span: end_span,
                })
            }
        }
    }

    fn parse_component_props(&mut self) -> Result<ComponentProps, ParseError> {
        let mut props = ComponentProps {
            id: None,
            width: None,
            height: None,
            padding: None,
            margin: None,
            align: None,
            on_click: None,
            flex_grow: None,
            visible: None,
            custom: HashMap::default(),
            span: self.current_span(),
            rect: None,
            style: None,
            value: None,
            color: None,
            font: None,
            wrap: None,
            line_spacing: None,
            scale: None,
            tile_id: None,
            tiles: None,
            repeat: None,
            orientation: None,
            cursor: None,
            selected: None,
            max_visible: None,
            footer: None,
            item_template: None,
            item_layout: None,
            gap: None,
            clip: None,
            flip_x: None,
            flip_y: None,
            palette: None,
        };
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::Identifier(_)) => {
                    if self.peek_n(1) == Some(&Token::Equals) {
                        if let Some(Token::Identifier(_)) = self.peek_n(2) {
                            if matches!(self.peek_n(3), Some(Token::LParen) | Some(Token::LBrace)) {
                                break; // child component, not a property
                            }
                        }
                    } else if matches!(self.peek_n(1), Some(Token::LParen) | Some(Token::LBrace)) {
                        break; // bare component, not a property (`text("...")` or `panel {`)
                    }

                    let key = self.expect_ident()?;
                    if self.peek() != Some(&Token::Equals) {
                        break;
                    }
                    // Special case: rect = { tx: N, ty: N, tw: N, th: N }
                    if key == "rect" && self.peek_n(1) == Some(&Token::LBrace) {
                        self.advance(); // consume '='
                        props.rect = Some(self.parse_rect_def()?);
                        continue;
                    }
                    self.advance();
                    let val = self.parse_expression()?;
                    match key.as_str() {
                        "width" => props.width = Some(val),
                        "height" => props.height = Some(val),
                        "padding" => {
                            props.padding = Some(match &val {
                                Expression::NumberLit(n) => vec![Expression::NumberLit(*n); 4],
                                other => vec![other.clone()],
                            })
                        }
                        "margin" => {
                            props.margin = Some(match &val {
                                Expression::NumberLit(n) => vec![Expression::NumberLit(*n); 4],
                                other => vec![other.clone()],
                            })
                        }
                        "align" => {
                            if let Expression::StringLit(s) = &val {
                                props.align = Some(s.clone());
                            }
                        }
                        "on_click" => match &val {
                            Expression::StringLit(s) => props.on_click = Some(s.clone()),
                            Expression::Variable(v) => props.on_click = Some(v.clone()),
                            _ => {}
                        },
                        "flex_grow" => {
                            if let Expression::NumberLit(n) = val {
                                props.flex_grow = Some(n as u32);
                            }
                        }
                        // Non-bool `visible` (e.g. the `"{binding}"` template form) must
                        // survive into the JSON output, where the runtime's `Visibility`
                        // deserializer accepts a bool or a template string.
                        "visible" => {
                            if let Expression::BoolLit(b) = &val {
                                props.visible = Some(*b);
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        // ─── pokered-specific props ───
                        "style" => {
                            if let Expression::StringLit(s) = &val {
                                props.style = Some(s.clone());
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "value" => {
                            if let Expression::StringLit(s) = &val {
                                props.value = Some(s.clone());
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "color" => {
                            if let Expression::StringLit(s) = &val {
                                props.color = Some(s.clone());
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "font" => {
                            if let Expression::StringLit(s) = &val {
                                props.font = Some(s.clone());
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "wrap" => {
                            if let Expression::StringLit(s) = &val {
                                props.wrap = Some(s.clone());
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "line_spacing" => {
                            if let Expression::NumberLit(n) = &val {
                                props.line_spacing = Some(*n as u32);
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "scale" => {
                            if let Expression::NumberLit(n) = &val {
                                props.scale = Some(*n as u32);
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "tile_id" => props.tile_id = Some(val),
                        "tiles" => {
                            if let Expression::ArrayLit(items) = val {
                                props.tiles = Some(items);
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "repeat" => {
                            if let Expression::NumberLit(n) = &val {
                                props.repeat = Some(*n as u32);
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "orientation" => {
                            if let Expression::StringLit(s) = &val {
                                props.orientation = Some(s.clone());
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "cursor" => props.cursor = Some(val),
                        "selected" => props.selected = Some(val),
                        "max_visible" => {
                            if let Expression::NumberLit(n) = &val {
                                props.max_visible = Some(*n as u32);
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "footer" => {
                            if let Expression::StringLit(s) = &val {
                                props.footer = Some(s.clone());
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "item_template" => props.item_template = Some(val),
                        "item_layout" => {
                            if let Expression::ArrayLit(items) = val {
                                props.item_layout = Some(items);
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "gap" => {
                            if let Expression::NumberLit(n) = &val {
                                props.gap = Some(*n as u32);
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "clip" => {
                            if let Expression::BoolLit(b) = &val {
                                props.clip = Some(*b);
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "flip_x" => {
                            if let Expression::BoolLit(b) = &val {
                                props.flip_x = Some(*b);
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "flip_y" => {
                            if let Expression::BoolLit(b) = &val {
                                props.flip_y = Some(*b);
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        "palette" => {
                            if let Expression::StringLit(s) = &val {
                                props.palette = Some(s.clone());
                            } else {
                                props.custom.insert(key, val);
                            }
                        }
                        _ => {
                            props.custom.insert(key, val);
                        }
                    }
                }
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => break,
            }
        }
        Ok(props)
    }

    fn parse_rect_def(&mut self) -> Result<RectDef, ParseError> {
        let span = self.current_span();
        self.expect_peek(&Token::LBrace)?;
        let mut tx = None;
        let mut ty = None;
        let mut tw = None;
        let mut th = None;
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::Identifier(_)) => {
                    let key = self.expect_ident()?;
                    self.expect_peek(&Token::Colon)?;
                    let val = self.parse_expression()?;
                    match key.as_str() {
                        "tx" => tx = Some(val),
                        "ty" => ty = Some(val),
                        "tw" => tw = Some(val),
                        "th" => th = Some(val),
                        _ => {}
                    }
                    self.skip_noise();
                    if self.peek() == Some(&Token::Comma) {
                        self.advance();
                    }
                }
                Some(Token::RBrace) => break,
                _ => break,
            }
        }
        self.expect_peek(&Token::RBrace)?;
        let end_span = self.current_span();
        Ok(RectDef {
            tx: tx.unwrap_or(Expression::NumberLit(0.0)),
            ty: ty.unwrap_or(Expression::NumberLit(0.0)),
            tw: tw.unwrap_or(Expression::NumberLit(0.0)),
            th: th.unwrap_or(Expression::NumberLit(0.0)),
            span: merge_span(&span, &end_span),
        })
    }

    fn parse_theme(&mut self) -> Result<Theme, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveTheme)?;
        let name = self.expect_ident()?;
        self.expect_peek(&Token::LBrace)?;
        let mut tokens = HashMap::default();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::Identifier(_)) => {
                    let key = self.expect_ident()?;
                    self.expect_peek(&Token::Equals)?;
                    let val = self.expect_string()?;
                    tokens.insert(key, val);
                }
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => break,
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        Ok(Theme {
            name,
            tokens,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_style(&mut self) -> Result<Style, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveStyle)?;
        let name = self.expect_ident()?;
        let extends = if self.peek() == Some(&Token::Colon) {
            self.advance();
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect_peek(&Token::LBrace)?;
        let mut properties = HashMap::default();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::Identifier(_)) => {
                    let key = self.expect_ident()?;
                    self.expect_peek(&Token::Equals)?;
                    let val = self.parse_expression()?;
                    properties.insert(key, val);
                }
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => break,
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        Ok(Style {
            name,
            extends,
            properties,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_atlas(&mut self) -> Result<Atlas, ParseError> {
        let span = self.current_span();
        self.expect_keyword(Token::DirectiveAtlas)?;
        let name = self.expect_string()?;
        self.expect_peek(&Token::LBrace)?;
        let mut source = String::new();
        let mut regions = Vec::new();
        loop {
            self.skip_noise();
            match self.peek() {
                Some(Token::Identifier(key)) if key == "source" => {
                    self.advance();
                    self.expect_peek(&Token::Equals)?;
                    source = self.expect_string()?;
                }
                Some(Token::Identifier(key)) if key == "regions" => {
                    self.advance();
                    self.expect_peek(&Token::Equals)?;
                    self.expect_peek(&Token::LBrace)?;
                    loop {
                        self.skip_noise();
                        match self.peek() {
                            Some(Token::Identifier(_)) => {
                                let rname = self.expect_ident()?;
                                self.expect_peek(&Token::Equals)?;
                                regions.push(self.parse_atlas_region(rname)?);
                            }
                            Some(Token::RBrace) | Some(Token::Eof) | None => break,
                            _ => break,
                        }
                    }
                    self.expect_peek(&Token::RBrace).ok();
                }
                Some(Token::RBrace) | Some(Token::Eof) | None => break,
                _ => break,
            }
        }
        self.expect_peek(&Token::RBrace).ok();
        Ok(Atlas {
            name,
            source,
            regions,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_atlas_region(&mut self, name: String) -> Result<AtlasRegion, ParseError> {
        let span = self.current_span();
        self.expect_peek(&Token::LBracket)?;
        let x = self.parse_number()?;
        self.expect_peek(&Token::Comma)?;
        let y = self.parse_number()?;
        self.expect_peek(&Token::Comma)?;
        let w = self.parse_number()?;
        self.expect_peek(&Token::Comma)?;
        let h = self.parse_number()?;
        let mut nine_slice = None;
        self.skip_noise();
        if self.peek() == Some(&Token::Comma) {
            self.advance();
            self.skip_noise();
            if let Some(Token::Identifier(s)) = self.peek() {
                if s == "slice" {
                    self.advance();
                    self.expect_peek(&Token::Equals)?;
                    nine_slice = Some(self.parse_slice_value()?);
                }
            }
        }
        self.expect_peek(&Token::RBracket).ok();
        Ok(AtlasRegion {
            name,
            x,
            y,
            w,
            h,
            nine_slice,
            span: merge_span(&span, &self.current_span()),
        })
    }

    fn parse_number(&mut self) -> Result<u32, ParseError> {
        match self.advance() {
            Some(SpannedToken {
                token: Token::NumberLit(n),
                ..
            }) => Ok(n as u32),
            Some(st) => Err(ParseError::UnexpectedToken {
                expected: vec!["number".into()],
                found: st.token,
                span: st.span,
            }),
            None => Err(ParseError::UnexpectedEof {
                expected: vec!["number".into()],
                span: self.current_span(),
            }),
        }
    }

    fn parse_slice_value(&mut self) -> Result<[u32; 4], ParseError> {
        if self.peek() == Some(&Token::LBracket) {
            self.advance();
            let t = self.parse_number()?;
            self.skip_noise();
            self.expect_peek(&Token::Comma)?;
            let r = self.parse_number()?;
            self.skip_noise();
            self.expect_peek(&Token::Comma)?;
            let b = self.parse_number()?;
            self.skip_noise();
            self.expect_peek(&Token::Comma)?;
            let l = self.parse_number()?;
            self.expect_peek(&Token::RBracket)?;
            Ok([t, r, b, l])
        } else {
            let v = self.parse_number()?;
            Ok([v, v, v, v])
        }
    }
}

pub struct SemanticValidator {
    pub errors: Vec<SemanticError>,
}

impl SemanticValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn validate_scene(&mut self, scene: &GameScene) {
        let mut declared_vars: Vec<String> = scene
            .variables
            .as_ref()
            .map(|v| v.decls.iter().map(|d| d.name.clone()).collect())
            .unwrap_or_default();

        for storyline in &scene.storylines {
            self.collect_assigns(&storyline.statements, &mut declared_vars);
        }

        for storyline in &scene.storylines {
            for stmt in &storyline.statements {
                self.check_stmt_vars(stmt, &declared_vars);
            }
        }

        if let Some(vb) = &scene.variables {
            for decl in &vb.decls {
                for v in extract_variables(&decl.value) {
                    if !declared_vars.contains(&v) && v != decl.name {
                        self.errors.push(SemanticError::UndefinedVariable {
                            name: v,
                            defined_vars: declared_vars.clone(),
                            span: value_span(&decl.value),
                        });
                    }
                }
            }
        }

        for storyline in &scene.storylines {
            self.check_choice_has_options(&storyline.statements);
        }
        self.check_style_cycles(&scene.styles);
        self.check_uniqueness(&scene.themes, &scene.styles);
    }

    fn collect_assigns(&mut self, stmts: &[StoryStmt], declared: &mut Vec<String>) {
        for stmt in stmts {
            match stmt {
                StoryStmt::Assign { name, .. } => {
                    if !declared.contains(name) {
                        declared.push(name.clone());
                    }
                }
                StoryStmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.collect_assigns(then_branch, declared);
                    self.collect_assigns(else_branch, declared);
                }
                StoryStmt::Each { body, .. } => self.collect_assigns(body, declared),
                StoryStmt::Choice { options, .. } => {
                    for opt in options {
                        self.collect_assigns(&opt.body, declared);
                    }
                }
                StoryStmt::Run { .. } => {}
                _ => {}
            }
        }
    }

    fn check_stmt_vars(&mut self, stmt: &StoryStmt, declared: &[String]) {
        match stmt {
            StoryStmt::Assign { value, .. } => {
                for v in extract_variables(value) {
                    if !declared.contains(&v) {
                        self.errors.push(SemanticError::UndefinedVariable {
                            name: v,
                            defined_vars: declared.to_vec(),
                            span: value_span(value),
                        });
                    }
                }
            }
            StoryStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                for v in extract_variables(condition) {
                    if !declared.contains(&v) {
                        self.errors.push(SemanticError::UndefinedVariable {
                            name: v,
                            defined_vars: declared.to_vec(),
                            span: value_span(condition),
                        });
                    }
                }
                for s in then_branch {
                    self.check_stmt_vars(s, declared);
                }
                for s in else_branch {
                    self.check_stmt_vars(s, declared);
                }
            }
            StoryStmt::Each { source, body, .. } => {
                for v in extract_variables(source) {
                    if !declared.contains(&v) {
                        self.errors.push(SemanticError::UndefinedVariable {
                            name: v,
                            defined_vars: declared.to_vec(),
                            span: value_span(source),
                        });
                    }
                }
                for s in body {
                    self.check_stmt_vars(s, declared);
                }
            }
            StoryStmt::Choice { options, .. } => {
                for opt in options {
                    for s in &opt.body {
                        self.check_stmt_vars(s, declared);
                    }
                }
            }
            StoryStmt::Speaker { name, .. } => {
                for v in extract_variables(name) {
                    if !declared.contains(&v) {
                        self.errors.push(SemanticError::UndefinedVariable {
                            name: v,
                            defined_vars: declared.to_vec(),
                            span: value_span(name),
                        });
                    }
                }
            }
            StoryStmt::Run { .. } => {}
            _ => {}
        }
    }

    fn check_choice_has_options(&mut self, stmts: &[StoryStmt]) {
        for stmt in stmts {
            match stmt {
                StoryStmt::Choice { options, span } => {
                    if options.is_empty() {
                        self.errors
                            .push(SemanticError::EmptyChoice { span: span.clone() });
                    }
                    for opt in options {
                        self.check_choice_has_options(&opt.body);
                    }
                }
                StoryStmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.check_choice_has_options(then_branch);
                    self.check_choice_has_options(else_branch);
                }
                StoryStmt::Each { body, .. } => self.check_choice_has_options(body),
                StoryStmt::Run { .. } => {}
                _ => {}
            }
        }
    }

    fn check_style_cycles(&mut self, styles: &[Style]) {
        let style_names: HashSet<&str> = styles.iter().map(|s| s.name.as_str()).collect();
        for style in styles {
            if let Some(ref parent) = style.extends {
                if !style_names.contains(parent.as_str()) {
                    self.errors.push(SemanticError::MissingStyleParent {
                        parent: parent.clone(),
                        span: style.span.clone(),
                    });
                }
            }
        }
        for style in styles {
            let mut visited = Vec::new();
            if self.detect_cycle(style, styles, &mut visited) {
                self.errors.push(SemanticError::CircularStyleInheritance {
                    chain: visited,
                    span: style.span.clone(),
                });
            }
        }
    }

    fn detect_cycle(&self, style: &Style, all: &[Style], visited: &mut Vec<String>) -> bool {
        visited.push(style.name.clone());
        if let Some(ref parent) = style.extends {
            if visited.contains(parent) {
                visited.push(parent.clone());
                return true;
            }
            if let Some(parent_style) = all.iter().find(|s| &s.name == parent) {
                if self.detect_cycle(parent_style, all, visited) {
                    return true;
                }
            }
        }
        visited.pop();
        false
    }

    fn check_uniqueness(&mut self, themes: &[Theme], styles: &[Style]) {
        let mut seen = HashSet::default();
        for t in themes {
            if !seen.insert(&t.name) {
                self.errors.push(SemanticError::DuplicateName {
                    name: t.name.clone(),
                    kind: "@theme".into(),
                    span: t.span.clone(),
                });
            }
        }
        seen.clear();
        for s in styles {
            if !seen.insert(&s.name) {
                self.errors.push(SemanticError::DuplicateName {
                    name: s.name.clone(),
                    kind: "@style".into(),
                    span: s.span.clone(),
                });
            }
        }
    }
}

fn value_span(expr: &Expression) -> SourceSpan {
    match expr {
        Expression::BinaryOp { left, .. } => value_span(left),
        Expression::TernaryOp { condition, .. } => value_span(condition),
        _ => SourceSpan::point("", 0, 0),
    }
}

fn merge_span(start: &SourceSpan, end: &SourceSpan) -> SourceSpan {
    SourceSpan::new(
        &start.file,
        start.line_start,
        start.col_start,
        end.line_end,
        end.col_end,
        start.byte_start,
        end.byte_end,
    )
}

/// Convert a parsed `text()`/`button()` positional argument into a
/// [`LocalizedText`]. A plain `"…"` string becomes `Plain`; an `@t("en",
/// "中文")` literal becomes `Localized`; anything else (or absent) yields an
/// empty `Plain` (matching the previous narrow-to-string behavior).
fn localized_from_expr(expr: Option<Expression>) -> LocalizedText {
    match expr {
        Some(Expression::StringLit(s)) => LocalizedText::Plain(s),
        Some(Expression::Localized(pairs)) => LocalizedText::Localized(pairs),
        _ => LocalizedText::Plain(String::new()),
    }
}

fn extract_variables(expr: &Expression) -> Vec<String> {
    let mut vars = Vec::new();
    match expr {
        Expression::Variable(name) => vars.push(name.clone()),
        Expression::BinaryOp { left, right, .. } => {
            vars.extend(extract_variables(left));
            vars.extend(extract_variables(right));
        }
        Expression::TernaryOp {
            condition,
            then_expr,
            else_expr,
        } => {
            vars.extend(extract_variables(condition));
            vars.extend(extract_variables(then_expr));
            vars.extend(extract_variables(else_expr));
        }
        Expression::ObjectLit(fields) => {
            for (_, v) in fields {
                vars.extend(extract_variables(v));
            }
        }
        Expression::Call { args, .. } => {
            for a in args {
                vars.extend(extract_variables(a));
            }
        }
        Expression::ArrayLit(elements) => {
            for e in elements {
                vars.extend(extract_variables(e));
            }
        }
        Expression::UnaryOp { operand, .. } => {
            vars.extend(extract_variables(operand));
        }
        _ => {}
    }
    vars
}

/// Validate a custom-component use site against its `component` declaration:
/// no undeclared props, all `required` props present, value kinds matching.
fn validate_custom_props(
    decl: &ComponentDecl,
    props: &ComponentProps,
    use_span: &SourceSpan,
) -> Result<(), ParseError> {
    let declared: HashSet<&str> = decl.props.iter().map(|p| p.name.as_str()).collect();
    for key in props.custom.keys() {
        if !declared.contains(key.as_str()) {
            return Err(ParseError::UnknownProp {
                component: decl.name.clone(),
                prop: key.clone(),
                valid: decl.props.iter().map(|p| p.name.clone()).collect(),
                span: use_span.clone(),
            });
        }
    }
    for pd in &decl.props {
        match props.custom.get(&pd.name) {
            Some(expr) => {
                if !expr_matches_kind(expr, pd.kind) {
                    return Err(ParseError::PropTypeMismatch {
                        component: decl.name.clone(),
                        prop: pd.name.clone(),
                        expected: pd.kind.name().to_string(),
                        span: use_span.clone(),
                    });
                }
            }
            None => {
                if pd.required && !routed_prop_present(props, &pd.name) {
                    return Err(ParseError::MissingRequiredProp {
                        component: decl.name.clone(),
                        prop: pd.name.clone(),
                        span: use_span.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Whether `expr` is acceptable for a prop declared with `kind`.
///
/// `Expr` admits what a data binding can carry: a number literal or a
/// (possibly templated, `"{var}"`) string, or a bare variable reference.
fn expr_matches_kind(expr: &Expression, kind: PropKind) -> bool {
    match kind {
        PropKind::Int => matches!(expr, Expression::NumberLit(_)),
        PropKind::String => matches!(expr, Expression::StringLit(_) | Expression::Localized(_)),
        PropKind::Bool => matches!(expr, Expression::BoolLit(_)),
        PropKind::Color => matches!(expr, Expression::StringLit(_)),
        PropKind::Expr => matches!(
            expr,
            Expression::NumberLit(_) | Expression::StringLit(_) | Expression::Variable(_)
        ),
    }
}

/// Whether a prop with this name was routed into one of `ComponentProps`'
/// named fields by `parse_component_props` (rather than the `custom` map) —
/// needed when a declared prop shares a name with a standard layout prop
/// (e.g. `color`).
fn routed_prop_present(props: &ComponentProps, name: &str) -> bool {
    match name {
        "width" => props.width.is_some(),
        "height" => props.height.is_some(),
        "color" => props.color.is_some(),
        "value" => props.value.is_some(),
        "style" => props.style.is_some(),
        "font" => props.font.is_some(),
        "wrap" => props.wrap.is_some(),
        "orientation" => props.orientation.is_some(),
        "footer" => props.footer.is_some(),
        "palette" => props.palette.is_some(),
        "line_spacing" => props.line_spacing.is_some(),
        "scale" => props.scale.is_some(),
        "repeat" => props.repeat.is_some(),
        "max_visible" => props.max_visible.is_some(),
        "gap" => props.gap.is_some(),
        "clip" => props.clip.is_some(),
        "flip_x" => props.flip_x.is_some(),
        "flip_y" => props.flip_y.is_some(),
        "cursor" => props.cursor.is_some(),
        "selected" => props.selected.is_some(),
        "tile_id" => props.tile_id.is_some(),
        "tiles" => props.tiles.is_some(),
        "item_template" => props.item_template.is_some(),
        _ => false,
    }
}

pub fn parse(tokens: Vec<SpannedToken>) -> (Option<Document>, Vec<ParseError>) {
    Parser::new(tokens, "").parse()
}

/// Parse with a pre-registered set of component declarations (a shared
/// prelude such as `ui_layouts/components.gui`).
pub fn parse_with_components(
    tokens: Vec<SpannedToken>,
    decls: &[ComponentDecl],
) -> (Option<Document>, Vec<ParseError>) {
    Parser::new(tokens, "").with_components(decls).parse()
}

pub fn parse_and_validate(
    tokens: Vec<SpannedToken>,
    source: &str,
) -> (Option<Document>, Vec<ParseError>, Vec<SemanticError>) {
    let (doc, parse_errors) = Parser::new(tokens, source).parse();
    let mut semantic_errors = Vec::new();
    if let Some(Document::Scene(ref scene)) = doc {
        let mut validator = SemanticValidator::new();
        validator.validate_scene(scene);
        semantic_errors = validator.errors;
    }
    (doc, parse_errors, semantic_errors)
}

/// `parse_and_validate` with a pre-registered component prelude (e.g. the
/// shared `components.gui` declarations), so screens may reference custom
/// component types without declaring them locally.
pub fn parse_and_validate_with_components(
    tokens: Vec<SpannedToken>,
    source: &str,
    decls: &[ComponentDecl],
) -> (Option<Document>, Vec<ParseError>, Vec<SemanticError>) {
    let (doc, parse_errors) = Parser::new(tokens, source).with_components(decls).parse();
    let mut semantic_errors = Vec::new();
    if let Some(Document::Scene(ref scene)) = doc {
        let mut validator = SemanticValidator::new();
        validator.validate_scene(scene);
        semantic_errors = validator.errors;
    }
    (doc, parse_errors, semantic_errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_span() -> SourceSpan {
        SourceSpan::new("test", 1, 1, 1, 5, 0, 0)
    }
    fn tok(t: Token) -> SpannedToken {
        SpannedToken {
            token: t,
            span: dummy_span(),
        }
    }
    fn id(s: &str) -> Token {
        Token::Identifier(s.into())
    }
    fn s_(s: &str) -> Token {
        Token::StringLit(s.into())
    }
    fn n(n: f64) -> Token {
        Token::NumberLit(n)
    }

    #[test]
    fn test_parse_empty_scene() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Empty")),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty());
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert_eq!(s.name, "Empty");
    }

    #[test]
    fn test_parse_variables() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Test")),
            tok(Token::LBrace),
            tok(Token::DirectiveVariables),
            tok(Token::LBrace),
            tok(id("gold")),
            tok(Token::Equals),
            tok(n(500.0)),
            tok(Token::Newline),
            tok(id("name")),
            tok(Token::Equals),
            tok(s_("Hero")),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert_eq!(s.variables.unwrap().decls.len(), 2);
    }

    #[test]
    fn test_parse_speaker() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Dialog")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveSpeaker),
            tok(Token::LParen),
            tok(s_("Prof")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(s_("Hello!")),
            tok(s_("Welcome!")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Speaker { texts, .. } => assert_eq!(texts.len(), 2),
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_say_cutscene_line() {
        // `@say("Prof") { "text" }` parses as StoryStmt::Say (cutscene speech),
        // distinct from @speaker (player-initiated talk).
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Dialog")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveSay),
            tok(Token::LParen),
            tok(s_("Prof")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(s_("Hello!")),
            tok(s_("Welcome!")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        match &s.storylines[0].statements[0] {
            StoryStmt::Say { texts, .. } => assert_eq!(texts.len(), 2),
            other => panic!("`@say` must parse as StoryStmt::Say, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_speaker_rejects_second_argument() {
        // @speaker's meaning is fixed to player-initiated talk — a mode
        // argument is no longer accepted; cutscene speech uses @say instead.
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Dialog")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveSpeaker),
            tok(Token::LParen),
            tok(s_("Prof")),
            tok(Token::Comma),
            tok(id("auto")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(s_("Hello!")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(
            doc.is_none() || !errors.is_empty(),
            "`@speaker(name, mode)` must be rejected; use @say for cutscene speech"
        );
    }

    #[test]
    fn test_parse_choice() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("ChoiceTest")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveChoice),
            tok(Token::LBrace),
            tok(Token::DirectiveOption),
            tok(Token::LParen),
            tok(s_("Yes")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::DirectiveOption),
            tok(Token::LParen),
            tok(s_("No")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Choice { options, .. } => assert_eq!(options.len(), 2),
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_if_else() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("IfTest")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveIf),
            tok(Token::LParen),
            tok(id("gold")),
            tok(Token::Gt),
            tok(n(100.0)),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::DirectiveSpeaker),
            tok(Token::LParen),
            tok(s_("NPC")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(s_("Rich!")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::DirectiveElse),
            tok(Token::LBrace),
            tok(Token::DirectiveSpeaker),
            tok(Token::LParen),
            tok(s_("NPC")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(s_("Poor!")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                assert!(!then_branch.is_empty());
                assert!(!else_branch.is_empty());
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_nested_choice_in_if() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Nested")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveIf),
            tok(Token::LParen),
            tok(id("flag")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::DirectiveChoice),
            tok(Token::LBrace),
            tok(Token::DirectiveOption),
            tok(Token::LParen),
            tok(s_("A")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::If { then_branch, .. } => {
                assert!(matches!(then_branch[0], StoryStmt::Choice { .. }))
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_ui_panel() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("UITest")),
            tok(Token::LBrace),
            tok(Token::KeywordUi),
            tok(Token::LBrace),
            tok(id("panel")),
            tok(Token::LBrace),
            tok(id("title")),
            tok(Token::Equals),
            tok(id("text")),
            tok(Token::LParen),
            tok(s_("Shop")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert!(matches!(
            s.ui.unwrap().components[0],
            UiComponent::Panel { .. }
        ));
    }

    #[test]
    fn test_parse_theme_style() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("ThemeTest")),
            tok(Token::LBrace),
            tok(Token::DirectiveTheme),
            tok(id("dark")),
            tok(Token::LBrace),
            tok(id("primary")),
            tok(Token::Equals),
            tok(s_("#c9a03d")),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::DirectiveStyle),
            tok(id("base")),
            tok(Token::LBrace),
            tok(id("padding")),
            tok(Token::Equals),
            tok(n(12.0)),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::DirectiveStyle),
            tok(id("child")),
            tok(Token::Colon),
            tok(id("base")),
            tok(Token::LBrace),
            tok(id("color")),
            tok(Token::Equals),
            tok(s_("red")),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert_eq!(s.themes.len(), 1);
        assert_eq!(s.styles.len(), 2);
        assert_eq!(s.styles[1].extends.as_deref(), Some("base"));
    }

    #[test]
    fn test_parse_atlas() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("AtlasTest")),
            tok(Token::LBrace),
            tok(Token::DirectiveAtlas),
            tok(s_("ui_atlas")),
            tok(Token::LBrace),
            tok(id("source")),
            tok(Token::Equals),
            tok(s_("atlas.png")),
            tok(Token::Newline),
            tok(id("regions")),
            tok(Token::Equals),
            tok(Token::LBrace),
            tok(id("btn")),
            tok(Token::Equals),
            tok(Token::LBracket),
            tok(n(0.0)),
            tok(Token::Comma),
            tok(n(0.0)),
            tok(Token::Comma),
            tok(n(64.0)),
            tok(Token::Comma),
            tok(n(64.0)),
            tok(Token::Comma),
            tok(id("slice")),
            tok(Token::Equals),
            tok(n(8.0)),
            tok(Token::RBracket),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert_eq!(s.atlases[0].regions[0].nine_slice, Some([8, 8, 8, 8]));
    }

    #[test]
    fn test_parse_full_scene() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("FullScene")),
            tok(Token::LBrace),
            tok(Token::DirectiveVariables),
            tok(Token::LBrace),
            tok(id("gold")),
            tok(Token::Equals),
            tok(n(500.0)),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveSpeaker),
            tok(Token::LParen),
            tok(s_("Prof")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(s_("Hello!")),
            tok(Token::RBrace),
            tok(Token::DirectiveChoice),
            tok(Token::LBrace),
            tok(Token::DirectiveOption),
            tok(Token::LParen),
            tok(s_("Buy")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(id("gold")),
            tok(Token::Equals),
            tok(id("gold")),
            tok(Token::Minus),
            tok(n(100.0)),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::KeywordUi),
            tok(Token::LBrace),
            tok(id("panel")),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::DirectiveTheme),
            tok(id("default")),
            tok(Token::LBrace),
            tok(id("bg")),
            tok(Token::Equals),
            tok(s_("#000")),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::DirectiveStyle),
            tok(id("main")),
            tok(Token::LBrace),
            tok(id("pad")),
            tok(Token::Equals),
            tok(n(10.0)),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::DirectiveAtlas),
            tok(s_("ui")),
            tok(Token::LBrace),
            tok(id("source")),
            tok(Token::Equals),
            tok(s_("ui.png")),
            tok(Token::Newline),
            tok(id("regions")),
            tok(Token::Equals),
            tok(Token::LBrace),
            tok(id("btn")),
            tok(Token::Equals),
            tok(Token::LBracket),
            tok(n(0.0)),
            tok(Token::Comma),
            tok(n(0.0)),
            tok(Token::Comma),
            tok(n(64.0)),
            tok(Token::Comma),
            tok(n(64.0)),
            tok(Token::RBracket),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        assert!(doc.is_some());
    }

    #[test]
    fn test_parse_each() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("LoopTest")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveEach),
            tok(id("item")),
            tok(id("in")),
            tok(id("items")),
            tok(Token::LBrace),
            tok(id("count")),
            tok(Token::Equals),
            tok(n(1.0)),
            tok(Token::Plus),
            tok(n(1.0)),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        match &s.storylines[0].statements[0] {
            StoryStmt::Each { item_var, body, .. } => {
                assert_eq!(item_var, "item");
                assert_eq!(body.len(), 1);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_expression_binary() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("ExprTest")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(id("result")),
            tok(Token::Equals),
            tok(id("x")),
            tok(Token::Plus),
            tok(n(5.0)),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        match &s.storylines[0].statements[0] {
            StoryStmt::Assign { value, .. } => {
                assert!(matches!(value, Expression::BinaryOp { .. }))
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_semantic_undefined_variable() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Undef")),
            tok(Token::LBrace),
            tok(Token::DirectiveVariables),
            tok(Token::LBrace),
            tok(id("gold")),
            tok(Token::Equals),
            tok(n(500.0)),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveIf),
            tok(Token::LParen),
            tok(id("undefined_var")),
            tok(Token::Gt),
            tok(n(100.0)),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, _pe, se) = parse_and_validate(tokens, "");
        assert!(se.iter().any(|e| matches!(e, SemanticError::UndefinedVariable { name, .. } if name == "undefined_var")),
            "sem errors: {:?}", se);
    }

    #[test]
    fn test_semantic_circular_style() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Circ")),
            tok(Token::LBrace),
            tok(Token::DirectiveStyle),
            tok(id("A")),
            tok(Token::Colon),
            tok(id("B")),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::DirectiveStyle),
            tok(id("B")),
            tok(Token::Colon),
            tok(id("A")),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, _pe, se) = parse_and_validate(tokens, "");
        assert!(
            se.iter()
                .any(|e| matches!(e, SemanticError::CircularStyleInheritance { .. })),
            "sem errors: {:?}",
            se
        );
    }

    #[test]
    fn test_semantic_empty_choice() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("EmptyC")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveChoice),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, _pe, se) = parse_and_validate(tokens, "");
        assert!(
            se.iter()
                .any(|e| matches!(e, SemanticError::EmptyChoice { .. })),
            "sem errors: {:?}",
            se
        );
    }

    #[test]
    fn test_syntax_error_recovery() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Errors")),
            tok(Token::LBrace),
            tok(Token::KeywordUi),
            tok(Token::LBrace),
            tok(id("bad_widget")),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::DirectiveTheme),
            tok(id("ok")),
            tok(Token::LBrace),
            tok(id("clr")),
            tok(Token::Equals),
            tok(s_("#fff")),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.iter().any(|e| matches!(e, ParseError::InvalidComponentType { found, .. } if found == "bad_widget")),
            "errors: {:?}", errors);
        // The error should be collected even if doc becomes None due to error propagation
        assert!(doc.is_some() || !errors.is_empty());
    }

    #[test]
    fn test_unicode_identifiers() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("场景")),
            tok(Token::LBrace),
            tok(Token::DirectiveVariables),
            tok(Token::LBrace),
            tok(id("名称")),
            tok(Token::Equals),
            tok(s_("小明")),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveSpeaker),
            tok(Token::LParen),
            tok(s_("博士")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(s_("こんにちは")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert_eq!(s.name, "场景");
    }

    #[test]
    fn test_semantic_duplicate_theme() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Dup")),
            tok(Token::LBrace),
            tok(Token::DirectiveTheme),
            tok(id("same")),
            tok(Token::LBrace),
            tok(id("a")),
            tok(Token::Equals),
            tok(s_("#000")),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::DirectiveTheme),
            tok(id("same")),
            tok(Token::LBrace),
            tok(id("b")),
            tok(Token::Equals),
            tok(s_("#fff")),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, _pe, se) = parse_and_validate(tokens, "");
        assert!(se.iter().any(|e| matches!(e, SemanticError::DuplicateName { name, kind, .. } if name == "same" && kind == "@theme")),
            "sem errors: {:?}", se);
    }

    #[test]
    fn test_semantic_missing_style_parent() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Bad")),
            tok(Token::LBrace),
            tok(Token::DirectiveStyle),
            tok(id("child")),
            tok(Token::Colon),
            tok(id("nonexistent")),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, _pe, se) = parse_and_validate(tokens, "");
        assert!(se.iter().any(|e| matches!(e, SemanticError::MissingStyleParent { parent, .. } if parent == "nonexistent")),
            "sem errors: {:?}", se);
    }

    #[test]
    fn test_parse_screen() {
        let tokens = vec![
            tok(Token::KeywordScreen),
            tok(id("MainMenu")),
            tok(Token::LBrace),
            tok(id("panel")),
            tok(Token::LBrace),
            tok(id("text")),
            tok(Token::LParen),
            tok(s_("Title")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        match doc.unwrap() {
            Document::Screen(s) => {
                assert_eq!(s.name, "MainMenu");
                assert_eq!(s.components.len(), 1);
            }
            _ => panic!(),
        }
    }

    // ──────────────── NEW VALID INPUT TESTS ────────────────

    #[test]
    fn test_parse_variables_mixed() {
        // number, string, bool variables
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Mixed")),
            tok(Token::LBrace),
            tok(Token::DirectiveVariables),
            tok(Token::LBrace),
            tok(id("gold")),
            tok(Token::Equals),
            tok(n(500.0)),
            tok(Token::Newline),
            tok(id("name")),
            tok(Token::Equals),
            tok(s_("Hero")),
            tok(Token::Newline),
            tok(id("active")),
            tok(Token::Equals),
            tok(Token::BoolLit(true)),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let vb = s.variables.unwrap();
        assert_eq!(vb.decls.len(), 3);
        assert!(matches!(vb.decls[0].value, Expression::NumberLit(500.0)));
        assert!(matches!(&vb.decls[1].value, Expression::StringLit(n) if n == "Hero"));
        assert!(matches!(vb.decls[2].value, Expression::BoolLit(true)));
    }

    #[test]
    fn test_parse_choice_three_options() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("ThreeOpts")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveChoice),
            tok(Token::LBrace),
            tok(Token::DirectiveOption),
            tok(Token::LParen),
            tok(s_("A")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::DirectiveOption),
            tok(Token::LParen),
            tok(s_("B")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::DirectiveOption),
            tok(Token::LParen),
            tok(s_("C")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Choice { options, .. } => {
                assert_eq!(options.len(), 3);
                assert_eq!(options[0].label, "A");
                assert_eq!(options[1].label, "B");
                assert_eq!(options[2].label, "C");
            }
            _ => panic!("expected Choice, got {:?}", sb.statements[0]),
        }
    }

    #[test]
    fn test_parse_choice_option_empty_body() {
        // options with empty body {} (no statements inside)
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("EmptyBody")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveChoice),
            tok(Token::LBrace),
            tok(Token::DirectiveOption),
            tok(Token::LParen),
            tok(s_("Skip")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::DirectiveOption),
            tok(Token::LParen),
            tok(s_("Pass")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Choice { options, .. } => {
                assert_eq!(options.len(), 2);
                assert!(options[0].body.is_empty());
                assert!(options[1].body.is_empty());
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_if_else_if_chain() {
        // @if / @else @if / @else chain
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Chain")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveIf),
            tok(Token::LParen),
            tok(id("a")),
            tok(Token::Gt),
            tok(n(10.0)),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(id("x")),
            tok(Token::Equals),
            tok(n(1.0)),
            tok(Token::RBrace),
            tok(Token::DirectiveElse),
            tok(Token::DirectiveIf),
            tok(Token::LParen),
            tok(id("a")),
            tok(Token::Gt),
            tok(n(5.0)),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(id("x")),
            tok(Token::Equals),
            tok(n(2.0)),
            tok(Token::RBrace),
            tok(Token::DirectiveElse),
            tok(Token::LBrace),
            tok(id("x")),
            tok(Token::Equals),
            tok(n(3.0)),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                assert!(!then_branch.is_empty());
                assert!(!else_branch.is_empty());
                // else_branch should contain another If stmt (the elif)
                assert!(matches!(else_branch[0], StoryStmt::If { .. }));
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn test_parse_nested_if_in_choice_option() {
        // @choice → @option → @if inside option body
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("NestedIfOpt")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveChoice),
            tok(Token::LBrace),
            tok(Token::DirectiveOption),
            tok(Token::LParen),
            tok(s_("Check")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::DirectiveIf),
            tok(Token::LParen),
            tok(id("cond")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(id("result")),
            tok(Token::Equals),
            tok(n(42.0)),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Choice { options, .. } => {
                assert_eq!(options.len(), 1);
                assert_eq!(options[0].body.len(), 1);
                assert!(matches!(options[0].body[0], StoryStmt::If { .. }));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_each_with_source_expression() {
        // @each with variable and complex source expression
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("EachExpr")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveEach),
            tok(id("item")),
            tok(id("in")),
            tok(id("items")),
            tok(Token::Plus),
            tok(id("bonus")),
            tok(Token::LBrace),
            tok(id("count")),
            tok(Token::Equals),
            tok(id("count")),
            tok(Token::Plus),
            tok(n(1.0)),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Each {
                item_var,
                source,
                body,
                ..
            } => {
                assert_eq!(item_var, "item");
                assert!(matches!(source, Expression::BinaryOp { .. }));
                assert_eq!(body.len(), 1);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_expression_complex_nested() {
        // a + b * c - d / e (tests operator precedence: * and / > + and -)
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Complex")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(id("result")),
            tok(Token::Equals),
            tok(id("a")),
            tok(Token::Plus),
            tok(id("b")),
            tok(Token::Star),
            tok(id("c")),
            tok(Token::Minus),
            tok(id("d")),
            tok(Token::Slash),
            tok(id("e")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Assign { value, .. } => {
                // Should be: (a + (b * c)) - (d / e)
                assert!(matches!(value, Expression::BinaryOp { op: BinOp::Sub, .. }));
                if let Expression::BinaryOp {
                    op: BinOp::Sub,
                    left,
                    right,
                } = value
                {
                    // left = a + (b * c)
                    assert!(matches!(
                        **left,
                        Expression::BinaryOp { op: BinOp::Add, .. }
                    ));
                    // right = d / e
                    assert!(matches!(
                        **right,
                        Expression::BinaryOp { op: BinOp::Div, .. }
                    ));
                }
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_expression_unary_negation() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Negation")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(id("temp")),
            tok(Token::Equals),
            tok(Token::Minus),
            tok(n(10.0)),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Assign { value, .. } => {
                assert!(matches!(
                    value,
                    Expression::UnaryOp {
                        op: UnaryOp::Neg,
                        ..
                    }
                ));
                if let Expression::UnaryOp { op, operand } = value {
                    assert_eq!(*op, UnaryOp::Neg);
                    assert!(matches!(**operand, Expression::NumberLit(10.0)));
                }
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_expression_parens() {
        // (a + b) * c  — parentheses override precedence
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("ParenExpr")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(id("result")),
            tok(Token::Equals),
            tok(Token::LParen),
            tok(id("a")),
            tok(Token::Plus),
            tok(id("b")),
            tok(Token::RParen),
            tok(Token::Star),
            tok(id("c")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Assign { value, .. } => {
                assert!(matches!(value, Expression::BinaryOp { op: BinOp::Mul, .. }));
                if let Expression::BinaryOp { left, .. } = value {
                    assert!(matches!(
                        **left,
                        Expression::BinaryOp { op: BinOp::Add, .. }
                    ));
                }
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_expression_comparison_chain() {
        // a > b && b < c  (chained comparisons with AND)
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Cmp")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(id("ok")),
            tok(Token::Equals),
            tok(id("a")),
            tok(Token::Gt),
            tok(id("b")),
            tok(Token::AndAnd),
            tok(id("b")),
            tok(Token::Lt),
            tok(id("c")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Assign { value, .. } => {
                assert!(matches!(value, Expression::BinaryOp { op: BinOp::And, .. }));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_ui_all_components() {
        // all 8 component types: panel, container, text, button, list, image, input, dropdown
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("AllUI")),
            tok(Token::LBrace),
            tok(Token::KeywordUi),
            tok(Token::LBrace),
            tok(id("panel")),
            tok(Token::LBrace),
            tok(id("title")),
            tok(Token::Equals),
            tok(id("text")),
            tok(Token::LParen),
            tok(s_("Hello")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(id("child")),
            tok(Token::Equals),
            tok(id("panel")),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(id("container")),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(id("text")),
            tok(Token::LParen),
            tok(s_("World")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(id("visible")),
            tok(Token::Equals),
            tok(Token::BoolLit(true)),
            tok(Token::RBrace),
            tok(id("button")),
            tok(Token::LParen),
            tok(s_("OK")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(id("on_click")),
            tok(Token::Equals),
            tok(s_("handle_ok")),
            tok(Token::RBrace),
            tok(id("list")),
            tok(Token::LBrace),
            tok(id("source")),
            tok(Token::Equals),
            tok(id("items")),
            tok(Token::RBrace),
            tok(id("image")),
            tok(Token::LParen),
            tok(s_("sprite.png")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(id("width")),
            tok(Token::Equals),
            tok(n(64.0)),
            tok(Token::RBrace),
            tok(id("input")),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(id("dropdown")),
            tok(Token::LBrace),
            tok(id("on_click")),
            tok(Token::Equals),
            tok(s_("handle_select")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let ui = s.ui.unwrap();
        assert_eq!(ui.components.len(), 8, "expected 8 top-level components");
        // check each type
        let types: Vec<&str> = ui
            .components
            .iter()
            .map(|c| match c {
                UiComponent::Panel { .. } => "panel",
                UiComponent::Container { .. } => "container",
                UiComponent::Text { .. } => "text",
                UiComponent::Button { .. } => "button",
                UiComponent::List { .. } => "list",
                UiComponent::Image { .. } => "image",
                UiComponent::Input { .. } => "input",
                UiComponent::Dropdown { .. } => "dropdown",
                UiComponent::Tile { .. } => "tile",
                UiComponent::Divider { .. } => "divider",
                UiComponent::FlexList { .. } => "flex_list",
                UiComponent::Cursor { .. } => "cursor",
                UiComponent::Bracket { .. } => "bracket",
                UiComponent::PixelRect { .. } => "pixel_rect",
                UiComponent::Custom { .. } => "custom",
            })
            .collect();
        assert!(types.contains(&"panel"));
        assert!(types.contains(&"container"));
        assert!(types.contains(&"text"));
        assert!(types.contains(&"button"));
        assert!(types.contains(&"list"));
        assert!(types.contains(&"image"));
        assert!(types.contains(&"input"));
        assert!(types.contains(&"dropdown"));
    }

    #[test]
    fn test_parse_theme_many_tokens() {
        // theme with 5+ color tokens
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("ManyColors")),
            tok(Token::LBrace),
            tok(Token::DirectiveTheme),
            tok(id("dark")),
            tok(Token::LBrace),
            tok(id("primary")),
            tok(Token::Equals),
            tok(s_("#c9a03d")),
            tok(Token::Newline),
            tok(id("background")),
            tok(Token::Equals),
            tok(s_("#1a1a2e")),
            tok(Token::Newline),
            tok(id("text")),
            tok(Token::Equals),
            tok(s_("#ffffff")),
            tok(Token::Newline),
            tok(id("accent")),
            tok(Token::Equals),
            tok(s_("#ff6b6b")),
            tok(Token::Newline),
            tok(id("border")),
            tok(Token::Equals),
            tok(s_("#444444")),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert_eq!(s.themes.len(), 1);
        assert_eq!(s.themes[0].tokens.len(), 5);
        assert_eq!(
            s.themes[0].tokens.get("primary").map(|s| s.as_str()),
            Some("#c9a03d")
        );
        assert_eq!(
            s.themes[0].tokens.get("accent").map(|s| s.as_str()),
            Some("#ff6b6b")
        );
        assert_eq!(
            s.themes[0].tokens.get("border").map(|s| s.as_str()),
            Some("#444444")
        );
    }

    #[test]
    fn test_parse_style_inheritance_chain() {
        // A : B, B : C — three-level chain (A extends B, B extends C)
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("StyleChain")),
            tok(Token::LBrace),
            tok(Token::DirectiveStyle),
            tok(id("C")),
            tok(Token::LBrace),
            tok(id("pad")),
            tok(Token::Equals),
            tok(n(4.0)),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::DirectiveStyle),
            tok(id("B")),
            tok(Token::Colon),
            tok(id("C")),
            tok(Token::LBrace),
            tok(id("pad")),
            tok(Token::Equals),
            tok(n(8.0)),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::DirectiveStyle),
            tok(id("A")),
            tok(Token::Colon),
            tok(id("B")),
            tok(Token::LBrace),
            tok(id("pad")),
            tok(Token::Equals),
            tok(n(12.0)),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert_eq!(s.styles.len(), 3);
        assert_eq!(s.styles[0].name, "C");
        assert!(s.styles[0].extends.is_none());
        assert_eq!(s.styles[1].name, "B");
        assert_eq!(s.styles[1].extends.as_deref(), Some("C"));
        assert_eq!(s.styles[2].name, "A");
        assert_eq!(s.styles[2].extends.as_deref(), Some("B"));
    }

    #[test]
    fn test_parse_atlas_three_regions() {
        // atlas with 3 regions
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Atlas3")),
            tok(Token::LBrace),
            tok(Token::DirectiveAtlas),
            tok(s_("ui")),
            tok(Token::LBrace),
            tok(id("source")),
            tok(Token::Equals),
            tok(s_("atlas.png")),
            tok(Token::Newline),
            tok(id("regions")),
            tok(Token::Equals),
            tok(Token::LBrace),
            // region 1
            tok(id("btn_normal")),
            tok(Token::Equals),
            tok(Token::LBracket),
            tok(n(0.0)),
            tok(Token::Comma),
            tok(n(0.0)),
            tok(Token::Comma),
            tok(n(64.0)),
            tok(Token::Comma),
            tok(n(64.0)),
            tok(Token::Comma),
            tok(id("slice")),
            tok(Token::Equals),
            tok(n(8.0)),
            tok(Token::RBracket),
            tok(Token::Newline),
            // region 2
            tok(id("btn_hover")),
            tok(Token::Equals),
            tok(Token::LBracket),
            tok(n(64.0)),
            tok(Token::Comma),
            tok(n(0.0)),
            tok(Token::Comma),
            tok(n(64.0)),
            tok(Token::Comma),
            tok(n(64.0)),
            tok(Token::Comma),
            tok(id("slice")),
            tok(Token::Equals),
            tok(n(8.0)),
            tok(Token::RBracket),
            tok(Token::Newline),
            // region 3 (no slice)
            tok(id("btn_disabled")),
            tok(Token::Equals),
            tok(Token::LBracket),
            tok(n(128.0)),
            tok(Token::Comma),
            tok(n(0.0)),
            tok(Token::Comma),
            tok(n(64.0)),
            tok(Token::Comma),
            tok(n(64.0)),
            tok(Token::RBracket),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert_eq!(s.atlases.len(), 1);
        let atlas = &s.atlases[0];
        assert_eq!(atlas.regions.len(), 3);
        assert_eq!(atlas.regions[0].name, "btn_normal");
        assert_eq!(atlas.regions[0].nine_slice, Some([8, 8, 8, 8]));
        assert_eq!(atlas.regions[1].name, "btn_hover");
        assert_eq!(atlas.regions[1].nine_slice, Some([8, 8, 8, 8]));
        assert_eq!(atlas.regions[2].name, "btn_disabled");
        assert_eq!(atlas.regions[2].nine_slice, None);
    }

    #[test]
    fn test_parse_screen_with_theme() {
        // screen with theme reference (screen Main : dark)
        let tokens = vec![
            tok(Token::KeywordScreen),
            tok(id("MainMenu")),
            tok(Token::Colon),
            tok(id("dark")),
            tok(Token::LBrace),
            tok(id("panel")),
            tok(Token::LBrace),
            tok(id("text")),
            tok(Token::LParen),
            tok(s_("Title")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        match doc.unwrap() {
            Document::Screen(s) => {
                assert_eq!(s.name, "MainMenu");
                assert_eq!(s.theme.as_deref(), Some("dark"));
                assert_eq!(s.components.len(), 1);
            }
            _ => panic!("expected Screen"),
        }
    }

    #[test]
    fn test_parse_command_with_args() {
        // command statement with multiple args
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Cmd")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(id("give_item")),
            tok(Token::LParen),
            tok(s_("potion")),
            tok(Token::Comma),
            tok(n(3.0)),
            tok(Token::RParen),
            tok(Token::Newline),
            tok(id("set_flag")),
            tok(Token::LParen),
            tok(s_("found_rare")),
            tok(Token::RParen),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        assert_eq!(sb.statements.len(), 2);
        match &sb.statements[0] {
            StoryStmt::Command { name, args, .. } => {
                assert_eq!(name, "give_item");
                assert_eq!(args.len(), 2);
                assert!(matches!(&args[0], Expression::StringLit(s) if s == "potion"));
                assert!(matches!(args[1], Expression::NumberLit(3.0)));
            }
            _ => panic!("expected Command, got {:?}", sb.statements[0]),
        }
        match &sb.statements[1] {
            StoryStmt::Command { name, args, .. } => {
                assert_eq!(name, "set_flag");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn test_parse_directive_command_simple() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Cmd")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveCommand),
            tok(Token::LParen),
            tok(s_("heal")),
            tok(Token::RParen),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        assert_eq!(sb.statements.len(), 1);
        match &sb.statements[0] {
            StoryStmt::Command { name, args, .. } => {
                assert_eq!(name, "heal");
                assert!(args.is_empty());
            }
            _ => panic!("expected Command, got {:?}", sb.statements[0]),
        }
    }

    #[test]
    fn test_parse_directive_command_with_args() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Cmd")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveCommand),
            tok(Token::LParen),
            tok(s_("giveMonster")),
            tok(Token::Comma),
            tok(s_("SPARKIT")),
            tok(Token::Comma),
            tok(n(5.0)),
            tok(Token::RParen),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Command { name, args, .. } => {
                assert_eq!(name, "giveMonster");
                assert_eq!(args.len(), 2);
                assert!(matches!(&args[0], Expression::StringLit(s) if s == "SPARKIT"));
                assert!(matches!(args[1], Expression::NumberLit(5.0)));
            }
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn test_parse_directive_command_inside_choice() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Cmd")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveChoice),
            tok(Token::LBrace),
            tok(Token::DirectiveOption),
            tok(Token::LParen),
            tok(s_("Yes")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::DirectiveCommand),
            tok(Token::LParen),
            tok(s_("setFlag")),
            tok(Token::Comma),
            tok(s_("FLAG")),
            tok(Token::RParen),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Choice { options, .. } => {
                assert_eq!(options.len(), 1);
                assert_eq!(options[0].body.len(), 1);
                match &options[0].body[0] {
                    StoryStmt::Command { name, args, .. } => {
                        assert_eq!(name, "setFlag");
                        assert_eq!(args.len(), 1);
                    }
                    _ => panic!("expected Command inside option"),
                }
            }
            _ => panic!("expected Choice"),
        }
    }

    #[test]
    fn test_parse_directive_command_no_args_is_error() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Cmd")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveCommand),
            tok(Token::LParen),
            tok(Token::RParen),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, errors) = parse(tokens);
        assert!(
            !errors.is_empty(),
            "expected error for @command() with no args"
        );
    }

    #[test]
    fn test_parse_directive_command_non_string_first_arg_is_error() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Cmd")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveCommand),
            tok(Token::LParen),
            tok(n(42.0)),
            tok(Token::RParen),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, errors) = parse(tokens);
        assert!(
            !errors.is_empty(),
            "expected error for @command(42) with non-string first arg"
        );
    }

    #[test]
    fn test_bare_identifier_command_still_works() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Cmd")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::Identifier("heal".into())),
            tok(Token::LParen),
            tok(Token::RParen),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(
            errors.is_empty(),
            "bare identifier command should still work"
        );
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let sb = &s.storylines[0];
        match &sb.statements[0] {
            StoryStmt::Command { name, args, .. } => {
                assert_eq!(name, "heal");
                assert!(args.is_empty());
            }
            _ => panic!("expected Command from bare identifier"),
        }
    }

    // ──────────────── ERROR CONDITION TESTS ────────────────

    #[test]
    fn test_error_scene_missing_opening_brace() {
        // game_scene without { after name
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Bad")),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(!errors.is_empty(), "expected parse error, got none");
        assert!(doc.is_none(), "expected no document");
        // should be UnexpectedToken or UnexpectedEof (expecting {)
        let has_err = errors.iter().any(|e| {
            matches!(e, ParseError::UnexpectedToken { .. })
                || matches!(e, ParseError::UnexpectedEof { .. })
        });
        assert!(
            has_err,
            "expected UnexpectedToken or UnexpectedEof, got: {:?}",
            errors
        );
        // verify error has a span
        for err in &errors {
            let display = format!("{}", err);
            assert!(!display.is_empty(), "error should have a message");
        }
    }

    #[test]
    fn test_error_variables_without_block() {
        // @variables without { } block
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("BadVar")),
            tok(Token::LBrace),
            tok(Token::DirectiveVariables),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, errors) = parse(tokens);
        assert!(
            !errors.is_empty(),
            "expected parse error for missing variables block, got: {:?}",
            errors
        );
        let has_expected = errors
            .iter()
            .any(|e| matches!(e, ParseError::UnexpectedToken { .. }));
        assert!(has_expected, "expected UnexpectedToken, got: {:?}", errors);
    }

    #[test]
    fn test_error_bad_if_condition_empty() {
        // @if ( ) { } — empty condition expression
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("BadIf")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveIf),
            tok(Token::LParen),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, errors) = parse(tokens);
        assert!(
            !errors.is_empty(),
            "expected error for empty if condition, got none"
        );
        // The expression parser should error on the RParen
        let has_err = errors.iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("expected")
        });
        assert!(has_err, "expected 'expected' in error, got: {:?}", errors);
    }

    #[test]
    fn test_error_duplicate_style_name() {
        // two @style with same name
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("DupStyle")),
            tok(Token::LBrace),
            tok(Token::DirectiveStyle),
            tok(id("same")),
            tok(Token::LBrace),
            tok(id("a")),
            tok(Token::Equals),
            tok(n(1.0)),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::DirectiveStyle),
            tok(id("same")),
            tok(Token::LBrace),
            tok(id("b")),
            tok(Token::Equals),
            tok(n(2.0)),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, _pe, se) = parse_and_validate(tokens, "");
        assert!(
            !se.is_empty(),
            "expected semantic error for duplicate style, got none"
        );
        let has_dup = se.iter().any(|e| matches!(e, SemanticError::DuplicateName { name, kind, .. } if name == "same" && kind == "@style"));
        assert!(
            has_dup,
            "expected DuplicateName for @style 'same', got: {:?}",
            se
        );
    }

    #[test]
    fn test_error_style_self_reference() {
        // A : A — style references itself
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("SelfRef")),
            tok(Token::LBrace),
            tok(Token::DirectiveStyle),
            tok(id("A")),
            tok(Token::Colon),
            tok(id("A")),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, _pe, se) = parse_and_validate(tokens, "");
        assert!(
            !se.is_empty(),
            "expected semantic error for self-referencing style"
        );
        let has_cycle = se
            .iter()
            .any(|e| matches!(e, SemanticError::CircularStyleInheritance { .. }));
        assert!(
            has_cycle,
            "expected CircularStyleInheritance for A:A, got: {:?}",
            se
        );
    }

    #[test]
    fn test_error_unexpected_eof_in_expression() {
        // expression that ends prematurely — @if (a +   with no right operand
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("Truncated")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveIf),
            tok(Token::LParen),
            tok(id("a")),
            tok(Token::Plus),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, errors) = parse(tokens);
        assert!(
            !errors.is_empty(),
            "expected parse error for truncated expression"
        );
        // The expression parser tries to parse_factor after Plus, gets RParen
        let has_err = errors
            .iter()
            .any(|e| matches!(e, ParseError::UnexpectedToken { .. }));
        assert!(has_err, "expected UnexpectedToken, got: {:?}", errors);
    }

    #[test]
    fn test_error_invalid_top_level_keyword() {
        // unexpected keyword at top level
        let tokens = vec![
            tok(Token::KeywordUi),
            tok(id("Foo")),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(
            !errors.is_empty(),
            "expected error for invalid top-level keyword"
        );
        assert!(doc.is_none(), "expected no document");
    }

    #[test]
    fn test_error_invalid_story_stmt() {
        // unexpected token inside storylines
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("BadStmt")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::LBrace), // unexpected { in top-level story position
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, errors) = parse(tokens);
        assert!(
            !errors.is_empty(),
            "expected error for unexpected token in storylines"
        );
        let has_err = errors
            .iter()
            .any(|e| matches!(e, ParseError::UnexpectedToken { .. }));
        assert!(has_err, "expected UnexpectedToken, got: {:?}", errors);
    }

    #[test]
    fn test_error_variable_undefined_in_if() {
        // using undeclared variable in @if condition (semantic)
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("UndefVar")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveIf),
            tok(Token::LParen),
            tok(id("unknown")),
            tok(Token::Gt),
            tok(n(10.0)),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, _pe, se) = parse_and_validate(tokens, "");
        assert!(
            !se.is_empty(),
            "expected semantic error for undefined variable"
        );
        let has_undef = se.iter().any(
            |e| matches!(e, SemanticError::UndefinedVariable { name, .. } if name == "unknown"),
        );
        assert!(
            has_undef,
            "expected UndefinedVariable 'unknown', got: {:?}",
            se
        );
    }

    #[test]
    fn test_error_nested_empty_choice() {
        // @choice with no @option inside @if body (semantic)
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("NestedEmpty")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveIf),
            tok(Token::LParen),
            tok(Token::BoolLit(true)),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::DirectiveChoice),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (_doc, _pe, se) = parse_and_validate(tokens, "");
        let has_empty = se
            .iter()
            .any(|e| matches!(e, SemanticError::EmptyChoice { .. }));
        assert!(
            has_empty,
            "expected EmptyChoice semantic error, got: {:?}",
            se
        );
    }

    // ── @storyline("name") + @trigger tests ─────────────────────────────

    #[test]
    fn test_parse_named_storyline() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("S")),
            tok(Token::LBrace),
            tok(Token::DirectiveStoryline),
            tok(Token::LParen),
            tok(s_("delivery")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::DirectiveSpeaker),
            tok(Token::LParen),
            tok(s_("Prof")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(s_("Hi")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert_eq!(s.storylines.len(), 1);
        assert_eq!(s.storylines[0].name, "delivery");
    }

    #[test]
    fn test_parse_named_storyline_with_trigger() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("S")),
            tok(Token::LBrace),
            tok(Token::DirectiveStoryline),
            tok(Token::LParen),
            tok(s_("ask")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::DirectiveTrigger),
            tok(Token::LParen),
            tok(id("map")),
            tok(Token::Equals),
            tok(s_("ProfLab")),
            tok(Token::Comma),
            tok(id("npc")),
            tok(Token::Equals),
            tok(s_("Prof")),
            tok(Token::RParen),
            tok(Token::DirectiveSpeaker),
            tok(Token::LParen),
            tok(s_("Prof")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(s_("Hi")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert_eq!(s.storylines.len(), 1);
        let tr = s.storylines[0]
            .triggers
            .first()
            .expect("should have @trigger");
        assert_eq!(tr.map, "ProfLab");
        assert_eq!(tr.npc.as_deref(), Some("Prof"));
    }

    #[test]
    fn test_parse_named_storyline_with_on_enter() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("S")),
            tok(Token::LBrace),
            tok(Token::DirectiveStoryline),
            tok(Token::LParen),
            tok(s_("entry")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::DirectiveTrigger),
            tok(Token::LParen),
            tok(id("map")),
            tok(Token::Equals),
            tok(s_("Mart")),
            tok(Token::Comma),
            tok(id("onEnter")),
            tok(Token::Equals),
            tok(Token::BoolLit(true)),
            tok(Token::RParen),
            tok(Token::DirectiveSpeaker),
            tok(Token::LParen),
            tok(s_("Clerk")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(s_("Hi")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let tr = s.storylines[0]
            .triggers
            .first()
            .expect("should have @trigger");
        assert!(tr.on_enter, "onEnter should be true");
        assert!(tr.npc.is_none(), "npc should be None for onEnter");
    }

    #[test]
    fn test_parse_named_storyline_with_after() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("S")),
            tok(Token::LBrace),
            tok(Token::DirectiveStoryline),
            tok(Token::LParen),
            tok(s_("step2")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::DirectiveTrigger),
            tok(Token::LParen),
            tok(id("map")),
            tok(Token::Equals),
            tok(s_("Lab")),
            tok(Token::Comma),
            tok(id("npc")),
            tok(Token::Equals),
            tok(s_("Prof")),
            tok(Token::Comma),
            tok(id("after")),
            tok(Token::Equals),
            tok(s_("step1")),
            tok(Token::RParen),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let tr = s.storylines[0]
            .triggers
            .first()
            .expect("should have @trigger");
        assert_eq!(tr.after.as_deref(), Some("step1"));
    }

    #[test]
    fn test_parse_multiple_named_storylines() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("S")),
            tok(Token::LBrace),
            tok(Token::DirectiveStoryline),
            tok(Token::LParen),
            tok(s_("a")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::DirectiveStoryline),
            tok(Token::LParen),
            tok(s_("b")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert_eq!(s.storylines.len(), 2);
        assert_eq!(s.storylines[0].name, "a");
        assert_eq!(s.storylines[1].name, "b");
    }

    #[test]
    fn test_backward_compat_unnamed_storylines() {
        // Old @storylines { ... } without name → name defaults to "main"
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("S")),
            tok(Token::LBrace),
            tok(Token::DirectiveStorylines),
            tok(Token::LBrace),
            tok(Token::DirectiveSpeaker),
            tok(Token::LParen),
            tok(s_("Prof")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(s_("Hi")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        assert_eq!(s.storylines.len(), 1);
        assert_eq!(s.storylines[0].name, "main");
    }

    #[test]
    fn test_parse_trigger_with_name() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("S")),
            tok(Token::LBrace),
            tok(Token::DirectiveStoryline),
            tok(Token::LParen),
            tok(s_("myHandler")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(Token::DirectiveTrigger),
            tok(Token::LParen),
            tok(id("map")),
            tok(Token::Equals),
            tok(s_("TestMap")),
            tok(Token::Comma),
            tok(id("coord")),
            tok(Token::Equals),
            tok(Token::LBracket),
            tok(n(5.0)),
            tok(Token::Comma),
            tok(n(5.0)),
            tok(Token::RBracket),
            tok(Token::Comma),
            tok(id("name")),
            tok(Token::Equals),
            tok(s_("testCoord")),
            tok(Token::RParen),
            tok(Token::DirectiveSpeaker),
            tok(Token::LParen),
            tok(s_("")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(s_("hello")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let tr = s.storylines[0]
            .triggers
            .first()
            .expect("should have @trigger");
        assert_eq!(tr.name, "testCoord");
        assert_eq!(tr.map, "TestMap");
        assert_eq!(tr.coords, vec![(5, 5)]);
    }

    // ──────────────── POKERED-SPECIFIC PARSER TESTS ────────────────

    #[test]
    fn test_parse_object_literal_expression() {
        let tokens = vec![
            tok(Token::KeywordGameScene),
            tok(id("ObjLit")),
            tok(Token::LBrace),
            tok(Token::DirectiveVariables),
            tok(Token::LBrace),
            tok(id("cfg")),
            tok(Token::Equals),
            tok(Token::LBrace),
            tok(id("tile")),
            tok(Token::Colon),
            tok(n(223.0)),
            tok(Token::Comma),
            tok(id("position")),
            tok(Token::Colon),
            tok(s_("left")),
            tok(Token::RBrace),
            tok(Token::Newline),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let Document::Scene(s) = doc.unwrap() else {
            panic!()
        };
        let vb = s.variables.unwrap();
        assert_eq!(vb.decls.len(), 1);
        assert!(matches!(&vb.decls[0].value, Expression::ObjectLit(fields) if fields.len() == 2));
    }

    #[test]
    fn test_parse_gui_with_tile() {
        let tokens = vec![
            tok(Token::KeywordScreen),
            tok(id("Dialog")),
            tok(Token::LBrace),
            tok(id("tile")),
            tok(Token::LParen),
            tok(n(31.0)),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(id("rect")),
            tok(Token::Equals),
            tok(Token::LBrace),
            tok(id("tx")),
            tok(Token::Colon),
            tok(n(18.0)),
            tok(Token::Comma),
            tok(id("ty")),
            tok(Token::Colon),
            tok(n(16.0)),
            tok(Token::Comma),
            tok(id("tw")),
            tok(Token::Colon),
            tok(n(1.0)),
            tok(Token::Comma),
            tok(id("th")),
            tok(Token::Colon),
            tok(n(1.0)),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        match doc.unwrap() {
            Document::Screen(s) => {
                assert_eq!(s.components.len(), 1);
                match &s.components[0] {
                    UiComponent::Tile { tile_id, props, .. } => {
                        assert!(matches!(tile_id, Expression::NumberLit(31.0)));
                        assert!(props.rect.is_some());
                        let rect = props.rect.as_ref().unwrap();
                        assert!(matches!(rect.tx, Expression::NumberLit(18.0)));
                        assert!(matches!(rect.ty, Expression::NumberLit(16.0)));
                        assert!(matches!(rect.tw, Expression::NumberLit(1.0)));
                        assert!(matches!(rect.th, Expression::NumberLit(1.0)));
                    }
                    _ => panic!("expected Tile"),
                }
            }
            _ => panic!("expected Screen"),
        }
    }

    #[test]
    fn test_parse_gui_with_border_and_text() {
        let tokens = vec![
            tok(Token::KeywordScreen),
            tok(id("Dialog")),
            tok(Token::LBrace),
            tok(id("panel")),
            tok(Token::LBrace),
            tok(id("rect")),
            tok(Token::Equals),
            tok(Token::LBrace),
            tok(id("tx")),
            tok(Token::Colon),
            tok(n(0.0)),
            tok(Token::Comma),
            tok(id("ty")),
            tok(Token::Colon),
            tok(n(12.0)),
            tok(Token::Comma),
            tok(id("tw")),
            tok(Token::Colon),
            tok(n(20.0)),
            tok(Token::Comma),
            tok(id("th")),
            tok(Token::Colon),
            tok(n(6.0)),
            tok(Token::RBrace),
            tok(id("style")),
            tok(Token::Equals),
            tok(s_("default")),
            tok(Token::RBrace),
            tok(id("text")),
            tok(Token::LParen),
            tok(s_("{text}")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(id("rect")),
            tok(Token::Equals),
            tok(Token::LBrace),
            tok(id("tx")),
            tok(Token::Colon),
            tok(n(1.0)),
            tok(Token::Comma),
            tok(id("ty")),
            tok(Token::Colon),
            tok(n(13.0)),
            tok(Token::Comma),
            tok(id("tw")),
            tok(Token::Colon),
            tok(n(18.0)),
            tok(Token::Comma),
            tok(id("th")),
            tok(Token::Colon),
            tok(n(4.0)),
            tok(Token::RBrace),
            tok(id("value")),
            tok(Token::Equals),
            tok(s_("{text}")),
            tok(id("wrap")),
            tok(Token::Equals),
            tok(s_("word")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        match doc.unwrap() {
            Document::Screen(s) => {
                assert_eq!(s.components.len(), 2, "expected 2 components");
                match &s.components[0] {
                    UiComponent::Panel { props, .. } => {
                        assert_eq!(props.style.as_deref(), Some("default"));
                        assert!(props.rect.is_some());
                    }
                    _ => panic!("expected Panel"),
                }
                match &s.components[1] {
                    UiComponent::Text { content, props, .. } => {
                        assert_eq!(content, "{text}");
                        assert_eq!(props.value.as_deref(), Some("{text}"));
                        assert_eq!(props.wrap.as_deref(), Some("word"));
                        assert!(props.rect.is_some());
                    }
                    _ => panic!("expected Text"),
                }
            }
            _ => panic!("expected Screen"),
        }
    }

    #[test]
    fn test_parse_gui_with_divider() {
        let tokens = vec![
            tok(Token::KeywordScreen),
            tok(id("DividerTest")),
            tok(Token::LBrace),
            tok(id("divider")),
            tok(Token::LBrace),
            tok(id("tiles")),
            tok(Token::Equals),
            tok(Token::LBracket),
            tok(n(122.0)),
            tok(Token::RBracket),
            tok(id("repeat")),
            tok(Token::Equals),
            tok(n(17.0)),
            tok(id("orientation")),
            tok(Token::Equals),
            tok(s_("horizontal")),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        match doc.unwrap() {
            Document::Screen(s) => {
                assert_eq!(s.components.len(), 1);
                match &s.components[0] {
                    UiComponent::Divider { tiles, props, .. } => {
                        assert_eq!(tiles.len(), 1);
                        assert_eq!(props.repeat, Some(17));
                        assert_eq!(props.orientation.as_deref(), Some("horizontal"));
                    }
                    _ => panic!("expected Divider"),
                }
            }
            _ => panic!("expected Screen"),
        }
    }

    #[test]
    fn test_parse_t_localized_in_gui_text() {
        // `@t("en", "中文")` inside a GUI `text(...)` argument parses to a
        // localized component content.
        let src = "screen S {\n  text(@t(\"YES\", \"是\")) {\n    rect = {tx: 0, ty: 0, tw: 3, th: 1}\n  }\n}";
        let tokens = crate::lexer::Lexer::new(src, "t.gui")
            .tokenize()
            .expect("lex");
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        match doc.unwrap() {
            Document::Screen(s) => match &s.components[0] {
                UiComponent::Text { content, .. } => {
                    assert!(content.is_localized());
                    assert_eq!(content.get("en"), "YES");
                    assert_eq!(content.get("zh"), "是");
                }
                other => panic!("expected Text, got {:?}", other),
            },
            _ => panic!("expected Screen"),
        }
    }

    #[test]
    fn test_parse_t_localized_in_speaker_and_option() {
        // `@t(...)` is accepted both as a `@speaker` line and an `@option` label.
        let src = "game_scene S {\n  @storyline(\"x\") {\n    @speaker(\"\") {\n      @t(\"Hi\", \"你好\")\n    }\n    @choice {\n      @option(@t(\"YES\", \"是\")) {\n      }\n    }\n  }\n}";
        let tokens = crate::lexer::Lexer::new(src, "t.scene")
            .tokenize()
            .expect("lex");
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        let scene = match doc.unwrap() {
            Document::Scene(s) => s,
            _ => panic!("expected Scene"),
        };
        let stmts = &scene.storylines[0].statements;
        match &stmts[0] {
            StoryStmt::Speaker { texts, .. } => {
                assert!(texts[0].is_localized());
                assert_eq!(texts[0].get("zh"), "你好");
            }
            other => panic!("expected Speaker, got {:?}", other),
        }
        match &stmts[1] {
            StoryStmt::Choice { options, .. } => {
                assert!(options[0].label.is_localized());
                assert_eq!(options[0].label.get("zh"), "是");
            }
            other => panic!("expected Choice, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_gui_with_flex_list() {
        let tokens = vec![
            tok(Token::KeywordScreen),
            tok(id("FlexTest")),
            tok(Token::LBrace),
            tok(id("flex_list")),
            tok(Token::LParen),
            tok(id("items")),
            tok(Token::RParen),
            tok(Token::LBrace),
            tok(id("cursor")),
            tok(Token::Equals),
            tok(Token::LBrace),
            tok(id("tile")),
            tok(Token::Colon),
            tok(n(223.0)),
            tok(Token::Comma),
            tok(id("position")),
            tok(Token::Colon),
            tok(s_("left")),
            tok(Token::RBrace),
            tok(id("max_visible")),
            tok(Token::Equals),
            tok(n(4.0)),
            tok(id("gap")),
            tok(Token::Equals),
            tok(n(1.0)),
            tok(Token::RBrace),
            tok(Token::RBrace),
            tok(Token::Eof),
        ];
        let (doc, errors) = parse(tokens);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        match doc.unwrap() {
            Document::Screen(s) => {
                assert_eq!(s.components.len(), 1);
                match &s.components[0] {
                    UiComponent::FlexList { source, props, .. } => {
                        assert!(matches!(source, Expression::Variable(v) if v == "items"));
                        assert_eq!(props.max_visible, Some(4));
                        assert_eq!(props.gap, Some(1));
                        assert!(props.cursor.is_some());
                    }
                    _ => panic!("expected FlexList"),
                }
            }
            _ => panic!("expected Screen"),
        }
    }
}
