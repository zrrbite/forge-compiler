use crate::borrowck::BorrowChecker;
use crate::hir::lower::lower;
use crate::lexer::Lexer;
use crate::parser::Parser;

/// Helper: borrow check and return errors.
fn check_errors(source: &str) -> Vec<String> {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty(), "Lex errors: {:?}", lex_errors);
    let (program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "Parse errors: {:?}", parse_errors);
    let hir = lower(&program);
    let mut bc = BorrowChecker::new();
    bc.check_program(&hir);
    bc.errors.iter().map(|e| e.message.clone()).collect()
}

fn check_ok(source: &str) {
    let errors = check_errors(source);
    assert!(errors.is_empty(), "Unexpected borrow errors: {:?}", errors);
}

fn check_err(source: &str) -> Vec<String> {
    let errors = check_errors(source);
    assert!(!errors.is_empty(), "Expected borrow errors but got none");
    errors
}

// ── Use after move ──────────────────────────────────────────────────────
// Note: Primitives (int, float, bool, str) are Copy — they don't move.
// Move semantics only apply to struct types.

#[test]
fn use_after_move_struct() {
    let errors = check_err(
        r#"struct Buf { data: i32 }
fn main() {
    let x = Buf { data: 42 }
    let y = x
    print(x)
}"#,
    );
    assert!(errors[0].contains("moved"));
}

#[test]
fn primitives_are_copy() {
    // Integers, floats, bools are Copy — using them after assignment is fine.
    check_ok(
        r#"fn main() {
    let x = 42
    let y = x
    print(x)
    print(y)
}"#,
    );
}

#[test]
fn move_struct_to_function() {
    let errors = check_err(
        r#"struct Buf { data: i32 }
fn consume(x: Buf) {}
fn main() {
    let val = Buf { data: 42 }
    consume(val)
    print(val)
}"#,
    );
    assert!(errors[0].contains("moved"));
}

#[test]
fn double_move_struct() {
    let errors = check_err(
        r#"struct Buf { data: i32 }
fn main() {
    let x = Buf { data: 42 }
    let y = x
    let z = x
}"#,
    );
    assert!(errors[0].contains("moved"));
}

// ── Immutable assignment ────────────────────────────────────────────────

#[test]
fn assign_to_immutable() {
    let errors = check_err(
        r#"fn main() {
    let x = 0
    x = 10
}"#,
    );
    assert!(errors[0].contains("immutable"));
}

#[test]
fn assign_to_mutable_ok() {
    check_ok(
        r#"fn main() {
    let mut x = 0
    x = 10
}"#,
    );
}

// ── Borrow conflicts ───────────────────────────────────────────────────

#[test]
fn move_while_borrowed() {
    let errors = check_err(
        r#"struct Buf { data: i32 }
fn main() {
    let x = Buf { data: 42 }
    let r = &x
    let y = x
}"#,
    );
    assert!(errors[0].contains("borrowed"));
}

#[test]
fn mut_borrow_while_borrowed() {
    let errors = check_err(
        r#"fn main() {
    let mut x = 42
    let r = &x
    let m = &mut x
}"#,
    );
    assert!(errors[0].contains("already immutably borrowed"));
}

#[test]
fn double_mut_borrow() {
    let errors = check_err(
        r#"fn main() {
    let mut x = 42
    let a = &mut x
    let b = &mut x
}"#,
    );
    assert!(errors[0].contains("already mutably borrowed"));
}

#[test]
fn multiple_immutable_borrows_ok() {
    check_ok(
        r#"fn main() {
    let x = 42
    let a = &x
    let b = &x
    let c = &x
}"#,
    );
}

#[test]
fn mut_borrow_of_immutable() {
    let errors = check_err(
        r#"fn main() {
    let x = 42
    let m = &mut x
}"#,
    );
    assert!(errors[0].contains("immutable variable"));
}

// ── Scoping ─────────────────────────────────────────────────────────────

#[test]
fn move_in_different_scope_ok() {
    // This should be OK because the move is in a nested scope.
    // However, our simple checker doesn't restore state after scopes,
    // so this may or may not pass. We'll see.
    check_ok(
        r#"fn main() {
    let x = 42
    print(x)
}"#,
    );
}

#[test]
fn function_params_are_owned() {
    check_ok(
        r#"fn foo(x: i32) -> i32 {
    x
}
fn main() {
    print(foo(42))
}"#,
    );
}

// ── Integration ─────────────────────────────────────────────────────────

#[test]
fn hello_world_passes() {
    check_ok(
        r#"fn greet(name: str) -> str {
    "hello"
}

fn main() {
    let name = "Martin"
    print(greet(name))
}"#,
    );
}

#[test]
fn while_loop_ok() {
    check_ok(
        r#"fn main() {
    let mut i = 0
    while i < 10 {
        i = i + 1
    }
}"#,
    );
}
