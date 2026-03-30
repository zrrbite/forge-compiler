# Forge Compiler

A compiler for the **Forge** programming language, written in Rust.

Forge is a systems language combining Rust's memory safety with C++'s zero-cost
abstractions and clean, enjoyable syntax.

## Prerequisites

Install the Rust toolchain via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the prompts (defaults are fine), then restart your terminal or run:

```bash
source "$HOME/.cargo/env"
```

This gives you `cargo` (build tool + package manager), `rustc` (compiler),
`rustfmt` (formatter), and `clippy` (linter).

## Building

```bash
cargo build                 # debug build
cargo build --release       # optimized release build
```

The binary is placed in `target/debug/forge` (or `target/release/forge`).

## Running

```bash
cargo run -- <file.fg>      # build + run on a .fg file
cargo run -- tests/samples/hello.fg   # try a sample
```

Or run the binary directly after building:

```bash
./target/debug/forge tests/samples/hello.fg
```

Full compiler pipeline: lexer, parser, HIR, type checker, borrow checker, LLVM codegen, interpreter. Also includes a self-hosted compiler written in Forge itself.

File extension: `.fg`

## Testing

```bash
cargo test                  # run all tests
cargo test lexer            # run lexer tests only
cargo test parser           # run parser tests only
```

## Code Quality

```bash
cargo fmt                   # auto-format all Rust code
cargo fmt -- --check        # check formatting without modifying
cargo clippy                # run linter
cargo clippy -- -D warnings # treat warnings as errors
```

## Git Hooks

Pre-commit and pre-push hooks are set up in `.git/hooks/`:

- **pre-commit**: checks formatting (`cargo fmt --check`) and runs tests (`cargo test`)
- **pre-push**: formatting + tests + clippy (`cargo clippy -- -D warnings`)

## Project Structure

```
src/
  main.rs                   # CLI entry point
  lib.rs                    # library root (for testing)
  lexer/
    mod.rs                  # Lexer implementation
    token.rs                # Token types and Span
    tests.rs                # Lexer tests
  parser/                   # Parser -> AST
  ast/                      # AST node definitions
  hir/                      # High-level IR (desugared AST)
  typeck/                   # Type checking + borrow checking
  mir/                      # Mid-level IR
  codegen/                  # LLVM IR generation
  errors/                   # Diagnostics and error reporting
```

## Compiler Pipeline

```
Source (.fg)
  → Lexer     → Token stream
  → Parser    → AST (Abstract Syntax Tree)
  → Lowering  → HIR (High-level IR, desugared)
  → Type Check + Borrow Check
  → Lowering  → MIR (Mid-level IR)
  → Codegen   → LLVM IR
  → LLVM      → Native binary
```

## Language Design

### Philosophy

- Rust's safety without Rust's ceremony
- C++'s expressiveness without C++'s footguns
- Readable at a glance — a junior dev should be able to follow ownership examples

### Syntax Overview

- **No semicolons** — newlines are statement terminators
- **Implicit returns** — last expression in a block is the return value
- **String interpolation** — `"Hello, {name}!"` with format specs `"{val:.2}"`
- **Method calls** — `Type.new()` not `Type::new()`
- **Mutability** — explicit `let mut`, `mut self`

### Keywords

```
fn let mut struct impl trait enum match if else while for in
return break continue use comptime spawn where pub mod defer
true false self Self
```

### Operators (by precedence, high to low)

```
.                           field access, method call
()  []                      call, index
!  -  &  *                  unary: not, negate, borrow, deref
*  /  %                     multiplicative
+  -                        additive
..  ..=                     range
<  >  <=  >=                comparison
==  !=                      equality
&&                          logical and
||                          logical or
=  +=  -=  *=  /=  %=      assignment
```

### Special Tokens

```
->                          return type
=>                          match arm
?                           error propagation
|...|                       closure parameters
:                           type annotation
::                          turbofish (parse::<u16>())
,                           separator
@                           compiler builtin (@compile_error)
```

### Type System

- **Ownership:** Rust-style affine types. Values moved by default, borrowed with `&`.
- **Lifetimes:** Aggressively elided (~95% of cases need no annotation).
- **Generics:** Monomorphized. Trait bounds with `+`, `where` clauses.
- **Traits:** The only abstraction mechanism. No inheritance. Default methods supported.
- **Error handling:** `Result<T>` with `?` propagation. `Option<T>` for nullable values.
- **Concurrency:** Compile-time data race prevention. Explicit `.share()` for threads.
- **comptime:** Zig-style compile-time execution blocks. Replaces macros and templates.

### Primitive Types

```
i8  i16  i32  i64  i128    signed integers
u8  u16  u32  u64  u128    unsigned integers
f32  f64                    floating point
bool                        boolean
str                         string (UTF-8)
usize  isize                pointer-sized integers
```

### String Interpolation

Strings use `{}` for interpolation with optional format specifiers:

```
"plain string"
"Hello, {name}"
"Value: {x:.2}"             // 2 decimal places
"Debug: {obj:?}"            // debug format
```

The lexer breaks interpolated strings into fragments:
`"Hello, {name}!"` → `StringStart` `StringFragment("Hello, ")` `Identifier(name)` `StringFragment("!")` `StringEnd`

### Reference Programs

11 sample programs define the language spec (see `tests/samples/`):

1. **hello.fg** — Basics: functions, let bindings, interpolation, mutability
2. **vec2.fg** — Structs, methods, operator overloading
3. **buffer.fg** — Ownership, borrowing, move semantics
4. **errors.fg** — Result<T>, ? propagation, match on errors
5. **concurrency.fg** — Atomic, spawn, compile-time race detection
6. **traits.fg** — Trait definitions, default methods, impl params
7. **generics.fg** — Stack<T>, trait bounds, where clauses
8. **comptime.fg** — Compile-time execution, validation, tables
9. **iterators.fg** — Iterator trait, combinators, zero-cost chains
10. **http_server.fg** — Full app: JSON, database, routing
11. **mini_lexer.fg** — Self-hosting: a minimal lexer written in Forge
