# The Interpreter

The interpreter is a tree-walk evaluator that executes Forge programs directly
from the AST. It exists as a stepping stone — letting us run and validate
programs before building the full LLVM compilation backend.

**Source files:** `src/interpreter/mod.rs`, `src/interpreter/tests.rs`

## Architecture

### Values

The runtime value system:

```
Value::Int(i128)          — all integer types
Value::Float(f64)         — all float types
Value::Bool(bool)
Value::String(String)
Value::Array(Vec<Value>)
Value::Struct { name, fields: HashMap<String, Value> }
Value::Variant { name, fields: Vec<Value> }
Value::Function(FnValue)
Value::Unit               — void / no value
```

Functions come in three flavors:
- `UserDefined` — parsed from Forge source (name, params, body)
- `Closure` — captures its environment at creation time
- `Builtin` — implemented in Rust (currently just `print`)

### Environment

Variables live in a scope chain — a stack of `HashMap<String, Value>`. Looking
up a variable walks the stack from top to bottom. This gives us lexical scoping
naturally:

```
env.push_scope()    — entering a block or function
env.define(name, v) — new binding in current scope
env.get(name)       — look up, walking outward
env.set(name, v)    — update existing binding
env.pop_scope()     — leaving a block
```

### Control Flow with `Outcome`

The trickiest part of a tree-walk interpreter is handling control flow —
`return`, `break`, and `continue` need to unwind through nested expressions
and blocks. We model this with an `Outcome` enum:

```rust
enum Outcome {
    Val(Value),       // normal value
    Return(Value),    // early return from function
    Break,            // break from loop
    Continue,         // continue to next iteration
    Error(String),    // runtime error
}
```

A `try_val!` macro extracts the value from `Outcome::Val` and propagates
everything else upward. At function call boundaries, `Return(v)` is caught
and converted to `Val(v)`. At loop boundaries, `Break` and `Continue` are
caught. `Error` propagates all the way to the top.

## What Works

### Basics
- Integer, float, boolean, string literals
- `let` and `let mut` bindings
- Assignment (`=`) and compound assignment (`+=`, `-=`, etc.)
- All arithmetic and comparison operators
- Unary operators (`-`, `!`)
- String interpolation with arbitrary expressions

### Functions
- Named functions with parameters and return types
- Implicit returns (last expression in block)
- Explicit `return` statements
- Nested function calls
- Closures (`|x| x * 2`) with captured environments
- Passing functions/closures as arguments

### Structs
- Struct literal creation (`Point { x: 1.0, y: 2.0 }`)
- Field shorthand (`Point { x, y }`)
- Field access (`p.x`)
- Static method calls (`Vec2.new(3.0, 4.0)`)
- Instance method calls (`v.length()`)

### Control Flow
- `if` / `else` (as expressions — they return values)
- `match` with literal, identifier, wildcard, and variant patterns
- `for x in collection { ... }`
- `while condition { ... }`
- `break` and `continue`

### Collections
- Array literals (`[1, 2, 3]`)
- Indexing (`arr[0]`)
- Built-in methods: `len`, `push`, `pop`, `last`, `map`, `filter`, `fold`, `each`
- Ranges (`0..10`, `1..=5`) materialize as arrays

### Built-in Methods
- Arrays: `len`, `push`, `pop`, `last`, `map`, `filter`, `fold`, `each`
- Strings: `len`, `trim`, `contains`
- Floats: `sqrt`

## What Doesn't Work Yet

- Ownership / borrow checking (everything is cloned)
- `comptime` blocks
- Generics (type params are erased)
- `spawn` / concurrency
- Multi-file programs / `use` imports
- Operator overloading via trait impls

These require either static analysis (ownership, generics) or a runtime
model we haven't built yet (concurrency). They'll come with the type
checker and LLVM backend.

## Testing

36 tests, including integration tests that run the hello.fg and vec2.fg
sample programs end-to-end and verify their output.

```bash
cargo test interpreter
```

## Using the Interpreter

```bash
# Run a Forge program
cargo run -- myprogram.fg

# Dump tokens instead
cargo run -- --tokens myprogram.fg

# Dump AST instead
cargo run -- --ast myprogram.fg
```
