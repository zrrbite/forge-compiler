use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process;

use forge::codegen;
use forge::hir::lower::lower;
use forge::interpreter::{Interpreter, Value};
use forge::lexer::Lexer;
use forge::parser::Parser;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut dump_tokens = false;
    let mut dump_ast = false;
    let mut dump_ir = false;
    let mut compile = false;
    let mut output = None;
    let mut filename = None;
    let mut eval_expr = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "init" => {
                let name = if i + 1 < args.len() {
                    i += 1;
                    args[i].clone()
                } else {
                    "my-project".to_string()
                };
                init_project(&name);
                return;
            }
            "--help" | "-h" => {
                print_usage();
                return;
            }
            "--version" | "-V" => {
                println!("forge 0.7.0");
                return;
            }
            "--tokens" => dump_tokens = true,
            "--ast" => dump_ast = true,
            "--ir" => dump_ir = true,
            "--compile" | "-c" => compile = true,
            "-e" => {
                i += 1;
                if i < args.len() {
                    eval_expr = Some(args[i].clone());
                }
            }
            "-o" => {
                i += 1;
                if i < args.len() {
                    output = Some(args[i].clone());
                }
            }
            _ => {
                if filename.is_none() {
                    filename = Some(args[i].as_str());
                }
                // Extra positional args are available to the script via args()
            }
        }
        i += 1;
    }

    // -e flag: evaluate an expression directly
    if let Some(expr) = eval_expr {
        let source = format!("fn main() {{\n{expr}\n}}");
        let (tokens, _) = Lexer::new(&source).tokenize();
        let (program, _) = Parser::new(tokens).parse();
        let mut interp = Interpreter::new();
        if let Err(e) = interp.run(&program) {
            eprintln!("{e}");
            process::exit(1);
        }
        return;
    }

    let filename = match filename {
        Some(f) => f,
        None => {
            // No file given — launch REPL
            run_repl();
            return;
        }
    };

    let mut source = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {filename}: {e}");
            process::exit(1);
        }
    };

    // Skip shebang line (#!/usr/bin/env forge)
    if source.starts_with("#!") {
        source = source.lines().skip(1).collect::<Vec<_>>().join("\n");
    }

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
    let (mut program, parse_errors) = Parser::new(tokens).parse();

    // Implicit main: if parse fails or no fn main() exists, try wrapping in fn main() { ... }
    let has_main = program
        .items
        .iter()
        .any(|item| matches!(&item.kind, forge::ast::ItemKind::Function(f) if f.name == "main"));
    if !has_main {
        let wrapped = format!("fn main() {{\n{source}\n}}");
        let (wrapped_tokens, _) = Lexer::new(&wrapped).tokenize();
        let (wrapped_program, wrapped_errors) = Parser::new(wrapped_tokens).parse();
        if wrapped_errors.is_empty() {
            program = wrapped_program;
        } else if !parse_errors.is_empty() {
            // Both failed — show original errors
            for err in &parse_errors {
                eprintln!("{err}");
            }
            process::exit(1);
        }
    } else if !parse_errors.is_empty() {
        for err in &parse_errors {
            eprintln!("{err}");
        }
        process::exit(1);
    }

    // Resolve modules (use declarations).
    if let Err(errors) = forge::resolve::resolve_modules(&mut program, Path::new(filename)) {
        for err in &errors {
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

    // Compile or interpret
    if compile || dump_ir {
        let hir = lower(&program);

        if dump_ir {
            let context = inkwell::context::Context::create();
            let mut cg = forge::codegen::Codegen::new(&context, "forge");
            if let Err(e) = cg.compile_program(&hir) {
                eprintln!("{e}");
                process::exit(1);
            }
            println!("{}", cg.get_ir());
            return;
        }

        let output_path = output.map(|s| s.to_string()).unwrap_or_else(|| {
            Path::new(filename)
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string()
        });

        if let Err(e) = codegen::compile_to_binary(&hir, Path::new(&output_path)) {
            eprintln!("{e}");
            process::exit(1);
        }

        eprintln!("Compiled to {output_path}");
    } else {
        // Interpret
        let mut interp = Interpreter::new();
        if let Err(e) = interp.run(&program) {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}

fn run_repl() {
    eprintln!("Forge REPL (type Ctrl-D to exit)");
    let stdin = io::stdin();
    let mut interp = Interpreter::new();
    let mut buffer = String::new();
    let mut continuation = false;

    loop {
        // Prompt
        if continuation {
            eprint!("... ");
        } else {
            eprint!(">>> ");
        }
        io::stderr().flush().ok();

        // Read line
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                eprintln!();
                break; // EOF (Ctrl-D)
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Read error: {e}");
                break;
            }
        }

        buffer.push_str(&line);

        // Check for incomplete input (unclosed braces)
        let open = buffer.chars().filter(|&c| c == '{').count();
        let close = buffer.chars().filter(|&c| c == '}').count();
        if open > close {
            continuation = true;
            continue;
        }
        continuation = false;

        let input = buffer.trim().to_string();
        buffer.clear();
        if input.is_empty() {
            continue;
        }

        // Try to parse as-is first (might be a fn/struct definition)
        let (tokens, lex_errors) = Lexer::new(&input).tokenize();
        if !lex_errors.is_empty() {
            // Try wrapping in main
            let wrapped = format!("fn main() {{\n{input}\n}}");
            let (wtokens, wlex_errors) = Lexer::new(&wrapped).tokenize();
            if !wlex_errors.is_empty() {
                for err in &wlex_errors {
                    eprintln!("{err}");
                }
                continue;
            }
            let (program, parse_errors) = forge::parser::Parser::new(wtokens).parse();
            if !parse_errors.is_empty() {
                for err in &parse_errors {
                    eprintln!("{err}");
                }
                continue;
            }
            match interp.eval_repl(&program) {
                Ok(Some(val)) => print_repl_value(&val),
                Ok(None) => {}
                Err(e) => eprintln!("{e}"),
            }
            continue;
        }

        let (program, parse_errors) = forge::parser::Parser::new(tokens).parse();

        // If parsing as items succeeded and has items, register them
        let has_items = !program.items.is_empty() && parse_errors.is_empty();
        if has_items {
            // Check if any item is a real definition (not a failed parse)
            let has_main = program.items.iter().any(
                |item| matches!(&item.kind, forge::ast::ItemKind::Function(f) if f.name == "main"),
            );
            if has_main {
                // User defined main inline — run it
                match interp.eval_repl(&program) {
                    Ok(Some(val)) => print_repl_value(&val),
                    Ok(None) => {}
                    Err(e) => eprintln!("{e}"),
                }
            } else {
                // Register definitions (fn, struct, impl)
                match interp.eval_repl(&program) {
                    Ok(_) => {}
                    Err(e) => eprintln!("{e}"),
                }
            }
            continue;
        }

        // Try wrapping in fn main() for expressions/statements
        let wrapped = format!("fn main() {{\n{input}\n}}");
        let (wtokens, _) = Lexer::new(&wrapped).tokenize();
        let (program, parse_errors) = forge::parser::Parser::new(wtokens).parse();
        if !parse_errors.is_empty() {
            for err in &parse_errors {
                eprintln!("{err}");
            }
            continue;
        }
        match interp.eval_repl(&program) {
            Ok(Some(val)) => print_repl_value(&val),
            Ok(None) => {}
            Err(e) => eprintln!("{e}"),
        }
    }
}

fn print_repl_value(val: &Value) {
    match val {
        Value::Unit => {} // Don't print ()
        _ => println!("{val}"),
    }
}

fn print_usage() {
    eprintln!("Forge 0.7.0 — A systems language with Rust's safety and C++'s power");
    eprintln!();
    eprintln!("Usage: forge [command] [options] [file.fg] [args...]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  init [name]       Create a new Forge project");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -e <code>         Evaluate code directly");
    eprintln!("  --compile, -c     Compile to native binary");
    eprintln!("  -o <file>         Output binary name (with --compile)");
    eprintln!("  --tokens          Dump token stream");
    eprintln!("  --ast             Dump AST");
    eprintln!("  --ir              Dump LLVM IR");
    eprintln!("  --help, -h        Show this help");
    eprintln!("  --version, -V     Show version");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  forge                          Start REPL");
    eprintln!("  forge hello.fg                 Run a program");
    eprintln!("  forge -e 'print(2 + 2)'        Evaluate inline");
    eprintln!("  forge --compile hello.fg        Compile to binary");
    eprintln!("  forge init my-project           Create new project");
}

fn init_project(name: &str) {
    let dir = Path::new(name);
    if dir.exists() {
        eprintln!("Error: '{name}' already exists");
        process::exit(1);
    }

    fs::create_dir_all(dir).unwrap_or_else(|e| {
        eprintln!("Error creating directory: {e}");
        process::exit(1);
    });

    let main_content = format!(
        r#"// {name} — a Forge project

fn main() {{
    print("Hello from {name}!")
}}
"#
    );

    fs::write(dir.join("main.fg"), main_content).unwrap_or_else(|e| {
        eprintln!("Error writing main.fg: {e}");
        process::exit(1);
    });

    println!("Created project '{name}'");
    println!();
    println!("  cd {name}");
    println!("  forge main.fg");
}
