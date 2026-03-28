# What's Next

The compiler pipeline so far:

```
Source (.fg) → Lexer → Tokens → Parser → AST → Interpreter → Output
                ✅               ✅              ✅
```

The full pipeline we're building toward:

```
Source (.fg)
  → Lexer      → Tokens                    ✅ done
  → Parser     → AST                       ✅ done
  → Interpreter (tree-walk)                 ✅ done (development aid)
  → Lowering   → HIR (desugared)           ⬜ next
  → Type Check + Borrow Check              ⬜
  → Lowering   → MIR                       ⬜
  → Codegen    → LLVM IR                   ⬜
  → LLVM       → Native binary             ⬜
```

## Possible Next Steps

### 1. HIR Lowering (desugar the AST)

The AST mirrors the source syntax closely. The HIR (High-level IR) desugars
syntactic conveniences into simpler forms:

- `for x in iter { body }` → loop with iterator protocol
- `x += 1` → `x = x + 1`
- String interpolation → format function calls
- `let x = expr else { ... }` → match + early return
- Method calls → function calls with explicit self

This makes later phases (type checking, borrow checking) much simpler because
they only deal with a small set of core constructs.

### 2. Name Resolution + Symbol Table

Before type checking, we need to resolve what every identifier refers to:

- Which function does `foo()` call?
- Which struct does `Vec2 { x, y }` create?
- Is `x` a local variable, a parameter, or a captured closure variable?

This builds a symbol table that maps identifiers to their declarations.

### 3. Type Checking

Forge uses Hindley-Milner-style type inference with trait bounds. The type
checker will:

- Infer types for `let` bindings without annotations
- Check that function arguments match parameter types
- Verify trait bounds on generic type parameters
- Resolve operator overloading through trait impls

### 4. Borrow Checking

The crown jewel. This is a constraint solver over the control flow graph:

- Track which variables own which values
- Ensure references don't outlive their referents
- Prevent aliased mutation (no `&mut` while `&` exists)
- Catch use-after-move errors

This is the hardest phase to implement correctly. Rust spent years getting
their borrow checker right.

### 5. LLVM Code Generation

Once we have a type-checked, borrow-checked IR, we lower it to LLVM IR
using the `inkwell` crate (Rust bindings to LLVM). LLVM handles:

- Optimization passes (inlining, dead code elimination, etc.)
- Machine code generation for all major architectures
- Linking

This turns Forge from an interpreted language into a compiled one that
produces native binaries.

## Current Test Coverage

| Phase       | Tests | Status |
| ----------- | ----- | ------ |
| Lexer       | 33    | ✅     |
| Parser      | 40    | ✅     |
| Interpreter | 36    | ✅     |
| **Total**   | **109** | **All passing** |
