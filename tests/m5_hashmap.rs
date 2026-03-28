//! M5: HashMap tests.

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
fn create_empty_map() {
    let out = run(r#"fn main() {
    let m = HashMap()
    print(m.len())
    print(m.is_empty())
}"#);
    assert_eq!(out, vec!["0", "true"]);
}

#[test]
fn insert_and_get() {
    let out = run(r#"fn main() {
    let mut m = HashMap()
    m.insert("name", "Forge")
    m.insert("version", "0.3")
    print(m.get("name").unwrap())
    print(m.get("version").unwrap())
    print(m.len())
}"#);
    assert_eq!(out, vec!["Forge", "0.3", "2"]);
}

#[test]
fn get_missing_key() {
    let out = run(r#"fn main() {
    let m = HashMap()
    print(m.get("missing").is_none())
}"#);
    assert_eq!(out, vec!["true"]);
}

#[test]
fn contains_key() {
    let out = run(r#"fn main() {
    let mut m = HashMap()
    m.insert("x", 1)
    print(m.contains_key("x"))
    print(m.contains_key("y"))
}"#);
    assert_eq!(out, vec!["true", "false"]);
}

#[test]
fn remove_key() {
    let out = run(r#"fn main() {
    let mut m = HashMap()
    m.insert("a", 1)
    m.insert("b", 2)
    m.remove("a")
    print(m.len())
    print(m.contains_key("a"))
}"#);
    assert_eq!(out, vec!["1", "false"]);
}

#[test]
fn keys_and_values() {
    let out = run(r#"fn main() {
    let mut m = HashMap()
    m.insert("x", 10)
    m.insert("y", 20)
    print(m.keys())
    print(m.values())
}"#);
    assert_eq!(out, vec!["[x, y]", "[10, 20]"]);
}

#[test]
fn update_existing_key() {
    let out = run(r#"fn main() {
    let mut m = HashMap()
    m.insert("count", 1)
    m.insert("count", 2)
    print(m.get("count").unwrap())
    print(m.len())
}"#);
    assert_eq!(out, vec!["2", "1"]);
}

#[test]
fn symbol_table_pattern() {
    // Practical test: use HashMap as a symbol table.
    let out = run(r#"fn main() {
    let mut symbols = HashMap()
    symbols.insert("x", 42)
    symbols.insert("y", 100)
    symbols.insert("pi", 3)

    let mut sum = 0
    let keys = symbols.keys()
    for key in keys {
        let val = symbols.get(key).unwrap()
        sum = sum + val
    }
    print(sum)
}"#);
    assert_eq!(out, vec!["145"]);
}
