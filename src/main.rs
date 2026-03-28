use std::env;
use std::fs;
use std::process;

use forge::lexer::Lexer;

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

    let (tokens, errors) = Lexer::new(&source).tokenize();

    if !errors.is_empty() {
        for err in &errors {
            eprintln!("{err}");
        }
    }

    for tok in &tokens {
        println!("{:?}", tok);
    }

    if !errors.is_empty() {
        process::exit(1);
    }
}
