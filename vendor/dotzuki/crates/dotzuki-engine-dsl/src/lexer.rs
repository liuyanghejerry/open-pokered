use crate::ast::SourceSpan;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    KeywordGameScene,
    KeywordScreen,
    KeywordUi,
    DirectiveVariables,
    DirectiveTheme,
    DirectiveStyle,
    DirectiveAtlas,
    DirectiveStorylines,
    DirectiveStoryline,
    DirectiveOnLoad,
    DirectiveTrigger,
    DirectiveSpeaker,
    /// `@say("Name") { "text" }` — a cutscene line: speech inside a
    /// scripted storyline (auto-triggered), where NPCs take turns talking
    /// in sequence. Distinct from `@speaker`, whose meaning is fixed to
    /// player-initiated dialogue (talking to an NPC).
    DirectiveSay,
    DirectiveChoice,
    DirectiveOption,
    DirectiveRun,
    DirectiveIf,
    DirectiveElse,
    DirectiveEach,
    DirectiveCommand,
    /// `@t("en", "中文")` — a localized (i18n) string literal.
    DirectiveT,
    Identifier(String),
    StringLit(String),
    NumberLit(f64),
    BoolLit(bool),
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Equals,
    Comma,
    Colon,
    Plus,
    Minus,
    Star,
    Slash,
    Not,
    EqEq,
    NotEq,
    Question,
    Gt,
    Lt,
    GtEq,
    LtEq,
    AndAnd,
    OrOr,
    BitAnd,
    BitOr,
    Indent(u32),
    Dedent(u32),
    Newline,
    Comment(String),
    RawBlock(String),
    Error(String),
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub message: String,
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
    file: String,
    indent_stack: Vec<u32>,
    indent_unit: Option<u32>,
    run_block_pending: bool,
}

