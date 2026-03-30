//! AST → HIR lowering pass.
//!
//! This pass desugars the AST into a simpler HIR representation:
//! - Compound assignments `x += 1` become `x = x + 1`
//! - Field shorthand `Foo { x }` becomes `Foo { x: x }`
//! - String interpolation becomes string concatenation
//! - All nodes get unique HirIds for later annotation

use crate::ast;
use crate::hir::*;

/// Lowers an AST program into HIR.
pub struct Lowering {
    next_id: u32,
}

impl Default for Lowering {
    fn default() -> Self {
        Self::new()
    }
}

impl Lowering {
    pub fn new() -> Self {
        Self { next_id: 0 }
    }

    fn next_id(&mut self) -> HirId {
        let id = HirId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn lower_program(&mut self, program: &ast::Program) -> HirProgram {
        HirProgram {
            items: program
                .items
                .iter()
                .filter(|i| !matches!(&i.kind, ast::ItemKind::Use(_)))
                .map(|i| self.lower_item(i))
                .collect(),
        }
    }

    fn lower_item(&mut self, item: &ast::Item) -> HirItem {
        let id = self.next_id();
        let kind = match &item.kind {
            ast::ItemKind::Function(f) => HirItemKind::Function(self.lower_function(f)),
            ast::ItemKind::Struct(s) => HirItemKind::Struct(self.lower_struct(s)),
            ast::ItemKind::Enum(e) => HirItemKind::Enum(self.lower_enum(e)),
            ast::ItemKind::Impl(i) => HirItemKind::Impl(self.lower_impl(i)),
            ast::ItemKind::Trait(t) => HirItemKind::Trait(self.lower_trait(t)),
            ast::ItemKind::Use(_) => unreachable!("Use items should be resolved before lowering"),
        };
        HirItem {
            id,
            kind,
            span: item.span,
        }
    }

    fn lower_function(&mut self, func: &ast::Function) -> HirFunction {
        HirFunction {
            name: func.name.clone(),
            params: func.params.iter().map(|p| self.lower_param(p)).collect(),
            return_type: func.return_type.as_ref().map(|t| self.lower_type(t)),
            body: self.lower_block(&func.body),
        }
    }

    fn lower_param(&mut self, param: &ast::Param) -> HirParam {
        HirParam {
            id: self.next_id(),
            mutable: param.mutable,
            name: param.name.clone(),
            ty: self.lower_type(&param.ty),
            span: param.span,
        }
    }

    fn lower_struct(&mut self, s: &ast::StructDef) -> HirStructDef {
        HirStructDef {
            name: s.name.clone(),
            generic_params: s
                .generic_params
                .iter()
                .map(|g| self.lower_generic_param(g))
                .collect(),
            fields: s
                .fields
                .iter()
                .map(|f| HirField {
                    name: f.name.clone(),
                    ty: self.lower_type(&f.ty),
                    span: f.span,
                })
                .collect(),
        }
    }

    fn lower_enum(&mut self, e: &ast::EnumDef) -> HirEnumDef {
        HirEnumDef {
            name: e.name.clone(),
            variants: e
                .variants
                .iter()
                .map(|v| HirVariant {
                    name: v.name.clone(),
                    fields: v.fields.iter().map(|t| self.lower_type(t)).collect(),
                    span: v.span,
                })
                .collect(),
        }
    }

    fn lower_impl(&mut self, imp: &ast::ImplBlock) -> HirImplBlock {
        HirImplBlock {
            generic_params: imp
                .generic_params
                .iter()
                .map(|g| self.lower_generic_param(g))
                .collect(),
            trait_name: imp.trait_name.clone(),
            target: imp.target.clone(),
            methods: imp.methods.iter().map(|m| self.lower_function(m)).collect(),
        }
    }

    fn lower_trait(&mut self, t: &ast::TraitDef) -> HirTraitDef {
        HirTraitDef {
            name: t.name.clone(),
            generic_params: t
                .generic_params
                .iter()
                .map(|g| self.lower_generic_param(g))
                .collect(),
            methods: t
                .methods
                .iter()
                .map(|m| HirTraitMethod {
                    name: m.name.clone(),
                    params: m.params.iter().map(|p| self.lower_param(p)).collect(),
                    return_type: m.return_type.as_ref().map(|t| self.lower_type(t)),
                    body: m.body.as_ref().map(|b| self.lower_block(b)),
                    span: m.span,
                })
                .collect(),
        }
    }

    fn lower_generic_param(&mut self, g: &ast::GenericParam) -> HirGenericParam {
        HirGenericParam {
            name: g.name.clone(),
            bounds: g.bounds.iter().map(|b| self.lower_type(b)).collect(),
            span: g.span,
        }
    }

    // ── Blocks and statements ───────────────────────────────────────────

    fn lower_block(&mut self, block: &ast::Block) -> HirBlock {
        HirBlock {
            id: self.next_id(),
            stmts: block.stmts.iter().map(|s| self.lower_stmt(s)).collect(),
            span: block.span,
        }
    }

    fn lower_stmt(&mut self, stmt: &ast::Stmt) -> HirStmt {
        let id = self.next_id();
        let kind = match &stmt.kind {
            ast::StmtKind::Let {
                mutable,
                name,
                ty,
                value,
            } => HirStmtKind::Let {
                mutable: *mutable,
                name: name.clone(),
                ty: ty.as_ref().map(|t| self.lower_type(t)),
                value: value.as_ref().map(|e| self.lower_expr(e)),
            },
            ast::StmtKind::Expr(expr) => HirStmtKind::Expr(self.lower_expr(expr)),
            ast::StmtKind::Return(expr) => {
                HirStmtKind::Return(expr.as_ref().map(|e| self.lower_expr(e)))
            }
            ast::StmtKind::Break => HirStmtKind::Break,
            ast::StmtKind::Continue => HirStmtKind::Continue,
        };
        HirStmt {
            id,
            kind,
            span: stmt.span,
        }
    }

    // ── Expressions ─────────────────────────────────────────────────────

    fn lower_expr(&mut self, expr: &ast::Expr) -> HirExpr {
        let id = self.next_id();
        let span = expr.span;

        let kind = match &expr.kind {
            ast::ExprKind::IntLiteral(n) => HirExprKind::IntLiteral(*n),
            ast::ExprKind::FloatLiteral(f) => HirExprKind::FloatLiteral(*f),
            ast::ExprKind::BoolLiteral(b) => HirExprKind::BoolLiteral(*b),
            ast::ExprKind::StringLiteral(s) => HirExprKind::StringLiteral(s.clone()),

            // DESUGAR: string interpolation → StringConcat
            ast::ExprKind::InterpolatedString(parts) => {
                let hir_parts = parts
                    .iter()
                    .map(|part| match part {
                        ast::StringPart::Literal(s) => HirExpr {
                            id: self.next_id(),
                            kind: HirExprKind::StringLiteral(s.clone()),
                            span,
                        },
                        ast::StringPart::Expr(e) => self.lower_expr(e),
                    })
                    .collect();
                HirExprKind::StringConcat(hir_parts)
            }

            ast::ExprKind::Identifier(name) => HirExprKind::Identifier(name.clone()),
            ast::ExprKind::SelfValue => HirExprKind::SelfValue,

            ast::ExprKind::BinaryOp { left, op, right } => HirExprKind::BinaryOp {
                left: Box::new(self.lower_expr(left)),
                op: *op,
                right: Box::new(self.lower_expr(right)),
            },

            ast::ExprKind::UnaryOp { op, expr } => HirExprKind::UnaryOp {
                op: *op,
                expr: Box::new(self.lower_expr(expr)),
            },

            // DESUGAR: compound assignment `x += 1` → `x = x + 1`
            ast::ExprKind::Assign { target, op, value } => {
                let hir_target = self.lower_expr(target);
                let hir_value = self.lower_expr(value);

                let final_value = match op {
                    Some(bin_op) => {
                        // Clone the target to use on the right side of the binop.
                        let target_copy = self.clone_hir_expr(&hir_target);
                        HirExpr {
                            id: self.next_id(),
                            kind: HirExprKind::BinaryOp {
                                left: Box::new(target_copy),
                                op: *bin_op,
                                right: Box::new(hir_value),
                            },
                            span,
                        }
                    }
                    None => hir_value,
                };

                HirExprKind::Assign {
                    target: Box::new(hir_target),
                    value: Box::new(final_value),
                }
            }

            ast::ExprKind::Call { callee, args } => HirExprKind::Call {
                callee: Box::new(self.lower_expr(callee)),
                args: args.iter().map(|a| self.lower_expr(a)).collect(),
            },

            ast::ExprKind::FieldAccess { object, field } => HirExprKind::FieldAccess {
                object: Box::new(self.lower_expr(object)),
                field: field.clone(),
            },

            ast::ExprKind::Index { object, index } => HirExprKind::Index {
                object: Box::new(self.lower_expr(object)),
                index: Box::new(self.lower_expr(index)),
            },

            ast::ExprKind::Turbofish { expr, types } => HirExprKind::Turbofish {
                expr: Box::new(self.lower_expr(expr)),
                types: types.iter().map(|t| self.lower_type(t)).collect(),
            },

            ast::ExprKind::Block(block) => HirExprKind::Block(self.lower_block(block)),

            ast::ExprKind::If {
                condition,
                then_block,
                else_block,
            } => HirExprKind::If {
                condition: Box::new(self.lower_expr(condition)),
                then_block: self.lower_block(then_block),
                else_block: else_block.as_ref().map(|e| Box::new(self.lower_expr(e))),
            },

            ast::ExprKind::Match { expr, arms } => HirExprKind::Match {
                expr: Box::new(self.lower_expr(expr)),
                arms: arms
                    .iter()
                    .map(|a| HirMatchArm {
                        pattern: self.lower_pattern(&a.pattern),
                        body: self.lower_expr(&a.body),
                        span: a.span,
                    })
                    .collect(),
            },

            ast::ExprKind::For {
                binding,
                iter,
                body,
            } => HirExprKind::For {
                binding: binding.clone(),
                iter: Box::new(self.lower_expr(iter)),
                body: self.lower_block(body),
            },

            ast::ExprKind::While { condition, body } => HirExprKind::While {
                condition: Box::new(self.lower_expr(condition)),
                body: self.lower_block(body),
            },

            ast::ExprKind::Comptime(block) => HirExprKind::Comptime(self.lower_block(block)),

            ast::ExprKind::Closure { params, body } => HirExprKind::Closure {
                params: params
                    .iter()
                    .map(|p| HirClosureParam {
                        name: p.name.clone(),
                        ty: p.ty.as_ref().map(|t| self.lower_type(t)),
                        span: p.span,
                    })
                    .collect(),
                body: Box::new(self.lower_expr(body)),
            },

            // DESUGAR: struct literal field shorthand
            // `Foo { x }` → `Foo { x: x }`
            ast::ExprKind::StructLiteral { name, fields } => {
                let type_name = match &name.kind {
                    ast::ExprKind::Identifier(n) => n.clone(),
                    _ => "<unknown>".to_string(),
                };

                let hir_fields = fields
                    .iter()
                    .map(|f| {
                        let value = match &f.value {
                            Some(expr) => self.lower_expr(expr),
                            // Shorthand: `Foo { x }` → `Foo { x: x }`
                            None => HirExpr {
                                id: self.next_id(),
                                kind: HirExprKind::Identifier(f.name.clone()),
                                span: f.span,
                            },
                        };
                        HirFieldInit {
                            name: f.name.clone(),
                            value,
                            span: f.span,
                        }
                    })
                    .collect();

                HirExprKind::StructLiteral {
                    name: type_name,
                    fields: hir_fields,
                }
            }

            ast::ExprKind::Array(elements) => {
                HirExprKind::Array(elements.iter().map(|e| self.lower_expr(e)).collect())
            }

            ast::ExprKind::Reference { mutable, expr } => HirExprKind::Reference {
                mutable: *mutable,
                expr: Box::new(self.lower_expr(expr)),
            },

            ast::ExprKind::Dereference(expr) => {
                HirExprKind::Dereference(Box::new(self.lower_expr(expr)))
            }

            ast::ExprKind::Try(expr) => HirExprKind::Try(Box::new(self.lower_expr(expr))),

            ast::ExprKind::SafeNav {
                object,
                field,
                call_args,
            } => HirExprKind::SafeNav {
                object: Box::new(self.lower_expr(object)),
                field: field.clone(),
                call_args: call_args
                    .as_ref()
                    .map(|args| args.iter().map(|a| self.lower_expr(a)).collect()),
            },

            ast::ExprKind::NullCoalesce { expr, default } => HirExprKind::NullCoalesce {
                expr: Box::new(self.lower_expr(expr)),
                default: Box::new(self.lower_expr(default)),
            },

            ast::ExprKind::Range {
                start,
                end,
                inclusive,
            } => HirExprKind::Range {
                start: start.as_ref().map(|e| Box::new(self.lower_expr(e))),
                end: end.as_ref().map(|e| Box::new(self.lower_expr(e))),
                inclusive: *inclusive,
            },
        };

        HirExpr { id, kind, span }
    }

    /// Clone a HIR expression, assigning fresh IDs.
    fn clone_hir_expr(&mut self, expr: &HirExpr) -> HirExpr {
        let new_id = self.next_id();
        HirExpr {
            id: new_id,
            kind: expr.kind.clone(),
            span: expr.span,
        }
    }

    // ── Patterns ────────────────────────────────────────────────────────

    fn lower_pattern(&mut self, pat: &ast::Pattern) -> HirPattern {
        let id = self.next_id();
        let kind = match &pat.kind {
            ast::PatternKind::Wildcard => HirPatternKind::Wildcard,
            ast::PatternKind::Identifier(name) => HirPatternKind::Identifier(name.clone()),
            ast::PatternKind::Literal(expr) => HirPatternKind::Literal(self.lower_expr(expr)),
            ast::PatternKind::Variant { path, fields } => HirPatternKind::Variant {
                path: path.clone(),
                fields: fields.iter().map(|p| self.lower_pattern(p)).collect(),
            },
        };
        HirPattern {
            id,
            kind,
            span: pat.span,
        }
    }

    // ── Types ───────────────────────────────────────────────────────────

    fn lower_type(&mut self, ty: &ast::TypeExpr) -> HirType {
        let id = self.next_id();
        let kind = match &ty.kind {
            ast::TypeExprKind::Named(name) => HirTypeKind::Named(name.clone()),
            ast::TypeExprKind::Generic { name, args } => HirTypeKind::Generic {
                name: name.clone(),
                args: args.iter().map(|a| self.lower_type(a)).collect(),
            },
            ast::TypeExprKind::Reference { mutable, inner } => HirTypeKind::Reference {
                mutable: *mutable,
                inner: Box::new(self.lower_type(inner)),
            },
            ast::TypeExprKind::Array { element, size } => HirTypeKind::Array {
                element: Box::new(self.lower_type(element)),
                size: size.as_ref().map(|s| Box::new(self.lower_expr(s))),
            },
            ast::TypeExprKind::ImplTrait(name) => HirTypeKind::ImplTrait(name.clone()),
            ast::TypeExprKind::Function {
                params,
                return_type,
            } => HirTypeKind::Function {
                params: params.iter().map(|p| self.lower_type(p)).collect(),
                return_type: Box::new(self.lower_type(return_type)),
            },
        };
        HirType {
            id,
            kind,
            span: ty.span,
        }
    }
}

/// Convenience function: lower an AST program to HIR.
pub fn lower(program: &ast::Program) -> HirProgram {
    Lowering::new().lower_program(program)
}
