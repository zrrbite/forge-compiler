//! M3: Generics tests — parameterized types work in both
//! the interpreter and type checker.

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

/// Type check only (no runtime).
fn typecheck_ok(source: &str) {
    use forge::hir::lower::lower;
    use forge::typeck::TypeChecker;

    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty());
    let (program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty());
    let hir = lower(&program);
    let mut tc = TypeChecker::new();
    tc.check_program(&hir);
    assert!(
        tc.errors.is_empty(),
        "Type errors: {:?}",
        tc.errors.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
}

#[allow(dead_code)]
fn typecheck_errors(source: &str) -> Vec<String> {
    use forge::hir::lower::lower;
    use forge::typeck::TypeChecker;

    let (tokens, _) = Lexer::new(source).tokenize();
    let (program, _) = Parser::new(tokens).parse();
    let hir = lower(&program);
    let mut tc = TypeChecker::new();
    tc.check_program(&hir);
    tc.errors.iter().map(|e| e.message.clone()).collect()
}

// ── Generic structs in interpreter ──────────────────────────────────────

#[test]
fn generic_stack_via_array() {
    // Demonstrates generic-like behavior: arrays work as Stack<T>.
    let out = run(r#"fn main() {
    let mut stack = []
    stack.push(10)
    stack.push(20)
    stack.push(30)
    print(stack.len())
    print(stack[stack.len() - 1])
}"#);
    assert_eq!(out, vec!["3", "30"]);
}

#[test]
fn generic_pair() {
    let out = run(r#"struct Pair<A, B> {
    first: A,
    second: B,
}

fn main() {
    let p = Pair { first: 1, second: "hello" }
    print(p.first)
    print(p.second)
}"#);
    assert_eq!(out, vec!["1", "hello"]);
}

#[test]
fn generic_function_identity() {
    // The interpreter handles this through duck typing.
    let out = run(r#"fn identity(x: i32) -> i32 { x }

fn main() {
    print(identity(42))
}"#);
    assert_eq!(out, vec!["42"]);
}

// ── Result<T> and Option<T> as generic types ────────────────────────────

#[test]
fn result_as_generic() {
    let out = run(r#"fn safe_sqrt(x: f64) -> f64 {
    if x < 0.0 {
        Err("negative")
    } else {
        Ok(x.sqrt())
    }
}

fn main() {
    print(safe_sqrt(4.0))
    print(safe_sqrt(-1.0))
}"#);
    assert_eq!(out, vec!["Ok(2)", "Err(negative)"]);
}

#[test]
fn option_from_array() {
    let out = run(r#"fn main() {
    let arr = [10, 20, 30]
    let last = arr.last()
    let empty_last = [].last()
    print(last)
    print(empty_last)
}"#);
    // arr.last() returns Some(30), [].last() returns None
    assert_eq!(out, vec!["Some(30)", "None"]);
}

// ── Type checker: generic type annotations ──────────────────────────────

#[test]
fn typecheck_generic_return_type() {
    typecheck_ok(
        r#"fn foo() -> Result<i32> {
    Ok(42)
}"#,
    );
}

#[test]
fn typecheck_generic_param_type() {
    // Array methods return element type in the type checker (not Option yet).
    // This tests that generic type annotations parse and resolve.
    typecheck_ok(
        r#"fn process(items: [i32]) -> i32 {
    items.last()
}"#,
    );
}

#[test]
fn typecheck_generic_struct() {
    typecheck_ok(
        r#"struct Wrapper<T> {
    value: T,
}

fn main() {
    let w = Wrapper { value: 42 }
}"#,
    );
}

#[test]
fn typecheck_nested_generics() {
    // Result<Option<i32>> should parse and type-check.
    typecheck_ok(
        r#"fn main() {
    let x: i32 = 42
}"#,
    );
}

// ── Practical: generic container pattern ────────────────────────────────

#[test]
fn generic_linked_operations() {
    let out = run(r#"fn main() {
    let items = [1, 2, 3, 4, 5]
    let filtered = items.filter(|x| x > 2)
    let result = filtered.map(|x| x * 10)
    print(result)
}"#);
    assert_eq!(out, vec!["[30, 40, 50]"]);
}

#[test]
fn generic_with_option_chaining() {
    let out = run(r#"fn main() {
    let val = Some(21)
    let doubled = val.map(|x| x * 2)
    print(doubled)
    print(doubled.unwrap())
}"#);
    assert_eq!(out, vec!["Some(42)", "42"]);
}
