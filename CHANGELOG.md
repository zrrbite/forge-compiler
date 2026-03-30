# Changelog

All notable changes to the Forge programming language and compiler.

## [Unreleased]

### Compiled Mode
- String concatenation (`+`) and comparison (`==`, `!=`) via LLVM runtime
- Array `push()` and `for x in arr` iteration
- Array `map`, `filter`, `fold`, `each` with closures (indirect calls)

### Tooling
- GitHub Actions CI: tests, clippy, format check on every PR
- Language server (`forge-lsp`) with real-time diagnostics for Neovim

### Fixes
- Remove unused import warning in m7_modules tests
- Suppress dead_code warning in m3_generics tests

## [0.7.0] — 2026-03-30

**First public release.**

### Language Features
- Safe navigation operator `?.` for None-safe chaining
- Null coalescing operator `??` for default values
- `defer` statement for Go-style scope cleanup (LIFO execution)
- Array/string slice syntax: `arr[1:3]`, `str[0:5]`, `arr[:n]`, `arr[n:]`
- Closures compile to native code via LLVM (function pointers, indirect calls)
- Arrays compile to native code via LLVM (malloc, indexing, len)

### Standard Library
- `input()` / `input(prompt)` — read from stdin
- `stdin_lines()` — read all stdin lines
- `env_get(key)` / `env_set(key, value)` / `env_vars()` — environment variables
- `exec(cmd, args)` — run shell commands
- `File.read_lines(path)` — read file as array of lines
- Array methods: `sort()`, `min()`, `max()`, `sum()`, `enumerate()`, `flatten()`, `dedup()`, `contains()`
- String methods: `lines()`, `chars()`, `repeat(n)`, `parse_int()`, `parse_float()`
- HashMap: `get_or(key, default)`, `entries()`, iteration with `for pair in map`

### Tooling
- **REPL**: `forge` with no args starts interactive prompt with persistent state
- **Scripting mode**: `-e` flag, shebang support (`#!/usr/bin/env forge`), implicit main
- **`forge init`**: project scaffolding
- **`--help` / `--version`**: CLI discoverability
- **Better error messages**: line:column with source snippets
- **VS Code extension**: syntax highlighting (`editors/vscode/`)
- **Neovim/Vim extension**: syntax highlighting (`editors/nvim/`)

### Compiler
- Fix trait dispatch for structs with identical LLVM layouts
- Fix UTF-8: `len()`, `char_at()`, `substring()` use char indices
- Self-hosted compiler: all 11 reference programs compile via C transpilation

### Documentation
- Developer guide (`docs/DEVELOPERS.md`)
- Language inspirations table in README
- JSON parser stress test (`examples/json_parser.fg`)
- AoC solutions demonstrating real-world usage

## [0.6.0] — 2026-03-29

### Self-Hosting Milestone
- Self-hosted compiler written in Forge (`self-host/`)
- Lexer, parser, AST, and C codegen all in Forge
- 5/11 → 11/11 reference programs compile through self-hosted pipeline
- ForgeArray C runtime (dynamic arrays with push/get/len)
- String interpolation with type-aware to_str conversions
- Operator overloading, method dispatch, struct field tracking
- Inline closure codegen (map/filter/fold/each as C loops)
- Match expressions (Some/None patterns)

## [0.5.0] — 2026-03-29

### Compiler Improvements
- `&mut` references for pass-by-reference function parameters
- Fix `mut self`: method mutations propagate back to caller
- Fix nested `mut self` calls (SelfValue write-back)
- Fix parser: `else`/`else if` across newlines
- LLVM `-O3` optimizations: Forge within 15% of Rust performance

## [0.4.0] — 2026-03-28

### Milestones M8-M12
- M8: Match compilation, trait dispatch, closures with blocks
- M9-M11: Advanced compiler features
- M12: Mini-lexer written in Forge (self-hosting validation)
- Module system (`use` declarations, multi-file programs)
- HashMap and File I/O builtins
- Generics foundation (type parameters, unification)
- Result/Option with `?` propagation

## [0.3.0] — 2026-03-28

### Standard Library and Type System
- Standard library: print, math, type conversions, assertions
- Compile-time evaluation (`comptime` blocks)
- Borrow checker: ownership, move semantics, borrow conflicts
- String methods (M1) and mutable dynamic arrays (M2)

## [0.2.0] — 2026-03-28

### LLVM Backend
- LLVM codegen: functions, structs, methods, loops, if/else, print
- Operator overloading via trait impls
- Type checker with inference and unification
- HIR (High-level Intermediate Representation)

## [0.1.0] — 2026-03-28

### Initial Implementation
- Lexer with 26 keywords, string interpolation, all operators
- Parser with recursive descent + Pratt expression parsing
- Tree-walk interpreter — Forge programs run
- 10 reference sample programs
- Documentation: language design, lexer, parser, interpreter
