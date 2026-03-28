pub mod token;

#[cfg(test)]
mod tests;

use token::{Span, Token, TokenKind};

/// Errors produced during lexing.
#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Lex error at {}..{}: {}",
            self.span.start, self.span.end, self.message
        )
    }
}

/// The Forge lexer. Converts source text into a stream of tokens.
pub struct Lexer<'src> {
    source: &'src str,
    bytes: &'src [u8],
    pos: usize,
    errors: Vec<LexError>,
    /// Stack tracking brace depth inside string interpolations.
    /// Each entry is the brace depth when we entered an interpolation.
    interp_brace_stack: Vec<usize>,
    brace_depth: usize,
    /// Buffer for tokens that need to be emitted before the next lex call.
    pending: Vec<Token>,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            pos: 0,
            errors: Vec::new(),
            interp_brace_stack: Vec::new(),
            brace_depth: 0,
            pending: Vec::new(),
        }
    }

    /// Tokenize the entire source, returning tokens and any errors.
    pub fn tokenize(mut self) -> (Vec<Token>, Vec<LexError>) {
        let mut tokens = Vec::new();

        loop {
            // Drain pending tokens first.
            if let Some(tok) = self.pending.pop() {
                tokens.push(tok);
                continue;
            }

            self.skip_whitespace_and_comments();
            if self.is_at_end() {
                tokens.push(Token::new(TokenKind::Eof, Span::new(self.pos, self.pos)));
                break;
            }

            match self.next_token() {
                Some(tok) => tokens.push(tok),
                None => {
                    // Error already recorded, skip the bad character.
                    if !self.is_at_end() {
                        let ch = self.current_char();
                        self.pos += ch.len_utf8();
                    }
                }
            }
        }

        // Collapse redundant newlines (multiple consecutive newlines -> one).
        let tokens = collapse_newlines(tokens);

        (tokens, self.errors)
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn peek(&self) -> u8 {
        if self.is_at_end() {
            0
        } else {
            self.bytes[self.pos]
        }
    }

    fn peek_next(&self) -> u8 {
        if self.pos + 1 >= self.bytes.len() {
            0
        } else {
            self.bytes[self.pos + 1]
        }
    }

    fn advance(&mut self) -> u8 {
        let b = self.bytes[self.pos];
        self.pos += 1;
        b
    }

    fn skip_whitespace_and_comments(&mut self) {
        while !self.is_at_end() {
            match self.peek() {
                // Spaces and tabs are insignificant whitespace.
                b' ' | b'\t' | b'\r' => {
                    self.pos += 1;
                }
                // Line comments: // until end of line.
                b'/' if self.peek_next() == b'/' => {
                    self.pos += 2;
                    while !self.is_at_end() && self.peek() != b'\n' {
                        self.pos += 1;
                    }
                    // Don't consume the \n — it will become a Newline token.
                }
                _ => break,
            }
        }
    }

    fn next_token(&mut self) -> Option<Token> {
        let start = self.pos;
        let b = self.peek();

        // If we're inside a string interpolation and hit a closing brace at
        // the interpolation level, return to string scanning mode.
        if b == b'}'
            && self.in_interpolation()
            && self.brace_depth == *self.interp_brace_stack.last().unwrap()
        {
            self.interp_brace_stack.pop();
            // Continue scanning the rest of the string.
            return self.lex_string_continuation(start);
        }

        match b {
            b'\n' => {
                self.advance();
                Some(Token::new(TokenKind::Newline, Span::new(start, self.pos)))
            }

            b'"' => self.lex_string(start),

            b'0'..=b'9' => self.lex_number(start),

            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.lex_identifier(start),

            // Two-char operators first, then single-char.
            b'+' => self.lex_maybe_eq(start, TokenKind::Plus, TokenKind::PlusEq),
            b'-' => {
                if self.peek_next() == b'>' {
                    self.pos += 2;
                    Some(Token::new(TokenKind::Arrow, Span::new(start, self.pos)))
                } else {
                    self.lex_maybe_eq(start, TokenKind::Minus, TokenKind::MinusEq)
                }
            }
            b'*' => self.lex_maybe_eq(start, TokenKind::Star, TokenKind::StarEq),
            b'/' => self.lex_maybe_eq(start, TokenKind::Slash, TokenKind::SlashEq),
            b'%' => self.lex_maybe_eq(start, TokenKind::Percent, TokenKind::PercentEq),

            b'=' => {
                self.advance();
                if self.peek() == b'=' {
                    self.advance();
                    Some(Token::new(TokenKind::EqEq, Span::new(start, self.pos)))
                } else if self.peek() == b'>' {
                    self.advance();
                    Some(Token::new(TokenKind::FatArrow, Span::new(start, self.pos)))
                } else {
                    Some(Token::new(TokenKind::Eq, Span::new(start, self.pos)))
                }
            }

            b'!' => {
                self.advance();
                if self.peek() == b'=' {
                    self.advance();
                    Some(Token::new(TokenKind::BangEq, Span::new(start, self.pos)))
                } else {
                    Some(Token::new(TokenKind::Bang, Span::new(start, self.pos)))
                }
            }

            b'<' => {
                self.advance();
                if self.peek() == b'=' {
                    self.advance();
                    Some(Token::new(TokenKind::LtEq, Span::new(start, self.pos)))
                } else {
                    Some(Token::new(TokenKind::Lt, Span::new(start, self.pos)))
                }
            }

            b'>' => {
                self.advance();
                if self.peek() == b'=' {
                    self.advance();
                    Some(Token::new(TokenKind::GtEq, Span::new(start, self.pos)))
                } else {
                    Some(Token::new(TokenKind::Gt, Span::new(start, self.pos)))
                }
            }

            b'&' => {
                self.advance();
                if self.peek() == b'&' {
                    self.advance();
                    Some(Token::new(TokenKind::AmpAmp, Span::new(start, self.pos)))
                } else {
                    Some(Token::new(TokenKind::Ampersand, Span::new(start, self.pos)))
                }
            }

            b'|' => {
                self.advance();
                if self.peek() == b'|' {
                    self.advance();
                    Some(Token::new(TokenKind::PipePipe, Span::new(start, self.pos)))
                } else {
                    Some(Token::new(TokenKind::Pipe, Span::new(start, self.pos)))
                }
            }

            b'.' => {
                self.advance();
                if self.peek() == b'.' {
                    self.advance();
                    if self.peek() == b'=' {
                        self.advance();
                        Some(Token::new(TokenKind::DotDotEq, Span::new(start, self.pos)))
                    } else {
                        Some(Token::new(TokenKind::DotDot, Span::new(start, self.pos)))
                    }
                } else {
                    Some(Token::new(TokenKind::Dot, Span::new(start, self.pos)))
                }
            }

            b':' => {
                self.advance();
                if self.peek() == b':' {
                    self.advance();
                    Some(Token::new(
                        TokenKind::ColonColon,
                        Span::new(start, self.pos),
                    ))
                } else {
                    Some(Token::new(TokenKind::Colon, Span::new(start, self.pos)))
                }
            }

            b';' => {
                self.advance();
                Some(Token::new(TokenKind::Semicolon, Span::new(start, self.pos)))
            }

            b'?' => {
                self.advance();
                Some(Token::new(TokenKind::Question, Span::new(start, self.pos)))
            }
            b'@' => {
                self.advance();
                Some(Token::new(TokenKind::At, Span::new(start, self.pos)))
            }

            b'(' => {
                self.advance();
                Some(Token::new(TokenKind::LParen, Span::new(start, self.pos)))
            }
            b')' => {
                self.advance();
                Some(Token::new(TokenKind::RParen, Span::new(start, self.pos)))
            }
            b'{' => {
                self.brace_depth += 1;
                self.advance();
                Some(Token::new(TokenKind::LBrace, Span::new(start, self.pos)))
            }
            b'}' => {
                if self.brace_depth > 0 {
                    self.brace_depth -= 1;
                }
                self.advance();
                Some(Token::new(TokenKind::RBrace, Span::new(start, self.pos)))
            }
            b'[' => {
                self.advance();
                Some(Token::new(TokenKind::LBracket, Span::new(start, self.pos)))
            }
            b']' => {
                self.advance();
                Some(Token::new(TokenKind::RBracket, Span::new(start, self.pos)))
            }
            b',' => {
                self.advance();
                Some(Token::new(TokenKind::Comma, Span::new(start, self.pos)))
            }

            _ => {
                let ch = self.current_char();
                self.errors.push(LexError {
                    message: format!("Unexpected character: '{}'", ch),
                    span: Span::new(start, start + ch.len_utf8()),
                });
                None
            }
        }
    }

    /// Lex a `=`-suffixed operator: e.g. `+` vs `+=`.
    fn lex_maybe_eq(
        &mut self,
        start: usize,
        plain: TokenKind,
        with_eq: TokenKind,
    ) -> Option<Token> {
        self.advance();
        if self.peek() == b'=' {
            self.advance();
            Some(Token::new(with_eq, Span::new(start, self.pos)))
        } else {
            Some(Token::new(plain, Span::new(start, self.pos)))
        }
    }

    /// Lex a number literal (integer or float).
    fn lex_number(&mut self, start: usize) -> Option<Token> {
        // Handle hex: 0x..., binary: 0b..., octal: 0o...
        if self.peek() == b'0' && !self.is_at_end() {
            match self.peek_next() {
                b'x' | b'X' => return self.lex_prefixed_int(start, 2, 16),
                b'b' | b'B' => return self.lex_prefixed_int(start, 2, 2),
                b'o' | b'O' => return self.lex_prefixed_int(start, 2, 8),
                _ => {}
            }
        }

        // Decimal digits (with optional _ separators).
        while !self.is_at_end() && (self.peek().is_ascii_digit() || self.peek() == b'_') {
            self.advance();
        }

        // Float: decimal point followed by digit.
        let mut is_float = false;
        if self.peek() == b'.' && self.peek_next().is_ascii_digit() {
            is_float = true;
            self.advance(); // consume '.'
            while !self.is_at_end() && (self.peek().is_ascii_digit() || self.peek() == b'_') {
                self.advance();
            }
        }

        // Float: exponent.
        if self.peek() == b'e' || self.peek() == b'E' {
            is_float = true;
            self.advance();
            if self.peek() == b'+' || self.peek() == b'-' {
                self.advance();
            }
            while !self.is_at_end() && (self.peek().is_ascii_digit() || self.peek() == b'_') {
                self.advance();
            }
        }

        let text: String = self.source[start..self.pos]
            .chars()
            .filter(|&c| c != '_')
            .collect();
        let span = Span::new(start, self.pos);

        if is_float {
            match text.parse::<f64>() {
                Ok(val) => Some(Token::new(TokenKind::FloatLiteral(val), span)),
                Err(_) => {
                    self.errors.push(LexError {
                        message: format!("Invalid float literal: {text}"),
                        span,
                    });
                    None
                }
            }
        } else {
            match text.parse::<i128>() {
                Ok(val) => Some(Token::new(TokenKind::IntLiteral(val), span)),
                Err(_) => {
                    self.errors.push(LexError {
                        message: format!("Invalid integer literal: {text}"),
                        span,
                    });
                    None
                }
            }
        }
    }

    /// Lex 0x, 0b, 0o prefixed integers.
    fn lex_prefixed_int(&mut self, start: usize, prefix_len: usize, radix: u32) -> Option<Token> {
        self.pos += prefix_len; // skip 0x / 0b / 0o
        let digit_start = self.pos;

        while !self.is_at_end() && (self.peek().is_ascii_alphanumeric() || self.peek() == b'_') {
            self.advance();
        }

        let digits: String = self.source[digit_start..self.pos]
            .chars()
            .filter(|&c| c != '_')
            .collect();
        let span = Span::new(start, self.pos);

        match i128::from_str_radix(&digits, radix) {
            Ok(val) => Some(Token::new(TokenKind::IntLiteral(val), span)),
            Err(_) => {
                self.errors.push(LexError {
                    message: format!("Invalid integer literal: {}", &self.source[start..self.pos]),
                    span,
                });
                None
            }
        }
    }

    /// Lex an identifier or keyword.
    fn lex_identifier(&mut self, start: usize) -> Option<Token> {
        while !self.is_at_end() && (self.peek().is_ascii_alphanumeric() || self.peek() == b'_') {
            self.advance();
        }

        let text = &self.source[start..self.pos];
        let span = Span::new(start, self.pos);

        let kind =
            TokenKind::keyword(text).unwrap_or_else(|| TokenKind::Identifier(text.to_string()));
        Some(Token::new(kind, span))
    }

    fn in_interpolation(&self) -> bool {
        !self.interp_brace_stack.is_empty()
    }

    /// Lex a string starting with `"`.
    fn lex_string(&mut self, start: usize) -> Option<Token> {
        self.advance(); // consume opening "
        self.lex_string_body(start, true)
    }

    /// Continue scanning a string after an interpolation expression ends.
    fn lex_string_continuation(&mut self, start: usize) -> Option<Token> {
        self.advance(); // consume the closing } of the interpolation
        self.lex_string_body(start, false)
    }

    /// Scan string contents. `is_start` indicates whether this is the beginning
    /// of a new string or a continuation after interpolation.
    ///
    /// Returns the first token and pushes any additional tokens to `self.pending`
    /// (in reverse order so they can be popped).
    fn lex_string_body(&mut self, start: usize, is_start: bool) -> Option<Token> {
        let mut buf = String::new();
        let frag_start = self.pos;

        loop {
            if self.is_at_end() {
                self.errors.push(LexError {
                    message: "Unterminated string literal".to_string(),
                    span: Span::new(start, self.pos),
                });
                return None;
            }

            match self.peek() {
                b'"' => {
                    self.advance(); // consume closing "

                    if is_start && !self.in_interpolation() {
                        // Simple string with no interpolations at all.
                        return Some(Token::new(
                            TokenKind::StringLiteral(buf),
                            Span::new(start, self.pos),
                        ));
                    }

                    // End of an interpolated string. Emit fragment + StringEnd.
                    let end_span = Span::new(start, self.pos);
                    let frag_span = Span::new(frag_start, self.pos - 1);

                    // Push StringEnd to pending (will be emitted after the fragment).
                    self.pending
                        .push(Token::new(TokenKind::StringEnd, end_span));

                    // Return the final fragment.
                    return Some(Token::new(TokenKind::StringFragment(buf), frag_span));
                }
                b'{' => {
                    self.advance(); // consume {

                    // Escaped brace: {{ produces a literal {
                    if self.peek() == b'{' {
                        self.advance();
                        buf.push('{');
                        continue;
                    }

                    // Start of an interpolation expression.
                    self.interp_brace_stack.push(self.brace_depth);

                    return Some(Token::new(
                        TokenKind::StringFragment(buf),
                        Span::new(frag_start, self.pos),
                    ));
                }
                b'}' if !self.in_interpolation() => {
                    // Escaped brace: }} produces a literal }
                    // (only when not tracking interpolation depth)
                    if self.peek_next() == b'}' {
                        self.advance();
                        self.advance();
                        buf.push('}');
                        continue;
                    }
                    // Single } outside interpolation inside a string — just a char.
                    let ch = self.current_char();
                    self.pos += ch.len_utf8();
                    buf.push(ch);
                }
                b'\\' => {
                    self.advance();
                    if self.is_at_end() {
                        self.errors.push(LexError {
                            message: "Unterminated escape sequence".to_string(),
                            span: Span::new(start, self.pos),
                        });
                        return None;
                    }
                    match self.advance() {
                        b'n' => buf.push('\n'),
                        b't' => buf.push('\t'),
                        b'r' => buf.push('\r'),
                        b'\\' => buf.push('\\'),
                        b'"' => buf.push('"'),
                        b'{' => buf.push('{'),
                        b'}' => buf.push('}'),
                        b'0' => buf.push('\0'),
                        other => {
                            self.errors.push(LexError {
                                message: format!("Unknown escape sequence: \\{}", other as char),
                                span: Span::new(self.pos - 2, self.pos),
                            });
                        }
                    }
                }
                _ => {
                    // Regular character. Handle UTF-8 properly.
                    let ch = self.current_char();
                    self.pos += ch.len_utf8();
                    buf.push(ch);
                }
            }
        }
    }

    /// Get the current character (handles UTF-8).
    fn current_char(&self) -> char {
        self.source[self.pos..].chars().next().unwrap_or('\0')
    }
}

/// Collapse sequences of Newline tokens into a single Newline.
/// Also strip leading and trailing newlines.
fn collapse_newlines(tokens: Vec<Token>) -> Vec<Token> {
    let mut result = Vec::with_capacity(tokens.len());
    let mut prev_was_newline = true; // treat start as after a newline (strip leading)

    for tok in tokens {
        match tok.kind {
            TokenKind::Newline => {
                if !prev_was_newline {
                    result.push(tok);
                    prev_was_newline = true;
                }
            }
            TokenKind::Eof => {
                // Strip trailing newline before EOF.
                if prev_was_newline {
                    result.pop();
                }
                result.push(tok);
                break;
            }
            _ => {
                result.push(tok);
                prev_was_newline = false;
            }
        }
    }

    result
}
