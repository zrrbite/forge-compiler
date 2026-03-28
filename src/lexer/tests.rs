use super::Lexer;
use super::token::TokenKind;

/// Helper: lex source and return just the token kinds (excluding Eof).
fn lex_kinds(source: &str) -> Vec<TokenKind> {
    let (tokens, errors) = Lexer::new(source).tokenize();
    assert!(errors.is_empty(), "Unexpected lex errors: {:?}", errors);
    tokens
        .into_iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .map(|t| t.kind)
        .collect()
}

/// Helper: lex source, expect errors, return them.
fn lex_errors(source: &str) -> Vec<String> {
    let (_tokens, errors) = Lexer::new(source).tokenize();
    errors.into_iter().map(|e| e.message).collect()
}

// ── Basics ──────────────────────────────────────────────────────────────────

#[test]
fn empty_source() {
    let kinds = lex_kinds("");
    assert!(kinds.is_empty());
}

#[test]
fn whitespace_only() {
    let kinds = lex_kinds("   \t  ");
    assert!(kinds.is_empty());
}

#[test]
fn newlines_collapsed() {
    let kinds = lex_kinds("let x\n\n\nlet y");
    assert_eq!(
        kinds,
        vec![
            TokenKind::Let,
            TokenKind::Identifier("x".into()),
            TokenKind::Newline,
            TokenKind::Let,
            TokenKind::Identifier("y".into()),
        ]
    );
}

#[test]
fn leading_trailing_newlines_stripped() {
    let kinds = lex_kinds("\n\nlet x\n\n");
    assert_eq!(
        kinds,
        vec![TokenKind::Let, TokenKind::Identifier("x".into())]
    );
}

// ── Keywords ────────────────────────────────────────────────────────────────

#[test]
fn all_keywords() {
    let source = "fn let mut struct impl trait enum match if else while for in return break continue use comptime spawn where pub mod self Self true false";
    let kinds = lex_kinds(source);
    assert_eq!(
        kinds,
        vec![
            TokenKind::Fn,
            TokenKind::Let,
            TokenKind::Mut,
            TokenKind::Struct,
            TokenKind::Impl,
            TokenKind::Trait,
            TokenKind::Enum,
            TokenKind::Match,
            TokenKind::If,
            TokenKind::Else,
            TokenKind::While,
            TokenKind::For,
            TokenKind::In,
            TokenKind::Return,
            TokenKind::Break,
            TokenKind::Continue,
            TokenKind::Use,
            TokenKind::Comptime,
            TokenKind::Spawn,
            TokenKind::Where,
            TokenKind::Pub,
            TokenKind::Mod,
            TokenKind::SelfValue,
            TokenKind::SelfType,
            TokenKind::BoolLiteral(true),
            TokenKind::BoolLiteral(false),
        ]
    );
}

#[test]
fn identifier_not_keyword() {
    let kinds = lex_kinds("fns letters selfish");
    assert_eq!(
        kinds,
        vec![
            TokenKind::Identifier("fns".into()),
            TokenKind::Identifier("letters".into()),
            TokenKind::Identifier("selfish".into()),
        ]
    );
}

// ── Numbers ─────────────────────────────────────────────────────────────────

#[test]
fn integer_literals() {
    let kinds = lex_kinds("0 42 1_000_000");
    assert_eq!(
        kinds,
        vec![
            TokenKind::IntLiteral(0),
            TokenKind::IntLiteral(42),
            TokenKind::IntLiteral(1_000_000),
        ]
    );
}

#[test]
fn hex_binary_octal() {
    let kinds = lex_kinds("0xFF 0b1010 0o77");
    assert_eq!(
        kinds,
        vec![
            TokenKind::IntLiteral(0xFF),
            TokenKind::IntLiteral(0b1010),
            TokenKind::IntLiteral(0o77),
        ]
    );
}

