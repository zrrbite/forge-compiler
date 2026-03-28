use crate::ast::*;
use crate::lexer::Lexer;
use crate::parser::Parser;

/// Helper: lex + parse source, assert no errors, return the program.
fn parse_ok(source: &str) -> Program {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(
        lex_errors.is_empty(),
        "Unexpected lex errors: {:?}",
        lex_errors
    );
    let (program, parse_errors) = Parser::new(tokens).parse();
    assert!(
        parse_errors.is_empty(),
        "Unexpected parse errors: {:?}",
        parse_errors
    );
    program
}

/// Helper: lex + parse source, expect parse errors.
fn parse_errors(source: &str) -> Vec<String> {
    let (tokens, _) = Lexer::new(source).tokenize();
    let (_, errors) = Parser::new(tokens).parse();
    errors.into_iter().map(|e| e.message).collect()
}

// ── Functions ───────────────────────────────────────────────────────────

#[test]
fn empty_function() {
    let prog = parse_ok("fn main() {}");
    assert_eq!(prog.items.len(), 1);
    match &prog.items[0].kind {
        ItemKind::Function(f) => {
            assert_eq!(f.name, "main");
            assert!(f.params.is_empty());
            assert!(f.return_type.is_none());
            assert!(f.body.stmts.is_empty());
        }
        other => panic!("Expected function, got {:?}", other),
    }
}

#[test]
fn function_with_params_and_return() {
    let prog = parse_ok("fn add(a: i32, b: i32) -> i32 { a + b }");
    match &prog.items[0].kind {
        ItemKind::Function(f) => {
            assert_eq!(f.name, "add");
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.params[0].name, "a");
            assert_eq!(f.params[1].name, "b");
            assert!(f.return_type.is_some());
        }
        other => panic!("Expected function, got {:?}", other),
    }
}

#[test]
fn function_with_self_param() {
    let prog = parse_ok(
        r#"impl Foo {
    fn bar(self) -> i32 { 42 }
}"#,
    );
    match &prog.items[0].kind {
        ItemKind::Impl(imp) => {
            assert_eq!(imp.target, "Foo");
            assert_eq!(imp.methods[0].params[0].name, "self");
        }
        other => panic!("Expected impl, got {:?}", other),
    }
}

#[test]
fn function_with_mut_self() {
    let prog = parse_ok(
        r#"impl Buf {
    fn push(mut self, val: u8) {}
}"#,
    );
    match &prog.items[0].kind {
        ItemKind::Impl(imp) => {
            assert!(imp.methods[0].params[0].mutable);
            assert_eq!(imp.methods[0].params[0].name, "self");
        }
        other => panic!("Expected impl, got {:?}", other),
    }
}

// ── Let bindings ────────────────────────────────────────────────────────

#[test]
fn let_binding_simple() {
    let prog = parse_ok("fn main() { let x = 42 }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Let {
            mutable,
            name,
            ty,
            value,
        } => {
            assert!(!mutable);
            assert_eq!(name, "x");
            assert!(ty.is_none());
            assert!(value.is_some());
        }
        other => panic!("Expected let, got {:?}", other),
    }
}

#[test]
fn let_binding_with_type() {
    let prog = parse_ok("fn main() { let x: i32 = 42 }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Let { name, ty, .. } => {
            assert_eq!(name, "x");
            assert!(ty.is_some());
        }
        other => panic!("Expected let, got {:?}", other),
    }
}

#[test]
fn let_mut_binding() {
    let prog = parse_ok("fn main() { let mut count = 0 }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Let { mutable, name, .. } => {
            assert!(mutable);
            assert_eq!(name, "count");
        }
        other => panic!("Expected let, got {:?}", other),
    }
}

// ── Expressions ─────────────────────────────────────────────────────────

