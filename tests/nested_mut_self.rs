//! Tests for nested mut self — methods calling other mut self methods.

use forge::interpreter::Interpreter;
use forge::lexer::Lexer;
use forge::parser::Parser;

fn run(source: &str) -> Vec<String> {
    let (tokens, _) = Lexer::new(source).tokenize();
    let (program, _) = Parser::new(tokens).parse();
    let mut interp = Interpreter::new_capturing();
    interp.run(&program).expect("Runtime error");
    interp.get_output().to_vec()
}

#[test]
fn nested_mut_self_basic() {
    let out = run(r#"struct Foo { x: i64 }
impl Foo {
    fn step(mut self) -> i64 {
        let old = self.x
        self.x = self.x + 1
        old
    }
    fn double_step(mut self) -> i64 {
        self.step()
        self.step()
    }
}
fn main() {
    let mut f = Foo { x: 0 }
    print(f.double_step())
    print(f.x)
}"#);
    assert_eq!(out, vec!["1", "2"]);
}

#[test]
fn nested_mut_self_through_mut_ref() {
    let out = run(r#"struct Foo { x: i64 }
impl Foo {
    fn step(mut self) -> i64 {
        let old = self.x
        self.x = self.x + 1
        old
    }
    fn double_step(mut self) -> i64 {
        self.step()
        self.step()
    }
}
fn do_it(f: &mut Foo) -> i64 {
    f.double_step()
}
fn main() {
    let mut f = Foo { x: 0 }
    let r = do_it(f)
    print(r)
    print(f.x)
}"#);
    assert_eq!(out, vec!["1", "2"]);
}

#[test]
fn parser_expect_advance_pattern() {
    let out = run(r#"struct Token { kind: str, value: str }
struct Parser { tokens: [Token], pos: i64 }
impl Parser {
    fn new(tokens: [Token]) -> Parser { Parser { tokens, pos: 0 } }
    fn peek(self) -> Token {
        if self.pos < self.tokens.len() { self.tokens[self.pos] }
        else { Token { kind: "eof", value: "" } }
    }
    fn advance(mut self) -> Token {
        let tok = self.peek()
        if self.pos < self.tokens.len() { self.pos = self.pos + 1 }
        tok
    }
    fn expect(mut self, kind: str, value: str) -> Token {
        let tok = self.peek()
        if tok.kind == kind {
            if value == "" || tok.value == value { self.advance() }
            else { tok }
        } else { tok }
    }
}
fn parse_fn(p: &mut Parser) -> str {
    let kw = p.expect("kw", "fn")
    let name = p.expect("id", "")
    p.expect("op", "(")
    p.expect("op", ")")
    kw.value + " " + name.value + "()"
}
fn main() {
    let tokens = [
        Token { kind: "kw", value: "fn" },
        Token { kind: "id", value: "main" },
        Token { kind: "op", value: "(" },
        Token { kind: "op", value: ")" },
    ]
    let mut p = Parser.new(tokens)
    print(parse_fn(p))
    print(p.pos)
}"#);
    assert_eq!(out, vec!["fn main()", "4"]);
}
