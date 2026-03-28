# Compile-Time Evaluation (comptime)

Forge's `comptime` is inspired by Zig. It lets you run ordinary Forge code at
compile time — no macros, no preprocessor, no template metaprogramming. Same
language, all the way down.

**Source files:** `src/comptime/mod.rs`, `src/comptime/tests.rs`

## How It Works

A `comptime { }` block is evaluated during compilation. The result replaces the
block in the compiled output as a constant.

```
fn main() {
    // This computation happens at compile time.
    let factorial = comptime {
        let mut result = 1
        for i in 1..11 {
            result = result * i
        }
        result
    }
    // At runtime, factorial is just the constant 3628800.
    print(factorial)
}
```

The compiler:
1. Encounters the `comptime` block
2. Evaluates it using the tree-walk interpreter
3. Replaces the block with the resulting constant value
4. Continues compilation with the constant in place

## What You Can Do

### Compute constants

```
let PI_SQUARED = comptime { 3.14159 * 3.14159 }
```

### Generate lookup tables

```
let fib10 = comptime {
    let mut a = 0
    let mut b = 1
    let mut i = 0
    while i < 10 {
        let temp = b
        b = a + b
        a = temp
        i = i + 1
    }
    a
}
// fib10 is 55 at compile time — zero runtime cost.
```

### Validate at compile time

Future: `comptime` will be able to trigger compile errors:
```
fn checked_port(comptime port: i32) -> i32 {
    comptime {
        if port < 1024 {
            @compile_error("Port must be >= 1024")
        }
    }
    port
}
```

## Why Not Macros?

Languages handle metaprogramming differently:

| Language | Approach | Complexity |
|----------|----------|------------|
| C/C++    | Preprocessor macros | Text substitution, no type safety |
| Rust     | `macro_rules!` + proc macros | Powerful but a second language |
| C++      | Templates + `constexpr` | Turing-complete but unreadable |
| Zig      | `comptime` | Same language, compile-time |
| **Forge** | **`comptime`** | **Same language, compile-time** |

With `comptime`, you already know the language. There's no new syntax to learn,
no pattern matching DSL, no hygiene rules to understand. If you can write a
Forge function, you can write a `comptime` block.

## Implementation

The implementation reuses the existing tree-walk interpreter:

1. **Parsing**: `comptime { ... }` is parsed as a `Comptime(Block)` expression
2. **HIR lowering**: passed through as `HirExprKind::Comptime(HirBlock)`
3. **Evaluation**: the `ComptimeEvaluator` converts the HIR block back to an
   AST, wraps it in a `main()` function, runs the interpreter, captures the
   output, and converts the result to a constant HIR expression
4. **Result**: the `comptime` node is replaced with `IntLiteral`, `FloatLiteral`,
   `BoolLiteral`, or `StringLiteral`

This approach is elegant because we didn't need to build a new evaluator — the
interpreter we built for development doubles as the comptime engine.

## Limitations

- Comptime blocks can't reference runtime variables (they run before runtime)
- Can't return struct values (yet) — only primitives and strings
- No `comptime` function parameters (yet) — that requires type-level analysis
- No `@compile_error` (yet)

## Testing

6 tests: HIR-level evaluation (integer, string, computation) and
interpreter-level execution (sum, fibonacci).

```bash
cargo test comptime
```
