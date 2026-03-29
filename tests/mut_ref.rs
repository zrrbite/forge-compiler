//! Tests for &mut reference passing.

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
fn mut_ref_basic() {
    let out = run(r#"fn increment(x: &mut i64) {
    x = x + 1
}

fn main() {
    let mut val = 10
    increment(val)
    print(val)
}"#);
    assert_eq!(out, vec!["11"]);
}

#[test]
fn mut_ref_struct() {
    let out = run(r#"struct Counter { value: i64 }

fn bump(c: &mut Counter) {
    c.value = c.value + 1
}

fn main() {
    let mut c = Counter { value: 0 }
    bump(c)
    bump(c)
    bump(c)
    print(c.value)
}"#);
    assert_eq!(out, vec!["3"]);
}

#[test]
fn mut_ref_parser_pattern() {
    // This is the exact pattern the self-hosted parser needs.
    let out = run(r#"struct Parser {
    tokens: [str],
    pos: i64,
}

impl Parser {
    fn new(tokens: [str]) -> Parser {
        Parser { tokens, pos: 0 }
    }
    fn peek(self) -> str {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos]
        } else {
            "EOF"
        }
    }
    fn advance(mut self) -> str {
        let tok = self.peek()
        if self.pos < self.tokens.len() {
            self.pos = self.pos + 1
        }
        tok
    }
}

fn parse_items(p: &mut Parser) -> [str] {
    let mut items = []
    while p.peek() != "EOF" {
        let tok = p.advance()
        items.push(tok)
    }
    items
}

fn main() {
    let mut p = Parser.new(["fn", "main", "(", ")"])
    let items = parse_items(p)
    print(items)
    print(p.pos)
}"#);
    assert_eq!(out, vec!["[fn, main, (, )]", "4"]);
}

#[test]
fn mut_ref_multiple_calls() {
    let out = run(r#"fn add_item(list: &mut [i64], val: i64) {
    list.push(val)
}

fn main() {
    let mut items = []
    add_item(items, 10)
    add_item(items, 20)
    add_item(items, 30)
    print(items)
}"#);
    assert_eq!(out, vec!["[10, 20, 30]"]);
}

#[test]
fn non_mut_ref_doesnt_modify() {
    let out = run(r#"fn try_modify(x: i64) {
    x = 999
}

fn main() {
    let mut val = 42
    try_modify(val)
    print(val)
}"#);
    // Without &mut, the modification shouldn't propagate.
    assert_eq!(out, vec!["42"]);
}
