# What's Next

## Current pipeline

```
Source (.fg)
  → Module resolver (use declarations)      ✅ done
  → Lexer      → Tokens                     ✅ done
  → Parser     → AST                        ✅ done
  → Interpreter (tree-walk)                  ✅ done
  → Lowering   → HIR (desugared)            ✅ done
  → Type Check                              ✅ done
  → Borrow Check                            ✅ done
  → Comptime evaluation                     ✅ done
  → Codegen    → LLVM IR → Native binary    ✅ done (structs, methods, loops)
```

## Self-hosting roadmap

The goal is to rewrite the Forge compiler in Forge itself. Progress:

| Milestone | Description | Status |
|-----------|-------------|--------|
| M1 | String methods (char_at, substring, split, etc.) | ✅ |
| M2 | Mutable dynamic arrays (push, pop, insert in-place) | ✅ |
| M3 | Generics foundation (TypeParam, GenericInstance) | ✅ |
| M4 | Result/Option with ? propagation | ✅ |
| M5 | HashMap / dictionary type | ✅ |
| M6 | File I/O (File.read, File.write, args, exit) | ✅ |
| M7 | Module system (use declarations, multi-file) | ✅ |
| M8 | Enum variants in LLVM codegen | Queued |
| M9 | Recursive types and Box\<T\> | Queued |
| M10 | Trait dispatch and conformance checking | Queued |
| M11 | Multi-line closures and closure codegen | Queued |
| M12 | Integration: mini-lexer written in Forge | Queued |
| #1 | **Self-hosting: rewrite compiler in Forge** | **The finale** |

## Test coverage

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
| **Total** | **292** |
