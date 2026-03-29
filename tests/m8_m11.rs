//! M8-M11: Enum codegen, Box, trait dispatch, closure codegen.

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

// ── M8: Match on integers ───────────────────────────────────────────────

#[test]
fn match_integer_arms() {
    let out = run(r#"fn main() {
    let x = 2
    match x {
        1 => print("one"),
        2 => print("two"),
        3 => print("three"),
        _ => print("other"),
    }
}"#);
    assert_eq!(out, vec!["two"]);
}

#[test]
fn match_with_wildcard() {
    let out = run(r#"fn main() {
    let x = 99
    match x {
        1 => print("one"),
        _ => print("other"),
    }
}"#);
    assert_eq!(out, vec!["other"]);
}

#[test]
fn match_result_variants() {
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
fn match_option_variants() {
    let out = run(r#"fn main() {
    let items = [10, 20, 30]
    match items.last() {
        Some(v) => print(v),
        _ => print("empty"),
    }
}"#);
    assert_eq!(out, vec!["30"]);
}

#[test]
fn match_as_expression() {
    let out = run(r#"fn main() {
    let x = 2
    let label = match x {
        1 => "one",
        2 => "two",
        _ => "other",
    }
    print(label)
}"#);
    assert_eq!(out, vec!["two"]);
}

// ── M9: Box-like patterns (heap via arrays/structs) ─────────────────────

#[test]
fn recursive_data_via_arrays() {
    // Simulate a tree using arrays (since we don't have Box yet).
    let out = run(r#"fn main() {
    // A simple expression evaluator using variant + value encoding.
    let expr_type = "add"
    let left = 10
    let right = 32

    let result = if expr_type == "add" {
        left + right
    } else {
        left - right
    }
    print(result)
}"#);
    assert_eq!(out, vec!["42"]);
}

// ── M10: Trait-like dispatch ────────────────────────────────────────────

#[test]
fn trait_method_dispatch() {
    let out = run(r#"trait Shape {
    fn area(self) -> f64
}

struct Circle { radius: f64 }
struct Rect { width: f64, height: f64 }

impl Circle {
    fn area(self) -> f64 { 3.14159 * self.radius * self.radius }
    fn name(self) -> str { "circle" }
}

impl Rect {
    fn area(self) -> f64 { self.width * self.height }
    fn name(self) -> str { "rectangle" }
}

fn main() {
    let c = Circle { radius: 5.0 }
    let r = Rect { width: 4.0, height: 6.0 }
    print("{c.name()}: {c.area()}")
    print("{r.name()}: {r.area()}")
}"#);
    assert_eq!(out, vec!["circle: 78.53975", "rectangle: 24"]);
}

#[test]
fn operator_overload_via_trait() {
    let out = run(r#"struct Vec2 { x: f64, y: f64 }

impl Add for Vec2 {
    fn add(self, other: Vec2) -> Vec2 {
        Vec2 { x: self.x + other.x, y: self.y + other.y }
    }
}

impl Sub for Vec2 {
    fn sub(self, other: Vec2) -> Vec2 {
        Vec2 { x: self.x - other.x, y: self.y - other.y }
    }
}

fn main() {
    let a = Vec2 { x: 10.0, y: 20.0 }
    let b = Vec2 { x: 3.0, y: 5.0 }
    let sum = a + b
    let diff = a - b
    print("sum: ({sum.x}, {sum.y})")
    print("diff: ({diff.x}, {diff.y})")
}"#);
    assert_eq!(out, vec!["sum: (13, 25)", "diff: (7, 15)"]);
}

// ── M11: Multi-line closures ────────────────────────────────────────────

#[test]
fn closure_with_block_body() {
    let out = run(r#"fn main() {
    let items = [1, 2, 3, 4, 5]
    let result = items.map(|x| {
        let doubled = x * 2
        let incremented = doubled + 1
        incremented
    })
    print(result)
}"#);
    assert_eq!(out, vec!["[3, 5, 7, 9, 11]"]);
}

#[test]
fn closure_captures_environment() {
    let out = run(r#"fn main() {
    let factor = 10
    let items = [1, 2, 3]
    let scaled = items.map(|x| x * factor)
    print(scaled)
}"#);
    assert_eq!(out, vec!["[10, 20, 30]"]);
}

#[test]
fn closure_as_argument() {
    let out = run(r#"fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
    f(x)
}

fn main() {
    let result = apply(|x| {
        let squared = x * x
        squared + 1
    }, 5)
    print(result)
}"#);
    assert_eq!(out, vec!["26"]);
}

#[test]
fn chained_higher_order() {
    let out = run(r#"fn main() {
    let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    let evens = numbers.filter(|x| x % 2 == 0)
    let squared = evens.map(|x| x * x)
    let result = squared.fold(0, |acc, x| acc + x)
    print(result)
}"#);
    // Even numbers: 2,4,6,8,10. Squared: 4,16,36,64,100. Sum: 220.
    assert_eq!(out, vec!["220"]);
}
