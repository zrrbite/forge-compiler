//! Compile-time evaluation for Forge.
//!
//! Evaluates `comptime { }` blocks at compile time using the tree-walk
//! interpreter. The result replaces the comptime block in the HIR with
//! a constant literal.
//!
//! This is Forge's answer to macros and templates: same language, no
//! second syntax to learn.

#[cfg(test)]
mod tests;

use crate::ast;
use crate::hir::*;
use crate::interpreter::{Interpreter, Value};
use crate::lexer::token::Span;

/// Errors from comptime evaluation.
#[derive(Debug, Clone)]
pub struct ComptimeError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for ComptimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Comptime error at {}..{}: {}",
            self.span.start, self.span.end, self.message
        )
    }
}

/// Evaluate all comptime blocks in a program.
/// Returns a new program with comptime blocks replaced by their results.
pub fn evaluate_comptime(program: &HirProgram) -> (HirProgram, Vec<ComptimeError>) {
    let mut evaluator = ComptimeEvaluator::new();
    let new_program = evaluator.eval_program(program);
    (new_program, evaluator.errors)
}

struct ComptimeEvaluator {
    errors: Vec<ComptimeError>,
    next_id: u32,
}

impl ComptimeEvaluator {
    fn new() -> Self {
        Self {
            errors: Vec::new(),
            next_id: 100_000, // Start high to avoid conflicts with lowering IDs.
        }
    }

    fn next_id(&mut self) -> HirId {
        let id = HirId(self.next_id);
        self.next_id += 1;
        id
    }

    fn eval_program(&mut self, program: &HirProgram) -> HirProgram {
        HirProgram {
            items: program.items.iter().map(|i| self.eval_item(i)).collect(),
        }
    }

    fn eval_item(&mut self, item: &HirItem) -> HirItem {
        let kind = match &item.kind {
            HirItemKind::Function(func) => HirItemKind::Function(self.eval_function(func)),
            HirItemKind::Impl(imp) => HirItemKind::Impl(HirImplBlock {
                generic_params: imp.generic_params.clone(),
                trait_name: imp.trait_name.clone(),
                target: imp.target.clone(),
                methods: imp.methods.iter().map(|m| self.eval_function(m)).collect(),
            }),
            other => other.clone(),
        };
        HirItem {
            id: item.id,
            kind,
            span: item.span,
        }
    }

    fn eval_function(&mut self, func: &HirFunction) -> HirFunction {
        HirFunction {
            name: func.name.clone(),
            params: func.params.clone(),
            return_type: func.return_type.clone(),
            body: self.eval_block(&func.body),
        }
    }

    fn eval_block(&mut self, block: &HirBlock) -> HirBlock {
        HirBlock {
            id: block.id,
            stmts: block.stmts.iter().map(|s| self.eval_stmt(s)).collect(),
            span: block.span,
        }
    }

    fn eval_stmt(&mut self, stmt: &HirStmt) -> HirStmt {
        let kind = match &stmt.kind {
            HirStmtKind::Let {
                mutable,
                name,
                ty,
                value,
            } => HirStmtKind::Let {
                mutable: *mutable,
                name: name.clone(),
                ty: ty.clone(),
                value: value.as_ref().map(|e| self.eval_expr(e)),
            },
            HirStmtKind::Expr(expr) => HirStmtKind::Expr(self.eval_expr(expr)),
            HirStmtKind::Return(expr) => {
                HirStmtKind::Return(expr.as_ref().map(|e| self.eval_expr(e)))
            }
            other => other.clone(),
        };
        HirStmt {
            id: stmt.id,
            kind,
            span: stmt.span,
        }
    }

