use crate::hir::lower::lower;
use crate::hir::*;
use crate::lexer::Lexer;
use crate::parser::Parser;

/// Helper: parse source and lower to HIR.
fn lower_ok(source: &str) -> HirProgram {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty(), "Lex errors: {:?}", lex_errors);
    let (program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "Parse errors: {:?}", parse_errors);
    lower(&program)
}

// ── Basic lowering ──────────────────────────────────────────────────────

#[test]
fn lower_empty_function() {
    let hir = lower_ok("fn main() {}");
    assert_eq!(hir.items.len(), 1);
    match &hir.items[0].kind {
        HirItemKind::Function(f) => {
            assert_eq!(f.name, "main");
            assert!(f.body.stmts.is_empty());
        }
        _ => panic!("Expected function"),
    }
}

#[test]
fn lower_function_with_params() {
    let hir = lower_ok("fn add(a: i32, b: i32) -> i32 { a + b }");
    match &hir.items[0].kind {
        HirItemKind::Function(f) => {
            assert_eq!(f.params.len(), 2);
            assert!(f.return_type.is_some());
        }
        _ => panic!("Expected function"),
    }
}

#[test]
fn lower_struct() {
    let hir = lower_ok("struct Vec2 { x: f64, y: f64 }");
    match &hir.items[0].kind {
        HirItemKind::Struct(s) => {
            assert_eq!(s.name, "Vec2");
            assert_eq!(s.fields.len(), 2);
        }
        _ => panic!("Expected struct"),
    }
}

#[test]
fn lower_enum() {
    let hir = lower_ok("enum Color { Red, Green, Blue }");
    match &hir.items[0].kind {
        HirItemKind::Enum(e) => {
            assert_eq!(e.name, "Color");
            assert_eq!(e.variants.len(), 3);
        }
        _ => panic!("Expected enum"),
    }
}

// ── Desugaring: compound assignment ─────────────────────────────────────

