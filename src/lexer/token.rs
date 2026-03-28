/// Source location tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Byte offset of the start of the token.
    pub start: usize,
    /// Byte offset one past the end of the token.
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// A token with its source span.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // -- Literals --
    IntLiteral(i128),
    FloatLiteral(f64),
    BoolLiteral(bool),

    // Interpolated strings are emitted as a sequence:
    //   StringStart  StringFragment*  (expr tokens  StringFragment*)*  StringEnd
    // Plain strings (no interpolation) are emitted as:
    //   StringLiteral("contents")
    StringLiteral(String),
    StringStart,
    StringFragment(String),
    StringEnd,

    // -- Identifiers & keywords --
    Identifier(String),

    // Keywords
    Fn,
    Let,
    Mut,
    Struct,
    Impl,
    Trait,
    Enum,
    Match,
    If,
    Else,
    While,
    For,
    In,
    Return,
    Break,
    Continue,
    Use,
    Comptime,
    Spawn,
    Where,
    Pub,
    Mod,
    SelfValue, // self
    SelfType,  // Self

    // -- Operators --
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %
    Ampersand,  // &
    Pipe,       // |
    Bang,       // !
    Dot,        // .
    DotDot,     // ..
    DotDotEq,   // ..=
    Eq,         // =
    EqEq,       // ==
    BangEq,     // !=
    Lt,         // <
    Gt,         // >
    LtEq,       // <=
    GtEq,       // >=
    AmpAmp,     // &&
    PipePipe,   // ||
    PlusEq,     // +=
    MinusEq,    // -=
    StarEq,     // *=
    SlashEq,    // /=
    PercentEq,  // %=
    Arrow,      // ->
    FatArrow,   // =>
    Question,   // ?
    At,         // @
    ColonColon, // ::

    // -- Delimiters --
    LParen,   // (
    RParen,   // )
    LBrace,   // {
    RBrace,   // }
    LBracket, // [
    RBracket, // ]

    // -- Punctuation --
    Colon,     // :
    Comma,     // ,
    Semicolon, // ;  (used in array type syntax: [T; n])
    Newline,   // significant newline (statement terminator)

    // -- Special --
    Eof,
}

impl TokenKind {
    /// Look up a keyword from an identifier string. Returns None if not a keyword.
    pub fn keyword(s: &str) -> Option<TokenKind> {
        match s {
            "fn" => Some(TokenKind::Fn),
            "let" => Some(TokenKind::Let),
            "mut" => Some(TokenKind::Mut),
            "struct" => Some(TokenKind::Struct),
            "impl" => Some(TokenKind::Impl),
            "trait" => Some(TokenKind::Trait),
            "enum" => Some(TokenKind::Enum),
            "match" => Some(TokenKind::Match),
            "if" => Some(TokenKind::If),
            "else" => Some(TokenKind::Else),
            "while" => Some(TokenKind::While),
            "for" => Some(TokenKind::For),
            "in" => Some(TokenKind::In),
            "return" => Some(TokenKind::Return),
            "break" => Some(TokenKind::Break),
            "continue" => Some(TokenKind::Continue),
            "use" => Some(TokenKind::Use),
            "comptime" => Some(TokenKind::Comptime),
            "spawn" => Some(TokenKind::Spawn),
            "where" => Some(TokenKind::Where),
            "pub" => Some(TokenKind::Pub),
            "mod" => Some(TokenKind::Mod),
            "self" => Some(TokenKind::SelfValue),
            "Self" => Some(TokenKind::SelfType),
            "true" => Some(TokenKind::BoolLiteral(true)),
            "false" => Some(TokenKind::BoolLiteral(false)),
            _ => None,
        }
    }
}
