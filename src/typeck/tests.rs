use crate::hir::lower::lower;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::typeck::TypeChecker;

/// Helper: parse, lower, type check. Return errors.
fn check_errors(source: &str) -> Vec<String> {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty(), "Lex errors: {:?}", lex_errors);
    let (program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "Parse errors: {:?}", parse_errors);
    let hir = lower(&program);
    let mut tc = TypeChecker::new();
    tc.check_program(&hir);
    tc.errors.iter().map(|e| e.message.clone()).collect()
}

/// Helper: type check and assert no errors.
fn check_ok(source: &str) {
    let errors = check_errors(source);
    assert!(errors.is_empty(), "Unexpected type errors: {:?}", errors);
}

/// Helper: type check and assert there are errors.
fn check_err(source: &str) -> Vec<String> {
    let errors = check_errors(source);
    assert!(!errors.is_empty(), "Expected type errors but got none");
    errors
}

// ── Basic functions ─────────────────────────────────────────────────────

#[test]
fn empty_main() {
    check_ok("fn main() {}");
}

#[test]
fn function_returns_correct_type() {
    check_ok("fn foo() -> i32 { 42 }");
}

#[test]
fn function_return_type_mismatch() {
    let errors = check_err(r#"fn foo() -> i32 { "hello" }"#);
    assert!(errors[0].contains("mismatch") || errors[0].contains("Type"));
}

#[test]
fn function_with_params() {
    check_ok(
        r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}"#,
    );
}

// ── Let bindings ────────────────────────────────────────────────────────

#[test]
fn let_with_type_annotation() {
    check_ok("fn main() { let x: i32 = 42 }");
}

#[test]
fn let_type_mismatch() {
    let errors = check_err(r#"fn main() { let x: i32 = "hello" }"#);
    assert!(errors[0].contains("mismatch"));
}

#[test]
fn let_inferred_type() {
    check_ok("fn main() { let x = 42 }");
}

#[test]
fn let_mut_assignment() {
    check_ok(
        r#"fn main() {
    let mut x = 0
    x = 10
}"#,
    );
}

#[test]
fn immutable_assignment_error() {
    let errors = check_err(
        r#"fn main() {
    let x = 0
    x = 10
}"#,
    );
    assert!(errors[0].contains("immutable"));
}

// ── Arithmetic ──────────────────────────────────────────────────────────

#[test]
fn arithmetic_on_integers() {
    check_ok("fn main() { let x = 1 + 2 * 3 }");
}

#[test]
fn arithmetic_on_floats() {
    check_ok("fn main() { let x = 1.0 + 2.0 }");
}

#[test]
fn arithmetic_on_strings_error() {
    let errors = check_err("fn main() { let x = true + false }");
    assert!(errors[0].contains("Cannot apply"));
}

#[test]
fn string_concatenation() {
    check_ok(r#"fn main() { let x = "a" + "b" }"#);
}

// ── Boolean operators ───────────────────────────────────────────────────

#[test]
fn boolean_logic() {
    check_ok("fn main() { let x = true && false }");
}

#[test]
fn boolean_logic_type_error() {
    let errors = check_err("fn main() { let x = 1 && 2 }");
    assert!(errors[0].contains("bool"));
}

// ── Comparisons ─────────────────────────────────────────────────────────

#[test]
fn comparison_returns_bool() {
    check_ok(
        r#"fn main() {
    let x = 1 < 2
    let y = 3 >= 4
}"#,
    );
}

// ── Unary operators ─────────────────────────────────────────────────────

#[test]
fn negate_number() {
    check_ok("fn main() { let x = -42 }");
}

#[test]
fn negate_bool_error() {
    let errors = check_err("fn main() { let x = -true }");
    assert!(errors[0].contains("negate"));
}

#[test]
fn logical_not() {
    check_ok("fn main() { let x = !true }");
}

// ── Function calls ──────────────────────────────────────────────────────

#[test]
fn call_defined_function() {
    check_ok(
        r#"fn double(x: i32) -> i32 { x * 2 }
fn main() { let r = double(21) }"#,
    );
}

#[test]
fn call_wrong_arg_count() {
    let errors = check_err(
        r#"fn double(x: i32) -> i32 { x * 2 }
fn main() { double(1, 2) }"#,
    );
    assert!(errors[0].contains("arguments"));
}

#[test]
fn call_undefined_function() {
    let errors = check_err("fn main() { foo() }");
    assert!(errors[0].contains("Undefined") || errors[0].contains("callable"));
}

#[test]
fn print_accepts_anything() {
    check_ok(
        r#"fn main() {
    print(42)
    print("hello")
    print(true)
}"#,
    );
}

