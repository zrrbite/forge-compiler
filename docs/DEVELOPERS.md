# Forge Compiler Developer Guide

This guide is for anyone who wants to understand, modify, or contribute to the
Forge compiler. It covers the architecture, compiler pipeline, testing, and the
self-hosted compiler.

## Quick Start

```bash
# Build
cargo build

# Run a Forge program (interpreted)
cargo run -- tests/samples/hello.fg

# Run tests
cargo test

# Format + lint
cargo fmt && cargo clippy -- -D warnings

# Compile to native binary (via LLVM)
cargo run -- --compile tests/samples/hello.fg
```

## Project Layout

```
src/
  main.rs             CLI entry point — parses flags, orchestrates pipeline
  lib.rs              Library root — exports all modules

  lexer/
    mod.rs            Tokenizer: source → token stream
    token.rs          Token, TokenKind, Span definitions
    tests.rs          Lexer unit tests

  parser/
    mod.rs            Recursive descent + Pratt parser: tokens → AST
    tests.rs          Parser unit tests

  ast/
    mod.rs            AST node definitions (Program, Item, Expr, Stmt, etc.)

  resolve/
    mod.rs            Module resolver: processes `use` declarations

  hir/
    mod.rs            HIR (High-level IR) node definitions
    lower.rs          AST → HIR lowering (desugaring)

  typeck/
    check.rs          Type checker: inference, unification, validation
    types.rs          Type definitions (Ty enum)
    scope.rs          Variable binding scope management
    tests.rs          Type checker tests

  borrowck/
    mod.rs            Borrow checker: ownership, moves, borrows
    tests.rs          Borrow checker tests

  comptime/
    mod.rs            Compile-time evaluation of `comptime { ... }` blocks
    tests.rs          Comptime tests

  interpreter/
    mod.rs            Tree-walk interpreter: runs AST directly
    tests.rs          Interpreter tests

  codegen/
    mod.rs            LLVM IR generation via inkwell
    tests.rs          Codegen tests

  stdlib/
    mod.rs            Built-in functions and methods
    tests.rs          Stdlib tests

tests/
  m1_strings.rs       Integration tests by milestone
  m2_arrays.rs
  ...
  m12_integration.rs
  mut_self.rs          Special case: mutable self in methods
  mut_ref.rs           Special case: &mut references
  nested_mut_self.rs   Special case: nested mut self calls

tests/samples/         11 reference programs defining the language spec
  hello.fg             Basics: functions, let bindings, interpolation
  vec2.fg              Structs, methods, operator overloading
  buffer.fg            Ownership, borrowing, arrays
  errors.fg            Result/Option patterns, match
  traits.fg            Trait definitions, default methods
  generics.fg          Generic-like patterns (Stack)
  iterators.fg         map/filter/fold combinators
  concurrency.fg       Spawn/atomic patterns (placeholder)
  comptime.fg          Compile-time execution (placeholder)
  http_server.fg       Full app pattern (placeholder)
  mini_lexer.fg        Self-hosting: minimal lexer in Forge

self-host/             Self-hosted compiler written in Forge
  lexer.fg             Tokenizer
  ast.fg               AST definitions
  parser.fg            Recursive descent parser
  codegen.fg           AST → C99 transpiler
  compile.fg           Compiler driver (lex → parse → codegen → cc)

docs/                  Design documents (language, each compiler phase)
```

## Compiler Pipeline

There are two execution paths:

```
Source (.fg)
  → Lexer          → Token stream
  → Parser         → AST
  → Module Resolve → Merged AST (with `use` imports)
  ↓
  ├─ Interpret path (default):
  │    → Interpreter  → Output
  │
  └─ Compile path (--compile):
       → Lower        → HIR (desugared AST)
       → Type Check   → Typed HIR
       → Borrow Check → Validated HIR
       → Comptime     → Evaluated comptime blocks
       → Codegen      → LLVM IR
       → LLVM         → Native binary
```

