# Self-Hosting: The Forge Compiler Written in Forge

The `self-host/` directory contains a Forge compiler written in Forge itself.
It transpiles Forge source code to C99, then calls `cc` to produce a native
binary. All 11 reference programs compile and run correctly through this
pipeline.

## Usage

```bash
# Compile a Forge program to a native binary
forge self-host/compile.fg input.fg output

# Example
forge self-host/compile.fg tests/samples/hello.fg hello
./hello
# Hello, Martin!
```

## Architecture

```
Source (.fg)
  → self-host/lexer.fg     → [Token]
  → self-host/parser.fg    → [Item]  (AST)
  → self-host/codegen.fg   → C99 source code
  → cc -std=c11            → Native binary
```

The codegen targets C as a transpilation target (like Cfront for C++). This is
simpler than emitting LLVM IR directly and is a proven bootstrap strategy.

## Components

### Lexer (`self-host/lexer.fg`)

Tokenizes Forge source. Handles keywords, identifiers, numbers, floats,
strings (with escape sequences), operators, comments, and newline collapsing.

### AST (`self-host/ast.fg`)

AST node definitions using a tag+fields approach (string `kind` field for
discrimination, since full enum codegen isn't needed). Covers: Item (fn,
struct, impl, use, trait, enum), Stmt (let, expr, return, break, continue),
Expr (30+ variants including if, while, for, match, closures, struct literals).

### Parser (`self-host/parser.fg`)

Recursive descent parser with Pratt expression parsing. All parse functions
take `p: &mut Parser` for pass-by-reference mutation. Handles operator
precedence, string interpolation, struct literals, closures, match expressions.

### Codegen (`self-host/codegen.fg`)

The largest component (~1500 lines). Transpiles AST to C99 with:

- **C runtime**: string concat, to_str conversions, ForgeArray (dynamic
  void*-based arrays with push/get/len), ForgeHashMap (linear-search string
  set), char_at, substring helpers
- **Type tracking**: string-encoded type map (`"name:type,name:type,..."`)
  with struct field types, function return types, and closure parameter types
- **Method dispatch**: static calls (`Vec2.new()` → `Vec2_new()`), instance
  calls (`v.length()` → `Vec2_length(v)`), operator overloading (`a + b` →
  `Vec2_add(a, b)`)
- **String interpolation**: splits `"Hello, {name}!"` into concat chains with
  type-aware to_str conversions
- **Closures**: `map`, `filter`, `fold`, `each` inlined as C loops
- **Match expressions**: `Some(val)`/`None` patterns compiled to if/else
  with array-length checks
- **Control flow**: if/else (with ternary optimization), while, for-in over
  ranges and arrays, implicit returns, return-if expressions

### Compiler Driver (`self-host/compile.fg`)

Orchestrates the pipeline: reads source, lexes, parses, generates C, writes
the C file, calls `cc` via the `exec()` builtin, cleans up.

## What Works

All 11 reference programs compile and produce correct output:

| Program | Features Exercised |
|---------|--------------------|
| hello.fg | Functions, let bindings, string interpolation, mutability |
| vec2.fg | Structs, methods, operator overloading, field access |
| buffer.fg | Arrays, push, indexed access, range for-loops |
| errors.fg | Match on Some/None, filter closures, array operations |
| traits.fg | Trait impls, multiple structs, method dispatch |
| generics.fg | Stack (push/pop/peek), array-to-string, implicit returns |
| iterators.fg | map, filter, fold, each, chained operations, ranges |
| concurrency.fg | Nested for-loops, compound assignment |
| comptime.fg | Range for-loops, fibonacci computation |
| http_server.fg | Struct arrays, for-in iteration, method calls in loops |
| mini_lexer.fg | String methods, char_at, is_digit, HashMap, tokenization |

## Known Limitations

- **No generics** — the self-hosted compiler uses concrete types only
- **Array push on struct fields** — doesn't persist (Forge copies structs
  on field access). Worked around with string-encoded type maps.
- **No short-circuit `&&`** — Forge evaluates both sides. Use nested `if`
  for guarded access (bounds checking before indexing).
- **Closures** — only array methods (map/filter/fold/each) are supported,
  not general closure values or captures.
- **Match** — only Some/None patterns using array-as-Option encoding.
- **Type inference is heuristic** — uses method-name guessing and
  string-encoded type maps rather than full unification.

## Next Steps

1. **Bootstrap** — compile the self-hosted compiler with the Rust compiler,
   then use the result to compile itself.
2. **Scripting mode** — shebang support, `-e` flag, REPL (#29).
