use crate::comptime::evaluate_comptime;
use crate::hir::lower::lower;
use crate::hir::*;
use crate::interpreter::Interpreter;
use crate::lexer::Lexer;
use crate::parser::Parser;

/// Helper: parse, lower, evaluate comptime, return HIR.
fn eval_ok(source: &str) -> HirProgram {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty(), "Lex errors: {:?}", lex_errors);
    let (program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "Parse errors: {:?}", parse_errors);
    let hir = lower(&program);
    let (result, errors) = evaluate_comptime(&hir);
    assert!(errors.is_empty(), "Comptime errors: {:?}", errors);
    result
}

/// Helper: run through the full pipeline (parse, lower, comptime, interpret).
fn run_with_comptime(source: &str) -> Vec<String> {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty());
    let (program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty());

    // For comptime, we run the interpreter on the original AST
    // (the interpreter handles comptime blocks natively since they're
    // just blocks in the AST).
    let mut interp = Interpreter::new_capturing();
    interp.run(&program).expect("Runtime error");
    interp.get_output().to_vec()
}

// ── Basic comptime evaluation ───────────────────────────────────────────

#[test]
fn comptime_integer() {
    let hir = eval_ok(
        r#"fn main() {
    let x = comptime { 2 + 3 }
}"#,
    );
    // The comptime block should be replaced with IntLiteral(5).
    let func = match &hir.items[0].kind {
        HirItemKind::Function(f) => f,
        _ => panic!("Expected function"),
    };
    match &func.body.stmts[0].kind {
        HirStmtKind::Let {
            value: Some(expr), ..
        } => {
            assert!(
                matches!(&expr.kind, HirExprKind::IntLiteral(5)),
                "Expected IntLiteral(5), got {:?}",
                expr.kind
            );
        }
        other => panic!("Expected Let with value, got {:?}", other),
    }
}

#[test]
fn comptime_string() {
    let hir = eval_ok(
        r#"fn main() {
    let msg = comptime { "hello from comptime" }
}"#,
    );
    let func = match &hir.items[0].kind {
        HirItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        HirStmtKind::Let {
            value: Some(expr), ..
        } => {
            assert!(
                matches!(&expr.kind, HirExprKind::StringLiteral(s) if s == "hello from comptime")
            );
        }
        other => panic!("Expected Let, got {:?}", other),
    }
}

#[test]
fn comptime_with_computation() {
    let hir = eval_ok(
        r#"fn main() {
    let factorial = comptime {
        let mut result = 1
        for i in 1..6 {
            result = result * i
        }
        result
    }
}"#,
    );
    // comptime evaluates the block: 1*1*2*3*4*5 = 120
    let func = match &hir.items[0].kind {
        HirItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        HirStmtKind::Let {
            value: Some(expr), ..
        } => {
            assert!(
                matches!(&expr.kind, HirExprKind::IntLiteral(120)),
                "Expected IntLiteral(120), got {:?}",
                expr.kind
            );
        }
        other => panic!("Expected Let, got {:?}", other),
    }
}

// ── Comptime in interpreter ─────────────────────────────────────────────

#[test]
fn interpreter_comptime_block() {
    let output = run_with_comptime(
        r#"fn main() {
    let x = comptime {
        let mut sum = 0
        for i in 1..11 {
            sum = sum + i
        }
        sum
    }
    print(x)
}"#,
    );
    // comptime evaluates to 55 (sum of 1..10).
    assert_eq!(output, vec!["55"]);
}

#[test]
fn comptime_fib() {
    let output = run_with_comptime(
        r#"fn main() {
    let fib10 = comptime {
        let mut a = 0
        let mut b = 1
        let mut i = 0
        while i < 10 {
            let temp = b
            b = a + b
            a = temp
            i = i + 1
        }
        a
    }
    print(fib10)
}"#,
    );
    assert_eq!(output, vec!["55"]);
}
