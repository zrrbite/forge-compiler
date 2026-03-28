use crate::interpreter::Interpreter;
use crate::lexer::Lexer;
use crate::parser::Parser;

/// Helper: run Forge source, capture output lines.
fn run_ok(source: &str) -> Vec<String> {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty(), "Lex errors: {:?}", lex_errors);
    let (program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "Parse errors: {:?}", parse_errors);
    let mut interp = Interpreter::new_capturing();
    interp.run(&program).expect("Runtime error");
    interp.get_output().to_vec()
}

/// Helper: run and expect a runtime error.
fn run_err(source: &str) -> String {
    let (tokens, _) = Lexer::new(source).tokenize();
    let (program, _) = Parser::new(tokens).parse();
    let mut interp = Interpreter::new_capturing();
    interp.run(&program).unwrap_err().0
}

// ── Basics ──────────────────────────────────────────────────────────────

#[test]
fn hello_print() {
    let out = run_ok(r#"fn main() { print("hello") }"#);
    assert_eq!(out, vec!["hello"]);
}

#[test]
fn print_integer() {
    let out = run_ok(r#"fn main() { print(42) }"#);
    assert_eq!(out, vec!["42"]);
}

#[test]
fn print_float() {
    let out = run_ok(r#"fn main() { print(3.14) }"#);
    assert_eq!(out, vec!["3.14"]);
}

#[test]
fn print_bool() {
    let out = run_ok(r#"fn main() { print(true) }"#);
    assert_eq!(out, vec!["true"]);
}

// ── Variables ───────────────────────────────────────────────────────────

#[test]
fn let_and_print() {
    let out = run_ok(
        r#"fn main() {
    let x = 42
    print(x)
}"#,
    );
    assert_eq!(out, vec!["42"]);
}

#[test]
fn let_mut_and_reassign() {
    let out = run_ok(
        r#"fn main() {
    let mut x = 0
    x = 10
    print(x)
}"#,
    );
    assert_eq!(out, vec!["10"]);
}

#[test]
fn compound_assignment() {
    let out = run_ok(
        r#"fn main() {
    let mut x = 5
    x += 3
    print(x)
}"#,
    );
    assert_eq!(out, vec!["8"]);
}

// ── Arithmetic ──────────────────────────────────────────────────────────

#[test]
fn integer_arithmetic() {
    let out = run_ok(
        r#"fn main() {
    print(2 + 3 * 4)
    print(10 - 3)
    print(15 / 4)
    print(15 % 4)
}"#,
    );
    assert_eq!(out, vec!["14", "7", "3", "3"]);
}

#[test]
fn float_arithmetic() {
    let out = run_ok(
        r#"fn main() {
    print(1.5 + 2.5)
}"#,
    );
    assert_eq!(out, vec!["4"]);
}

#[test]
fn unary_negation() {
    let out = run_ok(
        r#"fn main() {
    let x = 5
    print(-x)
}"#,
    );
    assert_eq!(out, vec!["-5"]);
}

#[test]
fn boolean_logic() {
    let out = run_ok(
        r#"fn main() {
    print(true && false)
    print(true || false)
    print(!true)
}"#,
    );
    assert_eq!(out, vec!["false", "true", "false"]);
}

// ── String interpolation ────────────────────────────────────────────────

#[test]
fn string_interpolation() {
    let out = run_ok(
        r#"fn main() {
    let name = "Forge"
    print("Hello, {name}!")
}"#,
    );
    assert_eq!(out, vec!["Hello, Forge!"]);
}

#[test]
fn string_interpolation_expr() {
    let out = run_ok(
        r#"fn main() {
    let x = 3
    let y = 4
    print("{x} + {y} = {x + y}")
}"#,
    );
    assert_eq!(out, vec!["3 + 4 = 7"]);
}

// ── Functions ───────────────────────────────────────────────────────────

#[test]
fn function_call() {
    let out = run_ok(
        r#"fn double(x: i32) -> i32 {
    x * 2
}

fn main() {
    print(double(21))
}"#,
    );
    assert_eq!(out, vec!["42"]);
}

#[test]
fn function_with_return() {
    let out = run_ok(
        r#"fn abs(x: i32) -> i32 {
    if x < 0 {
        return -x
    }
    x
}

fn main() {
    print(abs(-5))
    print(abs(3))
}"#,
    );
    assert_eq!(out, vec!["5", "3"]);
}

#[test]
fn nested_function_calls() {
    let out = run_ok(
        r#"fn add(a: i32, b: i32) -> i32 { a + b }
fn mul(a: i32, b: i32) -> i32 { a * b }

fn main() {
    print(add(mul(2, 3), mul(4, 5)))
}"#,
    );
    assert_eq!(out, vec!["26"]);
}

// ── Control flow ────────────────────────────────────────────────────────

#[test]
fn if_else() {
    let out = run_ok(
        r#"fn main() {
    let x = 10
    if x > 5 {
        print("big")
    } else {
        print("small")
    }
}"#,
    );
    assert_eq!(out, vec!["big"]);
}

