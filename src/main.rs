use std::env;
use std::fs;
use std::process;

use forge::lexer::Lexer;
use forge::parser::Parser;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: forge <file.fg>");
        process::exit(1);
    }

    let filename = &args[1];
    let source = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {filename}: {e}");
            process::exit(1);
        }
    };

    // Lex
    let (tokens, lex_errors) = Lexer::new(&source).tokenize();
    if !lex_errors.is_empty() {
        for err in &lex_errors {
            eprintln!("{err}");
        }
        process::exit(1);
    }

    // Parse
    let (program, parse_errors) = Parser::new(tokens).parse();
    if !parse_errors.is_empty() {
        for err in &parse_errors {
            eprintln!("{err}");
        }
        process::exit(1);
    }

    // Dump AST
    for item in &program.items {
        println!("{:#?}", item);
    }
}