#[test]
fn desugar_compound_assignment() {
    let hir = lower_ok("fn main() { x += 1 }");
    let func = match &hir.items[0].kind {
        HirItemKind::Function(f) => f,
        _ => panic!("Expected function"),
    };
    // Should be: x = x + 1
    match &func.body.stmts[0].kind {
        HirStmtKind::Expr(expr) => match &expr.kind {
            HirExprKind::Assign { target, value } => {
                // target is `x`
                assert!(matches!(&target.kind, HirExprKind::Identifier(n) if n == "x"));
                // value is `x + 1`
                match &value.kind {
                    HirExprKind::BinaryOp { left, op, right } => {
                        assert!(matches!(&left.kind, HirExprKind::Identifier(n) if n == "x"));
                        assert_eq!(*op, BinOp::Add);
                        assert!(matches!(&right.kind, HirExprKind::IntLiteral(1)));
                    }
                    other => panic!("Expected BinaryOp, got {:?}", other),
                }
            }
            other => panic!("Expected Assign, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn desugar_mul_assign() {
    let hir = lower_ok("fn main() { x *= 3 }");
    let func = match &hir.items[0].kind {
        HirItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        HirStmtKind::Expr(expr) => match &expr.kind {
            HirExprKind::Assign { value, .. } => match &value.kind {
                HirExprKind::BinaryOp { op, .. } => {
                    assert_eq!(*op, BinOp::Mul);
                }
                _ => panic!("Expected BinaryOp"),
            },
            _ => panic!("Expected Assign"),
        },
        _ => panic!("Expected Expr"),
    }
}

// ── Desugaring: field shorthand ─────────────────────────────────────────

#[test]
fn desugar_struct_shorthand() {
    let hir = lower_ok("fn main() { Vec2 { x, y } }");
    let func = match &hir.items[0].kind {
        HirItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        HirStmtKind::Expr(expr) => match &expr.kind {
            HirExprKind::StructLiteral { name, fields } => {
                assert_eq!(name, "Vec2");
                assert_eq!(fields.len(), 2);
                // Both fields should have explicit values (identifiers)
                assert!(matches!(
                    &fields[0].value.kind,
                    HirExprKind::Identifier(n) if n == "x"
                ));
                assert!(matches!(
                    &fields[1].value.kind,
                    HirExprKind::Identifier(n) if n == "y"
                ));
            }
            _ => panic!("Expected StructLiteral"),
        },
        _ => panic!("Expected Expr"),
    }
}

// ── Desugaring: string interpolation ────────────────────────────────────

#[test]
fn desugar_string_interpolation() {
    let hir = lower_ok(r#"fn main() { "Hello, {name}!" }"#);
    let func = match &hir.items[0].kind {
        HirItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        HirStmtKind::Expr(expr) => match &expr.kind {
            HirExprKind::StringConcat(parts) => {
                assert_eq!(parts.len(), 3);
                assert!(matches!(&parts[0].kind, HirExprKind::StringLiteral(s) if s == "Hello, "));
                assert!(matches!(&parts[1].kind, HirExprKind::Identifier(n) if n == "name"));
                assert!(matches!(&parts[2].kind, HirExprKind::StringLiteral(s) if s == "!"));
            }
            _ => panic!("Expected StringConcat"),
        },
        _ => panic!("Expected Expr"),
    }
}

#[test]
fn plain_string_not_concat() {
    let hir = lower_ok(r#"fn main() { "hello" }"#);
    let func = match &hir.items[0].kind {
        HirItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        HirStmtKind::Expr(expr) => {
            assert!(matches!(&expr.kind, HirExprKind::StringLiteral(s) if s == "hello"));
        }
        _ => panic!("Expected Expr"),
    }
}

// ── HirIds are unique ───────────────────────────────────────────────────

#[test]
fn hir_ids_are_unique() {
    let hir = lower_ok(
        r#"fn main() {
    let x = 1
    let y = 2
    x + y
}"#,
    );
    // Collect all IDs and check for duplicates.
    let mut ids = Vec::new();
    collect_item_ids(&hir.items[0], &mut ids);
    let unique: std::collections::HashSet<u32> = ids.iter().map(|id| id.0).collect();
    assert_eq!(ids.len(), unique.len(), "Duplicate HirIds found");
}

fn collect_item_ids(item: &HirItem, ids: &mut Vec<HirId>) {
    ids.push(item.id);
    match &item.kind {
        HirItemKind::Function(f) => {
            for p in &f.params {
                ids.push(p.id);
            }
            collect_block_ids(&f.body, ids);
        }
        _ => {}
    }
}

fn collect_block_ids(block: &HirBlock, ids: &mut Vec<HirId>) {
    ids.push(block.id);
    for stmt in &block.stmts {
        ids.push(stmt.id);
        match &stmt.kind {
            HirStmtKind::Expr(expr)
            | HirStmtKind::Let {
                value: Some(expr), ..
            } => {
                collect_expr_ids(expr, ids);
            }
            _ => {}
        }
    }
}

fn collect_expr_ids(expr: &HirExpr, ids: &mut Vec<HirId>) {
    ids.push(expr.id);
    match &expr.kind {
        HirExprKind::BinaryOp { left, right, .. } => {
            collect_expr_ids(left, ids);
            collect_expr_ids(right, ids);
        }
        HirExprKind::Assign { target, value } => {
            collect_expr_ids(target, ids);
            collect_expr_ids(value, ids);
        }
        HirExprKind::StringConcat(parts) => {
            for p in parts {
                collect_expr_ids(p, ids);
            }
        }
        _ => {}
    }
}

// ── Integration: full program ───────────────────────────────────────────

#[test]
fn lower_hello_world() {
    let hir = lower_ok(
        r#"fn greet(name: str) -> str {
    "Hello, {name}!"
}

fn main() {
    let name = "Martin"
    print(greet(name))
}"#,
    );
    assert_eq!(hir.items.len(), 2);
    // greet function body should contain a StringConcat
    match &hir.items[0].kind {
        HirItemKind::Function(f) => {
            assert_eq!(f.name, "greet");
            match &f.body.stmts[0].kind {
                HirStmtKind::Expr(expr) => {
                    assert!(matches!(&expr.kind, HirExprKind::StringConcat(_)));
                }
                _ => panic!("Expected StringConcat"),
            }
        }
        _ => panic!("Expected function"),
    }
}

#[test]
fn lower_vec2() {
    let hir = lower_ok(
        r#"struct Vec2 {
    x: f64,
    y: f64,
}

impl Vec2 {
    fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }
}

fn main() {
    let v = Vec2.new(3.0, 4.0)
    print(v.x)
}"#,
    );
    assert_eq!(hir.items.len(), 3); // struct, impl, fn main
    // The struct literal in new() should have desugared shorthand fields.
    match &hir.items[1].kind {
        HirItemKind::Impl(imp) => {
            let new_fn = &imp.methods[0];
            // Body should contain a struct literal with explicit field values.
            match &new_fn.body.stmts[0].kind {
                HirStmtKind::Expr(expr) => match &expr.kind {
                    HirExprKind::StructLiteral { fields, .. } => {
                        // Both shorthand fields should be desugared to identifiers.
                        assert!(matches!(
                            &fields[0].value.kind,
                            HirExprKind::Identifier(n) if n == "x"
                        ));
                        assert!(matches!(
                            &fields[1].value.kind,
                            HirExprKind::Identifier(n) if n == "y"
                        ));
                    }
                    _ => panic!("Expected StructLiteral"),
                },
                _ => panic!("Expected Expr"),
            }
        }
        _ => panic!("Expected Impl"),
    }
}