    fn eval_expr(&mut self, expr: &HirExpr) -> HirExpr {
        match &expr.kind {
            HirExprKind::Comptime(block) => {
                // Convert the HIR block back to an AST program and interpret it.
                match self.interpret_block(block, expr.span) {
                    Ok(value) => self.value_to_hir_expr(value, expr.span),
                    Err(msg) => {
                        self.errors.push(ComptimeError {
                            message: msg,
                            span: expr.span,
                        });
                        // Return a placeholder.
                        HirExpr {
                            id: self.next_id(),
                            kind: HirExprKind::IntLiteral(0),
                            span: expr.span,
                        }
                    }
                }
            }
            // Recurse into sub-expressions.
            HirExprKind::BinaryOp { left, op, right } => HirExpr {
                id: expr.id,
                kind: HirExprKind::BinaryOp {
                    left: Box::new(self.eval_expr(left)),
                    op: *op,
                    right: Box::new(self.eval_expr(right)),
                },
                span: expr.span,
            },
            HirExprKind::Call { callee, args } => HirExpr {
                id: expr.id,
                kind: HirExprKind::Call {
                    callee: Box::new(self.eval_expr(callee)),
                    args: args.iter().map(|a| self.eval_expr(a)).collect(),
                },
                span: expr.span,
            },
            HirExprKind::If {
                condition,
                then_block,
                else_block,
            } => HirExpr {
                id: expr.id,
                kind: HirExprKind::If {
                    condition: Box::new(self.eval_expr(condition)),
                    then_block: self.eval_block(then_block),
                    else_block: else_block.as_ref().map(|e| Box::new(self.eval_expr(e))),
                },
                span: expr.span,
            },
            HirExprKind::Block(block) => HirExpr {
                id: expr.id,
                kind: HirExprKind::Block(self.eval_block(block)),
                span: expr.span,
            },
            // Leaf nodes — no comptime inside.
            _ => expr.clone(),
        }
    }

    /// Interpret a comptime block using the tree-walk interpreter.
    fn interpret_block(&mut self, block: &HirBlock, span: Span) -> Result<Value, String> {
        // Build a minimal AST program with a main() that contains the block.
        // Wrap the last expression in print() so we can capture its value.
        let mut ast_block = hir_block_to_ast(block);
        if let Some(last_stmt) = ast_block.stmts.last()
            && let ast::StmtKind::Expr(expr) = &last_stmt.kind
        {
            let print_call = ast::Expr {
                kind: ast::ExprKind::Call {
                    callee: Box::new(ast::Expr {
                        kind: ast::ExprKind::Identifier("print".to_string()),
                        span,
                    }),
                    args: vec![expr.clone()],
                },
                span,
            };
            let last_idx = ast_block.stmts.len() - 1;
            ast_block.stmts[last_idx] = ast::Stmt {
                kind: ast::StmtKind::Expr(print_call),
                span,
            };
        }
        let program = ast::Program {
            items: vec![ast::Item {
                kind: ast::ItemKind::Function(ast::Function {
                    name: "main".to_string(),
                    params: vec![],
                    return_type: None,
                    body: ast_block,
                }),
                span,
            }],
        };

        let mut interp = Interpreter::new_capturing();
        match interp.run(&program) {
            Ok(()) => {
                // The output is the captured print output.
                // For comptime, we want the *value*, not the output.
                // The interpreter returns the last expression's value via
                // its block evaluation. Since we can't easily get that,
                // we'll use the captured output as a string value.
                let output = interp.get_output().join("\n");
                if output.is_empty() {
                    Ok(Value::Unit)
                } else if let Ok(n) = output.parse::<i128>() {
                    Ok(Value::Int(n))
                } else if let Ok(f) = output.parse::<f64>() {
                    Ok(Value::Float(f))
                } else {
                    Ok(Value::String(output))
                }
            }
            Err(e) => Err(format!("Comptime evaluation failed: {e}")),
        }
    }

    /// Convert an interpreter Value to a HIR expression.
    fn value_to_hir_expr(&mut self, value: Value, span: Span) -> HirExpr {
        let kind = match value {
            Value::Int(n) => HirExprKind::IntLiteral(n),
            Value::Float(f) => HirExprKind::FloatLiteral(f),
            Value::Bool(b) => HirExprKind::BoolLiteral(b),
            Value::String(s) => HirExprKind::StringLiteral(s),
            Value::Unit => HirExprKind::IntLiteral(0),
            _ => {
                self.errors.push(ComptimeError {
                    message: format!(
                        "Comptime block returned unsupported type: {}",
                        value.type_name()
                    ),
                    span,
                });
                HirExprKind::IntLiteral(0)
            }
        };
        HirExpr {
            id: self.next_id(),
            kind,
            span,
        }
    }
}

/// Convert a HIR block back to an AST block for interpretation.
/// This is a simplified conversion — it handles the common cases.
fn hir_block_to_ast(block: &HirBlock) -> ast::Block {
    ast::Block {
        stmts: block.stmts.iter().map(hir_stmt_to_ast).collect(),
        span: block.span,
    }
}

