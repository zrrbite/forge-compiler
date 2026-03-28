use std::env;
use std::fs;
use std::process;

use forge::interpreter::Interpreter;
use forge::lexer::Lexer;
use forge::parser::Parser;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut dump_tokens = false;
    let mut dump_ast = false;
    let mut filename = None;

    for arg in &args[1..] {
        match arg.as_str() {
            "--tokens" => dump_tokens = true,
            "--ast" => dump_ast = true,
            _ => filename = Some(arg.as_str()),
        }
    }

    let filename = match filename {
        Some(f) => f,
        None => {
            eprintln!("Usage: forge [--tokens] [--ast] <file.fg>");
            process::exit(1);
        }
    };

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

    if dump_tokens {
        for tok in &tokens {
            println!("{:?}", tok);
        }
        return;
    }

    // Parse
    let (program, parse_errors) = Parser::new(tokens).parse();
    if !parse_errors.is_empty() {
        for err in &parse_errors {
            eprintln!("{err}");
        }
        process::exit(1);
    }

    if dump_ast {
        for item in &program.items {
            println!("{:#?}", item);
        }
        return;
    }

    // Run
    let mut interp = Interpreter::new();
    if let Err(e) = interp.run(&program) {
        eprintln!("{e}");
        process::exit(1);
    }
}
