//! M2: Mutable dynamic array tests for self-hosting.

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
fn push_mutates() {
    let out = run(r#"fn main() {
    let mut arr = [1, 2, 3]
    arr.push(4)
    print(arr)
}"#);
    assert_eq!(out, vec!["[1, 2, 3, 4]"]);
}

#[test]
fn pop_mutates() {
    let out = run(r#"fn main() {
    let mut arr = [1, 2, 3]
    arr.pop()
    print(arr)
}"#);
    assert_eq!(out, vec!["[1, 2]"]);
}

#[test]
fn insert_at_index() {
    let out = run(r#"fn main() {
    let mut arr = [1, 3, 4]
    arr.insert(1, 2)
    print(arr)
}"#);
    assert_eq!(out, vec!["[1, 2, 3, 4]"]);
}

#[test]
fn remove_at_index() {
    let out = run(r#"fn main() {
    let mut arr = [1, 2, 3]
    let removed = arr.remove(1)
    print(removed)
    print(arr)
}"#);
    assert_eq!(out, vec!["2", "[1, 3]"]);
}

#[test]
fn clear_array() {
    let out = run(r#"fn main() {
    let mut arr = [1, 2, 3]
    arr.clear()
    print(arr)
    print(arr.is_empty())
}"#);
    assert_eq!(out, vec!["[]", "true"]);
}

#[test]
fn contains_value() {
    let out = run(r#"fn main() {
    let arr = [1, 2, 3]
    print(arr.contains(2))
    print(arr.contains(5))
}"#);
    assert_eq!(out, vec!["true", "false"]);
}

#[test]
fn reverse_array() {
    let out = run(r#"fn main() {
    let mut arr = [1, 2, 3]
    arr.reverse()
    print(arr)
}"#);
    assert_eq!(out, vec!["[3, 2, 1]"]);
}

#[test]
fn join_array() {
    let out = run(r#"fn main() {
    let arr = ["a", "b", "c"]
    print(arr.join(", "))
}"#);
    assert_eq!(out, vec!["a, b, c"]);
}

#[test]
fn get_and_set() {
    let out = run(r#"fn main() {
    let mut arr = [10, 20, 30]
    print(arr.get(1))
    arr.set(1, 99)
    print(arr)
}"#);
    assert_eq!(out, vec!["20", "[10, 99, 30]"]);
}

#[test]
fn build_array_incrementally() {
    let out = run(r#"fn main() {
    let mut result = []
    let mut i = 0
    while i < 5 {
        result.push(i * i)
        i = i + 1
    }
    print(result)
}"#);
    assert_eq!(out, vec!["[0, 1, 4, 9, 16]"]);
}

#[test]
fn sorted_array() {
    let out = run(r#"fn main() {
    let arr = [3, 1, 4, 1, 5, 9, 2, 6]
    print(arr.sorted())
}"#);
    assert_eq!(out, vec!["[1, 1, 2, 3, 4, 5, 6, 9]"]);
}