#[test]
fn binary_operator_precedence() {
    // a + b * c should parse as a + (b * c)
    let prog = parse_ok("fn main() { a + b * c }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::BinaryOp { op, right, .. } => {
                assert_eq!(*op, BinOp::Add);
                match &right.kind {
                    ExprKind::BinaryOp { op, .. } => assert_eq!(*op, BinOp::Mul),
                    other => panic!("Expected BinaryOp, got {:?}", other),
                }
            }
            other => panic!("Expected BinaryOp, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn unary_negation() {
    let prog = parse_ok("fn main() { -x }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::UnaryOp { op, .. } => assert_eq!(*op, UnaryOp::Neg),
            other => panic!("Expected UnaryOp, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn function_call() {
    let prog = parse_ok("fn main() { print(x, y) }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::Call { callee, args } => {
                assert!(matches!(&callee.kind, ExprKind::Identifier(n) if n == "print"));
                assert_eq!(args.len(), 2);
            }
            other => panic!("Expected Call, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn method_call() {
    let prog = parse_ok("fn main() { buf.push(0xFF) }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::Call { callee, args } => {
                match &callee.kind {
                    ExprKind::FieldAccess { field, .. } => assert_eq!(field, "push"),
                    other => panic!("Expected FieldAccess, got {:?}", other),
                }
                assert_eq!(args.len(), 1);
            }
            other => panic!("Expected Call, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn field_access() {
    let prog = parse_ok("fn main() { self.x }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::FieldAccess { object, field } => {
                assert!(matches!(&object.kind, ExprKind::SelfValue));
                assert_eq!(field, "x");
            }
            other => panic!("Expected FieldAccess, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn chained_method_calls() {
    let prog = parse_ok("fn main() { a.b().c() }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    // Should be: Call(FieldAccess(Call(FieldAccess(a, b)), c))
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::Call { callee, .. } => match &callee.kind {
                ExprKind::FieldAccess { field, object } => {
                    assert_eq!(field, "c");
                    assert!(matches!(&object.kind, ExprKind::Call { .. }));
                }
                other => panic!("Expected FieldAccess, got {:?}", other),
            },
            other => panic!("Expected Call, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn assignment() {
    let prog = parse_ok("fn main() { count += 1 }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::Assign { op, .. } => {
                assert_eq!(*op, Some(BinOp::Add));
            }
            other => panic!("Expected Assign, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn string_interpolation() {
    let prog = parse_ok(r#"fn main() { "Hello, {name}!" }"#);
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 3);
                assert!(matches!(&parts[0], StringPart::Literal(s) if s == "Hello, "));
                assert!(
                    matches!(&parts[1], StringPart::Expr(e) if matches!(&e.kind, ExprKind::Identifier(n) if n == "name"))
                );
                assert!(matches!(&parts[2], StringPart::Literal(s) if s == "!"));
            }
            other => panic!("Expected InterpolatedString, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn try_operator() {
    let prog = parse_ok("fn main() { foo()? }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => {
            assert!(matches!(&expr.kind, ExprKind::Try(_)));
        }
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn array_literal() {
    let prog = parse_ok("fn main() { [1, 2, 3] }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::Array(elems) => assert_eq!(elems.len(), 3),
            other => panic!("Expected Array, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn range_expr() {
    let prog = parse_ok("fn main() { 0..100 }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::Range {
                inclusive,
                start,
                end,
                ..
            } => {
                assert!(!inclusive);
                assert!(start.is_some());
                assert!(end.is_some());
            }
            other => panic!("Expected Range, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn closure_expr() {
    let prog = parse_ok("fn main() { |x| x * x }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::Closure { params, .. } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].name, "x");
            }
            other => panic!("Expected Closure, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

// ── Structs ─────────────────────────────────────────────────────────────

#[test]
fn struct_definition() {
    let prog = parse_ok(
        r#"struct Vec2 {
    x: f64,
    y: f64,
}"#,
    );
    match &prog.items[0].kind {
        ItemKind::Struct(s) => {
            assert_eq!(s.name, "Vec2");
            assert_eq!(s.fields.len(), 2);
            assert_eq!(s.fields[0].name, "x");
            assert_eq!(s.fields[1].name, "y");
        }
        other => panic!("Expected Struct, got {:?}", other),
    }
}

#[test]
fn struct_literal() {
    let prog = parse_ok("fn main() { Vec2 { x: 1.0, y: 2.0 } }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::StructLiteral { fields, .. } => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "x");
                assert!(fields[0].value.is_some());
                assert_eq!(fields[1].name, "y");
            }
            other => panic!("Expected StructLiteral, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn struct_literal_shorthand() {
    let prog = parse_ok("fn main() { Vec2 { x, y } }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::StructLiteral { fields, .. } => {
                assert_eq!(fields.len(), 2);
                assert!(fields[0].value.is_none()); // shorthand
                assert!(fields[1].value.is_none());
            }
            other => panic!("Expected StructLiteral, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

// ── Enums ───────────────────────────────────────────────────────────────

#[test]
fn enum_definition() {
    let prog = parse_ok(
        r#"enum AppError {
    NotFound(str),
    ParseError(str),
    IoError(str),
}"#,
    );
    match &prog.items[0].kind {
        ItemKind::Enum(e) => {
            assert_eq!(e.name, "AppError");
            assert_eq!(e.variants.len(), 3);
            assert_eq!(e.variants[0].name, "NotFound");
            assert_eq!(e.variants[0].fields.len(), 1);
        }
        other => panic!("Expected Enum, got {:?}", other),
    }
}

// ── Impl blocks ─────────────────────────────────────────────────────────

#[test]
fn impl_block() {
    let prog = parse_ok(
        r#"impl Vec2 {
    fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }
}"#,
    );
    match &prog.items[0].kind {
        ItemKind::Impl(imp) => {
            assert_eq!(imp.target, "Vec2");
            assert!(imp.trait_name.is_none());
            assert_eq!(imp.methods.len(), 1);
            assert_eq!(imp.methods[0].name, "new");
        }
        other => panic!("Expected Impl, got {:?}", other),
    }
}

#[test]
fn impl_trait_for_type() {
    let prog = parse_ok(
        r#"impl Add for Vec2 {
    fn add(self, other: Vec2) -> Vec2 {
        Vec2 { x: self.x + other.x, y: self.y + other.y }
    }
}"#,
    );
    match &prog.items[0].kind {
        ItemKind::Impl(imp) => {
            assert_eq!(imp.trait_name.as_deref(), Some("Add"));
            assert_eq!(imp.target, "Vec2");
        }
        other => panic!("Expected Impl, got {:?}", other),
    }
}

// ── Traits ──────────────────────────────────────────────────────────────

#[test]
fn trait_definition() {
    let prog = parse_ok(
        r#"trait Area {
    fn area(self) -> f64
    fn describe(self) -> str {
        "shape"
    }
}"#,
    );
    match &prog.items[0].kind {
        ItemKind::Trait(t) => {
            assert_eq!(t.name, "Area");
            assert_eq!(t.methods.len(), 2);
            assert!(t.methods[0].body.is_none()); // declaration only
            assert!(t.methods[1].body.is_some()); // default impl
        }
        other => panic!("Expected Trait, got {:?}", other),
    }
}

// ── Control flow ────────────────────────────────────────────────────────

#[test]
fn if_else_expr() {
    let prog = parse_ok(
        r#"fn main() {
    if x > 0 {
        "positive"
    } else {
        "non-positive"
    }
}"#,
    );
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::If { else_block, .. } => {
                assert!(else_block.is_some());
            }
            other => panic!("Expected If, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn match_expr() {
    let prog = parse_ok(
        r#"fn main() {
    match x {
        0 => "zero",
        _ => "other",
    }
}"#,
    );
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                assert!(matches!(&arms[1].pattern.kind, PatternKind::Wildcard));
            }
            other => panic!("Expected Match, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn for_loop() {
    let prog = parse_ok(
        r#"fn main() {
    for x in items {
        print(x)
    }
}"#,
    );
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::For { binding, .. } => {
                assert_eq!(binding, "x");
            }
            other => panic!("Expected For, got {:?}", other),
        },
        other => panic!("Expected Expr, got {:?}", other),
    }
}

#[test]
fn while_loop() {
    let prog = parse_ok(
        r#"fn main() {
    while !done {
        step()
    }
}"#,
    );
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.body.stmts[0].kind {
        StmtKind::Expr(expr) => {
            assert!(matches!(&expr.kind, ExprKind::While { .. }));
        }
        other => panic!("Expected Expr, got {:?}", other),
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[test]
fn generic_return_type() {
    let prog = parse_ok("fn foo() -> Result<u16> { Ok(42) }");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.return_type.as_ref().unwrap().kind {
        TypeExprKind::Generic { name, args } => {
            assert_eq!(name, "Result");
            assert_eq!(args.len(), 1);
        }
        other => panic!("Expected Generic type, got {:?}", other),
    }
}

#[test]
fn reference_type() {
    let prog = parse_ok("fn foo(x: &Buffer) {}");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    match &func.params[0].ty.kind {
        TypeExprKind::Reference { mutable, .. } => {
            assert!(!mutable);
        }
        other => panic!("Expected Reference type, got {:?}", other),
    }
}

#[test]
fn array_type() {
    let prog = parse_ok("fn foo(x: [u8]) {}");
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    assert!(matches!(
        &func.params[0].ty.kind,
        TypeExprKind::Array { size: None, .. }
    ));
}

// ── Integration: Forge sample programs ──────────────────────────────────

#[test]
fn forge_hello_world() {
    let source = r#"fn greet(name: str) -> str {
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
}"#;
    let prog = parse_ok(source);
    assert_eq!(prog.items.len(), 2);
    assert!(matches!(&prog.items[0].kind, ItemKind::Function(f) if f.name == "greet"));
    assert!(matches!(&prog.items[1].kind, ItemKind::Function(f) if f.name == "main"));
}

#[test]
fn forge_vec2() {
    let source = r#"struct Vec2 {
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

impl Add for Vec2 {
    fn add(self, other: Vec2) -> Vec2 {
        Vec2 { x: self.x + other.x, y: self.y + other.y }
    }
}

fn main() {
    let a = Vec2.new(3.0, 4.0)
    let b = Vec2.new(1.0, 2.0)
    let c = a + b
}"#;
    let prog = parse_ok(source);
    assert_eq!(prog.items.len(), 4); // struct, impl, impl Add, fn main
}

#[test]
fn forge_error_handling() {
    let source = r#"enum AppError {
    NotFound(str),
    ParseError(str),
    IoError(str),
}

fn load_config(path: str) -> Result<u16> {
    let content = fs.read(path)?
    let port_str = content.trim()
    parse_port(port_str)?
}

fn main() {
    match load_config("server.conf") {
        Ok(port)                     => print(port),
        Err(AppError.NotFound(path)) => print(path),
        _ => print("error"),
    }
}"#;
    let prog = parse_ok(source);
    assert_eq!(prog.items.len(), 3); // enum, fn load_config, fn main
}

#[test]
fn forge_trait() {
    let source = r#"trait Area {
    fn area(self) -> f64
    fn perimeter(self) -> f64

    fn describe(self) -> str {
        "shape"
    }
}

struct Circle { radius: f64 }

impl Area for Circle {
    fn area(self) -> f64 { 3.14 * self.radius * self.radius }
    fn perimeter(self) -> f64 { 2.0 * 3.14 * self.radius }
}"#;
    let prog = parse_ok(source);
    assert_eq!(prog.items.len(), 3); // trait, struct, impl
}

// ── Error handling ──────────────────────────────────────────────────────

#[test]
fn error_missing_brace() {
    let errors = parse_errors("fn main(");
    assert!(!errors.is_empty());
}

#[test]
fn error_unexpected_token() {
    let errors = parse_errors("42");
    assert!(!errors.is_empty());
}

#[test]
fn return_statement() {
    let prog = parse_ok(
        r#"fn foo() -> i32 {
    return 42
}"#,
    );
    let func = match &prog.items[0].kind {
        ItemKind::Function(f) => f,
        _ => panic!(),
    };
    assert!(matches!(
        &func.body.stmts[0].kind,
        StmtKind::Return(Some(_))
    ));
}
