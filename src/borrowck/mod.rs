//! Borrow checker for Forge.
//!
//! Enforces ownership rules at compile time:
//! - Values are moved on assignment (use-after-move is an error)
//! - Immutable borrows (`&T`) allow multiple simultaneous readers
//! - Mutable borrows (`&mut T`) are exclusive (no aliasing)
//! - Cannot assign to immutable variables
//!
//! The checker walks the HIR and tracks the state of each variable:
//! - **Owned**: the variable holds the value
//! - **Moved**: the value has been moved to another variable or function
//! - **Borrowed**: an immutable reference exists
//! - **MutBorrowed**: a mutable reference exists
//!
//! This is a simplified version of Rust's borrow checker. It doesn't do
//! full lifetime analysis or NLL (Non-Lexical Lifetimes) — those would
//! require a control flow graph and dataflow analysis. What it does catch:
//! - Use after move
//! - Move of borrowed value
//! - Mutable borrow while immutably borrowed
//! - Multiple mutable borrows
//! - Assignment to immutable variable

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::hir::*;
use crate::lexer::token::Span;

/// A borrow check error.
#[derive(Debug, Clone)]
pub struct BorrowError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for BorrowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Borrow error at {}..{}: {}",
            self.span.start, self.span.end, self.message
        )
    }
}

/// State of a variable's value.
#[derive(Debug, Clone, PartialEq)]
enum VarState {
    /// Variable owns its value and it's available.
    Owned,
    /// Value has been moved out. Contains the span where the move happened.
    Moved(Span),
    /// Variable is immutably borrowed. Count tracks number of active borrows.
    Borrowed(u32),
    /// Variable is mutably borrowed.
    MutBorrowed,
}

/// Information tracked per variable.
#[derive(Debug, Clone)]
struct VarInfo {
    mutable: bool,
    state: VarState,
    /// Is this a reference type? (borrows are transparent for references)
    is_ref: bool,
    /// Is this a Copy type? Primitives (int, float, bool, str) are Copy.
    is_copy: bool,
}

/// The borrow checker.
pub struct BorrowChecker {
    /// Stack of scopes. Each scope maps variable names to their state.
    scopes: Vec<HashMap<String, VarInfo>>,
    pub errors: Vec<BorrowError>,
}

