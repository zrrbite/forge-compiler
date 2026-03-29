//! Tests for mut self method write-back.

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
fn mut_self_modifies_caller() {
    let out = run(r#"struct Counter {
    value: i64,
}

impl Counter {
    fn new() -> Counter { Counter { value: 0 } }
    fn increment(mut self) { self.value = self.value + 1 }
    fn get(self) -> i64 { self.value }
}

fn main() {
    let mut c = Counter.new()
    c.increment()
    c.increment()
    c.increment()
    print(c.get())
}"#);
    assert_eq!(out, vec!["3"]);
}

#[test]
fn mut_self_parser_pattern() {
    // This is the pattern the self-hosted parser needs.
    let out = run(r#"struct Parser {
    pos: i64,
    len: i64,
}

impl Parser {
    fn new(len: i64) -> Parser { Parser { pos: 0, len } }
    fn advance(mut self) -> i64 {
        let current = self.pos
        self.pos = self.pos + 1
        current
    }
    fn current(self) -> i64 { self.pos }
}

fn main() {
    let mut p = Parser.new(10)
    let t1 = p.advance()
    let t2 = p.advance()
    let t3 = p.advance()
    print(t1)
    print(t2)
    print(t3)
    print(p.current())
}"#);
    assert_eq!(out, vec!["0", "1", "2", "3"]);
}

#[test]
fn mut_self_field_update() {
    // Direct field assignment on mut self works.
    let out = run(r#"struct Pair {
    a: i64,
    b: i64,
}

impl Pair {
    fn swap(mut self) {
        let tmp = self.a
        self.a = self.b
        self.b = tmp
    }
}

fn main() {
    let mut p = Pair { a: 1, b: 2 }
    p.swap()
    print(p.a)
    print(p.b)
}"#);
    assert_eq!(out, vec!["2", "1"]);
}

#[test]
fn non_mut_self_doesnt_modify() {
    // Without mut, self modifications should NOT propagate.
    let out = run(r#"struct Foo {
    x: i64,
}

impl Foo {
    fn try_modify(self) { self.x = 999 }
    fn get(self) -> i64 { self.x }
}

fn main() {
    let mut f = Foo { x: 42 }
    f.try_modify()
    print(f.get())
}"#);
    // x should still be 42 — non-mut self doesn't write back.
    assert_eq!(out, vec!["42"]);
}
