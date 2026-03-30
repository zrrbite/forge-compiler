use std::env;
use std::fs;
use std::path::Path;
use std::process;

use forge::codegen;
use forge::hir::lower::lower;
use forge::interpreter::Interpreter;
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
            eprintln!("Usage: forge [options] <file.fg>");
            eprintln!("Options:");
            eprintln!("  -e <code>    Evaluate code directly");
            eprintln!("  --tokens     Dump token stream");
            eprintln!("  --ast        Dump AST");
            eprintln!("  --ir         Dump LLVM IR");
            eprintln!("  --compile    Compile to native binary (default: interpret)");
            eprintln!("  -o <file>    Output binary name (with --compile)");
            process::exit(1);
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
