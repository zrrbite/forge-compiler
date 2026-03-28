//! M1: String method tests for self-hosting.

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
fn char_at() {
    let out = run(r#"fn main() { print("hello".char_at(1)) }"#);
    assert_eq!(out, vec!["e"]);
}

#[test]
fn byte_at() {
    let out = run(r#"fn main() { print("A".byte_at(0)) }"#);
    assert_eq!(out, vec!["65"]);
}

#[test]
fn substring() {
    let out = run(r#"fn main() { print("hello world".substring(0, 5)) }"#);
    assert_eq!(out, vec!["hello"]);
}

#[test]
fn starts_with() {
    let out = run(r#"fn main() { print("hello".starts_with("hel")) }"#);
    assert_eq!(out, vec!["true"]);
}

#[test]
fn ends_with() {
    let out = run(r#"fn main() { print("hello".ends_with("llo")) }"#);
    assert_eq!(out, vec!["true"]);
}

#[test]
fn find_found() {
    let out = run(r#"fn main() { print("hello world".find("world")) }"#);
    assert_eq!(out, vec!["6"]);
}

#[test]
fn find_not_found() {
    let out = run(r#"fn main() { print("hello".find("xyz")) }"#);
    assert_eq!(out, vec!["-1"]);
}

#[test]
fn split() {
    let out = run(r#"fn main() { print("a,b,c".split(",")) }"#);
    assert_eq!(out, vec!["[a, b, c]"]);
}

#[test]
fn replace() {
    let out = run(r#"fn main() { print("hello world".replace("world", "forge")) }"#);
    assert_eq!(out, vec!["hello forge"]);
}

#[test]
fn to_upper_lower() {
    let out = run(r#"fn main() {
    print("hello".to_upper())
    print("HELLO".to_lower())
}"#);
    assert_eq!(out, vec!["HELLO", "hello"]);
}

#[test]
fn is_digit() {
    let out = run(r#"fn main() {
    print("5".is_digit())
    print("a".is_digit())
}"#);
    assert_eq!(out, vec!["true", "false"]);
}

#[test]
fn is_alpha() {
    let out = run(r#"fn main() {
    print("a".is_alpha())
    print("5".is_alpha())
}"#);
    assert_eq!(out, vec!["true", "false"]);
}

#[test]
fn is_whitespace() {
    let out = run(r#"fn main() {
    print(" ".is_whitespace())
    print("a".is_whitespace())
}"#);
    assert_eq!(out, vec!["true", "false"]);
}

#[test]
fn is_empty_str() {
    let out = run(r#"fn main() {
    print("".is_empty())
    print("x".is_empty())
}"#);
    assert_eq!(out, vec!["true", "false"]);
}

#[test]
fn mini_lexer_proof_of_concept() {
    // A tiny lexer written in Forge — proof that M1 string methods work.
    let out = run(r#"fn main() {
    let source = "let x = 42"
    let mut tokens = []
    let mut i = 0
    let mut current = ""

    while i < source.len() {
        let ch = source.char_at(i)
        if ch == " " {
            if current.len() > 0 {
                tokens.push(current)
                current = ""
            }
        } else {
            current = current + ch
        }
        i = i + 1
    }
    if current.len() > 0 {
        tokens.push(current)
    }
    print(tokens)
}"#);
    assert_eq!(out, vec!["[let, x, =, 42]"]);
}