#[test]
fn float_literals() {
    let kinds = lex_kinds("3.14 0.5 1e10 2.5e-3");
    assert_eq!(
        kinds,
        vec![
            TokenKind::FloatLiteral(3.14),
            TokenKind::FloatLiteral(0.5),
            TokenKind::FloatLiteral(1e10),
            TokenKind::FloatLiteral(2.5e-3),
        ]
    );
}

#[test]
fn float_with_underscores() {
    let kinds = lex_kinds("1_000.000_1");
    assert_eq!(kinds, vec![TokenKind::FloatLiteral(1000.0001)]);
}

// ── Operators ───────────────────────────────────────────────────────────────

#[test]
fn single_char_operators() {
    let kinds = lex_kinds("+ - * / % & | ! . : ? @");
    assert_eq!(
        kinds,
        vec![
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::Ampersand,
            TokenKind::Pipe,
            TokenKind::Bang,
            TokenKind::Dot,
            TokenKind::Colon,
            TokenKind::Question,
            TokenKind::At,
        ]
    );
}

#[test]
fn multi_char_operators() {
    let kinds = lex_kinds("== != <= >= && || += -= *= /= %= -> => .. ..= ::");
    assert_eq!(
        kinds,
        vec![
            TokenKind::EqEq,
            TokenKind::BangEq,
            TokenKind::LtEq,
            TokenKind::GtEq,
            TokenKind::AmpAmp,
            TokenKind::PipePipe,
            TokenKind::PlusEq,
            TokenKind::MinusEq,
            TokenKind::StarEq,
            TokenKind::SlashEq,
            TokenKind::PercentEq,
            TokenKind::Arrow,
            TokenKind::FatArrow,
            TokenKind::DotDot,
            TokenKind::DotDotEq,
            TokenKind::ColonColon,
        ]
    );
}

// ── Delimiters ──────────────────────────────────────────────────────────────

#[test]
fn delimiters() {
    let kinds = lex_kinds("( ) { } [ ]");
    assert_eq!(
        kinds,
        vec![
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::LBracket,
            TokenKind::RBracket,
        ]
    );
}

// ── Strings ─────────────────────────────────────────────────────────────────

