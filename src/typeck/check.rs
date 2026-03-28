//! The Forge type checker.
//!
//! Walks the HIR, infers types for expressions, checks that operations
//! are type-safe, and reports errors with source spans.

use crate::hir::*;
use crate::lexer::token::Span;

use super::scope::*;
use super::types::*;

/// A type error with location information.
#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Type error at {}..{}: {}",
            self.span.start, self.span.end, self.message
        )
    }
}

/// The type checker state.
pub struct TypeChecker {
    pub symbols: SymbolTable,
    pub unifier: UnificationTable,
    pub errors: Vec<TypeError>,
    /// Currently active type parameters (e.g., ["T"] inside struct Stack<T>).
    active_type_params: Vec<String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut tc = Self {
            symbols: SymbolTable::new(),
            unifier: UnificationTable::new(),
            errors: Vec::new(),
            active_type_params: Vec::new(),
        };
        tc.register_builtins();
        tc
    }

    fn register_builtins(&mut self) {
        // print() accepts any type, returns unit.
        self.symbols.register_fn(
            "print".into(),
            FnInfo {
                params: vec![("value".into(), Ty::Str)],
                ret: Ty::Unit,
            },
        );
        // Result/Option constructors.
        let t = Ty::TypeParam("T".into());
        let e = Ty::TypeParam("E".into());
        self.symbols.register_fn(
            "Ok".into(),
            FnInfo {
                params: vec![("value".into(), t.clone())],
                ret: Ty::GenericInstance {
                    name: "Result".into(),
                    args: vec![t.clone()],
                },
            },
        );
        self.symbols.register_fn(
            "Err".into(),
            FnInfo {
                params: vec![("error".into(), e.clone())],
                ret: Ty::GenericInstance {
                    name: "Result".into(),
                    args: vec![e],
                },
            },
        );
        self.symbols.register_fn(
            "Some".into(),
            FnInfo {
                params: vec![("value".into(), t.clone())],
                ret: Ty::GenericInstance {
                    name: "Option".into(),
                    args: vec![t],
                },
            },
        );
        // Other stdlib builtins.
        for name in [
            "println",
            "eprint",
            "to_str",
            "to_int",
            "to_float",
            "abs",
            "min",
            "max",
            "assert",
            "assert_eq",
        ] {
            self.symbols.register_fn(
                name.into(),
                FnInfo {
                    params: vec![("arg".into(), Ty::Str)],
                    ret: Ty::Unit,
                },
            );
        }
    }

    fn error(&mut self, message: String, span: Span) {
        self.errors.push(TypeError { message, span });
    }

    /// Type check a complete program.
    pub fn check_program(&mut self, program: &HirProgram) {
        // First pass: register all type declarations and function signatures.
        for item in &program.items {
            self.register_item(item);
        }

        // Second pass: type check function bodies.
        for item in &program.items {
            self.check_item(item);
        }
    }

    // ── Registration (first pass) ───────────────────────────────────────

    fn register_item(&mut self, item: &HirItem) {
        match &item.kind {
            HirItemKind::Function(func) => {
                let params: Vec<(String, Ty)> = func
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), self.resolve_hir_type(&p.ty)))
                    .collect();
                let ret = func
                    .return_type
                    .as_ref()
                    .map(|t| self.resolve_hir_type(t))
                    .unwrap_or(Ty::Unit);
                self.symbols
                    .register_fn(func.name.clone(), FnInfo { params, ret });
            }
            HirItemKind::Struct(s) => {
                let type_params: Vec<String> =
                    s.generic_params.iter().map(|g| g.name.clone()).collect();
                // Activate type params during field type resolution.
                let saved = self.active_type_params.clone();
                self.active_type_params.extend(type_params.iter().cloned());
                let fields: Vec<(String, Ty)> = s
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), self.resolve_hir_type(&f.ty)))
                    .collect();
                self.active_type_params = saved;
                self.symbols.register_struct(StructInfo {
                    name: s.name.clone(),
                    type_params,
                    fields,
                });
            }
            HirItemKind::Enum(e) => {
                let variants: Vec<(String, Vec<Ty>)> = e
                    .variants
                    .iter()
                    .map(|v| {
                        let fields = v.fields.iter().map(|t| self.resolve_hir_type(t)).collect();
                        (v.name.clone(), fields)
                    })
                    .collect();
                self.symbols.register_enum(EnumInfo {
                    name: e.name.clone(),
                    variants,
                });
            }
            HirItemKind::Impl(imp) => {
                for method in &imp.methods {
                    let params: Vec<(String, Ty)> = method
                        .params
                        .iter()
                        .map(|p| (p.name.clone(), self.resolve_hir_type(&p.ty)))
                        .collect();
                    let ret = method
                        .return_type
                        .as_ref()
                        .map(|t| self.resolve_hir_type(t))
                        .unwrap_or(Ty::Unit);
                    let is_instance = method.params.first().is_some_and(|p| p.name == "self");
                    self.symbols.register_method(
                        imp.target.clone(),
                        MethodInfo {
                            name: method.name.clone(),
                            is_instance,
                            params,
                            ret,
                        },
                    );
                }
            }
            HirItemKind::Trait(_) => {
                // Trait definitions are registered for later constraint checking.
                // For now we just skip them.
            }
        }
    }

    // ── Checking (second pass) ──────────────────────────────────────────

    fn check_item(&mut self, item: &HirItem) {
        match &item.kind {
            HirItemKind::Function(func) => self.check_function(func, None),
            HirItemKind::Impl(imp) => {
                for method in &imp.methods {
                    self.check_function(method, Some(&imp.target));
                }
            }
            // Struct, Enum, Trait don't have bodies to check.
            _ => {}
        }
    }

    fn check_function(&mut self, func: &HirFunction, impl_target: Option<&str>) {
        self.symbols.push_scope();

        // Bind parameters, resolving Self to the impl target type.
        for param in &func.params {
            let mut ty = self.resolve_hir_type(&param.ty);
            // Replace Self/Named("Self") with the impl target.
            if let Some(target) = impl_target {
                ty = self.resolve_self_type(ty, target);
            }
            self.symbols.define_var(
                param.name.clone(),
                VarInfo {
                    ty,
                    mutable: param.mutable,
                },
            );
        }

        let body_ty = self.check_block(&func.body);

        // Check return type matches.
        let expected_ret = func
            .return_type
            .as_ref()
            .map(|t| self.resolve_hir_type(t))
            .unwrap_or(Ty::Unit);

        if let Err(msg) = self.unifier.unify(&body_ty, &expected_ret) {
            self.error(format!("Function '{}': {msg}", func.name), func.body.span);
        }

        self.symbols.pop_scope();
    }

    fn check_block(&mut self, block: &HirBlock) -> Ty {
        let mut last_ty = Ty::Unit;

        for (i, stmt) in block.stmts.iter().enumerate() {
            let is_last = i == block.stmts.len() - 1;
            match &stmt.kind {
                HirStmtKind::Let {
                    mutable,
                    name,
                    ty,
                    value,
                } => {
                    let annotated = ty.as_ref().map(|t| self.resolve_hir_type(t));
                    let inferred = value.as_ref().map(|e| self.check_expr(e));

                    let final_ty = match (annotated, inferred) {
                        (Some(ann), Some(inf)) => {
                            if let Err(msg) = self.unifier.unify(&ann, &inf) {
                                self.error(format!("In let binding '{name}': {msg}"), stmt.span);
                            }
                            ann
                        }
                        (Some(ann), None) => ann,
                        (None, Some(inf)) => inf,
                        (None, None) => {
                            self.error(
                                format!("Cannot infer type for '{name}' without initializer or annotation"),
                                stmt.span,
                            );
                            Ty::Error
                        }
                    };

                    self.symbols.define_var(
                        name.clone(),
                        VarInfo {
                            ty: final_ty,
                            mutable: *mutable,
                        },
                    );
                    last_ty = Ty::Unit;
                }
                HirStmtKind::Expr(expr) => {
                    let ty = self.check_expr(expr);
                    if is_last {
                        last_ty = ty;
                    }
                }
                HirStmtKind::Return(expr) => {
                    let _ty = expr.as_ref().map(|e| self.check_expr(e));
                    // Return type checking is handled at the function level.
                    last_ty = Ty::Unit;
                }
                HirStmtKind::Break | HirStmtKind::Continue => {
                    last_ty = Ty::Unit;
                }
            }
        }

        last_ty
    }

    // ── Expression type checking ────────────────────────────────────────

    fn check_expr(&mut self, expr: &HirExpr) -> Ty {
        match &expr.kind {
            HirExprKind::IntLiteral(_) => Ty::default_int(),
            HirExprKind::FloatLiteral(_) => Ty::default_float(),
            HirExprKind::BoolLiteral(_) => Ty::Bool,
            HirExprKind::StringLiteral(_) => Ty::Str,

            HirExprKind::StringConcat(parts) => {
                // All parts should be convertible to strings.
                for part in parts {
                    self.check_expr(part);
                }
                Ty::Str
            }

            HirExprKind::Identifier(name) => {
                // Look up as variable first, then as function.
                if let Some(info) = self.symbols.lookup_var(name) {
                    return info.ty.clone();
                }
                if let Some(info) = self.symbols.lookup_fn(name) {
                    return Ty::Function {
                        params: info.params.iter().map(|(_, t)| t.clone()).collect(),
                        ret: Box::new(info.ret.clone()),
                    };
                }
                // Could be a type name (for static method calls).
                if self.symbols.lookup_struct(name).is_some()
                    || self.symbols.lookup_enum(name).is_some()
                {
                    return Ty::Named(name.clone());
                }
                self.error(format!("Undefined variable: '{name}'"), expr.span);
                Ty::Error
            }

            HirExprKind::SelfValue => {
                if let Some(info) = self.symbols.lookup_var("self") {
                    info.ty.clone()
                } else {
                    self.error("'self' used outside of method".into(), expr.span);
                    Ty::Error
                }
            }

            HirExprKind::BinaryOp { left, op, right } => {
                let left_ty = self.check_expr(left);
                let right_ty = self.check_expr(right);
                self.check_binop(&left_ty, *op, &right_ty, expr.span)
            }

            HirExprKind::UnaryOp { op, expr: inner } => {
                let ty = self.check_expr(inner);
                match op {
                    UnaryOp::Neg => {
                        if ty.is_numeric() {
                            ty
                        } else {
                            self.error(format!("Cannot negate type {ty}"), expr.span);
                            Ty::Error
                        }
                    }
                    UnaryOp::Not => {
                        if ty == Ty::Bool {
                            Ty::Bool
                        } else {
                            self.error(format!("Cannot apply ! to type {ty}"), expr.span);
                            Ty::Error
                        }
                    }
                    UnaryOp::Ref => Ty::Ref {
                        mutable: false,
                        inner: Box::new(ty),
                    },
                    UnaryOp::Deref => match ty {
                        Ty::Ref { inner, .. } => *inner,
                        _ => {
                            self.error(format!("Cannot dereference type {ty}"), expr.span);
                            Ty::Error
                        }
                    },
                }
            }

            HirExprKind::Assign { target, value } => {
                let target_ty = self.check_expr(target);
                let value_ty = self.check_expr(value);

                // Check mutability.
                if let HirExprKind::Identifier(name) = &target.kind
                    && let Some(info) = self.symbols.lookup_var(name)
                    && !info.mutable
                {
                    self.error(
                        format!("Cannot assign to immutable variable '{name}'"),
                        expr.span,
                    );
                }

                if let Err(msg) = self.unifier.unify(&target_ty, &value_ty) {
                    self.error(format!("In assignment: {msg}"), expr.span);
                }
                Ty::Unit
            }

            HirExprKind::Call { callee, args } => self.check_call(callee, args, expr.span),

            HirExprKind::FieldAccess { object, field } => {
                let obj_ty = self.check_expr(object);
                self.check_field_access(&obj_ty, field, expr.span)
            }

            HirExprKind::Index { object, index } => {
                let obj_ty = self.check_expr(object);
                let idx_ty = self.check_expr(index);

                if !idx_ty.is_integer() && !idx_ty.is_error() {
                    self.error(
                        format!("Index must be an integer, found {idx_ty}"),
                        expr.span,
                    );
                }

                match &obj_ty {
                    Ty::Array(inner) => *inner.clone(),
                    Ty::Error => Ty::Error,
                    _ => {
                        self.error(format!("Cannot index into type {obj_ty}"), expr.span);
                        Ty::Error
                    }
                }
            }

            HirExprKind::Block(block) => {
                self.symbols.push_scope();
                let ty = self.check_block(block);
                self.symbols.pop_scope();
                ty
            }

            HirExprKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let cond_ty = self.check_expr(condition);
                if cond_ty != Ty::Bool && !cond_ty.is_error() {
                    self.error(
                        format!("If condition must be bool, found {cond_ty}"),
                        expr.span,
                    );
                }

                self.symbols.push_scope();
                let then_ty = self.check_block(then_block);
                self.symbols.pop_scope();

                if let Some(else_expr) = else_block {
                    let else_ty = self.check_expr(else_expr);
                    if let Err(msg) = self.unifier.unify(&then_ty, &else_ty) {
                        self.error(
                            format!("If/else branches have different types: {msg}"),
                            expr.span,
                        );
                    }
                    then_ty
                } else {
                    Ty::Unit
                }
            }

            HirExprKind::Match {
                expr: match_expr,
                arms,
            } => {
                let scrutinee_ty = self.check_expr(match_expr);
                let mut result_ty: Option<Ty> = None;

                for arm in arms {
                    self.symbols.push_scope();
                    self.check_pattern(&arm.pattern, &scrutinee_ty);
                    let arm_ty = self.check_expr(&arm.body);
                    self.symbols.pop_scope();

                    match &result_ty {
                        Some(prev) => {
                            if let Err(msg) = self.unifier.unify(prev, &arm_ty) {
                                self.error(
                                    format!("Match arms have different types: {msg}"),
                                    arm.span,
                                );
                            }
                        }
                        None => result_ty = Some(arm_ty),
                    }
                }

                result_ty.unwrap_or(Ty::Unit)
            }

            HirExprKind::For {
                binding,
                iter,
                body,
            } => {
                let iter_ty = self.check_expr(iter);
                let elem_ty = match &iter_ty {
                    Ty::Array(inner) => *inner.clone(),
                    Ty::Error => Ty::Error,
                    _ => {
                        self.error(format!("Cannot iterate over type {iter_ty}"), expr.span);
                        Ty::Error
                    }
                };

                self.symbols.push_scope();
                self.symbols.define_var(
                    binding.clone(),
                    VarInfo {
                        ty: elem_ty,
                        mutable: false,
                    },
                );
                self.check_block(body);
                self.symbols.pop_scope();
                Ty::Unit
            }

            HirExprKind::While { condition, body } => {
                let cond_ty = self.check_expr(condition);
                if cond_ty != Ty::Bool && !cond_ty.is_error() {
                    self.error(
                        format!("While condition must be bool, found {cond_ty}"),
                        expr.span,
                    );
                }
                self.symbols.push_scope();
                self.check_block(body);
                self.symbols.pop_scope();
                Ty::Unit
            }

            HirExprKind::Closure { params, body } => {
                self.symbols.push_scope();
                let mut param_types = Vec::new();
                for p in params {
                    let ty =
                        p.ty.as_ref()
                            .map(|t| self.resolve_hir_type(t))
                            .unwrap_or_else(|| self.unifier.fresh_var());
                    self.symbols.define_var(
                        p.name.clone(),
                        VarInfo {
                            ty: ty.clone(),
                            mutable: false,
                        },
                    );
                    param_types.push(ty);
                }
                let ret_ty = self.check_expr(body);
                self.symbols.pop_scope();

                Ty::Function {
                    params: param_types,
                    ret: Box::new(ret_ty),
                }
            }

            HirExprKind::StructLiteral { name, fields } => {
                let struct_info = self.symbols.lookup_struct(name).cloned();
                match struct_info {
                    Some(info) => {
                        // Check each field exists and has the right type.
                        for field_init in fields {
                            let value_ty = self.check_expr(&field_init.value);
                            if let Some((_, expected_ty)) =
                                info.fields.iter().find(|(n, _)| n == &field_init.name)
                            {
                                if let Err(msg) = self.unifier.unify(expected_ty, &value_ty) {
                                    self.error(
                                        format!("Field '{}' of {name}: {msg}", field_init.name),
                                        field_init.span,
                                    );
                                }
                            } else {
                                self.error(
                                    format!("No field '{}' on struct {name}", field_init.name),
                                    field_init.span,
                                );
                            }
                        }
                        Ty::Named(name.clone())
                    }
                    None => {
                        self.error(format!("Unknown struct type: {name}"), expr.span);
                        Ty::Error
                    }
                }
            }

            HirExprKind::Array(elements) => {
                if elements.is_empty() {
                    return Ty::Array(Box::new(self.unifier.fresh_var()));
                }
                let first_ty = self.check_expr(&elements[0]);
                for elem in &elements[1..] {
                    let ty = self.check_expr(elem);
                    if let Err(msg) = self.unifier.unify(&first_ty, &ty) {
                        self.error(format!("Array element type mismatch: {msg}"), elem.span);
                    }
                }
                Ty::Array(Box::new(first_ty))
            }

            HirExprKind::Reference {
                mutable,
                expr: inner,
            } => {
                let ty = self.check_expr(inner);
                Ty::Ref {
                    mutable: *mutable,
                    inner: Box::new(ty),
                }
            }

            HirExprKind::Dereference(inner) => {
                let ty = self.check_expr(inner);
                match ty {
                    Ty::Ref { inner, .. } => *inner,
                    Ty::Error => Ty::Error,
                    _ => {
                        self.error(format!("Cannot dereference type {ty}"), expr.span);
                        Ty::Error
                    }
                }
            }

            HirExprKind::Try(inner) => {
                // ? on Result<T> → T (propagating Err upward).
                // For now, just return the inner type.
                self.check_expr(inner)
            }

            HirExprKind::Turbofish { expr: inner, .. } => {
                // Type params are used for resolution — just check the inner expr.
                self.check_expr(inner)
            }

            HirExprKind::Comptime(block) => {
                // Comptime blocks are type-checked like regular blocks.
                self.symbols.push_scope();
                let ty = self.check_block(block);
                self.symbols.pop_scope();
                ty
            }

            HirExprKind::Range { start, end, .. } => {
                if let Some(s) = start {
                    let ty = self.check_expr(s);
                    if !ty.is_integer() && !ty.is_error() {
                        self.error(
                            format!("Range bounds must be integers, found {ty}"),
                            expr.span,
                        );
                    }
                }
                if let Some(e) = end {
                    let ty = self.check_expr(e);
                    if !ty.is_integer() && !ty.is_error() {
                        self.error(
                            format!("Range bounds must be integers, found {ty}"),
                            expr.span,
                        );
                    }
                }
                Ty::Array(Box::new(Ty::default_int()))
            }
        }
    }

    // ── Call checking ───────────────────────────────────────────────────

    fn check_call(&mut self, callee: &HirExpr, args: &[HirExpr], span: Span) -> Ty {
        // Method call: expr.method(args) — callee is FieldAccess.
        if let HirExprKind::FieldAccess { object, field } = &callee.kind {
            let obj_ty = self.check_expr(object);
            return self.check_method_call(&obj_ty, field, args, span);
        }

        // Regular function call.
        let callee_ty = self.check_expr(callee);
        let arg_types: Vec<Ty> = args.iter().map(|a| self.check_expr(a)).collect();

        match &callee_ty {
            Ty::Function { params, ret } => {
                // print() is special — accepts any type.
                if let HirExprKind::Identifier(name) = &callee.kind
                    && name == "print"
                {
                    return Ty::Unit;
                }

                if params.len() != arg_types.len() {
                    self.error(
                        format!(
                            "Expected {} arguments, found {}",
                            params.len(),
                            arg_types.len()
                        ),
                        span,
                    );
                } else {
                    for (i, (expected, actual)) in params.iter().zip(arg_types.iter()).enumerate() {
                        if let Err(msg) = self.unifier.unify(expected, actual) {
                            self.error(format!("Argument {}: {msg}", i + 1), span);
                        }
                    }
                }
                *ret.clone()
            }
            Ty::Error => Ty::Error,
            _ => {
                self.error(format!("Not callable: {callee_ty}"), span);
                Ty::Error
            }
        }
    }

    fn check_method_call(&mut self, obj_ty: &Ty, method: &str, args: &[HirExpr], span: Span) -> Ty {
        let type_name = match obj_ty {
            Ty::Named(name) => name.clone(),
            Ty::Struct { name, .. } => name.clone(),
            Ty::Array(_) => return self.check_array_method(obj_ty, method, args, span),
            Ty::Str => return self.check_str_method(method, args, span),
            Ty::Float(_) => return self.check_float_method(method, span),
            Ty::Error => return Ty::Error,
            _ => {
                self.error(
                    format!("Cannot call method '{method}' on type {obj_ty}"),
                    span,
                );
                return Ty::Error;
            }
        };

        // Look up user-defined method.
        let method_info = self.symbols.lookup_method(&type_name, method).cloned();
        match method_info {
            Some(info) => {
                let arg_types: Vec<Ty> = args.iter().map(|a| self.check_expr(a)).collect();

                // For instance methods, skip the self param in the check.
                let expected_params: Vec<&(String, Ty)> = if info.is_instance {
                    info.params.iter().skip(1).collect()
                } else {
                    info.params.iter().collect()
                };

                if expected_params.len() != arg_types.len() {
                    self.error(
                        format!(
                            "Method '{method}' expects {} arguments, found {}",
                            expected_params.len(),
                            arg_types.len()
                        ),
                        span,
                    );
                } else {
                    for (i, ((_name, expected), actual)) in
                        expected_params.iter().zip(arg_types.iter()).enumerate()
                    {
                        if let Err(msg) = self.unifier.unify(expected, actual) {
                            self.error(
                                format!("Method '{method}' argument {}: {msg}", i + 1),
                                span,
                            );
                        }
                    }
                }
                info.ret.clone()
            }
            None => {
                self.error(format!("No method '{method}' on type '{type_name}'"), span);
                Ty::Error
            }
        }
    }

    fn check_array_method(
        &mut self,
        arr_ty: &Ty,
        method: &str,
        args: &[HirExpr],
        span: Span,
    ) -> Ty {
        let elem_ty = match arr_ty {
            Ty::Array(inner) => *inner.clone(),
            _ => return Ty::Error,
        };

        match method {
            "len" => Ty::default_int(),
            "push" => {
                for arg in args {
                    let ty = self.check_expr(arg);
                    if let Err(msg) = self.unifier.unify(&elem_ty, &ty) {
                        self.error(format!("push: {msg}"), span);
                    }
                }
                arr_ty.clone()
            }
            "pop" | "last" => {
                // Returns Option<T> — for now just the element type.
                elem_ty
            }
            "map" => {
                if args.len() != 1 {
                    self.error("map() takes one argument".into(), span);
                    return Ty::Error;
                }
                let func_ty = self.check_expr(&args[0]);
                match func_ty {
                    Ty::Function { ret, .. } => Ty::Array(ret),
                    _ => {
                        // Closure type inference — just return Array of fresh var.
                        Ty::Array(Box::new(self.unifier.fresh_var()))
                    }
                }
            }
            "filter" => arr_ty.clone(),
            "fold" => {
                if args.len() != 2 {
                    self.error("fold() takes two arguments".into(), span);
                    return Ty::Error;
                }
                self.check_expr(&args[0])
            }
            "each" => {
                if args.len() == 1 {
                    self.check_expr(&args[0]);
                }
                Ty::Unit
            }
            _ => {
                self.error(format!("No method '{method}' on arrays"), span);
                Ty::Error
            }
        }
    }

    fn check_str_method(&mut self, method: &str, args: &[HirExpr], span: Span) -> Ty {
        match method {
            "len" => Ty::default_int(),
            "trim" => Ty::Str,
            "contains" => {
                if args.len() == 1 {
                    let ty = self.check_expr(&args[0]);
                    if ty != Ty::Str && !ty.is_error() {
                        self.error("contains() argument must be a string".into(), span);
                    }
                }
                Ty::Bool
            }
            _ => {
                self.error(format!("No method '{method}' on str"), span);
                Ty::Error
            }
        }
    }

    fn check_float_method(&mut self, method: &str, span: Span) -> Ty {
        match method {
            "sqrt" | "abs" | "sin" | "cos" => Ty::default_float(),
            _ => {
                self.error(format!("No method '{method}' on float"), span);
                Ty::Error
            }
        }
    }

    // ── Field access ────────────────────────────────────────────────────

    fn check_field_access(&mut self, obj_ty: &Ty, field: &str, span: Span) -> Ty {
        let type_name = match obj_ty {
            Ty::Named(name) => name.clone(),
            Ty::Struct { name, .. } => name.clone(),
            Ty::Error => return Ty::Error,
            _ => {
                self.error(
                    format!("Cannot access field '{field}' on type {obj_ty}"),
                    span,
                );
                return Ty::Error;
            }
        };

        // Look up in struct definition.
        if let Some(info) = self.symbols.lookup_struct(&type_name)
            && let Some((_, ty)) = info.fields.iter().find(|(n, _)| n == field)
        {
            return ty.clone();
        }

        self.error(format!("No field '{field}' on type '{type_name}'"), span);
        Ty::Error
    }

    // ── Patterns ────────────────────────────────────────────────────────

    fn check_pattern(&mut self, pattern: &HirPattern, expected: &Ty) {
        match &pattern.kind {
            HirPatternKind::Wildcard => {}
            HirPatternKind::Identifier(name) => {
                self.symbols.define_var(
                    name.clone(),
                    VarInfo {
                        ty: expected.clone(),
                        mutable: false,
                    },
                );
            }
            HirPatternKind::Literal(expr) => {
                let ty = self.check_expr(expr);
                if let Err(msg) = self.unifier.unify(expected, &ty) {
                    self.error(format!("Pattern type mismatch: {msg}"), pattern.span);
                }
            }
            HirPatternKind::Variant { path, fields } => {
                // For now, bind sub-patterns without deep checking.
                for field_pat in fields {
                    // Each field gets a fresh type var.
                    let ty = self.unifier.fresh_var();
                    self.check_pattern(field_pat, &ty);
                }
                let _ = path; // TODO: verify variant exists on enum
            }
        }
    }

    // ── Binary operators ────────────────────────────────────────────────

    /// Map a binary operator to its trait name for overloading.
    fn op_trait_name(op: BinOp) -> Option<&'static str> {
        match op {
            BinOp::Add => Some("Add"),
            BinOp::Sub => Some("Sub"),
            BinOp::Mul => Some("Mul"),
            BinOp::Div => Some("Div"),
            BinOp::Mod => Some("Mod"),
            BinOp::Eq => Some("Eq"),
            BinOp::NotEq => Some("Eq"),
            BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => Some("Ord"),
            BinOp::And | BinOp::Or => None,
        }
    }

    /// Map a binary operator to the method name on its trait.
    fn op_method_name(op: BinOp) -> &'static str {
        match op {
            BinOp::Add => "add",
            BinOp::Sub => "sub",
            BinOp::Mul => "mul",
            BinOp::Div => "div",
            BinOp::Mod => "mod",
            BinOp::Eq | BinOp::NotEq => "eq",
            BinOp::Lt => "lt",
            BinOp::Gt => "gt",
            BinOp::LtEq => "le",
            BinOp::GtEq => "ge",
            BinOp::And => "and",
            BinOp::Or => "or",
        }
    }

    fn check_binop(&mut self, left: &Ty, op: BinOp, right: &Ty, span: Span) -> Ty {
        // Error propagation.
        if left.is_error() || right.is_error() {
            return Ty::Error;
        }

        // Type variables: defer checking until the variable is resolved.
        if matches!(left, Ty::Var(_)) || matches!(right, Ty::Var(_)) {
            let _ = self.unifier.unify(left, right);
            return left.clone();
        }

        match op {
            // Arithmetic: both sides must be numeric, same type — or operator overloaded.
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                // String concatenation.
                if op == BinOp::Add && *left == Ty::Str && *right == Ty::Str {
                    return Ty::Str;
                }

                // Primitive numeric types.
                if left.is_numeric() {
                    if let Err(msg) = self.unifier.unify(left, right) {
                        self.error(format!("In {op:?}: {msg}"), span);
                    }
                    return left.clone();
                }

                // Operator overloading: look for impl Add/Sub/etc for this type.
                if let Some(_trait_name) = Self::op_trait_name(op) {
                    let type_name = match left {
                        Ty::Named(name) => name.clone(),
                        Ty::Struct { name, .. } => name.clone(),
                        _ => {
                            self.error(format!("Cannot apply {op:?} to {left}"), span);
                            return Ty::Error;
                        }
                    };
                    let method_name = Self::op_method_name(op);
                    if let Some(info) = self.symbols.lookup_method(&type_name, method_name) {
                        return info.ret.clone();
                    }
                }

                self.error(format!("Cannot apply {op:?} to {left}"), span);
                Ty::Error
            }

            // Comparison: numeric, returns bool.
            BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => {
                if !left.is_numeric() {
                    self.error(format!("Cannot compare {left} with {op:?}"), span);
                }
                Ty::Bool
            }

            // Equality: any type, returns bool.
            BinOp::Eq | BinOp::NotEq => Ty::Bool,

            // Logical: both bool.
            BinOp::And | BinOp::Or => {
                if *left != Ty::Bool {
                    self.error(format!("Logical {op:?} requires bool, found {left}"), span);
                }
                if *right != Ty::Bool {
                    self.error(format!("Logical {op:?} requires bool, found {right}"), span);
                }
                Ty::Bool
            }
        }
    }

    // ── Type resolution ─────────────────────────────────────────────────

    /// Convert a HIR type to an internal Ty.
    fn resolve_hir_type(&mut self, ty: &HirType) -> Ty {
        match &ty.kind {
            HirTypeKind::Named(name) => {
                // Check if this is an active type parameter.
                if self.active_type_params.contains(name) {
                    return Ty::TypeParam(name.clone());
                }
                Ty::from_name(name)
            }
            HirTypeKind::Generic { name, args } => {
                let resolved_args: Vec<Ty> =
                    args.iter().map(|a| self.resolve_hir_type(a)).collect();
                if resolved_args.is_empty() {
                    Ty::Named(name.clone())
                } else {
                    Ty::GenericInstance {
                        name: name.clone(),
                        args: resolved_args,
                    }
                }
            }
            HirTypeKind::Reference { mutable, inner } => Ty::Ref {
                mutable: *mutable,
                inner: Box::new(self.resolve_hir_type(inner)),
            },
            HirTypeKind::Array { element, .. } => {
                Ty::Array(Box::new(self.resolve_hir_type(element)))
            }
            HirTypeKind::ImplTrait(name) => {
                // impl Trait in param position → fresh type var with trait bound.
                // For now, just use the trait name as a type.
                Ty::Named(name.clone())
            }
            HirTypeKind::Function {
                params,
                return_type,
            } => Ty::Function {
                params: params.iter().map(|p| self.resolve_hir_type(p)).collect(),
                ret: Box::new(self.resolve_hir_type(return_type)),
            },
        }
    }

    /// Replace `Ty::Named("Self")` with the actual target type throughout a type.
    fn resolve_self_type(&self, ty: Ty, target: &str) -> Ty {
        match ty {
            Ty::Named(ref name) if name == "Self" => Ty::Named(target.to_string()),
            Ty::Array(inner) => Ty::Array(Box::new(self.resolve_self_type(*inner, target))),
            Ty::Ref { mutable, inner } => Ty::Ref {
                mutable,
                inner: Box::new(self.resolve_self_type(*inner, target)),
            },
            Ty::Function { params, ret } => Ty::Function {
                params: params
                    .into_iter()
                    .map(|p| self.resolve_self_type(p, target))
                    .collect(),
                ret: Box::new(self.resolve_self_type(*ret, target)),
            },
            _ => ty,
        }
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}