fn hir_stmt_to_ast(stmt: &HirStmt) -> ast::Stmt {
    let kind = match &stmt.kind {
        HirStmtKind::Let {
            mutable,
            name,
            value,
            ..
        } => ast::StmtKind::Let {
            mutable: *mutable,
            name: name.clone(),
            ty: None,
            value: value.as_ref().map(hir_expr_to_ast),
        },
        HirStmtKind::Expr(expr) => ast::StmtKind::Expr(hir_expr_to_ast(expr)),
        HirStmtKind::Return(expr) => ast::StmtKind::Return(expr.as_ref().map(hir_expr_to_ast)),
        HirStmtKind::Break => ast::StmtKind::Break,
        HirStmtKind::Continue => ast::StmtKind::Continue,
        HirStmtKind::Defer(expr) => ast::StmtKind::Expr(hir_expr_to_ast(expr)),
    };
    ast::Stmt {
        kind,
        span: stmt.span,
    }
}

fn hir_expr_to_ast(expr: &HirExpr) -> ast::Expr {
    let kind = match &expr.kind {
        HirExprKind::IntLiteral(n) => ast::ExprKind::IntLiteral(*n),
        HirExprKind::FloatLiteral(f) => ast::ExprKind::FloatLiteral(*f),
        HirExprKind::BoolLiteral(b) => ast::ExprKind::BoolLiteral(*b),
        HirExprKind::StringLiteral(s) => ast::ExprKind::StringLiteral(s.clone()),
        HirExprKind::Identifier(name) => ast::ExprKind::Identifier(name.clone()),
        HirExprKind::BinaryOp { left, op, right } => ast::ExprKind::BinaryOp {
            left: Box::new(hir_expr_to_ast(left)),
            op: *op,
            right: Box::new(hir_expr_to_ast(right)),
        },
        HirExprKind::UnaryOp { op, expr } => ast::ExprKind::UnaryOp {
            op: *op,
            expr: Box::new(hir_expr_to_ast(expr)),
        },
        HirExprKind::Call { callee, args } => ast::ExprKind::Call {
            callee: Box::new(hir_expr_to_ast(callee)),
            args: args.iter().map(hir_expr_to_ast).collect(),
        },
        HirExprKind::If {
            condition,
            then_block,
            else_block,
        } => ast::ExprKind::If {
            condition: Box::new(hir_expr_to_ast(condition)),
            then_block: hir_block_to_ast(then_block),
            else_block: else_block.as_ref().map(|e| Box::new(hir_expr_to_ast(e))),
        },
        HirExprKind::Block(block) => ast::ExprKind::Block(hir_block_to_ast(block)),
        HirExprKind::For {
            binding,
            iter,
            body,
        } => ast::ExprKind::For {
            binding: binding.clone(),
            iter: Box::new(hir_expr_to_ast(iter)),
            body: hir_block_to_ast(body),
        },
        HirExprKind::While { condition, body } => ast::ExprKind::While {
            condition: Box::new(hir_expr_to_ast(condition)),
            body: hir_block_to_ast(body),
        },
        HirExprKind::Range {
            start,
            end,
            inclusive,
        } => ast::ExprKind::Range {
            start: start.as_ref().map(|e| Box::new(hir_expr_to_ast(e))),
            end: end.as_ref().map(|e| Box::new(hir_expr_to_ast(e))),
            inclusive: *inclusive,
        },
        HirExprKind::Array(elements) => {
            ast::ExprKind::Array(elements.iter().map(hir_expr_to_ast).collect())
        }
        HirExprKind::Assign { target, value } => ast::ExprKind::Assign {
            target: Box::new(hir_expr_to_ast(target)),
            op: None,
            value: Box::new(hir_expr_to_ast(value)),
        },
        HirExprKind::SelfValue => ast::ExprKind::SelfValue,
        HirExprKind::FieldAccess { object, field } => ast::ExprKind::FieldAccess {
            object: Box::new(hir_expr_to_ast(object)),
            field: field.clone(),
        },
        HirExprKind::Index { object, index } => ast::ExprKind::Index {
            object: Box::new(hir_expr_to_ast(object)),
            index: Box::new(hir_expr_to_ast(index)),
        },
        HirExprKind::Closure { params, body } => ast::ExprKind::Closure {
            params: params
                .iter()
                .map(|p| ast::ClosureParam {
                    name: p.name.clone(),
                    ty: None,
                    span: p.span,
                })
                .collect(),
            body: Box::new(hir_expr_to_ast(body)),
        },
        // Default: just return an int literal 0 for unsupported nodes.
        _ => ast::ExprKind::IntLiteral(0),
    };
    ast::Expr {
        kind,
        span: expr.span,
    }
}
