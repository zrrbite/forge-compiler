# What's Next

## Current pipeline

```
Source (.fg)
  → Lexer      → Tokens                    ✅ done
  → Parser     → AST                       ✅ done
  → Interpreter (tree-walk)                 ✅ done (development aid)
  → Lowering   → HIR (desugared)           ✅ done
  → Type Check                             ✅ done (inference + unification)
  → Codegen    → LLVM IR → Native binary   ✅ done (basic subset)
  → Borrow Check                           ⬜ not started
  → MIR (Mid-level IR)                     ⬜ not started
```

The compiler can interpret any Forge program via the tree-walk interpreter,
and can compile a subset (functions, arithmetic, if/else, strings) to native
binaries via LLVM.

## Open issues

| # | Issue | Status |
|---|-------|--------|
| #1 | Self-hosting: rewrite compiler in Forge | Long-term |
| #4 | Borrow checker: ownership verification | Not started |
| #6 | Standard library: core types and functions | Not started |
| #7 | Operator overloading via trait impls | Not started |
| #8 | comptime: compile-time execution | Not started |

## Possible next steps

### 1. Expand codegen to cover more of the language

The LLVM backend currently handles functions, integers, floats, strings,
arithmetic, and if/else. Adding support for:

- **Structs** — LLVM struct types, GEP for field access
- **Loops** — basic blocks with back-edges
- **Arrays** — heap allocation, bounds checking
- **Closures** — function pointers + captured environment
- **Match** — lowered to a chain of conditional branches

This would let us compile most of the sample programs to native binaries.

### 2. Borrow checker (#4)

The crown jewel. Requires:
- Control flow graph (CFG) construction
- Liveness analysis
- Move tracking
- Borrow conflict detection

This is the hardest phase and the one that makes Forge more than just
"another language."

### 3. Standard library (#6)

Essential for real programs:
- File I/O
- String methods
- Collections (HashMap, Vec as proper types)
- Math functions

### 4. Operator overloading (#7)

Make `a + b` dispatch to `impl Add for Type`. Requires the type checker
to resolve operators to trait method calls during compilation.

### 5. comptime (#8)

Zig-style compile-time execution. The tree-walk interpreter already exists —
it could be reused to evaluate comptime blocks during compilation.

## Test coverage

| Phase       | Tests | Status |
|-------------|-------|--------|
| Lexer       | 33    | ✅     |
| Parser      | 40    | ✅     |
| Interpreter | 36    | ✅     |
| HIR         | 12    | ✅     |
| Type Checker| 44    | ✅     |
| Codegen     | 12    | ✅     |
| **Total**   | **177** | **All passing** |