// ── Structs ─────────────────────────────────────────────────────────────

#[test]
fn struct_literal() {
    check_ok(
        r#"struct Point { x: f64, y: f64 }
fn main() { let p = Point { x: 1.0, y: 2.0 } }"#,
    );
}

#[test]
fn struct_field_access() {
    check_ok(
        r#"struct Point { x: f64, y: f64 }
fn main() {
    let p = Point { x: 1.0, y: 2.0 }
    let x = p.x
}"#,
    );
}

#[test]
fn struct_unknown_field_error() {
    let errors = check_err(
        r#"struct Point { x: f64, y: f64 }
fn main() {
    let p = Point { x: 1.0, y: 2.0 }
    let z = p.z
}"#,
    );
    assert!(errors[0].contains("No field 'z'"));
}

#[test]
fn struct_field_type_mismatch() {
    let errors = check_err(
        r#"struct Point { x: f64, y: f64 }
fn main() { let p = Point { x: "wrong", y: 2.0 } }"#,
    );
    assert!(errors[0].contains("mismatch"));
}

// ── Methods ─────────────────────────────────────────────────────────────

#[test]
fn method_call() {
    check_ok(
        r#"struct Vec2 { x: f64, y: f64 }

impl Vec2 {
    fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }
}

fn main() {
    let v = Vec2.new(3.0, 4.0)
}"#,
    );
}

#[test]
fn instance_method() {
    check_ok(
        r#"struct Vec2 { x: f64, y: f64 }

impl Vec2 {
    fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

fn main() {
    let v = Vec2 { x: 3.0, y: 4.0 }
    let len = v.length()
}"#,
    );
}

// ── Arrays ──────────────────────────────────────────────────────────────

#[test]
fn array_literal() {
    check_ok("fn main() { let a = [1, 2, 3] }");
}

#[test]
fn array_index() {
    check_ok("fn main() { let a = [1, 2, 3]\n let x = a[0] }");
}

#[test]
fn array_mixed_types_error() {
    let errors = check_err(r#"fn main() { let a = [1, "two", 3] }"#);
    assert!(errors[0].contains("mismatch"));
}

#[test]
fn array_map() {
    check_ok("fn main() { let a = [1, 2, 3].map(|x| x * 2) }");
}

#[test]
fn array_filter() {
    check_ok("fn main() { let a = [1, 2, 3].filter(|x| x > 1) }");
}

#[test]
fn array_fold() {
    check_ok("fn main() { let s = [1, 2, 3].fold(0, |acc, x| acc + x) }");
}

// ── Control flow ────────────────────────────────────────────────────────

#[test]
fn if_condition_must_be_bool() {
    let errors = check_err(
        r#"fn main() {
    if 42 { print("yes") }
}"#,
    );
    assert!(errors[0].contains("bool"));
}

#[test]
fn if_else_same_type() {
    check_ok(
        r#"fn main() {
    let x = if true { 1 } else { 2 }
}"#,
    );
}

#[test]
fn while_condition_must_be_bool() {
    let errors = check_err(
        r#"fn main() {
    while 42 { break }
}"#,
    );
    assert!(errors[0].contains("bool"));
}

#[test]
fn for_loop_over_array() {
    check_ok(
        r#"fn main() {
    for x in [1, 2, 3] {
        print(x)
    }
}"#,
    );
}

#[test]
fn for_loop_over_non_array_error() {
    let errors = check_err(
        r#"fn main() {
    for x in 42 {
        print(x)
    }
}"#,
    );
    assert!(errors[0].contains("iterate"));
}

// ── Closures ────────────────────────────────────────────────────────────

#[test]
fn closure_basic() {
    check_ok("fn main() { let f = |x: i32| x * 2 }");
}

// ── Ranges ──────────────────────────────────────────────────────────────

#[test]
fn range_type() {
    check_ok("fn main() { let r = 0..10 }");
}

// ── Integration: sample programs ────────────────────────────────────────

#[test]
fn check_hello_world() {
    check_ok(
        r#"fn greet(name: str) -> str {
    "Hello, {name}!"
}

fn main() {
    let name = "Martin"
    print(greet(name))
    let x: i32 = 42
    let pi = 3.14159
    let active = true
    let mut count = 0
    count += 1
    print("Count: {count}")
}"#,
    );
}

#[test]
fn check_vec2() {
    check_ok(
        r#"struct Vec2 {
    x: f64,
    y: f64,
}

impl Vec2 {
    fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }

    fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

fn main() {
    let v = Vec2.new(3.0, 4.0)
    print(v.length())
    print(v.x)
}"#,
    );
}
