//! M7: Module system tests.

use forge::interpreter::Interpreter;
use forge::lexer::Lexer;
use forge::parser::Parser;
use forge::resolve;
use std::path::Path;

/// Run a .fg file through the full pipeline (resolve modules, then interpret).
fn run_file(path: &str) -> Vec<String> {
    let source = std::fs::read_to_string(path).expect("Failed to read file");
    let (tokens, lex_errors) = Lexer::new(&source).tokenize();
    assert!(lex_errors.is_empty(), "Lex errors: {:?}", lex_errors);
    let (mut program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "Parse errors: {:?}", parse_errors);

    // Resolve modules relative to the file's directory.
    resolve::resolve_modules(&mut program, Path::new(path)).expect("Module resolution failed");

    let mut interp = Interpreter::new_capturing();
    interp.run(&program).expect("Runtime error");
    interp.get_output().to_vec()
}

/// Helper to create temp test files and run them.
fn run_with_modules(main_source: &str, modules: &[(&str, &str)]) -> Vec<String> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let tmp_dir = std::env::temp_dir().join(format!("forge_m7_test_{id}"));
    std::fs::create_dir_all(&tmp_dir).unwrap();

    // Write module files.
    for (name, content) in modules {
        std::fs::write(tmp_dir.join(name), content).unwrap();
    }

    // Write main file.
    let main_path = tmp_dir.join("main.fg");
    std::fs::write(&main_path, main_source).unwrap();

    let result = run_file(main_path.to_str().unwrap());

    // Clean up.
    let _ = std::fs::remove_dir_all(&tmp_dir);

    result
}

#[test]
fn import_function_from_module() {
    let out = run_with_modules(
        r#"use math

fn main() {
    print(double(21))
}"#,
        &[(
            "math.fg",
            r#"fn double(x: i32) -> i32 {
    x * 2
}"#,
        )],
    );
    assert_eq!(out, vec!["42"]);
}

#[test]
fn import_struct_and_impl() {
    let out = run_with_modules(
        r#"use geometry

fn main() {
    let c = Circle.new(5.0)
    print(c.area())
}"#,
        &[(
            "geometry.fg",
            r#"struct Circle {
    radius: f64,
}

impl Circle {
    fn new(r: f64) -> Circle {
        Circle { radius: r }
    }

    fn area(self) -> f64 {
        3.14159 * self.radius * self.radius
    }
}"#,
        )],
    );
    assert_eq!(out[0], "78.53975");
}

#[test]
fn import_multiple_modules() {
    let out = run_with_modules(
        r#"use math
use strings

fn main() {
    print(double(21))
    print(greet("World"))
}"#,
        &[
            ("math.fg", r#"fn double(x: i32) -> i32 { x * 2 }"#),
            (
                "strings.fg",
                r#"fn greet(name: str) -> str { "Hello, {name}!" }"#,
            ),
        ],
    );
    assert_eq!(out, vec!["42", "Hello, World!"]);
}

#[test]
fn module_does_not_import_main() {
    // The module has a main() but it shouldn't be imported.
    let out = run_with_modules(
        r#"use helper

fn main() {
    print(add(1, 2))
}"#,
        &[(
            "helper.fg",
            r#"fn add(a: i32, b: i32) -> i32 { a + b }
fn main() { print("this should not run") }"#,
        )],
    );
    assert_eq!(out, vec!["3"]);
}

#[test]
fn circular_import_handled() {
    // a.fg imports b.fg, b.fg imports a.fg — should not infinite loop.
    let out = run_with_modules(
        r#"use a

fn main() {
    print(from_a())
}"#,
        &[
            (
                "a.fg",
                r#"use b
fn from_a() -> i32 { from_b() + 1 }"#,
            ),
            (
                "b.fg",
                r#"use a
fn from_b() -> i32 { 41 }"#,
            ),
        ],
    );
    assert_eq!(out, vec!["42"]);
}

#[test]
fn use_parses_correctly() {
    // Verify the parser handles `use` without errors.
    let source = "use foo\nuse bar.baz\nfn main() {}";
    let (tokens, _) = Lexer::new(source).tokenize();
    let (program, errors) = Parser::new(tokens).parse();
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    assert_eq!(program.items.len(), 3); // 2 use + 1 fn
}