#[test]
fn match_expr() {
    let out = run_ok(
        r#"fn main() {
    let x = 2
    match x {
        1 => print("one"),
        2 => print("two"),
        _ => print("other"),
    }
}"#,
    );
    assert_eq!(out, vec!["two"]);
}

#[test]
fn for_loop() {
    let out = run_ok(
        r#"fn main() {
    let mut sum = 0
    for x in 1..4 {
        sum += x
    }
    print(sum)
}"#,
    );
    assert_eq!(out, vec!["6"]);
}

#[test]
fn while_loop() {
    let out = run_ok(
        r#"fn main() {
    let mut i = 0
    while i < 3 {
        print(i)
        i += 1
    }
}"#,
    );
    assert_eq!(out, vec!["0", "1", "2"]);
}

#[test]
fn while_with_break() {
    let out = run_ok(
        r#"fn main() {
    let mut i = 0
    while true {
        if i == 3 {
            break
        }
        print(i)
        i += 1
    }
}"#,
    );
    assert_eq!(out, vec!["0", "1", "2"]);
}

// ── Structs ─────────────────────────────────────────────────────────────

#[test]
fn struct_create_and_access() {
    let out = run_ok(
        r#"struct Point {
    x: f64,
    y: f64,
}

fn main() {
    let p = Point { x: 3.0, y: 4.0 }
    print(p.x)
    print(p.y)
}"#,
    );
    assert_eq!(out, vec!["3", "4"]);
}

#[test]
fn struct_shorthand() {
    let out = run_ok(
        r#"struct Point {
    x: f64,
    y: f64,
}

fn main() {
    let x = 1.0
    let y = 2.0
    let p = Point { x, y }
    print(p.x)
}"#,
    );
    assert_eq!(out, vec!["1"]);
}

#[test]
fn struct_with_methods() {
    let out = run_ok(
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
}"#,
    );
    assert_eq!(out, vec!["5"]);
}

// ── Arrays ──────────────────────────────────────────────────────────────

#[test]
fn array_literal_and_index() {
    let out = run_ok(
        r#"fn main() {
    let arr = [10, 20, 30]
    print(arr[0])
    print(arr[2])
    print(arr.len())
}"#,
    );
    assert_eq!(out, vec!["10", "30", "3"]);
}

#[test]
fn array_map() {
    let out = run_ok(
        r#"fn main() {
    let items = [1, 2, 3]
    let doubled = items.map(|x| x * 2)
    print(doubled)
}"#,
    );
    assert_eq!(out, vec!["[2, 4, 6]"]);
}

#[test]
fn array_filter() {
    let out = run_ok(
        r#"fn main() {
    let items = [1, 2, 3, 4, 5]
    let evens = items.filter(|x| x % 2 == 0)
    print(evens)
}"#,
    );
    assert_eq!(out, vec!["[2, 4]"]);
}

#[test]
fn array_fold() {
    let out = run_ok(
        r#"fn main() {
    let items = [1, 2, 3, 4, 5]
    let sum = items.fold(0, |acc, x| acc + x)
    print(sum)
}"#,
    );
    assert_eq!(out, vec!["15"]);
}

// ── Closures ────────────────────────────────────────────────────────────

#[test]
fn closure_basic() {
    let out = run_ok(
        r#"fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
    f(x)
}

fn main() {
    let result = apply(|x| x * x, 5)
    print(result)
}"#,
    );
    assert_eq!(out, vec!["25"]);
}

// ── Ranges ──────────────────────────────────────────────────────────────

#[test]
fn range_to_array() {
    let out = run_ok(
        r#"fn main() {
    let r = 0..5
    print(r)
}"#,
    );
    assert_eq!(out, vec!["[0, 1, 2, 3, 4]"]);
}

#[test]
fn inclusive_range() {
    let out = run_ok(
        r#"fn main() {
    let r = 1..=3
    print(r)
}"#,
    );
    assert_eq!(out, vec!["[1, 2, 3]"]);
}

// ── Integration: Forge sample programs ──────────────────────────────────

#[test]
fn forge_hello_world() {
    let out = run_ok(
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

    print("Count: {count}, x: {x}, active: {active}")
}"#,
    );
    assert_eq!(out[0], "Hello, Martin!");
    assert_eq!(out[1], "Count: 1, x: 42, active: true");
}

#[test]
fn forge_vec2_full() {
    let out = run_ok(
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
    let a = Vec2.new(3.0, 4.0)
    print(a.length())
    print(a.x)
    print(a.y)
}"#,
    );
    assert_eq!(out, vec!["5", "3", "4"]);
}

// ── Errors ──────────────────────────────────────────────────────────────

#[test]
fn error_undefined_variable() {
    let err = run_err("fn main() { print(x) }");
    assert!(err.contains("Undefined variable"));
}

#[test]
fn error_no_main() {
    let err = run_err("fn foo() {}");
    assert!(err.contains("No main()"));
}

#[test]
fn error_division_by_zero() {
    let err = run_err("fn main() { print(10 / 0) }");
    assert!(err.contains("Division by zero"));
}
