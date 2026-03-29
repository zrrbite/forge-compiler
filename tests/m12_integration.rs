//! M12: Integration test — mini-lexer written in Forge.
//! Proves Forge has enough features for self-hosting.

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

fn run_file(path: &str) -> Vec<String> {
    let source = std::fs::read_to_string(path).expect("Failed to read file");
    let (tokens, lex_errors) = Lexer::new(&source).tokenize();
    assert!(lex_errors.is_empty(), "Lex errors: {:?}", lex_errors);
    let (mut program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "Parse errors: {:?}", parse_errors);
    forge::resolve::resolve_modules(&mut program, std::path::Path::new(path)).ok();
    let mut interp = Interpreter::new_capturing();
    interp.run(&program).expect("Runtime error");
    interp.get_output().to_vec()
}

#[test]
fn mini_lexer_runs() {
    let out = run_file("tests/samples/mini_lexer.fg");
    // Should tokenize "fn add(a, b) = a + b\nlet x = 42 + 3"
    assert!(out[0].contains("Source:"));
    assert!(out[1].contains("Tokens: 17"));
    // Check some token classifications.
    assert!(out.iter().any(|l| l.contains("keyword: fn")));
    assert!(out.iter().any(|l| l.contains("identifier: add")));
    assert!(out.iter().any(|l| l.contains("number: 42")));
    assert!(out.iter().any(|l| l.contains("keyword: let")));
}

#[test]
fn mini_lexer_inline() {
    // The core tokenizer logic as an inline test.
    let out = run(r#"fn tokenize(source: str) -> [str] {
    let mut tokens = []
    let mut i = 0
    let len = source.len()

    while i < len {
        let ch = source.char_at(i)

        if ch.is_whitespace() {
            i = i + 1
        } else if ch.is_digit() {
            let start = i
            let mut scanning = true
            while scanning {
                if i < len {
                    if source.char_at(i).is_digit() {
                        i = i + 1
                    } else {
                        scanning = false
                    }
                } else {
                    scanning = false
                }
            }
            tokens.push(source.substring(start, i))
        } else if ch.is_alpha() || ch == "_" {
            let start = i
            let mut scanning = true
            while scanning {
                if i < len {
                    let c = source.char_at(i)
                    if c.is_alpha() || c == "_" || c.is_digit() {
                        i = i + 1
                    } else {
                        scanning = false
                    }
                } else {
                    scanning = false
                }
            }
            tokens.push(source.substring(start, i))
        } else {
            tokens.push(ch)
            i = i + 1
        }
    }

    tokens
}

fn main() {
    let tokens = tokenize("let x = 42 + y")
    print(tokens)
    print(tokens.len())
}"#);
    assert_eq!(out[0], "[let, x, =, 42, +, y]");
    assert_eq!(out[1], "6");
}

#[test]
fn file_io_plus_lexer() {
    // Read a real .fg file and count its tokens.
    let out = run(r#"fn tokenize(source: str) -> [str] {
    let mut tokens = []
    let mut i = 0
    let len = source.len()

    while i < len {
        let ch = source.char_at(i)
        if ch.is_whitespace() {
            i = i + 1
        } else if ch.is_alpha() || ch == "_" {
            let start = i
            let mut go = true
            while go {
                if i < len {
                    let c = source.char_at(i)
                    if c.is_alpha() || c == "_" || c.is_digit() {
                        i = i + 1
                    } else {
                        go = false
                    }
                } else {
                    go = false
                }
            }
            tokens.push(source.substring(start, i))
        } else {
            tokens.push(ch)
            i = i + 1
        }
    }
    tokens
}

fn main() {
    let source = File.read("tests/samples/hello.fg").unwrap()
    let tokens = tokenize(source)
    print(tokens.len() > 0)
    // First token of hello.fg should be "fn"
    print(tokens[0])
}"#);
    assert_eq!(out[0], "true");
    assert_eq!(out[1], "fn");
}

#[test]
fn all_features_together() {
    // Exercise: file I/O, string methods, arrays, HashMap, Result,
    // closures, structs, methods — all in one program.
    let out = run(r#"struct Token {
    kind: str,
    value: str,
}

impl Token {
    fn new(kind: str, value: str) -> Token {
        Token { kind, value }
    }

    fn describe(self) -> str {
        "{self.kind}:{self.value}"
    }
}

fn main() {
    let mut keywords = HashMap()
    keywords.insert("fn", true)
    keywords.insert("let", true)

    let words = "fn hello let world".split(" ")
    let mut tokens = []

    for word in words {
        if keywords.contains_key(word) {
            tokens.push(Token.new("kw", word))
        } else {
            tokens.push(Token.new("id", word))
        }
    }

    let descriptions = tokens.map(|t| t.describe())
    print(descriptions.join(", "))

    let result = Ok(tokens.len())
    print(result.unwrap())
}"#);
    assert_eq!(out[0], "kw:fn, id:hello, kw:let, id:world");
    assert_eq!(out[1], "4");
}
