# Self-Hosting: Rewriting the Compiler in Forge

The grand finale — writing the Forge compiler in Forge itself.

## Status

Work is on the `self-hosting` branch.

### What's Done

**Lexer (`self-host/lexer.fg`)** — Complete. Tokenizes all sample `.fg` files
correctly. Handles keywords, identifiers, numbers, floats, strings, operators,
comments, and newline collapsing. Uses: string methods (`char_at`, `substring`,
`is_digit`), HashMap (keyword lookup), mutable arrays, File I/O.

**AST (`self-host/ast.fg`)** — Complete. All node types: Item (fn, struct, impl,
use), Stmt (let, expr, return, break, continue), Expr (30+ variants). Uses a
tag+fields approach since full enum codegen isn't available yet.

**Parser (`self-host/parser.fg`)** — Structure complete. Recursive descent for
items + Pratt parsing for expressions. All parse functions use `p: &mut Parser`
for pass-by-reference. Blocked by #28 (nested `mut self`).

### What's Remaining

1. **Fix #28** — nested `mut self` method calls need to compose. This is the
   only runtime blocker.
2. **Finish parser** — once #28 is fixed, the parser should work immediately.
3. **Write codegen in Forge** — either emit C (transpiler) or LLVM IR as text.
4. **Bootstrap** — compile the Forge compiler using the Rust compiler, then use
   the result to compile itself.

## Key Language Features Used

The self-hosted compiler exercises nearly every Forge feature:

- **String methods** — `char_at`, `substring`, `is_digit`, `is_alpha`, `starts_with`
- **Mutable arrays** — `push`, `pop`, `len`
- **HashMap** — keyword lookup table
- **Structs with methods** — Token, Parser, Item, Stmt, Expr
- **`mut self`** — Parser.advance(), Parser.expect()
- **`&mut` references** — all parse functions take `p: &mut Parser`
- **`if/else if` chains** — token dispatch
- **`while` loops** — scanning loops
- **File I/O** — reading `.fg` source files
- **Module system** — `use lexer`, `use ast`, `use parser`
- **Result/Option** — `File.read().unwrap()`

## Architecture

```
Source (.fg)
  → self-host/lexer.fg    → [Token]
  → self-host/parser.fg   → [Item]  (AST)
  → self-host/codegen.fg  → C source or LLVM IR text
  → cc / llc              → Native binary
```

The codegen will likely target C as a transpilation target (like Cfront for
C++). This is simpler than emitting LLVM IR directly and is a proven bootstrap
strategy.

## Blockers and Caveats

1. **Nested `mut self` (#28)** — when `expect()` calls `advance()` internally,
   the position change is lost. This is a runtime limitation, not a language
   design issue.

2. **String interpolation with braces** — strings containing literal `{` or `}`
   must use escape sequences (`\{`, `\}`). This affects parser code that checks
   for brace tokens.

3. **No `else if` on separate lines (fixed)** — the Forge parser now handles
   `else` after newlines. This was fixed during self-hosting development.

4. **Value semantics** — Forge copies structs on assignment and function calls.
   `&mut` references solve this for function parameters, but struct field
   mutation through nested method calls is still limited.