#[test]
fn simple_string() {
    let kinds = lex_kinds(r#""hello world""#);
    assert_eq!(kinds, vec![TokenKind::StringLiteral("hello world".into())]);
}

#[test]
fn string_with_escapes() {
    let kinds = lex_kinds(r#""line1\nline2\ttab\\slash""#);
    assert_eq!(
        kinds,
        vec![TokenKind::StringLiteral("line1\nline2\ttab\\slash".into())]
    );
}

#[test]
fn string_with_interpolation() {
    let kinds = lex_kinds(r#""Hello, {name}!""#);
    assert_eq!(
        kinds,
        vec![
            TokenKind::StringFragment("Hello, ".into()),
            TokenKind::Identifier("name".into()),
            TokenKind::StringFragment("!".into()),
            TokenKind::StringEnd,
        ]
    );
}

#[test]
fn string_with_multiple_interpolations() {
    let kinds = lex_kinds(r#""x={x}, y={y}""#);
    assert_eq!(
        kinds,
        vec![
            TokenKind::StringFragment("x=".into()),
            TokenKind::Identifier("x".into()),
            TokenKind::StringFragment(", y=".into()),
            TokenKind::Identifier("y".into()),
            TokenKind::StringFragment("".into()),
            TokenKind::StringEnd,
        ]
    );
}

#[test]
fn string_interpolation_with_expr() {
    let kinds = lex_kinds(r#""sum={a + b}""#);
    assert_eq!(
        kinds,
        vec![
            TokenKind::StringFragment("sum=".into()),
            TokenKind::Identifier("a".into()),
            TokenKind::Plus,
            TokenKind::Identifier("b".into()),
            TokenKind::StringFragment("".into()),
            TokenKind::StringEnd,
        ]
    );
}

#[test]
fn string_escaped_brace() {
    let kinds = lex_kinds(r#""use {{braces}}""#);
    assert_eq!(kinds, vec![TokenKind::StringLiteral("use {braces}".into())]);
}

#[test]
fn unterminated_string_error() {
    let errors = lex_errors(r#""unterminated"#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("Unterminated"));
}

// ── Comments ────────────────────────────────────────────────────────────────

#[test]
fn line_comments() {
    let kinds = lex_kinds("let x // this is a comment\nlet y");
    assert_eq!(
        kinds,
        vec![
            TokenKind::Let,
            TokenKind::Identifier("x".into()),
            TokenKind::Newline,
            TokenKind::Let,
            TokenKind::Identifier("y".into()),
        ]
    );
}

#[test]
fn comment_only_line() {
    let kinds = lex_kinds("// just a comment");
    assert!(kinds.is_empty());
}

// ── Spans ───────────────────────────────────────────────────────────────────

#[test]
fn spans_are_correct() {
    let source = "let x = 42";
    let (tokens, _) = Lexer::new(source).tokenize();
    // "let" at 0..3
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(tokens[0].span.end, 3);
    // "x" at 4..5
    assert_eq!(tokens[1].span.start, 4);
    assert_eq!(tokens[1].span.end, 5);
    // "=" at 6..7
    assert_eq!(tokens[2].span.start, 6);
    assert_eq!(tokens[2].span.end, 7);
    // "42" at 8..10
    assert_eq!(tokens[3].span.start, 8);
    assert_eq!(tokens[3].span.end, 10);
}

// ── Integration: Forge sample snippets ──────────────────────────────────────

#[test]
fn forge_hello_world() {
    let source = r#"fn main() {
    let name = "Martin"
    print(greet(name))
}"#;
    let kinds = lex_kinds(source);
    assert_eq!(
        kinds,
        vec![
            TokenKind::Fn,
            TokenKind::Identifier("main".into()),
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::Newline,
            TokenKind::Let,
            TokenKind::Identifier("name".into()),
            TokenKind::Eq,
            TokenKind::StringLiteral("Martin".into()),
            TokenKind::Newline,
            TokenKind::Identifier("print".into()),
            TokenKind::LParen,
            TokenKind::Identifier("greet".into()),
            TokenKind::LParen,
            TokenKind::Identifier("name".into()),
            TokenKind::RParen,
            TokenKind::RParen,
            TokenKind::Newline,
            TokenKind::RBrace,
        ]
    );
}

#[test]
fn forge_struct_and_impl() {
    let source = r#"struct Vec2 {
    x: f64,
    y: f64,
}

impl Vec2 {
    fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }
}"#;
    let kinds = lex_kinds(source);
    assert_eq!(
        kinds,
        vec![
            // struct Vec2 {
            TokenKind::Struct,
            TokenKind::Identifier("Vec2".into()),
            TokenKind::LBrace,
            TokenKind::Newline,
            // x: f64,
            TokenKind::Identifier("x".into()),
            TokenKind::Colon,
            TokenKind::Identifier("f64".into()),
            TokenKind::Comma,
            TokenKind::Newline,
            // y: f64,
            TokenKind::Identifier("y".into()),
            TokenKind::Colon,
            TokenKind::Identifier("f64".into()),
            TokenKind::Comma,
            TokenKind::Newline,
            // }
            TokenKind::RBrace,
            TokenKind::Newline,
            // impl Vec2 {
            TokenKind::Impl,
            TokenKind::Identifier("Vec2".into()),
            TokenKind::LBrace,
            TokenKind::Newline,
            // fn new(x: f64, y: f64) -> Vec2 {
            TokenKind::Fn,
            TokenKind::Identifier("new".into()),
            TokenKind::LParen,
            TokenKind::Identifier("x".into()),
            TokenKind::Colon,
            TokenKind::Identifier("f64".into()),
            TokenKind::Comma,
            TokenKind::Identifier("y".into()),
            TokenKind::Colon,
            TokenKind::Identifier("f64".into()),
            TokenKind::RParen,
            TokenKind::Arrow,
            TokenKind::Identifier("Vec2".into()),
            TokenKind::LBrace,
            TokenKind::Newline,
            // Vec2 { x, y }
            TokenKind::Identifier("Vec2".into()),
            TokenKind::LBrace,
            TokenKind::Identifier("x".into()),
            TokenKind::Comma,
            TokenKind::Identifier("y".into()),
            TokenKind::RBrace,
            TokenKind::Newline,
            // }
            TokenKind::RBrace,
            TokenKind::Newline,
            // }
            TokenKind::RBrace,
        ]
    );
}

#[test]
fn forge_ownership_and_borrowing() {
    let source = r#"fn report(buf: &Buffer) {
    print("Buffer contains {buf.len()} bytes")
}

fn main() {
    let mut buf = Buffer.new(64)
    buf.push(0xFF)
    let buf2 = buf
}"#;
    let kinds = lex_kinds(source);
    // Just check it lexes without error and spot-check key tokens.
    assert!(kinds.contains(&TokenKind::Mut));
    assert!(kinds.contains(&TokenKind::Ampersand));
    assert!(kinds.contains(&TokenKind::IntLiteral(0xFF)));
    assert!(kinds.contains(&TokenKind::IntLiteral(64)));
}

#[test]
fn forge_error_handling() {
    let source = r#"fn load_config(path: str) -> Result<u16> {
    let content = fs.read(path)?
    let port_str = content.trim()
    parse_port(port_str)?
}"#;
    let kinds = lex_kinds(source);
    assert!(kinds.contains(&TokenKind::Arrow));
    assert!(kinds.contains(&TokenKind::Lt));
    assert!(kinds.contains(&TokenKind::Gt));
    assert!(kinds.contains(&TokenKind::Question));
}

#[test]
fn forge_match_expression() {
    let source = r#"match req.method {
    Method.GET => {
        let id = req.param("id")?.parse::<u64>()?
        Ok(Response.json(to_json(user)))
    }
    _ => Ok(Response.method_not_allowed())
}"#;
    let kinds = lex_kinds(source);
    assert!(kinds.contains(&TokenKind::Match));
    assert!(kinds.contains(&TokenKind::FatArrow));
    assert!(kinds.contains(&TokenKind::ColonColon));
    // _ is an identifier
    assert!(kinds.contains(&TokenKind::Identifier("_".into())));
}

#[test]
fn forge_closure() {
    let kinds = lex_kinds("|x| x * x");
    assert_eq!(
        kinds,
        vec![
            TokenKind::Pipe,
            TokenKind::Identifier("x".into()),
            TokenKind::Pipe,
            TokenKind::Identifier("x".into()),
            TokenKind::Star,
            TokenKind::Identifier("x".into()),
        ]
    );
}

#[test]
fn forge_range() {
    let kinds = lex_kinds("0..100");
    assert_eq!(
        kinds,
        vec![
            TokenKind::IntLiteral(0),
            TokenKind::DotDot,
            TokenKind::IntLiteral(100),
        ]
    );
}

#[test]
fn forge_comptime() {
    let source = r#"fn make_sine_table(comptime n: usize) -> [f64; n] {
    comptime {
        let table = undefined
    }
}"#;
    let kinds = lex_kinds(source);
    // Two comptime keywords
    let comptime_count = kinds.iter().filter(|k| **k == TokenKind::Comptime).count();
    assert_eq!(comptime_count, 2);
}

// ── Error recovery ──────────────────────────────────────────────────────────

#[test]
fn unexpected_character_error() {
    let errors = lex_errors("let x = §");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("Unexpected character"));
}

#[test]
fn error_recovery_continues_lexing() {
    let source = "let § x = 42";
    let (tokens, errors) = Lexer::new(source).tokenize();
    assert_eq!(errors.len(), 1);
    // Should still have tokens for the valid parts.
    let kinds: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .map(|t| &t.kind)
        .collect();
    assert!(kinds.contains(&&TokenKind::Let));
    assert!(kinds.contains(&&TokenKind::Identifier("x".into())));
    assert!(kinds.contains(&&TokenKind::IntLiteral(42)));
}