impl Lexer {
    pub fn new(input: &str, file: impl Into<String>) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 0,
            file: file.into(),
            indent_stack: vec![0],
            indent_unit: None,
            run_block_pending: false,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<SpannedToken>, Vec<LexError>> {
        let mut tokens = Vec::new();
        let mut errors = Vec::new();

        loop {
            match self.next_token_inner() {
                Ok((tok, span)) => {
                    let is_eof = matches!(tok, Token::Eof);
                    tokens.push(SpannedToken { token: tok, span });
                    if is_eof {
                        self.emit_remaining_dedents(&mut tokens);
                        break;
                    }
                }
                Err(err) => {
                    errors.push(err);
                    if self.pos >= self.input.len() {
                        break;
                    }
                    while self.current_char().is_some() && self.current_char() != Some('\n') {
                        self.advance();
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(tokens)
        } else {
            Err(errors)
        }
    }

    fn emit_remaining_dedents(&self, tokens: &mut Vec<SpannedToken>) {
        for &level in self.indent_stack.iter().skip(1).rev() {
            tokens.push(SpannedToken {
                token: Token::Dedent(level),
                span: SourceSpan::point(&self.file, self.line, self.col),
            });
        }
    }

    fn current_char(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.input.get(self.pos).copied();
        if let Some(ch) = c {
            self.pos += 1;
            if ch == '\n' {
                self.line += 1;
                self.col = 0;
            } else {
                self.col += 1;
            }
        }
        c
    }

    fn peek(&self, offset: usize) -> Option<char> {
        self.input.get(self.pos + offset).copied()
    }

    fn span(&self, start_line: usize, start_col: usize, start_byte: usize) -> SourceSpan {
        SourceSpan::new(
            &self.file, start_line, start_col, self.line, self.col, start_byte, self.pos,
        )
    }

    fn next_token_inner(&mut self) -> Result<(Token, SourceSpan), LexError> {
        if self.col == 0 {
            let mut p = self.pos;
            if self.input.get(p).copied() == Some('\t') {
                return Err(
                    self.error("Tabs are not allowed for indentation. Use spaces (2 or 4).")
                );
            }
            while self.input.get(p).copied() == Some(' ') {
                p += 1;
            }
            if self.input.get(p).copied() == Some('\t') {
                return Err(
                    self.error("Tabs are not allowed for indentation. Use spaces (2 or 4).")
                );
            }
        }

        while let Some(c) = self.current_char() {
            if c == ' ' || c == '\r' {
                self.advance();
            } else {
                break;
            }
        }

        let start_line = self.line;
        let start_col = self.col;
        let start_byte = self.pos;

        // If run_block_pending is true but we're not looking at '{',
        // the @run directive was malformed — reset the flag so it
        // doesn't corrupt the next legitimate '{' token.
        if self.run_block_pending && self.current_char() != Some('{') {
            self.run_block_pending = false;
        }

        match self.current_char() {
            None => Ok((Token::Eof, self.span(start_line, start_col, start_byte))),
            Some('\n') => {
                self.advance();
                self.handle_newline()
            }
            Some('/') => match self.peek(1) {
                Some('/') => self.lex_single_line_comment(start_line, start_col, start_byte),
                Some('*') => self.lex_multi_line_comment(start_line, start_col, start_byte),
                _ => {
                    self.advance();
                    Ok((Token::Slash, self.span(start_line, start_col, start_byte)))
                }
            },
            Some('"') => self.lex_string('"', start_line, start_col, start_byte),
            Some('\'') => self.lex_string('\'', start_line, start_col, start_byte),
            Some('@') => {
                let result = self.lex_directive(start_line, start_col, start_byte);
                if let Ok((Token::DirectiveRun, _)) = &result {
                    self.run_block_pending = true;
                }
                result
            }
            Some('{') => {
                if self.run_block_pending {
                    self.run_block_pending = false;
                    self.lex_run_block(start_line, start_col, start_byte)
                } else {
                    self.advance();
                    Ok((Token::LBrace, self.span(start_line, start_col, start_byte)))
                }
            }
            Some('}') => {
                self.advance();
                Ok((Token::RBrace, self.span(start_line, start_col, start_byte)))
            }
            Some('(') => {
                self.advance();
                Ok((Token::LParen, self.span(start_line, start_col, start_byte)))
            }
            Some(')') => {
                self.advance();
                Ok((Token::RParen, self.span(start_line, start_col, start_byte)))
            }
            Some('[') => {
                self.advance();
                Ok((
                    Token::LBracket,
                    self.span(start_line, start_col, start_byte),
                ))
            }
            Some(']') => {
                self.advance();
                Ok((
                    Token::RBracket,
                    self.span(start_line, start_col, start_byte),
                ))
            }
            Some('=') => {
                self.advance();
                if self.current_char() == Some('=') {
                    self.advance();
                    Ok((Token::EqEq, self.span(start_line, start_col, start_byte)))
                } else {
                    Ok((Token::Equals, self.span(start_line, start_col, start_byte)))
                }
            }
            Some(',') => {
                self.advance();
                Ok((Token::Comma, self.span(start_line, start_col, start_byte)))
            }
            Some('?') => {
                self.advance();
                Ok((
                    Token::Question,
                    self.span(start_line, start_col, start_byte),
                ))
            }
            Some(':') => {
                self.advance();
                Ok((Token::Colon, self.span(start_line, start_col, start_byte)))
            }
            Some('+') => {
                self.advance();
                Ok((Token::Plus, self.span(start_line, start_col, start_byte)))
            }
            Some('-') => {
                if self
                    .peek(1)
                    .map_or(false, |c| c.is_ascii_digit() || c == '.')
                {
                    self.lex_number(start_line, start_col, start_byte)
                } else {
                    self.advance();
                    Ok((Token::Minus, self.span(start_line, start_col, start_byte)))
                }
            }
            Some('*') => {
                self.advance();
                Ok((Token::Star, self.span(start_line, start_col, start_byte)))
            }
            Some('!') => {
                self.advance();
                if self.current_char() == Some('=') {
                    self.advance();
                    Ok((Token::NotEq, self.span(start_line, start_col, start_byte)))
                } else {
                    Ok((Token::Not, self.span(start_line, start_col, start_byte)))
                }
            }
            Some('>') => {
                self.advance();
                if self.current_char() == Some('=') {
                    self.advance();
                    Ok((Token::GtEq, self.span(start_line, start_col, start_byte)))
                } else {
                    Ok((Token::Gt, self.span(start_line, start_col, start_byte)))
                }
            }
            Some('<') => {
                self.advance();
                if self.current_char() == Some('=') {
                    self.advance();
                    Ok((Token::LtEq, self.span(start_line, start_col, start_byte)))
                } else {
                    Ok((Token::Lt, self.span(start_line, start_col, start_byte)))
                }
            }
            Some('&') => {
                self.advance();
                if self.current_char() == Some('&') {
                    self.advance();
                    Ok((Token::AndAnd, self.span(start_line, start_col, start_byte)))
                } else {
                    Ok((Token::BitAnd, self.span(start_line, start_col, start_byte)))
                }
            }
            Some('|') => {
                self.advance();
                if self.current_char() == Some('|') {
                    self.advance();
                    Ok((Token::OrOr, self.span(start_line, start_col, start_byte)))
                } else {
                    Ok((Token::BitOr, self.span(start_line, start_col, start_byte)))
                }
            }
            Some(c)
                if c.is_ascii_digit()
                    || (c == '.' && self.peek(1).map_or(false, |cc| cc.is_ascii_digit())) =>
            {
                self.lex_number(start_line, start_col, start_byte)
            }
            Some('0') if self.peek(1) == Some('x') || self.peek(1) == Some('X') => {
                self.lex_number(start_line, start_col, start_byte)
            }
            Some(c) if c.is_alphabetic() || c == '_' => {
                self.lex_identifier_or_keyword(start_line, start_col, start_byte)
            }
            Some(c) => Err(self.error(&format!("Unexpected character: '{}'", c))),
        }
    }

    fn handle_newline(&mut self) -> Result<(Token, SourceSpan), LexError> {
        let start_line = self.line;
        let start_col = self.col;

        let indent_spaces = self.count_leading_spaces();

        if self.is_empty_or_comment_line() {
            return Ok((
                Token::Newline,
                SourceSpan::point(&self.file, start_line, start_col),
            ));
        }

        self.validate_indent_unit(indent_spaces)?;

        let current = *self.indent_stack.last().unwrap_or(&0);
        if indent_spaces > current {
            self.indent_stack.push(indent_spaces);
            Ok((
                Token::Indent(indent_spaces - current),
                SourceSpan::point(&self.file, start_line, start_col),
            ))
        } else if indent_spaces < current {
            while self
                .indent_stack
                .last()
                .map_or(false, |&t| t > indent_spaces)
            {
                self.indent_stack.pop();
            }
            let new_top = *self.indent_stack.last().unwrap_or(&0);
            if new_top != indent_spaces {
                return Err(self.error(&format!(
                    "Invalid dedent: expected indent level {}, found {}",
                    new_top, indent_spaces
                )));
            }
            Ok((
                Token::Dedent(current - indent_spaces),
                SourceSpan::point(&self.file, start_line, start_col),
            ))
        } else {
            Ok((
                Token::Newline,
                SourceSpan::point(&self.file, start_line, start_col),
            ))
        }
    }

    fn count_leading_spaces(&self) -> u32 {
        let mut pos = self.pos;
        let mut spaces = 0u32;
        while self.input.get(pos).copied() == Some(' ') {
            spaces += 1;
            pos += 1;
        }
        spaces
    }

    fn is_empty_or_comment_line(&self) -> bool {
        let mut pos = self.pos;
        loop {
            match self.input.get(pos).copied() {
                Some(' ') => {
                    pos += 1;
                }
                Some('\n') | None => return true,
                Some('/') if self.input.get(pos + 1).copied() == Some('/') => return true,
                _ => return false,
            }
        }
    }

    fn validate_indent_unit(&mut self, spaces: u32) -> Result<(), LexError> {
        if spaces == 0 {
            return Ok(());
        }
        match self.indent_unit {
            None => {
                if spaces == 2 || spaces == 4 {
                    self.indent_unit = Some(spaces);
                    Ok(())
                } else {
                    Err(self.error(&format!(
                        "Invalid indentation: {} spaces. Use 2 or 4 spaces.",
                        spaces
                    )))
                }
            }
            Some(unit) if spaces % unit != 0 => Err(self.error(&format!(
                "Inconsistent indentation: expected multiple of {} spaces, found {}.",
                unit, spaces
            ))),
            _ => Ok(()),
        }
    }

    fn lex_single_line_comment(
        &mut self,
        start_line: usize,
        start_col: usize,
        start_byte: usize,
    ) -> Result<(Token, SourceSpan), LexError> {
        self.advance();
        self.advance();
        let mut text = String::new();
        while let Some(c) = self.current_char() {
            if c == '\n' {
                break;
            }
            text.push(c);
            self.advance();
        }
        Ok((
            Token::Comment(text),
            self.span(start_line, start_col, start_byte),
        ))
    }

    fn lex_multi_line_comment(
        &mut self,
        start_line: usize,
        start_col: usize,
        start_byte: usize,
    ) -> Result<(Token, SourceSpan), LexError> {
        self.advance();
        self.advance();
        let mut text = String::new();
        loop {
            match self.current_char() {
                None => return Err(self.error("Unterminated multi-line comment")),
                Some('*') => {
                    self.advance();
                    if self.current_char() == Some('/') {
                        self.advance();
                        break;
                    } else {
                        text.push('*');
                    }
                }
                Some(c) => {
                    text.push(c);
                    self.advance();
                }
            }
        }
        Ok((
            Token::Comment(text),
            self.span(start_line, start_col, start_byte),
        ))
    }

    fn lex_string(
        &mut self,
        quote: char,
        start_line: usize,
        start_col: usize,
        start_byte: usize,
    ) -> Result<(Token, SourceSpan), LexError> {
        self.advance();
        let mut value = String::new();
        loop {
            match self.current_char() {
                None => return Err(self.error("Unterminated string literal")),
                Some('\\') => {
                    self.advance();
                    match self.current_char() {
                        Some('"') => {
                            value.push('"');
                            self.advance();
                        }
                        Some('\'') => {
                            value.push('\'');
                            self.advance();
                        }
                        Some('\\') => {
                            value.push('\\');
                            self.advance();
                        }
                        Some('n') => {
                            value.push('\n');
                            self.advance();
                        }
                        Some('t') => {
                            value.push('\t');
                            self.advance();
                        }
                        Some('r') => {
                            value.push('\r');
                            self.advance();
                        }
                        Some(c) => {
                            value.push('\\');
                            value.push(c);
                            self.advance();
                        }
                        None => return Err(self.error("Unterminated escape sequence")),
                    }
                }
                Some(c) if c == quote => {
                    self.advance();
                    break;
                }
                Some(c) => {
                    value.push(c);
                    self.advance();
                }
            }
        }
        Ok((
            Token::StringLit(value),
            self.span(start_line, start_col, start_byte),
        ))
    }

    fn lex_number(
        &mut self,
        start_line: usize,
        start_col: usize,
        start_byte: usize,
    ) -> Result<(Token, SourceSpan), LexError> {
        let sign = if self.current_char() == Some('-') {
            self.advance();
            -1.0
        } else {
            1.0
        };
        let mut int_part = String::new();
        let mut frac_part = String::new();
        let mut has_dot = false;

        if self.current_char() == Some('0') && matches!(self.peek(1), Some('x') | Some('X')) {
            self.advance();
            self.advance();
            while let Some(c) = self.current_char() {
                if c.is_ascii_hexdigit() {
                    int_part.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
            let value = u64::from_str_radix(&int_part, 16).unwrap_or(0) as f64;
            return Ok((
                Token::NumberLit(sign * value),
                self.span(start_line, start_col, start_byte),
            ));
        }

        while let Some(c) = self.current_char() {
            if c.is_ascii_digit() {
                if has_dot {
                    frac_part.push(c);
                } else {
                    int_part.push(c);
                }
                self.advance();
            } else if c == '.' && !has_dot {
                has_dot = true;
                self.advance();
            } else {
                break;
            }
        }

        if int_part.is_empty() && frac_part.is_empty() {
            return Err(self.error("Invalid number literal"));
        }

        let num_str = if has_dot {
            format!(
                "{}.{}",
                if int_part.is_empty() { "0" } else { &int_part },
                frac_part
            )
        } else {
            int_part.clone()
        };

        let value: f64 = num_str.parse().unwrap_or(0.0);
        Ok((
            Token::NumberLit(sign * value),
            self.span(start_line, start_col, start_byte),
        ))
    }

    fn lex_identifier_or_keyword(
        &mut self,
        start_line: usize,
        start_col: usize,
        start_byte: usize,
    ) -> Result<(Token, SourceSpan), LexError> {
        let mut ident = String::new();
        while let Some(c) = self.current_char() {
            if c.is_alphanumeric() || c == '_' || c == '.' || c == '-' {
                ident.push(c);
                self.advance();
            } else {
                break;
            }
        }
        Ok((
            self.keyword_or_ident(&ident),
            self.span(start_line, start_col, start_byte),
        ))
    }

    fn keyword_or_ident(&self, s: &str) -> Token {
        match s {
            "game_scene" => Token::KeywordGameScene,
            "screen" => Token::KeywordScreen,
            "ui" => Token::KeywordUi,
            "true" => Token::BoolLit(true),
            "false" => Token::BoolLit(false),
            _ => Token::Identifier(s.to_string()),
        }
    }

    fn lex_directive(
        &mut self,
        start_line: usize,
        start_col: usize,
        start_byte: usize,
    ) -> Result<(Token, SourceSpan), LexError> {
        self.advance();
        let mut name = String::new();
        while let Some(c) = self.current_char() {
            if c.is_alphanumeric() || c == '_' {
                name.push(c);
                self.advance();
            } else {
                break;
            }
        }
        if name.is_empty() {
            return Err(self.error("Expected directive name after '@'"));
        }
        Ok((
            match name.as_str() {
                "variables" => Token::DirectiveVariables,
                "theme" => Token::DirectiveTheme,
                "style" => Token::DirectiveStyle,
                "atlas" => Token::DirectiveAtlas,
                "storylines" => Token::DirectiveStorylines,
                "storyline" => Token::DirectiveStoryline,
                "load" => Token::DirectiveOnLoad,
                "speaker" => Token::DirectiveSpeaker,
                "say" => Token::DirectiveSay,
                "choice" => Token::DirectiveChoice,
                "option" => Token::DirectiveOption,
                "run" => Token::DirectiveRun,
                "if" => Token::DirectiveIf,
                "else" => Token::DirectiveElse,
                "each" => Token::DirectiveEach,
                "command" => Token::DirectiveCommand,
                "trigger" => Token::DirectiveTrigger,
                "t" => Token::DirectiveT,
                _ => Token::Identifier(format!("@{}", name)),
            },
            self.span(start_line, start_col, start_byte),
        ))
    }

    /// Consume the raw content of an `@run { ... }` block.
    ///
    /// Called after `@run` has been lexed and `run_block_pending` is true.
    /// We expect the current character to be `{`. Consume everything until
    /// the matching `}`, tracking brace nesting, and emit a `RawBlock` token
    /// containing the raw text between the braces.
    fn lex_run_block(
        &mut self,
        start_line: usize,
        start_col: usize,
        start_byte: usize,
    ) -> Result<(Token, SourceSpan), LexError> {
        // Current character should be '{' — consume it.
        self.advance();
        let content_start = self.pos;

        let mut depth: usize = 1;
        while depth > 0 {
            match self.current_char() {
                Some('{') => {
                    depth += 1;
                    self.advance();
                }
                Some('}') => {
                    depth -= 1;
                    if depth > 0 {
                        self.advance();
                    }
                }
                Some(_) => {
                    self.advance();
                }
                None => {
                    return Err(self.error("Unclosed @run block — expected '}'"));
                }
            }
        }

        let content_end = self.pos; // just before the closing '}'
        self.advance(); // consume the closing '}'

        let raw: String = self.input[content_start..content_end].iter().collect();
        let span = SourceSpan::new(
            &self.file, start_line, start_col, self.line, self.col, start_byte, self.pos,
        );
        Ok((Token::RawBlock(raw), span))
    }

    fn error(&self, message: &str) -> LexError {
        LexError {
            file: self.file.clone(),
            line: self.line,
            col: self.col,
            message: message.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(input: &str) -> Result<Vec<SpannedToken>, Vec<LexError>> {
        let mut lexer = Lexer::new(input, "test.dsl");
        lexer.tokenize()
    }

    fn token_types(tokens: &[SpannedToken]) -> Vec<Token> {
        tokens.iter().map(|t| t.token.clone()).collect()
    }

    #[test]
    fn test_simple_identifiers() {
        let tokens = lex("game_scene ShopScene { }").unwrap();
        let types = token_types(&tokens);
        assert!(types.contains(&Token::KeywordGameScene));
        assert!(types.contains(&Token::Identifier("ShopScene".to_string())));
        assert!(types.contains(&Token::LBrace));
        assert!(types.contains(&Token::RBrace));
        assert!(types.contains(&Token::Eof));
    }

    #[test]
    fn test_indent_2_spaces() {
        let input = "game_scene Test {\n  ui {\n    color = \"red\"\n  }\n}";
        let tokens = lex(input).unwrap();
        let types = token_types(&tokens);
        let indent_count = types
            .iter()
            .filter(|t| matches!(t, Token::Indent(_)))
            .count();
        let dedent_count = types
            .iter()
            .filter(|t| matches!(t, Token::Dedent(_)))
            .count();
        assert!(indent_count > 0, "Expected Indent tokens");
        assert!(dedent_count > 0, "Expected Dedent tokens");
        assert_eq!(indent_count, dedent_count, "Indent/Dedent must balance");
    }

    #[test]
    fn test_indent_4_spaces() {
        let input = "game_scene Test {\n    ui {\n        color = \"red\"\n    }\n}";
        let tokens = lex(input).unwrap();
        let types = token_types(&tokens);
        let indent_count = types
            .iter()
            .filter(|t| matches!(t, Token::Indent(_)))
            .count();
        assert!(
            indent_count > 0,
            "Expected Indent tokens with 4-space indentation"
        );
    }

    #[test]
    fn test_tab_rejection() {
        let input = "game_scene Test {\n\tui {\n\t}\n}";
        let result = lex(input);
        match result {
            Ok(tokens) => {
                let has_tab = tokens
                    .iter()
                    .any(|t| matches!(&t.token, Token::Error(msg) if msg.contains("Tabs")));
                assert!(has_tab, "Expected tab error token");
            }
            Err(errors) => {
                assert!(!errors.is_empty(), "Expected errors for tab");
                assert!(
                    errors.iter().any(|e| e.message.contains("Tabs")),
                    "{:?}",
                    errors
                );
            }
        }
    }

    #[test]
    fn test_string_literals() {
        let tokens = lex(r#"game_scene Test { name = "hello" }"#).unwrap();
        assert!(token_types(&tokens).contains(&Token::StringLit("hello".to_string())));
    }

    #[test]
    fn test_single_quoted_strings() {
        let tokens = lex("game_scene Test { name = 'world' }").unwrap();
        assert!(token_types(&tokens).contains(&Token::StringLit("world".to_string())));
    }

    #[test]
    fn test_comments() {
        let input =
            "// top comment\ngame_scene Test {\n  /* multi\n     line */\n  color = \"red\"\n}";
        let tokens = lex(input).unwrap();
        let types = token_types(&tokens);
        let comment_count = types
            .iter()
            .filter(|t| matches!(t, Token::Comment(_)))
            .count();
        assert!(
            comment_count >= 2,
            "Expected >=2 comments, got {}",
            comment_count
        );
        assert!(types.contains(&Token::StringLit("red".to_string())));
    }

    #[test]
    fn test_empty_lines() {
        let input = "game_scene Test {\n\n  ui {\n\n    color = \"red\"\n\n  }\n\n}";
        let tokens = lex(input).unwrap();
        let types = token_types(&tokens);
        assert!(types.contains(&Token::KeywordUi));
        assert!(types.contains(&Token::StringLit("red".to_string())));
    }

    #[test]
    fn test_directives() {
        let input = "game_scene Test {\n  @variables { gold = 500 }\n  @storylines {\n    @speaker(\"Prof\") \"Hello\"\n  }\n}";
        let tokens = lex(input).unwrap();
        let types = token_types(&tokens);
        assert!(types.contains(&Token::DirectiveVariables));
        assert!(types.contains(&Token::DirectiveStorylines));
        assert!(types.contains(&Token::DirectiveSpeaker));
        assert!(types.contains(&Token::NumberLit(500.0)));
    }

    #[test]
    fn test_nested_indent() {
        let input = "game_scene Test {\n  @storylines {\n    @if (true) {\n      @speaker(\"A\") \"text\"\n    }\n  }\n}";
        let tokens = lex(input).unwrap();
        let types = token_types(&tokens);
        let indents = types
            .iter()
            .filter(|t| matches!(t, Token::Indent(_)))
            .count();
        let dedents = types
            .iter()
            .filter(|t| matches!(t, Token::Dedent(_)))
            .count();
        assert!(indents >= 3, "Expected >=3 Indents, got {}", indents);
        assert_eq!(indents, dedents, "Balanced: {} vs {}", indents, dedents);
    }

    #[test]
    fn test_escape_sequences() {
        let tokens = lex(r#"game_scene Test { text = "hello\nworld\t!" }"#).unwrap();
        assert!(token_types(&tokens).contains(&Token::StringLit("hello\nworld\t!".to_string())));
    }

    #[test]
    fn test_escaped_quotes_in_strings() {
        let tokens = lex(r#"game_scene Test { text = "he said \"hello\"" }"#).unwrap();
        assert!(token_types(&tokens).contains(&Token::StringLit("he said \"hello\"".to_string())));
    }

    #[test]
    fn test_numbers() {
        let tokens = lex("gold = 500 price = 3.14").unwrap();
        let types = token_types(&tokens);
        assert!(types.contains(&Token::NumberLit(500.0)));
        assert!(types.contains(&Token::NumberLit(3.14)));
    }

    #[test]
    fn test_negative_number() {
        let tokens = lex("temp = -10").unwrap();
        assert!(token_types(&tokens).contains(&Token::NumberLit(-10.0)));
    }

    #[test]
    fn test_operators() {
        let tokens = lex("a == b c != d e > f g < h i >= j k <= l m && n p || q + - * /").unwrap();
        let types = token_types(&tokens);
        for expected in &[
            Token::EqEq,
            Token::NotEq,
            Token::Gt,
            Token::Lt,
            Token::GtEq,
            Token::LtEq,
            Token::AndAnd,
            Token::OrOr,
            Token::Plus,
            Token::Minus,
            Token::Star,
            Token::Slash,
        ] {
            assert!(types.contains(expected), "Missing {:?}", expected);
        }
    }

    #[test]
    fn test_bool_literals() {
        let tokens = lex("visible = true enabled = false").unwrap();
        let types = token_types(&tokens);
        assert!(types.contains(&Token::BoolLit(true)));
        assert!(types.contains(&Token::BoolLit(false)));
    }

    #[test]
    fn test_choice_directive() {
        let input = "game_scene Test {\n  @choice {\n    @option(\"Yes\") \"ok\"\n    @option(\"No\") \"never\"\n  }\n}";
        let tokens = lex(input).unwrap();
        let types = token_types(&tokens);
        assert!(types.contains(&Token::DirectiveChoice));
        assert!(types.contains(&Token::DirectiveOption));
        assert!(types.contains(&Token::StringLit("Yes".to_string())));
    }

    #[test]
    fn test_spans_have_file() {
        let mut lexer = Lexer::new("game_scene Test { }", "my_file.scene");
        let tokens = lexer.tokenize().unwrap();
        for t in &tokens {
            assert_eq!(t.span.file, "my_file.scene");
        }
    }

    #[test]
    fn test_eof_token() {
        let tokens = lex("").unwrap();
        assert!(token_types(&tokens).contains(&Token::Eof));
    }

    #[test]
    fn test_screen_keyword() {
        let tokens = lex("screen Main { panel { } }").unwrap();
        let types = token_types(&tokens);
        assert!(types.contains(&Token::KeywordScreen));
    }

    #[test]
    fn test_directive_command() {
        let tokens = lex("@command(\"heal\")").unwrap();
        let types = token_types(&tokens);
        assert!(types.contains(&Token::DirectiveCommand));
    }
}