impl BorrowChecker {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            errors: Vec::new(),
        }
    }

    fn error(&mut self, message: String, span: Span) {
        self.errors.push(BorrowError { message, span });
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: String, mutable: bool, is_ref: bool, is_copy: bool) {
        self.scopes.last_mut().unwrap().insert(
            name,
            VarInfo {
                mutable,
                state: VarState::Owned,
                is_ref,
                is_copy,
            },
        );
    }

    fn lookup(&self, name: &str) -> Option<&VarInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }

    fn lookup_mut(&mut self, name: &str) -> Option<&mut VarInfo> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(info) = scope.get_mut(name) {
                return Some(info);
            }
        }
        None
    }

    /// Check that a variable is usable (not moved).
    fn check_use(&mut self, name: &str, span: Span) {
        if let Some(info) = self.lookup(name)
            && let VarState::Moved(move_span) = &info.state
        {
            self.errors.push(BorrowError {
                message: format!(
                    "Use of moved value: '{name}' was moved at {}..{}",
                    move_span.start, move_span.end
                ),
                span,
            });
        }
    }

    /// Mark a variable as moved. Primitives and references are Copy — they
    /// don't move, just get copied.
    fn mark_moved(&mut self, name: &str, span: Span) {
        if let Some(info) = self.lookup(name).cloned() {
            // Copy types don't move — they're implicitly copied.
            if info.is_ref || info.is_copy {
                return;
            }
            // Check if borrowed.
            match &info.state {
                VarState::Borrowed(_) => {
                    self.error(format!("Cannot move '{name}' while it is borrowed"), span);
                }
                VarState::MutBorrowed => {
                    self.error(
                        format!("Cannot move '{name}' while it is mutably borrowed"),
                        span,
                    );
                }
                _ => {}
            }
            if let Some(info) = self.lookup_mut(name) {
                info.state = VarState::Moved(span);
            }
        }
    }

    /// Mark a variable as immutably borrowed.
    fn mark_borrowed(&mut self, name: &str, span: Span) {
        if let Some(info) = self.lookup(name).cloned() {
            match &info.state {
                VarState::Moved(move_span) => {
                    self.error(
                        format!(
                            "Cannot borrow '{name}': value was moved at {}..{}",
                            move_span.start, move_span.end
                        ),
                        span,
                    );
                }
                VarState::MutBorrowed => {
                    self.error(
                        format!(
                            "Cannot borrow '{name}' as immutable: it is already mutably borrowed"
                        ),
                        span,
                    );
                }
                _ => {}
            }
            if let Some(info) = self.lookup_mut(name) {
                match &info.state {
                    VarState::Borrowed(n) => info.state = VarState::Borrowed(n + 1),
                    _ => info.state = VarState::Borrowed(1),
                }
            }
        }
    }

    /// Mark a variable as mutably borrowed.
    fn mark_mut_borrowed(&mut self, name: &str, span: Span) {
        if let Some(info) = self.lookup(name).cloned() {
            match &info.state {
                VarState::Moved(move_span) => {
                    self.error(
                        format!(
                            "Cannot borrow '{name}': value was moved at {}..{}",
                            move_span.start, move_span.end
                        ),
                        span,
                    );
                }
                VarState::Borrowed(_) => {
                    self.error(
                        format!(
                            "Cannot borrow '{name}' as mutable: it is already immutably borrowed"
                        ),
                        span,
                    );
                }
                VarState::MutBorrowed => {
                    self.error(
                        format!(
                            "Cannot borrow '{name}' as mutable: it is already mutably borrowed"
                        ),
                        span,
                    );
                }
                _ => {}
            }
            if !info.mutable {
                self.error(
                    format!("Cannot mutably borrow immutable variable '{name}'"),
                    span,
                );
            }
            if let Some(info) = self.lookup_mut(name) {
                info.state = VarState::MutBorrowed;
            }
        }
    }

    /// Determine if an expression produces a Copy type (primitive).
    /// Struct literals and function calls returning structs are not Copy.
    fn is_copy_expr(expr: &HirExpr) -> bool {
        match &expr.kind {
            HirExprKind::IntLiteral(_)
            | HirExprKind::FloatLiteral(_)
            | HirExprKind::BoolLiteral(_)
            | HirExprKind::StringLiteral(_)
            | HirExprKind::StringConcat(_) => true,
            HirExprKind::BinaryOp { .. } | HirExprKind::UnaryOp { .. } => true,
            HirExprKind::Range { .. } | HirExprKind::Array(_) => true,
            HirExprKind::StructLiteral { .. } => false,
            // Function calls might return non-Copy, but we don't know.
            // Default to non-Copy to be safe.
            HirExprKind::Call { .. } => false,
            HirExprKind::Reference { .. } => true,
            _ => true, // Default to Copy for unknown expressions.
        }
    }

    /// Determine if a HIR type annotation is Copy.
    fn is_copy_type(ty: &HirType) -> bool {
        match &ty.kind {
            HirTypeKind::Named(name) => matches!(
                name.as_str(),
                "i8" | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "f32"
                    | "f64"
                    | "bool"
                    | "str"
                    | "isize"
                    | "usize"
            ),
            HirTypeKind::Reference { .. } => true,
            _ => false,
        }
    }

    // ── Program checking ────────────────────────────────────────────────

    pub fn check_program(&mut self, program: &HirProgram) {
        for item in &program.items {
            self.check_item(item);
        }
    }

    fn check_item(&mut self, item: &HirItem) {
        match &item.kind {
            HirItemKind::Function(func) => self.check_function(func),
            HirItemKind::Impl(imp) => {
                for method in &imp.methods {
                    self.check_function(method);
                }
            }
            _ => {}
        }
    }

    fn check_function(&mut self, func: &HirFunction) {
        self.push_scope();
        for param in &func.params {
            let is_ref = matches!(&param.ty.kind, HirTypeKind::Reference { .. });
            let is_copy = Self::is_copy_type(&param.ty);
            self.define(param.name.clone(), param.mutable, is_ref, is_copy);
        }
        self.check_block(&func.body);
        self.pop_scope();
    }

    fn check_block(&mut self, block: &HirBlock) {
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
    }

    fn check_stmt(&mut self, stmt: &HirStmt) {
        match &stmt.kind {
            HirStmtKind::Let {
                mutable,
                name,
                ty,
                value,
            } => {
                if let Some(expr) = value {
                    self.check_expr_move(expr);
                }
                let is_ref = ty
                    .as_ref()
                    .is_some_and(|t| matches!(&t.kind, HirTypeKind::Reference { .. }));
                let is_copy = if let Some(t) = ty {
                    Self::is_copy_type(t)
                } else if let Some(expr) = value {
                    Self::is_copy_expr(expr)
                } else {
                    true
                };
                self.define(name.clone(), *mutable, is_ref, is_copy);
            }
            HirStmtKind::Expr(expr) => {
                self.check_expr(expr);
            }
            HirStmtKind::Return(expr) => {
                if let Some(e) = expr {
                    self.check_expr_move(e);
                }
            }
            HirStmtKind::Break | HirStmtKind::Continue => {}
        }
    }

    // ── Expression checking ─────────────────────────────────────────────

    /// Check an expression, treating identifier uses as reads (not moves).
    fn check_expr(&mut self, expr: &HirExpr) {
        match &expr.kind {
            HirExprKind::Identifier(name) => {
                self.check_use(name, expr.span);
            }
            HirExprKind::SelfValue => {
                self.check_use("self", expr.span);
            }
            HirExprKind::BinaryOp { left, right, .. } => {
                self.check_expr(left);
                self.check_expr(right);
            }
            HirExprKind::UnaryOp { expr: inner, .. } => {
                self.check_expr(inner);
            }
            HirExprKind::Assign { target, value } => {
                // Check mutability.
                if let HirExprKind::Identifier(name) = &target.kind {
                    if let Some(info) = self.lookup(name)
                        && !info.mutable
                    {
                        self.error(
                            format!("Cannot assign to immutable variable '{name}'"),
                            expr.span,
                        );
                    }
                    self.check_use(name, expr.span);
                }
                self.check_expr_move(value);
            }
            HirExprKind::Call { callee, args } => {
                self.check_expr(callee);
                // Function arguments are moved (unless they're references).
                for arg in args {
                    self.check_expr_move(arg);
                }
            }
            HirExprKind::FieldAccess { object, .. } => {
                self.check_expr(object);
            }
            HirExprKind::Index { object, index } => {
                self.check_expr(object);
                self.check_expr(index);
            }
            HirExprKind::Block(block) => {
                self.push_scope();
                self.check_block(block);
                self.pop_scope();
            }
            HirExprKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.check_expr(condition);
                self.push_scope();
                self.check_block(then_block);
                self.pop_scope();
                if let Some(else_expr) = else_block {
                    self.check_expr(else_expr);
                }
            }
            HirExprKind::Match { expr, arms } => {
                self.check_expr(expr);
                for arm in arms {
                    self.push_scope();
                    self.check_expr(&arm.body);
                    self.pop_scope();
                }
            }
            HirExprKind::For {
                binding,
                iter,
                body,
            } => {
                self.check_expr(iter);
                self.push_scope();
                self.define(binding.clone(), false, false, true);
                self.check_block(body);
                self.pop_scope();
            }
            HirExprKind::While { condition, body } => {
                self.check_expr(condition);
                self.push_scope();
                self.check_block(body);
                self.pop_scope();
            }
            HirExprKind::Closure { body, .. } => {
                self.check_expr(body);
            }
            HirExprKind::StructLiteral { fields, .. } => {
                for field in fields {
                    self.check_expr_move(&field.value);
                }
            }
            HirExprKind::Array(elements) => {
                for elem in elements {
                    self.check_expr(elem);
                }
            }
            HirExprKind::Reference {
                mutable,
                expr: inner,
            } => {
                // Track the borrow.
                if let HirExprKind::Identifier(name) = &inner.kind {
                    if *mutable {
                        self.mark_mut_borrowed(name, expr.span);
                    } else {
                        self.mark_borrowed(name, expr.span);
                    }
                }
                self.check_expr(inner);
            }
            HirExprKind::StringConcat(parts) => {
                for part in parts {
                    self.check_expr(part);
                }
            }
            HirExprKind::Try(inner) | HirExprKind::Dereference(inner) => {
                self.check_expr(inner);
            }
            HirExprKind::Turbofish { expr: inner, .. } => {
                self.check_expr(inner);
            }
            HirExprKind::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.check_expr(s);
                }
                if let Some(e) = end {
                    self.check_expr(e);
                }
            }
            // Literals don't involve ownership.
            HirExprKind::IntLiteral(_)
            | HirExprKind::FloatLiteral(_)
            | HirExprKind::BoolLiteral(_)
            | HirExprKind::StringLiteral(_) => {}
        }
    }

    /// Check an expression in a context where the value is moved (assignment RHS,
    /// function argument, return value).
    fn check_expr_move(&mut self, expr: &HirExpr) {
        match &expr.kind {
            HirExprKind::Identifier(name) => {
                self.check_use(name, expr.span);
                self.mark_moved(name, expr.span);
            }
            // For other expressions, just check them normally — the result
            // is a temporary that doesn't need move tracking.
            _ => self.check_expr(expr),
        }
    }
}

impl Default for BorrowChecker {
    fn default() -> Self {
        Self::new()
    }
}
