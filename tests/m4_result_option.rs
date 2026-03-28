//! M4: Result/Option types and ? propagation.

use forge::interpreter::Interpreter;
use forge::lexer::Lexer;
use forge::parser::Parser;

fn run(source: &str) -> Vec<String> {
    let (tokens, _) = Lexer::new(source).tokenize();
    let (program, _) = Parser::new(tokens).parse();
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

// ── Constructors ────────────────────────────────────────────────────────

#[test]
fn ok_and_err() {
    let out = run(r#"fn main() {
    let a = Ok(42)
    let b = Err("oops")
    print(a)
    print(b)
}"#);
    assert_eq!(out, vec!["Ok(42)", "Err(oops)"]);
}

#[test]
fn some_and_none() {
    let out = run(r#"fn main() {
    let a = Some(42)
    let b = None
    print(a)
    print(b)
}"#);
    assert_eq!(out, vec!["Some(42)", "None"]);
}

// ── Methods ─────────────────────────────────────────────────────────────

#[test]
fn is_ok_is_err() {
    let out = run(r#"fn main() {
    print(Ok(1).is_ok())
    print(Ok(1).is_err())
    print(Err("x").is_ok())
    print(Err("x").is_err())
}"#);
    assert_eq!(out, vec!["true", "false", "false", "true"]);
}

#[test]
fn is_some_is_none() {
    let out = run(r#"fn main() {
    print(Some(1).is_some())
    print(Some(1).is_none())
    print(None.is_some())
    print(None.is_none())
}"#);
    assert_eq!(out, vec!["true", "false", "false", "true"]);
}

#[test]
fn unwrap_ok() {
    let out = run("fn main() { print(Ok(42).unwrap()) }");
    assert_eq!(out, vec!["42"]);
}

#[test]
fn unwrap_err_panics() {
    let err = run_err(r#"fn main() { Err("bad").unwrap() }"#);
    assert!(err.contains("unwrap"));
}

#[test]
fn unwrap_or_ok() {
    let out = run("fn main() { print(Ok(42).unwrap_or(0)) }");
    assert_eq!(out, vec!["42"]);
}

#[test]
fn unwrap_or_err() {
    let out = run(r#"fn main() { print(Err("bad").unwrap_or(0)) }"#);
    assert_eq!(out, vec!["0"]);
}

#[test]
fn unwrap_or_none() {
    let out = run("fn main() { print(None.unwrap_or(99)) }");
    assert_eq!(out, vec!["99"]);
}

#[test]
fn map_ok() {
    let out = run("fn main() { print(Ok(21).map(|x| x * 2)) }");
    assert_eq!(out, vec!["Ok(42)"]);
}

#[test]
fn map_err_passes_through() {
    let out = run(r#"fn main() { print(Err("x").map(|x| x * 2)) }"#);
    assert_eq!(out, vec!["Err(x)"]);
}

// ── ? operator ──────────────────────────────────────────────────────────

#[test]
fn try_operator_unwraps_ok() {
    let out = run(r#"fn get_value() -> i32 {
    let result = Ok(42)
    result?
}

fn main() {
    print(get_value())
}"#);
    assert_eq!(out, vec!["42"]);
}

#[test]
fn try_operator_propagates_err() {
    let out = run(r#"fn might_fail() -> i32 {
    let result = Err("something went wrong")
    result?
    999
}

fn main() {
    let r = might_fail()
    print(r)
}"#);
    // ? returns Err(...) from might_fail, which becomes the return value.
    assert_eq!(out, vec!["Err(something went wrong)"]);
}

#[test]
fn try_operator_on_option() {
    let out = run(r#"fn get_first(arr: [i32]) -> i32 {
    arr.last()?
}

fn main() {
    let a = [10, 20, 30]
    print(get_first(a))
}"#);
    assert_eq!(out, vec!["30"]);
}

// ── Pattern matching ────────────────────────────────────────────────────

#[test]
fn match_on_result() {
    let out = run(r#"fn main() {
    let r = Ok(42)
    match r {
        Ok(v) => print(v),
        Err(e) => print(e),
    }
}"#);
    assert_eq!(out, vec!["42"]);
}

#[test]
fn match_on_option() {
    let out = run(r#"fn main() {
    let opt = Some(10)
    match opt {
        Some(v) => print(v),
        None => print("nothing"),
    }
}"#);
    // Note: pattern matching on variants uses the Variant path syntax
    assert_eq!(out, vec!["10"]);
}

#[test]
fn match_on_none() {
    let out = run(r#"fn main() {
    let opt = None
    match opt {
        Some(v) => print(v),
        _ => print("nothing"),
    }
}"#);
    assert_eq!(out, vec!["nothing"]);
}

// ── Practical: safe division ────────────────────────────────────────────

#[test]
fn safe_division() {
    let out = run(r#"fn safe_div(a: i64, b: i64) -> i64 {
    if b == 0 {
        Err("division by zero")
    } else {
        Ok(a / b)
    }
}

fn main() {
    let r1 = safe_div(10, 2)
    let r2 = safe_div(10, 0)
    print(r1)
    print(r2)
}"#);
    assert_eq!(out, vec!["Ok(5)", "Err(division by zero)"]);
}