The interpreter runs the AST directly (no type checking or lowering). The
compiled path goes through the full pipeline. Both paths share the lexer,
parser, and module resolver.

### Lexer

Converts source text to tokens. Key design decisions:

- **String interpolation**: `"Hello, {name}!"` is lexed as
  `StringStart`, `StringFragment("Hello, ")`, expression tokens,
  `StringFragment("!")`, `StringEnd`. The parser reassembles these.
- **No semicolons**: Newlines are statement terminators. The lexer emits
  `Newline` tokens which the parser uses for statement boundaries.
- **Escaped braces**: `\{` and `\}` produce literal braces in strings
  (not interpolation).

Key types: `Token`, `TokenKind` (enum with ~40 variants), `Span` (byte range).

### Parser

Recursive descent with Pratt parsing for expressions.

- **Items** (top-level): `fn`, `struct`, `enum`, `impl`, `trait`, `use`
- **Statements**: `let`, `return`, `break`, `continue`, expression statements
- **Expressions**: Pratt parser handles operator precedence. Prefix/infix
  dispatch, configurable binding powers.

The parser produces an `ast::Program` containing a `Vec<Item>`. Each node
carries a `Span` for error reporting.

Error recovery: on parse failure, `recover_to_item_boundary()` skips tokens
until the next top-level keyword, allowing multiple errors per file.

### Module Resolution

Processes `use` declarations:

```
use foo         →  loads ./foo.fg
use foo.bar     →  loads ./foo/bar.fg
```

Merges imported items into the main program (excluding imported `main()`
functions). Detects circular imports.

### HIR Lowering

Desugars AST → HIR:

- Compound assignment: `x += 1` → `x = x + 1`
- Field shorthand: `Foo { x }` → `Foo { x: x }`
- String interpolation: `"Hi {name}"` → `"Hi " + to_str(name)`
- Each node gets a `HirId` for later type annotation

### Type Checker

Bidirectional type inference with unification:

- **Type variables** for inference: `let x = 42` infers `x: i32`
- **Struct fields** checked against declarations
- **Method resolution** via impl blocks
- **Trait bounds** on generic parameters
- **Scope management** via `scope.rs`

Key type: `Ty` enum — primitives, arrays, refs, structs, enums, functions,
generics, type variables.

### Borrow Checker

Tracks variable states: `Owned`, `Moved`, `Borrowed`, `MutBorrowed`.

Detects:
- Use after move
- Borrow conflicts (shared + mutable)
- Assignment to immutable variables
- Moving borrowed values

Simplified compared to Rust — no NLL (non-lexical lifetimes) or full
control-flow-graph analysis yet.

### Interpreter

Tree-walk interpreter that runs the AST directly. Used as the default
execution mode and for `comptime` evaluation.

Key types:
- `Value` — runtime values (Int, Float, Bool, String, Array, Struct, etc.)
- `Outcome` — control flow (Val, Return, Break, Continue, Error)
- `Env` — scope stack of `HashMap<String, Value>`

Built-in functions: `print`, `args`, `exit`, `exec`, `File.read/write`,
`HashMap`, `Some`, `None`.

### LLVM Codegen

Uses the `inkwell` crate (safe Rust bindings to LLVM).

Three-pass compilation:
1. Register struct types as LLVM struct types
2. Declare all functions/methods (signatures only)
3. Compile function bodies (allocas, instructions, returns)

Variables are stack-allocated (`alloca`) and loaded/stored as needed.
Control flow uses LLVM basic blocks with conditional branches. If-expressions
use phi nodes to unify branch results.

Compile and link: `cargo run -- --compile -o myprogram input.fg`

## Self-Hosted Compiler

The `self-host/` directory contains a Forge compiler written in Forge itself.
It transpiles Forge source to C99, then calls `cc` to produce a native binary.

```
forge self-host/compile.fg <input.fg> [output]
```

### Architecture

```
input.fg → lexer.fg → parser.fg → codegen.fg → output.c → cc → binary
```

