//! M6: File I/O tests.

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
fn file_write_and_read() {
    let tmp = std::env::temp_dir().join("forge_test_m6.txt");
    let tmp_path = tmp.to_string_lossy().to_string();
    // Clean up from previous runs.
    let _ = std::fs::remove_file(&tmp);

    let source = format!(
        r#"fn main() {{
    let path = "{tmp_path}"
    let result = File.write(path, "hello from forge")
    print(result.is_ok())

    let content = File.read(path)
    print(content.unwrap())
}}"#
    );

    let out = run(&source);
    assert_eq!(out, vec!["true", "hello from forge"]);

    // Clean up.
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn file_read_nonexistent() {
    let out = run(r#"fn main() {
    let result = File.read("/tmp/forge_nonexistent_file_xyz.txt")
    print(result.is_err())
}"#);
    assert_eq!(out, vec!["true"]);
}

#[test]
fn file_exists() {
    let tmp = std::env::temp_dir().join("forge_test_m6_exists.txt");
    std::fs::write(&tmp, "test").unwrap();
    let tmp_path = tmp.to_string_lossy().to_string();

    let source = format!(
        r#"fn main() {{
    print(File.exists("{tmp_path}"))
    print(File.exists("/tmp/forge_definitely_not_here.txt"))
}}"#
    );

    let out = run(&source);
    assert_eq!(out, vec!["true", "false"]);

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn read_forge_source() {
    // Read an actual .fg file — proof we can read source files.
    let out = run(r#"fn main() {
    let result = File.read("tests/samples/hello.fg")
    let content = result.unwrap()
    // Check the file starts with "fn"
    print(content.starts_with("fn"))
    print(content.len() > 0)
}"#);
    assert_eq!(out, vec!["true", "true"]);
}

#[test]
fn practical_read_and_tokenize() {
    // Read a file, split into lines, count them.
    let tmp = std::env::temp_dir().join("forge_test_m6_lines.txt");
    std::fs::write(&tmp, "line1\nline2\nline3\n").unwrap();
    let tmp_path = tmp.to_string_lossy().to_string();

    let source = format!(
        r#"fn main() {{
    let content = File.read("{tmp_path}").unwrap()
    let lines = content.split("\n")
    print(lines.len())
}}"#
    );

    let out = run(&source);
    // "line1\nline2\nline3\n".split("\n") = ["line1", "line2", "line3", ""]
    assert_eq!(out, vec!["4"]);

    let _ = std::fs::remove_file(&tmp);
}
