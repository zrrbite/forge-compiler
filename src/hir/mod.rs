//! High-level Intermediate Representation (HIR).
//!
//! The HIR is a desugared version of the AST. Syntactic conveniences are
//! expanded into core constructs, making downstream passes (type checking,
//! borrow checking) simpler because they deal with fewer node variants.
//!
//! Desugarings performed:
//! - Compound assignment `x += 1` → `x = x + 1`
//! - Field shorthand `Foo { x }` → `Foo { x: x }`
//! - String interpolation → concatenation of parts
//! - Method syntax preserved (resolved to functions during type checking)
//! - For loops preserved (iterator protocol resolved during type checking)

pub mod lower;

#[cfg(test)]
mod tests;

use crate::lexer::token::Span;

/// A unique identifier for every node in the HIR. Used by later passes
/// to attach type information, borrow data, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HirId(pub u32);

/// A complete Forge program in HIR form.
#[derive(Debug, Clone)]
pub struct HirProgram {
    pub items: Vec<HirItem>,
}

/// A top-level item.
#[derive(Debug, Clone)]
pub struct HirItem {
    pub id: HirId,
    pub kind: HirItemKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirItemKind {
    Function(HirFunction),
    Struct(HirStructDef),
    Enum(HirEnumDef),
    Impl(HirImplBlock),
    Trait(HirTraitDef),
}

/// A function definition.
#[derive(Debug, Clone)]
pub struct HirFunction {
    pub name: String,
    pub params: Vec<HirParam>,
    pub return_type: Option<HirType>,
    pub body: HirBlock,
}

/// A function parameter.
#[derive(Debug, Clone)]
pub struct HirParam {
    pub id: HirId,
    pub mutable: bool,
    pub name: String,
    pub ty: HirType,
    pub span: Span,
}

/// A struct definition.
#[derive(Debug, Clone)]
pub struct HirStructDef {
    pub name: String,
    pub generic_params: Vec<HirGenericParam>,
    pub fields: Vec<HirField>,
}

/// A struct field.
#[derive(Debug, Clone)]
pub struct HirField {
    pub name: String,
    pub ty: HirType,
    pub span: Span,
}

/// An enum definition.
#[derive(Debug, Clone)]
pub struct HirEnumDef {
    pub name: String,
    pub variants: Vec<HirVariant>,
}

/// An enum variant.
#[derive(Debug, Clone)]
pub struct HirVariant {
    pub name: String,
    pub fields: Vec<HirType>,
    pub span: Span,
}

/// An impl block.
#[derive(Debug, Clone)]
pub struct HirImplBlock {
    pub generic_params: Vec<HirGenericParam>,
    pub trait_name: Option<String>,
    pub target: String,
    pub methods: Vec<HirFunction>,
}

/// A trait definition.
#[derive(Debug, Clone)]
pub struct HirTraitDef {
    pub name: String,
    pub generic_params: Vec<HirGenericParam>,
    pub methods: Vec<HirTraitMethod>,
}

/// A method in a trait.
#[derive(Debug, Clone)]
pub struct HirTraitMethod {
    pub name: String,
    pub params: Vec<HirParam>,
    pub return_type: Option<HirType>,
    pub body: Option<HirBlock>,
    pub span: Span,
}

/// A generic type parameter.
#[derive(Debug, Clone)]
pub struct HirGenericParam {
    pub name: String,
    pub bounds: Vec<HirType>,
    pub span: Span,
}

/// A block of statements.
#[derive(Debug, Clone)]
pub struct HirBlock {
    pub id: HirId,
    pub stmts: Vec<HirStmt>,
    pub span: Span,
}

/// A statement.
#[derive(Debug, Clone)]
pub struct HirStmt {
    pub id: HirId,
    pub kind: HirStmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirStmtKind {
    Let {
        mutable: bool,
        name: String,
        ty: Option<HirType>,
        value: Option<HirExpr>,
    },
    Expr(HirExpr),
    Return(Option<HirExpr>),
    Break,
    Continue,
}

/// An expression.
#[derive(Debug, Clone)]
pub struct HirExpr {
    pub id: HirId,
    pub kind: HirExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirExprKind {
    // -- Literals --
    IntLiteral(i128),
    FloatLiteral(f64),
    BoolLiteral(bool),
    StringLiteral(String),
    /// String concatenation (desugared from interpolation).
    StringConcat(Vec<HirExpr>),

    // -- Names --
    Identifier(String),
    SelfValue,

    // -- Operations --
    BinaryOp {
        left: Box<HirExpr>,
        op: BinOp,
        right: Box<HirExpr>,
    },
    UnaryOp {
        op: UnaryOp,
        expr: Box<HirExpr>,
    },
    /// Explicit assignment (compound assignments are desugared).
    /// `x = expr` or `x.field = expr`
    Assign {
        target: Box<HirExpr>,
        value: Box<HirExpr>,
    },

    // -- Access --
    Call {
        callee: Box<HirExpr>,
        args: Vec<HirExpr>,
    },
    FieldAccess {
        object: Box<HirExpr>,
        field: String,
    },
    Index {
        object: Box<HirExpr>,
        index: Box<HirExpr>,
    },
    Turbofish {
        expr: Box<HirExpr>,
        types: Vec<HirType>,
    },

    // -- Control flow --
    Block(HirBlock),
    If {
        condition: Box<HirExpr>,
        then_block: HirBlock,
        else_block: Option<Box<HirExpr>>,
    },
    Match {
        expr: Box<HirExpr>,
        arms: Vec<HirMatchArm>,
    },
    For {
        binding: String,
        iter: Box<HirExpr>,
        body: HirBlock,
    },
    While {
        condition: Box<HirExpr>,
        body: HirBlock,
    },

    // -- Compile-time --
    /// A comptime block — evaluated at compile time, replaced with its result.
    Comptime(HirBlock),

    // -- Closures --
    Closure {
        params: Vec<HirClosureParam>,
        body: Box<HirExpr>,
    },

    // -- Constructors --
    StructLiteral {
        name: String,
        fields: Vec<HirFieldInit>,
    },
    Array(Vec<HirExpr>),

    // -- Ownership --
    Reference {
        mutable: bool,
        expr: Box<HirExpr>,
    },
    Dereference(Box<HirExpr>),

    // -- Error handling --
    Try(Box<HirExpr>),

    // -- Range --
    Range {
        start: Option<Box<HirExpr>>,
        end: Option<Box<HirExpr>>,
        inclusive: bool,
    },
}

/// A match arm.
#[derive(Debug, Clone)]
pub struct HirMatchArm {
    pub pattern: HirPattern,
    pub body: HirExpr,
    pub span: Span,
}

/// A pattern.
#[derive(Debug, Clone)]
pub struct HirPattern {
    pub id: HirId,
    pub kind: HirPatternKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirPatternKind {
    Wildcard,
    Identifier(String),
    Literal(HirExpr),
    Variant {
        path: Vec<String>,
        fields: Vec<HirPattern>,
    },
}

/// A closure parameter.
#[derive(Debug, Clone)]
pub struct HirClosureParam {
    pub name: String,
    pub ty: Option<HirType>,
    pub span: Span,
}

/// A field initializer in a struct literal (always explicit after desugaring).
#[derive(Debug, Clone)]
pub struct HirFieldInit {
    pub name: String,
    pub value: HirExpr,
    pub span: Span,
}

/// A type expression.
#[derive(Debug, Clone)]
pub struct HirType {
    pub id: HirId,
    pub kind: HirTypeKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirTypeKind {
    Named(String),
    Generic {
        name: String,
        args: Vec<HirType>,
    },
    Reference {
        mutable: bool,
        inner: Box<HirType>,
    },
    Array {
        element: Box<HirType>,
        size: Option<Box<HirExpr>>,
    },
    ImplTrait(String),
    Function {
        params: Vec<HirType>,
        return_type: Box<HirType>,
    },
}

// Re-export operator types from AST (they're the same).
pub use crate::ast::{BinOp, UnaryOp};
