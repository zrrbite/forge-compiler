use crate::ast::*;
use crate::lexer::token::{Span, Token, TokenKind};

#[cfg(test)]
mod tests;

/// Errors produced during parsing.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Parse error at {}..{}: {}",
            self.span.start, self.span.end, self.message
        )
    }
}

/// The Forge parser. Converts a token stream into an AST.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<ParseError>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: Vec::new(),
        }
    }

    /// Parse a complete program.
    pub fn parse(mut self) -> (Program, Vec<ParseError>) {
        let mut items = Vec::new();

        self.skip_newlines();
        while !self.is_at_end() {
            match self.parse_item() {
                Some(item) => items.push(item),
                None => {
                    // Skip to next item boundary on error.
                    self.recover_to_item_boundary();
                }
            }
            self.skip_newlines();
        }

        (Program { items }, self.errors)
    }

    // ── Token helpers ───────────────────────────────────────────────────

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if tok.kind != TokenKind::Eof {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &TokenKind) -> Option<Span> {
        if self.peek_kind() == expected {
            Some(self.advance().span)
        } else {
            self.error(format!(
                "Expected {:?}, found {:?}",
                expected,
                self.peek_kind()
            ));
            None
        }
    }

    fn expect_identifier(&mut self) -> Option<(String, Span)> {
        if let TokenKind::Identifier(name) = self.peek_kind().clone() {
            let span = self.advance().span;
            Some((name, span))
        } else {
            self.error(format!("Expected identifier, found {:?}", self.peek_kind()));
            None
        }
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.peek_kind() == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    fn skip_newlines(&mut self) {
        while *self.peek_kind() == TokenKind::Newline {
            self.advance();
        }
    }

    fn eat_newline_or_eof(&mut self) {
        if *self.peek_kind() == TokenKind::Newline {
            self.advance();
        }
        // Also OK if we're at EOF or closing brace — no newline needed.
    }

    fn error(&mut self, message: String) {
        let span = self.peek().span;
        self.errors.push(ParseError { message, span });
    }

    fn recover_to_item_boundary(&mut self) {
        loop {
            match self.peek_kind() {
                TokenKind::Eof => break,
                TokenKind::Fn
                | TokenKind::Struct
                | TokenKind::Enum
                | TokenKind::Impl
                | TokenKind::Trait
                | TokenKind::Use
                | TokenKind::Pub => break,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn current_span(&self) -> Span {
        self.peek().span
    }

    // ── Items ───────────────────────────────────────────────────────────

    fn parse_item(&mut self) -> Option<Item> {
        let start = self.current_span();

        match self.peek_kind().clone() {
            TokenKind::Fn => self.parse_fn_item(start),
            TokenKind::Struct => self.parse_struct_item(start),
            TokenKind::Enum => self.parse_enum_item(start),
            TokenKind::Impl => self.parse_impl_item(start),
            TokenKind::Trait => self.parse_trait_item(start),
            TokenKind::Use => self.parse_use_item(start),
            _ => {
                self.error(format!(
                    "Expected item (fn, struct, enum, impl, trait, use), found {:?}",
                    self.peek_kind()
                ));
                None
            }
        }
    }

    fn parse_use_item(&mut self, start: Span) -> Option<Item> {
        self.expect(&TokenKind::Use)?;
        let mut segments = Vec::new();
        let (first, _) = self.expect_identifier()?;
        segments.push(first);
        while self.eat(&TokenKind::Dot) {
            let (seg, _) = self.expect_identifier()?;
            segments.push(seg);
        }
        let end = self.tokens[self.pos - 1].span;
        self.eat_newline_or_eof();
        Some(Item {
            kind: ItemKind::Use(UsePath { segments }),
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_fn_item(&mut self, start: Span) -> Option<Item> {
        let func = self.parse_function()?;
        let span = Span::new(start.start, func.body.span.end);
        Some(Item {
            kind: ItemKind::Function(func),
            span,
        })
    }

    fn parse_function(&mut self) -> Option<Function> {
        self.expect(&TokenKind::Fn)?;
        let (name, _) = self.expect_identifier()?;
        let generic_params = self.parse_optional_generic_params();
        let _ = generic_params; // Store on function later if needed.
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;

        let return_type = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.skip_newlines();
        let body = self.parse_block()?;

        Some(Function {
            name,
            params,
            return_type,
            body,
        })
    }

    fn parse_params(&mut self) -> Option<Vec<Param>> {
        let mut params = Vec::new();

        if *self.peek_kind() == TokenKind::RParen {
            return Some(params);
        }

        loop {
            self.skip_newlines();
            if *self.peek_kind() == TokenKind::RParen {
                break;
            }

            let start = self.current_span();
            let mutable = if *self.peek_kind() == TokenKind::Mut {
                self.advance();
                true
            } else {
                false
            };

            // Handle `self` and `mut self` as params.
            if *self.peek_kind() == TokenKind::SelfValue {
                let span = self.advance().span;
                params.push(Param {
                    mutable,
                    name: "self".to_string(),
                    ty: TypeExpr {
                        kind: TypeExprKind::Named("Self".to_string()),
                        span,
                    },
                    span: Span::new(start.start, span.end),
                });
            } else {
                let (name, _) = self.expect_identifier()?;
                self.expect(&TokenKind::Colon)?;
                let ty = self.parse_type()?;
                let end = ty.span.end;
                params.push(Param {
                    mutable,
                    name,
                    ty,
                    span: Span::new(start.start, end),
                });
            }

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        Some(params)
    }

    fn parse_struct_item(&mut self, start: Span) -> Option<Item> {
        self.expect(&TokenKind::Struct)?;
        let (name, _) = self.expect_identifier()?;
        let generic_params = self.parse_optional_generic_params();
        self.skip_newlines();
        self.expect(&TokenKind::LBrace)?;

        let mut fields = Vec::new();
        self.skip_newlines();
        while *self.peek_kind() != TokenKind::RBrace && !self.is_at_end() {
            let field_start = self.current_span();
            let (fname, _) = self.expect_identifier()?;
            self.expect(&TokenKind::Colon)?;
            let ty = self.parse_type()?;
            let end = ty.span.end;
            fields.push(Field {
                name: fname,
                ty,
                span: Span::new(field_start.start, end),
            });
            self.eat(&TokenKind::Comma);
            self.skip_newlines();
        }

        let end = self.expect(&TokenKind::RBrace)?;
        Some(Item {
            kind: ItemKind::Struct(StructDef {
                name,
                generic_params,
                fields,
            }),
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_enum_item(&mut self, start: Span) -> Option<Item> {
        self.expect(&TokenKind::Enum)?;
        let (name, _) = self.expect_identifier()?;
        self.skip_newlines();
        self.expect(&TokenKind::LBrace)?;

        let mut variants = Vec::new();
        self.skip_newlines();
        while *self.peek_kind() != TokenKind::RBrace && !self.is_at_end() {
            let v_start = self.current_span();
            let (vname, _) = self.expect_identifier()?;

            let mut fields = Vec::new();
            if self.eat(&TokenKind::LParen) {
                while *self.peek_kind() != TokenKind::RParen && !self.is_at_end() {
                    fields.push(self.parse_type()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen)?;
            }

            let end = self.tokens[self.pos - 1].span;
            variants.push(Variant {
                name: vname,
                fields,
                span: Span::new(v_start.start, end.end),
            });
            self.eat(&TokenKind::Comma);
            self.skip_newlines();
        }

        let end = self.expect(&TokenKind::RBrace)?;
        Some(Item {
            kind: ItemKind::Enum(EnumDef { name, variants }),
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_impl_item(&mut self, start: Span) -> Option<Item> {
        self.expect(&TokenKind::Impl)?;
        let generic_params = self.parse_optional_generic_params();

        let (first_name, _) = self.expect_identifier()?;

        // `impl Trait for Type` or `impl Type`
        let (trait_name, target) = if self.eat(&TokenKind::For) {
            let (target, _) = self.expect_identifier()?;
            (Some(first_name), target)
        } else {
            (None, first_name)
        };

        self.skip_newlines();
        self.expect(&TokenKind::LBrace)?;

        let mut methods = Vec::new();
        self.skip_newlines();
        while *self.peek_kind() != TokenKind::RBrace && !self.is_at_end() {
            if *self.peek_kind() == TokenKind::Fn {
                methods.push(self.parse_function()?);
            } else {
                self.error(format!(
                    "Expected fn in impl block, found {:?}",
                    self.peek_kind()
                ));
                self.advance();
            }
            self.skip_newlines();
        }

        let end = self.expect(&TokenKind::RBrace)?;
        Some(Item {
            kind: ItemKind::Impl(ImplBlock {
                generic_params,
                trait_name,
                target,
                methods,
            }),
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_trait_item(&mut self, start: Span) -> Option<Item> {
        self.expect(&TokenKind::Trait)?;
        let (name, _) = self.expect_identifier()?;
        let generic_params = self.parse_optional_generic_params();
        self.skip_newlines();
        self.expect(&TokenKind::LBrace)?;

        let mut methods = Vec::new();
        self.skip_newlines();
        while *self.peek_kind() != TokenKind::RBrace && !self.is_at_end() {
            let m_start = self.current_span();
            self.expect(&TokenKind::Fn)?;
            let (mname, _) = self.expect_identifier()?;
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;

            let return_type = if self.eat(&TokenKind::Arrow) {
                Some(self.parse_type()?)
            } else {
                None
            };

            // Body is optional — trait methods can be declarations or have defaults.
            self.skip_newlines();
            let body = if *self.peek_kind() == TokenKind::LBrace {
                Some(self.parse_block()?)
            } else {
                None
            };

            let end = self.tokens[self.pos - 1].span;
            methods.push(TraitMethod {
                name: mname,
                params,
                return_type,
                body,
                span: Span::new(m_start.start, end.end),
            });
            self.skip_newlines();
        }

        let end = self.expect(&TokenKind::RBrace)?;
        Some(Item {
            kind: ItemKind::Trait(TraitDef {
                name,
                generic_params,
                methods,
            }),
            span: Span::new(start.start, end.end),
        })
    }

    // ── Generic parameters ──────────────────────────────────────────────

    fn parse_optional_generic_params(&mut self) -> Vec<GenericParam> {
        if *self.peek_kind() != TokenKind::Lt {
            return Vec::new();
        }
        self.advance(); // consume <

        let mut params = Vec::new();
        while *self.peek_kind() != TokenKind::Gt && !self.is_at_end() {
            let start = self.current_span();
            if let Some((name, _)) = self.expect_identifier() {
                let mut bounds = Vec::new();
                if self.eat(&TokenKind::Colon) {
                    bounds.push(match self.parse_type() {
                        Some(t) => t,
                        None => break,
                    });
                    while self.eat(&TokenKind::Plus) {
                        bounds.push(match self.parse_type() {
                            Some(t) => t,
                            None => break,
                        });
                    }
                }
                let end = self.tokens[self.pos - 1].span;
                params.push(GenericParam {
                    name,
                    bounds,
                    span: Span::new(start.start, end.end),
                });
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let _ = self.expect(&TokenKind::Gt);
        params
    }

    // ── Types ───────────────────────────────────────────────────────────

    fn parse_type(&mut self) -> Option<TypeExpr> {
        let start = self.current_span();

        match self.peek_kind().clone() {
            TokenKind::Ampersand => {
                self.advance();
                let mutable = self.eat(&TokenKind::Mut);
                let inner = self.parse_type()?;
                let end = inner.span.end;
                Some(TypeExpr {
                    kind: TypeExprKind::Reference {
                        mutable,
                        inner: Box::new(inner),
                    },
                    span: Span::new(start.start, end),
                })
            }
            TokenKind::LBracket => {
                self.advance();
                let element = self.parse_type()?;
                let size = if self.eat(&TokenKind::Semicolon) {
                    Some(Box::new(self.parse_expr()?))
                } else {
                    None
                };
                let end = self.expect(&TokenKind::RBracket)?;
                Some(TypeExpr {
                    kind: TypeExprKind::Array {
                        element: Box::new(element),
                        size,
                    },
                    span: Span::new(start.start, end.end),
                })
            }
            TokenKind::Impl => {
                self.advance();
                let (name, name_span) = self.expect_identifier()?;
                Some(TypeExpr {
                    kind: TypeExprKind::ImplTrait(name),
                    span: Span::new(start.start, name_span.end),
                })
            }
            TokenKind::Fn => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let mut params = Vec::new();
                while *self.peek_kind() != TokenKind::RParen && !self.is_at_end() {
                    params.push(self.parse_type()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen)?;
                self.expect(&TokenKind::Arrow)?;
                let return_type = self.parse_type()?;
                let end = return_type.span.end;
                Some(TypeExpr {
                    kind: TypeExprKind::Function {
                        params,
                        return_type: Box::new(return_type),
                    },
                    span: Span::new(start.start, end),
                })
            }
            TokenKind::Identifier(_) | TokenKind::SelfType => {
                let name = if *self.peek_kind() == TokenKind::SelfType {
                    self.advance();
                    "Self".to_string()
                } else {
                    self.expect_identifier()?.0
                };

                if *self.peek_kind() == TokenKind::Lt {
                    self.advance();
                    let mut args = Vec::new();
                    while *self.peek_kind() != TokenKind::Gt && !self.is_at_end() {
                        args.push(self.parse_type()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    let end = self.expect(&TokenKind::Gt)?;
                    Some(TypeExpr {
                        kind: TypeExprKind::Generic { name, args },
                        span: Span::new(start.start, end.end),
                    })
                } else {
                    let end = self.tokens[self.pos - 1].span;
                    Some(TypeExpr {
                        kind: TypeExprKind::Named(name),
                        span: Span::new(start.start, end.end),
                    })
                }
            }
            _ => {
                self.error(format!("Expected type, found {:?}", self.peek_kind()));
                None
            }
        }
    }

    // ── Blocks ──────────────────────────────────────────────────────────

    fn parse_block(&mut self) -> Option<Block> {
        let start = self.expect(&TokenKind::LBrace)?;
        let mut stmts = Vec::new();

        self.skip_newlines();
        while *self.peek_kind() != TokenKind::RBrace && !self.is_at_end() {
            match self.parse_stmt() {
                Some(stmt) => stmts.push(stmt),
                None => {
                    // Skip to next statement boundary.
                    while !matches!(
                        self.peek_kind(),
                        TokenKind::Newline | TokenKind::RBrace | TokenKind::Eof
                    ) {
                        self.advance();
                    }
                }
            }
            self.skip_newlines();
        }

        let end = self.expect(&TokenKind::RBrace)?;
        Some(Block {
            stmts,
            span: Span::new(start.start, end.end),
        })
    }

    // ── Statements ──────────────────────────────────────────────────────

    fn parse_stmt(&mut self) -> Option<Stmt> {
        let start = self.current_span();

        match self.peek_kind().clone() {
            TokenKind::Let => self.parse_let_stmt(start),
            TokenKind::Return => {
                self.advance();
                let value = if matches!(
                    self.peek_kind(),
                    TokenKind::Newline | TokenKind::RBrace | TokenKind::Eof
                ) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                let end = self.tokens[self.pos - 1].span;
                self.eat_newline_or_eof();
                Some(Stmt {
                    kind: StmtKind::Return(value),
                    span: Span::new(start.start, end.end),
                })
            }
            TokenKind::Break => {
                let span = self.advance().span;
                self.eat_newline_or_eof();
                Some(Stmt {
                    kind: StmtKind::Break,
                    span,
                })
            }
            TokenKind::Continue => {
                let span = self.advance().span;
                self.eat_newline_or_eof();
                Some(Stmt {
                    kind: StmtKind::Continue,
                    span,
                })
            }
            _ => {
                let expr = self.parse_expr()?;

                // Check for assignment.
                let kind = match self.peek_kind() {
                    TokenKind::Eq => {
                        self.advance();
                        let value = self.parse_expr()?;
                        StmtKind::Expr(Expr {
                            span: Span::new(start.start, value.span.end),
                            kind: ExprKind::Assign {
                                target: Box::new(expr),
                                op: None,
                                value: Box::new(value),
                            },
                        })
                    }
                    TokenKind::PlusEq
                    | TokenKind::MinusEq
                    | TokenKind::StarEq
                    | TokenKind::SlashEq
                    | TokenKind::PercentEq => {
                        let op = match self.peek_kind() {
                            TokenKind::PlusEq => BinOp::Add,
                            TokenKind::MinusEq => BinOp::Sub,
                            TokenKind::StarEq => BinOp::Mul,
                            TokenKind::SlashEq => BinOp::Div,
                            TokenKind::PercentEq => BinOp::Mod,
                            _ => unreachable!(),
                        };
                        self.advance();
                        let value = self.parse_expr()?;
                        StmtKind::Expr(Expr {
                            span: Span::new(start.start, value.span.end),
                            kind: ExprKind::Assign {
                                target: Box::new(expr),
                                op: Some(op),
                                value: Box::new(value),
                            },
                        })
                    }
                    _ => StmtKind::Expr(expr),
                };

                let stmt_end = self.tokens[self.pos.saturating_sub(1)].span;
                self.eat_newline_or_eof();
                Some(Stmt {
                    kind,
                    span: Span::new(start.start, stmt_end.end),
                })
            }
        }
    }

    fn parse_let_stmt(&mut self, start: Span) -> Option<Stmt> {
        self.expect(&TokenKind::Let)?;
        let mutable = self.eat(&TokenKind::Mut);
        let (name, _) = self.expect_identifier()?;

        let ty = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let value = if self.eat(&TokenKind::Eq) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let end = self.tokens[self.pos - 1].span;
        self.eat_newline_or_eof();
        Some(Stmt {
            kind: StmtKind::Let {
                mutable,
                name,
                ty,
                value,
            },
            span: Span::new(start.start, end.end),
        })
    }

    // ── Expressions (Pratt parser) ──────────────────────────────────────

    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_expr_bp(0)
    }

    /// Pratt parser: parse expression with minimum binding power.
    fn parse_expr_bp(&mut self, min_bp: u8) -> Option<Expr> {
        let mut lhs = self.parse_prefix()?;

        loop {
            // Postfix operators: ?, field access, call, index.
            lhs = match self.peek_kind() {
                TokenKind::Question => {
                    self.advance();
                    Expr {
                        span: Span::new(lhs.span.start, self.tokens[self.pos - 1].span.end),
                        kind: ExprKind::Try(Box::new(lhs)),
                    }
                }
                TokenKind::Dot => {
                    self.advance();
                    let (field, field_span) = self.expect_identifier()?;

                    // Check for method call: expr.method(args)
                    if *self.peek_kind() == TokenKind::LParen {
                        self.advance();
                        let args = self.parse_call_args()?;
                        let end = self.expect(&TokenKind::RParen)?;
                        Expr {
                            span: Span::new(lhs.span.start, end.end),
                            kind: ExprKind::Call {
                                callee: Box::new(Expr {
                                    span: Span::new(lhs.span.start, field_span.end),
                                    kind: ExprKind::FieldAccess {
                                        object: Box::new(lhs),
                                        field,
                                    },
                                }),
                                args,
                            },
                        }
                    } else {
                        Expr {
                            span: Span::new(lhs.span.start, field_span.end),
                            kind: ExprKind::FieldAccess {
                                object: Box::new(lhs),
                                field,
                            },
                        }
                    }
                }
                TokenKind::LParen => {
                    self.advance();
                    let args = self.parse_call_args()?;
                    let end = self.expect(&TokenKind::RParen)?;
                    Expr {
                        span: Span::new(lhs.span.start, end.end),
                        kind: ExprKind::Call {
                            callee: Box::new(lhs),
                            args,
                        },
                    }
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    let end = self.expect(&TokenKind::RBracket)?;
                    Expr {
                        span: Span::new(lhs.span.start, end.end),
                        kind: ExprKind::Index {
                            object: Box::new(lhs),
                            index: Box::new(index),
                        },
                    }
                }
                TokenKind::ColonColon => {
                    // Turbofish: expr::<Type>
                    self.advance();
                    if *self.peek_kind() == TokenKind::Lt {
                        self.advance();
                        let mut types = Vec::new();
                        while *self.peek_kind() != TokenKind::Gt && !self.is_at_end() {
                            types.push(self.parse_type()?);
                            if !self.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                        let end = self.expect(&TokenKind::Gt)?;
                        Expr {
                            span: Span::new(lhs.span.start, end.end),
                            kind: ExprKind::Turbofish {
                                expr: Box::new(lhs),
                                types,
                            },
                        }
                    } else {
                        self.error("Expected < after ::".to_string());
                        return Some(lhs);
                    }
                }
                _ => break,
            };
        }

        // Infix operators with precedence.
        loop {
            let op = match self.peek_kind() {
                TokenKind::PipePipe => BinOp::Or,
                TokenKind::AmpAmp => BinOp::And,
                TokenKind::EqEq => BinOp::Eq,
                TokenKind::BangEq => BinOp::NotEq,
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::LtEq => BinOp::LtEq,
                TokenKind::GtEq => BinOp::GtEq,
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                // Range operators handled specially.
                TokenKind::DotDot | TokenKind::DotDotEq => {
                    let (l_bp, _) = (1, 2); // Very low precedence.
                    if l_bp < min_bp {
                        break;
                    }
                    let inclusive = *self.peek_kind() == TokenKind::DotDotEq;
                    self.advance();
                    let end = if matches!(
                        self.peek_kind(),
                        TokenKind::Newline
                            | TokenKind::RBrace
                            | TokenKind::RParen
                            | TokenKind::RBracket
                            | TokenKind::Comma
                            | TokenKind::Eof
                    ) {
                        None
                    } else {
                        Some(Box::new(self.parse_expr_bp(2)?))
                    };
                    lhs = Expr {
                        span: Span::new(
                            lhs.span.start,
                            end.as_ref()
                                .map(|e| e.span.end)
                                .unwrap_or(self.tokens[self.pos - 1].span.end),
                        ),
                        kind: ExprKind::Range {
                            start: Some(Box::new(lhs)),
                            end,
                            inclusive,
                        },
                    };
                    continue;
                }
                _ => break,
            };

            let (l_bp, r_bp) = infix_binding_power(op);
            if l_bp < min_bp {
                break;
            }

            self.advance(); // consume operator
            let rhs = self.parse_expr_bp(r_bp)?;
            lhs = Expr {
                span: Span::new(lhs.span.start, rhs.span.end),
                kind: ExprKind::BinaryOp {
                    left: Box::new(lhs),
                    op,
                    right: Box::new(rhs),
                },
            };
        }

        Some(lhs)
    }

    /// Parse a prefix expression (atom or unary operator).
    fn parse_prefix(&mut self) -> Option<Expr> {
        let start = self.current_span();

        match self.peek_kind().clone() {
            TokenKind::IntLiteral(n) => {
                let span = self.advance().span;
                Some(Expr {
                    kind: ExprKind::IntLiteral(n),
                    span,
                })
            }
            TokenKind::FloatLiteral(f) => {
                let span = self.advance().span;
                Some(Expr {
                    kind: ExprKind::FloatLiteral(f),
                    span,
                })
            }
            TokenKind::BoolLiteral(b) => {
                let span = self.advance().span;
                Some(Expr {
                    kind: ExprKind::BoolLiteral(b),
                    span,
                })
            }
            TokenKind::StringLiteral(s) => {
                let span = self.advance().span;
                Some(Expr {
                    kind: ExprKind::StringLiteral(s),
                    span,
                })
            }
            TokenKind::StringFragment(_) => self.parse_interpolated_string(start),
            TokenKind::SelfValue => {
                let span = self.advance().span;
                Some(Expr {
                    kind: ExprKind::SelfValue,
                    span,
                })
            }
            TokenKind::Identifier(_) => {
                let (name, span) = self.expect_identifier()?;

                // Check for struct literal: `Name { field: val }`
                // Only if { is on the same line (not after newline).
                if *self.peek_kind() == TokenKind::LBrace && self.could_be_struct_literal(&name) {
                    self.parse_struct_literal(name, start)
                } else {
                    Some(Expr {
                        kind: ExprKind::Identifier(name),
                        span,
                    })
                }
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                Some(expr)
            }
            TokenKind::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                self.skip_newlines();
                while *self.peek_kind() != TokenKind::RBracket && !self.is_at_end() {
                    elements.push(self.parse_expr()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                    self.skip_newlines();
                }
                let end = self.expect(&TokenKind::RBracket)?;
                Some(Expr {
                    kind: ExprKind::Array(elements),
                    span: Span::new(start.start, end.end),
                })
            }
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                let span = block.span;
                Some(Expr {
                    kind: ExprKind::Block(block),
                    span,
                })
            }
            TokenKind::If => self.parse_if_expr(start),
            TokenKind::Match => self.parse_match_expr(start),
            TokenKind::For => self.parse_for_expr(start),
            TokenKind::While => self.parse_while_expr(start),
            TokenKind::Comptime => {
                self.advance();
                self.skip_newlines();
                let block = self.parse_block()?;
                let end = block.span;
                Some(Expr {
                    kind: ExprKind::Comptime(block),
                    span: Span::new(start.start, end.end),
                })
            }
            TokenKind::Pipe => self.parse_closure(start),
            TokenKind::Bang => {
                self.advance();
                let expr = self.parse_expr_bp(PREFIX_BP)?;
                Some(Expr {
                    span: Span::new(start.start, expr.span.end),
                    kind: ExprKind::UnaryOp {
                        op: UnaryOp::Not,
                        expr: Box::new(expr),
                    },
                })
            }
            TokenKind::Minus => {
                self.advance();
                let expr = self.parse_expr_bp(PREFIX_BP)?;
                Some(Expr {
                    span: Span::new(start.start, expr.span.end),
                    kind: ExprKind::UnaryOp {
                        op: UnaryOp::Neg,
                        expr: Box::new(expr),
                    },
                })
            }
            TokenKind::Ampersand => {
                self.advance();
                let mutable = self.eat(&TokenKind::Mut);
                let expr = self.parse_expr_bp(PREFIX_BP)?;
                Some(Expr {
                    span: Span::new(start.start, expr.span.end),
                    kind: ExprKind::Reference {
                        mutable,
                        expr: Box::new(expr),
                    },
                })
            }
            TokenKind::Star => {
                self.advance();
                let expr = self.parse_expr_bp(PREFIX_BP)?;
                Some(Expr {
                    span: Span::new(start.start, expr.span.end),
                    kind: ExprKind::Dereference(Box::new(expr)),
                })
            }
            _ => {
                self.error(format!("Expected expression, found {:?}", self.peek_kind()));
                None
            }
        }
    }

    fn parse_interpolated_string(&mut self, start: Span) -> Option<Expr> {
        let mut parts = Vec::new();

        loop {
            match self.peek_kind().clone() {
                TokenKind::StringFragment(s) => {
                    self.advance();
                    if !s.is_empty() {
                        parts.push(StringPart::Literal(s));
                    }
                    // After a fragment, either we get expression tokens or StringEnd.
                    if *self.peek_kind() == TokenKind::StringEnd {
                        self.advance();
                        break;
                    }
                    // Otherwise, parse the interpolated expression.
                    let expr = self.parse_expr()?;
                    parts.push(StringPart::Expr(expr));
                }
                TokenKind::StringEnd => {
                    self.advance();
                    break;
                }
                _ => {
                    self.error("Expected string fragment or end".to_string());
                    break;
                }
            }
        }

        let end = self.tokens[self.pos - 1].span;
        Some(Expr {
            kind: ExprKind::InterpolatedString(parts),
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_call_args(&mut self) -> Option<Vec<Expr>> {
        let mut args = Vec::new();
        self.skip_newlines();
        while *self.peek_kind() != TokenKind::RParen && !self.is_at_end() {
            args.push(self.parse_expr()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
        }
        Some(args)
    }

    fn parse_if_expr(&mut self, start: Span) -> Option<Expr> {
        self.expect(&TokenKind::If)?;
        let condition = self.parse_expr()?;
        self.skip_newlines();
        let then_block = self.parse_block()?;

        // Check for else — may be after newlines.
        let saved_pos = self.pos;
        self.skip_newlines();
        let else_block = if self.eat(&TokenKind::Else) {
            self.skip_newlines();
            if *self.peek_kind() == TokenKind::If {
                // else if — parse as nested if expression.
                let else_start = self.current_span();
                Some(Box::new(self.parse_if_expr(else_start)?))
            } else {
                let block = self.parse_block()?;
                let span = block.span;
                Some(Box::new(Expr {
                    kind: ExprKind::Block(block),
                    span,
                }))
            }
        } else {
            self.pos = saved_pos;
            None
        };

        let end = self.tokens[self.pos - 1].span;
        Some(Expr {
            kind: ExprKind::If {
                condition: Box::new(condition),
                then_block,
                else_block,
            },
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_match_expr(&mut self, start: Span) -> Option<Expr> {
        self.expect(&TokenKind::Match)?;
        let expr = self.parse_expr()?;
        self.skip_newlines();
        self.expect(&TokenKind::LBrace)?;

        let mut arms = Vec::new();
        self.skip_newlines();
        while *self.peek_kind() != TokenKind::RBrace && !self.is_at_end() {
            let arm_start = self.current_span();
            let pattern = self.parse_pattern()?;
            self.expect(&TokenKind::FatArrow)?;
            self.skip_newlines();
            let body = if *self.peek_kind() == TokenKind::LBrace {
                let block = self.parse_block()?;
                let span = block.span;
                Expr {
                    kind: ExprKind::Block(block),
                    span,
                }
            } else {
                self.parse_expr()?
            };
            let end = body.span;
            arms.push(MatchArm {
                pattern,
                body,
                span: Span::new(arm_start.start, end.end),
            });
            self.eat(&TokenKind::Comma);
            self.skip_newlines();
        }

        let end = self.expect(&TokenKind::RBrace)?;
        Some(Expr {
            kind: ExprKind::Match {
                expr: Box::new(expr),
                arms,
            },
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_pattern(&mut self) -> Option<Pattern> {
        let start = self.current_span();

        match self.peek_kind().clone() {
            TokenKind::Identifier(name) if name == "_" => {
                let span = self.advance().span;
                Some(Pattern {
                    kind: PatternKind::Wildcard,
                    span,
                })
            }
            TokenKind::Identifier(name) => {
                let span = self.advance().span;

                // Check for variant pattern: Name.Variant(fields) or Name(fields)
                if self.eat(&TokenKind::Dot) {
                    let (variant, _) = self.expect_identifier()?;
                    let mut path = vec![name, variant];

                    // More dots?
                    while self.eat(&TokenKind::Dot) {
                        let (next, _) = self.expect_identifier()?;
                        path.push(next);
                    }

                    let fields = if self.eat(&TokenKind::LParen) {
                        let mut f = Vec::new();
                        while *self.peek_kind() != TokenKind::RParen && !self.is_at_end() {
                            f.push(self.parse_pattern()?);
                            if !self.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                        self.expect(&TokenKind::RParen)?;
                        f
                    } else {
                        Vec::new()
                    };

                    let end = self.tokens[self.pos - 1].span;
                    Some(Pattern {
                        kind: PatternKind::Variant { path, fields },
                        span: Span::new(start.start, end.end),
                    })
                } else if self.eat(&TokenKind::LParen) {
                    // Name(fields)
                    let mut fields = Vec::new();
                    while *self.peek_kind() != TokenKind::RParen && !self.is_at_end() {
                        fields.push(self.parse_pattern()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    let end = self.tokens[self.pos - 1].span;
                    Some(Pattern {
                        kind: PatternKind::Variant {
                            path: vec![name],
                            fields,
                        },
                        span: Span::new(start.start, end.end),
                    })
                } else {
                    Some(Pattern {
                        kind: PatternKind::Identifier(name),
                        span,
                    })
                }
            }
            TokenKind::IntLiteral(_)
            | TokenKind::FloatLiteral(_)
            | TokenKind::BoolLiteral(_)
            | TokenKind::StringLiteral(_) => {
                let expr = self.parse_prefix()?;
                let span = expr.span;
                Some(Pattern {
                    kind: PatternKind::Literal(expr),
                    span,
                })
            }
            _ => {
                self.error(format!("Expected pattern, found {:?}", self.peek_kind()));
                None
            }
        }
    }

    fn parse_for_expr(&mut self, start: Span) -> Option<Expr> {
        self.expect(&TokenKind::For)?;
        let (binding, _) = self.expect_identifier()?;
        self.expect(&TokenKind::In)?;
        let iter = self.parse_expr()?;
        self.skip_newlines();
        let body = self.parse_block()?;
        let end = body.span;
        Some(Expr {
            kind: ExprKind::For {
                binding,
                iter: Box::new(iter),
                body,
            },
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_while_expr(&mut self, start: Span) -> Option<Expr> {
        self.expect(&TokenKind::While)?;
        let condition = self.parse_expr()?;
        self.skip_newlines();
        let body = self.parse_block()?;
        let end = body.span;
        Some(Expr {
            kind: ExprKind::While {
                condition: Box::new(condition),
                body,
            },
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_closure(&mut self, start: Span) -> Option<Expr> {
        self.expect(&TokenKind::Pipe)?;
        let mut params = Vec::new();
        while *self.peek_kind() != TokenKind::Pipe && !self.is_at_end() {
            let p_start = self.current_span();
            let (name, _) = self.expect_identifier()?;
            let ty = if self.eat(&TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            let end = self.tokens[self.pos - 1].span;
            params.push(ClosureParam {
                name,
                ty,
                span: Span::new(p_start.start, end.end),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::Pipe)?;

        let body = if *self.peek_kind() == TokenKind::LBrace {
            let block = self.parse_block()?;
            let span = block.span;
            Expr {
                kind: ExprKind::Block(block),
                span,
            }
        } else {
            self.parse_expr()?
        };

        let end = body.span;
        Some(Expr {
            kind: ExprKind::Closure {
                params,
                body: Box::new(body),
            },
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_struct_literal(&mut self, name: String, start: Span) -> Option<Expr> {
        self.expect(&TokenKind::LBrace)?;
        let mut fields = Vec::new();

        self.skip_newlines();
        while *self.peek_kind() != TokenKind::RBrace && !self.is_at_end() {
            let f_start = self.current_span();
            let (fname, _) = self.expect_identifier()?;

            let value = if self.eat(&TokenKind::Colon) {
                Some(self.parse_expr()?)
            } else {
                None // shorthand: `Vec2 { x, y }` means `Vec2 { x: x, y: y }`
            };

            let end = self.tokens[self.pos - 1].span;
            fields.push(FieldInit {
                name: fname,
                value,
                span: Span::new(f_start.start, end.end),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
        }

        let end = self.expect(&TokenKind::RBrace)?;
        Some(Expr {
            kind: ExprKind::StructLiteral {
                name: Box::new(Expr {
                    kind: ExprKind::Identifier(name),
                    span: start,
                }),
                fields,
            },
            span: Span::new(start.start, end.end),
        })
    }

    /// Heuristic: is this identifier likely followed by a struct literal `{`?
    /// We check that the identifier starts with an uppercase letter (type name convention).
    fn could_be_struct_literal(&self, name: &str) -> bool {
        name.chars().next().is_some_and(|c| c.is_uppercase())
    }
}

/// Binding power for prefix operators.
const PREFIX_BP: u8 = 17;

/// Left and right binding power for infix binary operators.
fn infix_binding_power(op: BinOp) -> (u8, u8) {
    match op {
        BinOp::Or => (3, 4),
        BinOp::And => (5, 6),
        BinOp::Eq | BinOp::NotEq => (7, 8),
        BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => (9, 10),
        BinOp::Add | BinOp::Sub => (11, 12),
        BinOp::Mul | BinOp::Div | BinOp::Mod => (13, 14),
    }
}
