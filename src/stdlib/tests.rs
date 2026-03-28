use crate::interpreter::Interpreter;
use crate::lexer::Lexer;
use crate::parser::Parser;

fn run_ok(source: &str) -> Vec<String> {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty());
    let (program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty());
    let mut interp = Interpreter::new_capturing();
    interp.run(&program).expect("Runtime error");
    interp.get_output().to_vec()
}

fn run_err(source: &str) -> String {
    let (tokens, _) = Lexer::new(source).tokenize();
    let (program, _) = Parser::new(tokens).parse();
    let mut interp = Interpreter::new_capturing();
    interp.run(&program).unwrap_err().0
}

// ── I/O ─────────────────────────────────────────────────────────────────

#[test]
fn println_works() {
    let out = run_ok(r#"fn main() { println("hello") }"#);
    assert_eq!(out, vec!["hello"]);
}

// ── Type conversion ─────────────────────────────────────────────────────

#[test]
fn to_str_int() {
    let out = run_ok("fn main() { print(to_str(42)) }");
    assert_eq!(out, vec!["42"]);
}

#[test]
fn to_int_string() {
    let out = run_ok(r#"fn main() { print(to_int("123")) }"#);
    assert_eq!(out, vec!["123"]);
}

#[test]
fn to_float_string() {
    let out = run_ok(r#"fn main() { print(to_float("3.14")) }"#);
    assert_eq!(out, vec!["3.14"]);
}

#[test]
fn to_int_invalid() {
    let err = run_err(r#"fn main() { to_int("abc") }"#);
    assert!(err.contains("Cannot parse"));
}

// ── Math ────────────────────────────────────────────────────────────────

#[test]
fn abs_int() {
    let out = run_ok("fn main() { print(abs(-42)) }");
    assert_eq!(out, vec!["42"]);
}

#[test]
fn abs_float() {
    let out = run_ok("fn main() { print(abs(-3.14)) }");
    assert_eq!(out, vec!["3.14"]);
}

#[test]
fn min_max() {
    let out = run_ok(
        r#"fn main() {
    print(min(3, 7))
    print(max(3, 7))
}"#,
    );
    assert_eq!(out, vec!["3", "7"]);
}

#[test]
fn math_constants() {
    let out = run_ok(
        r#"fn main() {
    print(PI)
}"#,
    );
    assert!(out[0].starts_with("3.14159"));
}

// ── Assertions ──────────────────────────────────────────────────────────

#[test]
fn assert_true() {
    run_ok("fn main() { assert(true) }");
}

#[test]
fn assert_false_errors() {
    let err = run_err("fn main() { assert(false) }");
    assert!(err.contains("Assertion failed"));
}

#[test]
fn assert_eq_same() {
    run_ok("fn main() { assert_eq(42, 42) }");
}

#[test]
fn assert_eq_different() {
    let err = run_err("fn main() { assert_eq(1, 2) }");
    assert!(err.contains("Assertion failed"));
}
