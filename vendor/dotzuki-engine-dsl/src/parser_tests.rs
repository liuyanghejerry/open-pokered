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
    assert!(
        errors.iter().any(
            |e| matches!(e, ParseError::InvalidComponentType { found, .. } if found == "bad_widget")
        ),
        "errors: {:?}",
        errors
    );
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

#[test]
fn test_return_is_control_flow_not_a_host_command() {
    let tokens = vec![
        tok(Token::KeywordGameScene),
        tok(id("Return")),
        tok(Token::LBrace),
        tok(Token::DirectiveStorylines),
        tok(Token::LBrace),
        tok(Token::Identifier("return".into())),
        tok(Token::Newline),
        tok(Token::RBrace),
        tok(Token::RBrace),
        tok(Token::Eof),
    ];
    let (doc, errors) = parse(tokens);
    assert!(errors.is_empty(), "return should parse: {errors:?}");
    let Document::Scene(scene) = doc.expect("scene") else {
        panic!("expected scene")
    };
    assert!(matches!(
        scene.storylines[0].statements.as_slice(),
        [StoryStmt::Return { .. }]
    ));
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
    let has_undef = se
        .iter()
        .any(|e| matches!(e, SemanticError::UndefinedVariable { name, .. } if name == "unknown"));
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
