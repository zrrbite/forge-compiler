# What's Next

## Current Pipeline

```
Source (.fg)
  → Module resolver (use declarations)      ✅
  → Lexer      → Tokens                     ✅
  → Parser     → AST                        ✅
  → Interpreter (tree-walk)                  ✅
  → Lowering   → HIR (desugared)            ✅
  → Type Check                              ✅
  → Borrow Check                            ✅
  → Comptime evaluation                     ✅
  → Codegen    → LLVM IR → Native binary    ✅
  → Self-hosted compiler (Forge → C → cc)   ✅
```

## Completed Milestones

All core milestones are complete as of v0.7.0:

| Milestone | Description | Status |
|-----------|-------------|--------|
| M1 | String methods (char_at, substring, split, etc.) | ✅ |
| M2 | Mutable dynamic arrays (push, pop, insert in-place) | ✅ |
| M3 | Generics foundation (TypeParam, GenericInstance) | ✅ |
| M4 | Result/Option with ? propagation | ✅ |
| M5 | HashMap / dictionary type | ✅ |
| M6 | File I/O (File.read, File.write, args, exit) | ✅ |
| M7 | Module system (use declarations, multi-file) | ✅ |
| M8 | Closures, operators, traits | ✅ |
| M9 | Arrays in LLVM codegen | ✅ |
| M10 | Trait dispatch (struct name tracking) | ✅ |
| M11 | Closure codegen (function pointers) | ✅ |
| M12 | Integration: mini-lexer written in Forge | ✅ |
| Self-hosting | Forge compiler written in Forge (11/11 samples) | ✅ |

## Test Coverage

319 tests across all phases:

| Phase | Tests |
|-------|-------|
| Lexer | 33 |
| Parser | 40 |
| Interpreter | 36 |
| HIR | 12 |
| Type Checker | 44 |
| Codegen | 17 |
| Borrow Checker | 15 |
| Comptime | 6 |
| Stdlib | 13 |
| Operator Overloading | 2 |
| M1: Strings | 15 |
| M2: Arrays | 11 |
| M3: Generics | 11 |
| M4: Result/Option | 18 |
| M5: HashMap | 8 |
| M6: File I/O | 5 |
| M7: Modules | 6 |
| M8-M11: Closures, Operators, Traits | 12 |
| Mut Self / Mut Ref | 12 |
| Integration | 4 |
| **Total** | **319** |

## Future Work

- **Escaped closures**: heap-allocated environment for closures returned from functions
- **NLL borrow checker**: non-lexical lifetimes for more flexible borrowing
- **Package manager**: dependency management and project configuration
- **LSP server**: IDE integration beyond syntax highlighting
- **WebAssembly target**: compile to wasm for browser/edge use
- **Optimization**: tail call elimination, constant folding before LLVM
