//! Module resolver for Forge.
//!
//! Resolves `use` declarations by finding and parsing the referenced .fg files,
//! then merging their items into the main program's AST.
//!
//! Module resolution rules:
//! - `use foo` → look for `foo.fg` in the same directory as the source file
//! - `use foo.bar` → look for `foo/bar.fg`
//! - Items from imported modules are merged into the program

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::ast::{ItemKind, Program};
use crate::lexer::Lexer;
use crate::parser::Parser;

/// Errors during module resolution.
#[derive(Debug)]
pub struct ModuleError {
    pub message: String,
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Module error: {}", self.message)
    }
}

/// Resolve all `use` declarations in a program, loading and merging
/// imported modules. The source_path is the path to the main .fg file
/// (used to resolve relative imports).
pub fn resolve_modules(program: &mut Program, source_path: &Path) -> Result<(), Vec<ModuleError>> {
    let mut resolver = ModuleResolver {
        base_dir: source_path.parent().unwrap_or(Path::new(".")).to_path_buf(),
        loaded: HashSet::new(),
        errors: Vec::new(),
    };

    resolver.resolve(program);

    if resolver.errors.is_empty() {
        Ok(())
    } else {
        Err(resolver.errors)
    }
}

struct ModuleResolver {
    base_dir: PathBuf,
    /// Tracks loaded module paths to prevent circular imports.
    loaded: HashSet<PathBuf>,
    errors: Vec<ModuleError>,
}

impl ModuleResolver {
    fn resolve(&mut self, program: &mut Program) {
        // Collect use declarations.
        let use_paths: Vec<Vec<String>> = program
            .items
            .iter()
            .filter_map(|item| {
                if let ItemKind::Use(use_path) = &item.kind {
                    Some(use_path.segments.clone())
                } else {
                    None
                }
            })
            .collect();

        // Remove use declarations from the program.
        program
            .items
            .retain(|item| !matches!(&item.kind, ItemKind::Use(_)));

        // Load each module.
        for segments in use_paths {
            self.load_module(&segments, program);
        }
    }

    fn load_module(&mut self, segments: &[String], program: &mut Program) {
        let module_path = self.resolve_path(segments);

        if self.loaded.contains(&module_path) {
            return; // Already loaded.
        }

        let source = match std::fs::read_to_string(&module_path) {
            Ok(s) => s,
            Err(e) => {
                self.errors.push(ModuleError {
                    message: format!(
                        "Cannot load module '{}': {} (looked for {})",
                        segments.join("."),
                        e,
                        module_path.display()
                    ),
                });
                return;
            }
        };

        self.loaded.insert(module_path.clone());

        // Lex and parse the module.
        let (tokens, lex_errors) = Lexer::new(&source).tokenize();
        if !lex_errors.is_empty() {
            for err in &lex_errors {
                self.errors.push(ModuleError {
                    message: format!("In module '{}': {err}", segments.join(".")),
                });
            }
            return;
        }

        let (mut module_program, parse_errors) = Parser::new(tokens).parse();
        if !parse_errors.is_empty() {
            for err in &parse_errors {
                self.errors.push(ModuleError {
                    message: format!("In module '{}': {err}", segments.join(".")),
                });
            }
            return;
        }

        // Recursively resolve imports in the loaded module.
        self.resolve(&mut module_program);

        // Merge module items into the main program (excluding main functions).
        for item in module_program.items {
            if let ItemKind::Function(ref f) = item.kind
                && f.name == "main"
            {
                continue; // Don't import main() from modules.
            }
            program.items.push(item);
        }
    }

    fn resolve_path(&self, segments: &[String]) -> PathBuf {
        if segments.len() == 1 {
            self.base_dir.join(format!("{}.fg", segments[0]))
        } else {
            let mut path = self.base_dir.clone();
            for (i, seg) in segments.iter().enumerate() {
                if i == segments.len() - 1 {
                    path = path.join(format!("{seg}.fg"));
                } else {
                    path = path.join(seg);
                }
            }
            path
        }
    }
}
