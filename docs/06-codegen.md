# LLVM Code Generation

The codegen phase is where Forge programs become native binaries. It takes the
HIR and translates it into LLVM IR, which LLVM then optimizes and compiles to
machine code for your CPU.

**Source files:** `src/codegen/mod.rs`, `src/codegen/tests.rs`

## What is IR?

IR stands for **Intermediate Representation** — a form of your program that
sits between human-readable source code and the machine code your CPU runs.

Forge actually has two IRs:

```
Forge source       (what you write)
  → HIR            (desugared Forge — our IR, for type checking)
  → LLVM IR        (low-level portable assembly — LLVM's IR, for optimization)
  → Machine code   (what your CPU executes)
```

### Why not go straight from source to machine code?

Separation of concerns. We focus on turning Forge into *correct* IR, and
LLVM — which thousands of engineers have optimized over 20 years — handles
making it *fast*. This is why every major compiled language (Rust, C, C++,
Swift, Zig) uses LLVM as a backend.

### What LLVM IR looks like

A simple Forge function:

```
fn double(x: i64) -> i64 { x * 2 }
```

Becomes this LLVM IR:

```llvm
define i64 @double(i64 %0) {
entry:
  %x = alloca i64            ; allocate space for x on the stack
  store i64 %0, ptr %x       ; store the parameter value
  %x1 = load i64, ptr %x     ; load it back
  %mul = mul i64 %x1, 2      ; multiply by 2
  ret i64 %mul                ; return the result
}
```

You don't need to understand LLVM IR to use Forge — but you can inspect it
with `forge --ir file.fg` when debugging the compiler.

## Architecture

The codegen module uses [inkwell](https://github.com/TheDan64/inkwell), a
safe Rust wrapper around the LLVM C API.

### Key components

- **`Codegen` struct** — holds the LLVM context, module, builder, and variable
  tables
- **`compile_program()`** — two-pass: declare all functions, then compile bodies
- **`compile_expr()`** — recursive expression compilation, emitting LLVM
  instructions
- **`compile_binop()`** — integer and float arithmetic/comparison instructions
- **`compile_if()`** — conditional branches with phi nodes for if-as-expression
- **`compile_print()`** — calls C's `printf` with format strings

### Variable tracking

LLVM 15+ uses "opaque pointers" — all pointers are just `ptr` with no type
information. This means when we load a variable, we need to remember what type
it holds. The codegen tracks this with:

```
variables: Vec<HashMap<String, (PointerValue, BasicTypeEnum)>>
```

Each entry stores both the alloca pointer and the type to use when loading.

### The compilation pipeline

```
1. Declare functions  →  LLVM function signatures
2. Compile bodies     →  For each function:
   a. Create entry basic block
   b. Allocate and store parameters
   c. Compile statements and expressions
   d. Build return instruction
3. Write object file  →  LLVM emits native .o file
4. Link               →  cc links .o into executable
```

## What works

- Integer and float arithmetic (`+`, `-`, `*`, `/`, `%`)
- Comparisons (`<`, `>`, `<=`, `>=`, `==`, `!=`)
- Boolean logic (`&&`, `||`, `!`)
- Unary operators (`-`, `!`)
- Let bindings with type inference
- Function definitions and calls
- If/else as expressions (with phi nodes)
- String literals
- `print()` for integers, floats, and strings
- Assignment

## What doesn't work yet

- Structs and field access
- Method calls
- Arrays and indexing
- Closures
- For/while loops
- Match expressions
- String interpolation (prints parts separately, doesn't build a string)

These require more sophisticated LLVM IR generation — struct types, GEP
instructions for field access, loop basic blocks, etc. They'll come as the
codegen matures.

## Using it

```bash
# Compile to native binary
cargo run -- --compile program.fg -o program
./program

# Compile with explicit output name
cargo run -- --compile program.fg -o myapp

# Dump LLVM IR (useful for debugging)
cargo run -- --ir program.fg

# Still works: interpret without compiling
cargo run -- program.fg
```

## Testing

12 tests: 3 verify LLVM IR structure, 9 compile-and-run end-to-end tests
that build a native binary, execute it, and check stdout.

```bash
cargo test codegen
```
