use crate::lexer::token::Span;

/// A complete Forge source file.
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

/// A top-level item in a program.
#[derive(Debug, Clone)]
pub struct Item {
    pub kind: ItemKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ItemKind {
    Function(Function),
    Struct(StructDef),
    Enum(EnumDef),
    Impl(ImplBlock),
    Trait(TraitDef),
    /// Use declaration: `use path.to.module`
    Use(UsePath),
}

/// A use declaration: `use math` or `use utils.helpers`
#[derive(Debug, Clone)]
pub struct UsePath {
    /// Path segments: ["math"] or ["utils", "helpers"]
    pub segments: Vec<String>,
}

/// A function definition.
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
}

/// A function parameter.
#[derive(Debug, Clone)]
pub struct Param {
    pub mutable: bool,
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

/// A struct definition.
#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub fields: Vec<Field>,
}

/// A struct field.
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

/// An enum definition.
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<Variant>,
}

/// An enum variant.
#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<TypeExpr>,
    pub span: Span,
}

/// An impl block.
#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub generic_params: Vec<GenericParam>,
    pub trait_name: Option<String>,
    pub target: String,
    pub methods: Vec<Function>,
}

/// A trait definition.
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub methods: Vec<TraitMethod>,
}

/// A method in a trait — may or may not have a body (default implementation).
#[derive(Debug, Clone)]
pub struct TraitMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Option<Block>,
    pub span: Span,
}

/// A generic type parameter.
#[derive(Debug, Clone)]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<TypeExpr>,
    pub span: Span,
}

/// A block of statements, optionally producing a value (the last expression).
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

/// A statement.
#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Let {
        mutable: bool,
        name: String,
        ty: Option<TypeExpr>,
        value: Option<Expr>,
    },
    Expr(Expr),
    Return(Option<Expr>),
    Break,
    Continue,
}

/// An expression.
#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    /// Integer literal: `42`, `0xFF`
    IntLiteral(i128),
    /// Float literal: `3.14`
    FloatLiteral(f64),
    /// Bool literal: `true`, `false`
    BoolLiteral(bool),
    /// Plain string: `"hello"`
    StringLiteral(String),
    /// Interpolated string: `"Hello, {name}!"`
    InterpolatedString(Vec<StringPart>),
    /// Identifier: `x`, `foo`
    Identifier(String),
    /// Self value: `self`
    SelfValue,
    /// Binary operation: `a + b`
    BinaryOp {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },
    /// Unary operation: `!x`, `-x`, `&x`, `*x`
    UnaryOp { op: UnaryOp, expr: Box<Expr> },
    /// Function/method call: `foo(a, b)`, `x.bar()`
    Call { callee: Box<Expr>, args: Vec<Expr> },
    /// Field access: `x.y`
    FieldAccess { object: Box<Expr>, field: String },
    /// Index: `x[i]`
    Index { object: Box<Expr>, index: Box<Expr> },
    /// Slice: `x[start:end]`, `x[:end]`, `x[start:]`
    Slice {
        object: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
    },
    /// Block expression: `{ ... }`
    Block(Block),
    /// If expression: `if cond { ... } else { ... }`
    If {
        condition: Box<Expr>,
        then_block: Block,
        else_block: Option<Box<Expr>>,
    },
    /// Match expression
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    /// Closure: `|x, y| x + y`
    Closure {
        params: Vec<ClosureParam>,
        body: Box<Expr>,
    },
    /// Assignment: `x = 5`, `x += 1`
    Assign {
        target: Box<Expr>,
        op: Option<BinOp>,
        value: Box<Expr>,
    },
    /// Range: `0..10`, `0..=10`
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },
    /// Reference: `&x`, `&mut x`
    Reference { mutable: bool, expr: Box<Expr> },
    /// Dereference: `*x`
    Dereference(Box<Expr>),
    /// Struct literal: `Vec2 { x: 1.0, y: 2.0 }`
    StructLiteral {
        name: Box<Expr>,
        fields: Vec<FieldInit>,
    },
    /// Error propagation: `expr?`
    Try(Box<Expr>),
    /// Safe navigation: `expr?.field` or `expr?.method(args)`
    SafeNav {
        object: Box<Expr>,
        field: String,
        call_args: Option<Vec<Expr>>,
    },
    /// Null coalescing: `expr ?? default`
    NullCoalesce { expr: Box<Expr>, default: Box<Expr> },
    /// Turbofish: `expr::<Type>`
    Turbofish {
        expr: Box<Expr>,
        types: Vec<TypeExpr>,
    },
    /// Array literal: `[1, 2, 3]`
    Array(Vec<Expr>),
    /// For loop: `for x in iter { ... }`
    For {
        binding: String,
        iter: Box<Expr>,
        body: Block,
    },
    /// While loop: `while cond { ... }`
    While { condition: Box<Expr>, body: Block },
    /// Compile-time evaluation: `comptime { ... }`
    Comptime(Block),
}

/// Part of an interpolated string.
#[derive(Debug, Clone)]
pub enum StringPart {
    Literal(String),
    Expr(Expr),
}

/// A field initializer in a struct literal.
#[derive(Debug, Clone)]
pub struct FieldInit {
    pub name: String,
    pub value: Option<Expr>,
    pub span: Span,
}

/// A match arm.
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

/// A pattern (used in match arms, let bindings, etc).
#[derive(Debug, Clone)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PatternKind {
    /// Wildcard: `_`
    Wildcard,
    /// Identifier binding: `x`
    Identifier(String),
    /// Literal: `42`, `"hello"`, `true`
    Literal(Expr),
    /// Enum variant: `Some(x)`, `AppError.NotFound(msg)`
    Variant {
        path: Vec<String>,
        fields: Vec<Pattern>,
    },
}

/// A closure parameter.
#[derive(Debug, Clone)]
pub struct ClosureParam {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub span: Span,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    Ref,
    Deref,
}

/// A type expression.
#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeExprKind {
    /// Simple named type: `i32`, `Vec2`, `str`
    Named(String),
    /// Generic type: `Stack<T>`, `Result<u16>`
    Generic { name: String, args: Vec<TypeExpr> },
    /// Reference type: `&T`, `&mut T`
    Reference { mutable: bool, inner: Box<TypeExpr> },
    /// Array type: `[T]`, `[T; n]`
    Array {
        element: Box<TypeExpr>,
        size: Option<Box<Expr>>,
    },
    /// impl Trait: `impl Area`
    ImplTrait(String),
    /// Function type: `fn(T) -> U`
    Function {
        params: Vec<TypeExpr>,
        return_type: Box<TypeExpr>,
    },
}