- **lexer.fg**: Tokenizes Forge source. Handles keywords, operators, strings,
  numbers, comments.
- **ast.fg**: Tag-based AST nodes (uses string `kind` field instead of enums,
  since Forge enum codegen isn't complete).
- **parser.fg**: Recursive descent parser. All parse functions take
  `p: &mut Parser` for pass-by-reference mutation.
- **codegen.fg**: AST → C99 transpiler. Includes a ForgeArray runtime
  (void*-based dynamic arrays), string interpolation with type-aware
  conversions, method dispatch, operator overloading.
- **compile.fg**: Driver that orchestrates the pipeline and calls `cc` via
  the `exec()` builtin.

### Current Status

All 11 reference programs compile and run correctly via the self-hosted
compiler.

### Known Limitations

- **No array push on struct fields**: Forge's interpreter doesn't support
  mutating arrays inside struct fields via `.push()`. The codegen works
  around this using string-encoded type maps.
- **No short-circuit `&&`**: Forge evaluates both sides of `&&`. Use nested
  `if` statements for guarded access (e.g., bounds checking before indexing).
- **Type inference is heuristic**: The self-hosted codegen uses a string-based
  type map and method-name heuristics rather than full type inference.

## Testing

### Running Tests

```bash
cargo test                  # all tests (319)
cargo test lexer            # lexer tests only
cargo test parser           # parser tests only
cargo test interpreter      # interpreter tests
cargo test codegen          # codegen tests
cargo test borrowck         # borrow checker tests
```

### Test Structure

Integration tests use a `run()` helper that lexes, parses, and interprets
source code, capturing print output:

```rust
#[test]
fn my_test() {
    let out = run(r#"
        fn main() {
            print("hello")
        }
    "#);
    assert_eq!(out, vec!["hello"]);
}
```

### Adding Tests

1. **Unit tests**: Add to the `tests.rs` file in the relevant module
2. **Integration tests**: Add to the appropriate `tests/m*.rs` file
3. **Sample programs**: Add a new `.fg` file to `tests/samples/` — these
   serve as both tests and language specification

### Self-Hosted Tests

```bash
# Test the self-hosted lexer
cargo run -- self-host/test_lexer.fg

# Test the self-hosted parser
cargo run -- self-host/test_parser.fg

# Test codegen on hello.fg
cargo run -- self-host/test_codegen.fg

# Compile a sample through the self-hosted compiler
cargo run -- self-host/compile.fg tests/samples/hello.fg /tmp/hello
/tmp/hello
```

## Contributing

### Code Style

- `cargo fmt` for formatting (enforced by pre-commit hook)
- `cargo clippy -- -D warnings` for linting (enforced by pre-push hook)
- No semicolons in Forge code, implicit returns preferred
- Tests for any new feature or bug fix

### Git Workflow

- `main` branch: stable Rust compiler
- `self-hosting` branch: self-hosted compiler development
- Pre-commit hook: format check + tests
- Pre-push hook: format + tests + clippy

### What Needs Work

See [GitHub Issues](https://github.com/zrrbite/forge-compiler/issues) for the
current list. Key areas:

- **Self-hosting**: closures, match expressions, HashMap for remaining samples
- **Scripting mode**: shebang support, `-e` flag, REPL (#29)
- **Error messages**: better diagnostics with source spans (#23)
- **VS Code extension**: syntax highlighting (#21)
- **Bootstrap**: compile the Forge compiler using itself

### Architecture Tips

- The interpreter is the source of truth for language semantics. If you're
  adding a new language feature, implement it in the interpreter first.
- The codegen follows the interpreter's semantics. Test features in the
  interpreter before attempting codegen.
- The self-hosted compiler is a subset — it doesn't need to handle everything
  the Rust compiler does, just enough to compile itself.
- String interpolation touches lexer, parser, HIR lowering, interpreter, and
  codegen — it's a good case study for understanding the full pipeline.
