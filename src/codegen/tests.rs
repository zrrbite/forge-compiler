use crate::codegen::Codegen;
use crate::hir::lower::lower;
use crate::lexer::Lexer;
use crate::parser::Parser;
use inkwell::context::Context;

/// Helper: compile source to LLVM IR string.
fn compile_ir(source: &str) -> String {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty(), "Lex errors: {:?}", lex_errors);
    let (program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "Parse errors: {:?}", parse_errors);
    let hir = lower(&program);
    let context = Context::create();
    let mut codegen = Codegen::new(&context, "test");
    codegen.compile_program(&hir).expect("Codegen error");
    codegen.get_ir()
}

/// Helper: compile to binary, run, capture output.
fn run_forge(source: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty(), "Lex errors: {:?}", lex_errors);
    let (program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "Parse errors: {:?}", parse_errors);
    let hir = lower(&program);

    let tmp_dir = std::env::temp_dir().join("forge_test");
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let bin_path = tmp_dir.join(format!("test_bin_{id}"));

    crate::codegen::compile_to_binary(&hir, &bin_path).expect("Compilation failed");

    let output = std::process::Command::new(&bin_path)
        .output()
        .expect("Failed to run binary");

    // Clean up.
    let _ = std::fs::remove_file(&bin_path);

    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

// ── IR generation ───────────────────────────────────────────────────────

#[test]
fn ir_has_main() {
    let ir = compile_ir("fn main() {}");
    assert!(ir.contains("define i32 @main()"));
}

#[test]
fn ir_has_function_with_return() {
    let ir = compile_ir("fn answer() -> i64 { 42 }");
    assert!(ir.contains("define i64 @answer()"));
    assert!(ir.contains("ret i64 42"));
}

#[test]
fn ir_has_add() {
    let ir = compile_ir("fn add(a: i64, b: i64) -> i64 { a + b }");
    assert!(ir.contains("add i64"));
}

// ── End-to-end: compile + run ───────────────────────────────────────────

#[test]
fn run_print_integer() {
    let output = run_forge("fn main() { print(42) }");
    assert_eq!(output, "42");
}

#[test]
fn run_print_arithmetic() {
    let output = run_forge("fn main() { print(2 + 3 * 4) }");
    assert_eq!(output, "14");
}

#[test]
fn run_print_string() {
    let output = run_forge(r#"fn main() { print("hello") }"#);
    assert_eq!(output, "hello");
}

#[test]
fn run_let_and_print() {
    let output = run_forge(
        r#"fn main() {
    let x = 10
    let y = 20
    print(x + y)
}"#,
    );
    assert_eq!(output, "30");
}

#[test]
fn run_function_call() {
    let output = run_forge(
        r#"fn double(x: i64) -> i64 { x * 2 }
fn main() { print(double(21)) }"#,
    );
    assert_eq!(output, "42");
}

#[test]
fn run_nested_calls() {
    let output = run_forge(
        r#"fn add(a: i64, b: i64) -> i64 { a + b }
fn mul(a: i64, b: i64) -> i64 { a * b }
fn main() { print(add(mul(2, 3), mul(4, 5))) }"#,
    );
    assert_eq!(output, "26");
}

#[test]
fn run_if_else() {
    let output = run_forge(
        r#"fn max(a: i64, b: i64) -> i64 {
    if a > b { a } else { b }
}
fn main() { print(max(10, 20)) }"#,
    );
    assert_eq!(output, "20");
}

#[test]
fn run_float_arithmetic() {
    let output = run_forge("fn main() { print(3.14) }");
    assert_eq!(output, "3.14");
}

#[test]
fn run_multiple_prints() {
    let output = run_forge(
        r#"fn main() {
    print(1)
    print(2)
    print(3)
}"#,
    );
    assert_eq!(output, "1\n2\n3");
}

// ── Structs and methods ─────────────────────────────────────────────────

#[test]
fn run_struct_and_field_access() {
    let output = run_forge(
        r#"struct Point { x: f64, y: f64 }

fn main() {
    let p = Point { x: 3.0, y: 4.0 }
    print(p.x)
    print(p.y)
}"#,
    );
    assert_eq!(output, "3\n4");
}

#[test]
fn run_static_method() {
    let output = run_forge(
        r#"struct Vec2 { x: f64, y: f64 }

impl Vec2 {
    fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }
}

fn main() {
    let v = Vec2.new(5.0, 12.0)
    print(v.x)
    print(v.y)
}"#,
    );
    assert_eq!(output, "5\n12");
}

#[test]
fn run_instance_method() {
    let output = run_forge(
        r#"struct Vec2 { x: f64, y: f64 }

impl Vec2 {
    fn new(x: f64, y: f64) -> Vec2 { Vec2 { x, y } }
    fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

fn main() {
    let v = Vec2.new(3.0, 4.0)
    print(v.length())
}"#,
    );
    assert_eq!(output, "5");
}

// ── While loops ─────────────────────────────────────────────────────────

#[test]
fn run_while_loop() {
    let output = run_forge(
        r#"fn main() {
    let mut i = 0
    while i < 3 {
        print(i)
        i = i + 1
    }
}"#,
    );
    assert_eq!(output, "0\n1\n2");
}

// ── For loops ───────────────────────────────────────────────────────────

#[test]
fn run_for_loop() {
    let output = run_forge(
        r#"fn main() {
    let mut sum = 0
    for i in 0..5 {
        sum = sum + i
    }
    print(sum)
}"#,
    );
    assert_eq!(output, "10");
}
