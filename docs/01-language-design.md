# Language Design

Forge is a systems programming language that combines Rust's memory safety with
C++'s zero-cost abstractions, wrapped in syntax designed to be enjoyable to
write.

## Design Goals

1. **Rust's safety without Rust's ceremony** — ownership and borrowing are
   enforced at compile time, but lifetime annotations are rarely needed (~95%
   of cases are elided automatically).

2. **C++'s expressiveness without C++'s footguns** — zero-cost generics,
   operator overloading, and RAII, without the 40 years of historical baggage.

3. **Readable at a glance** — a junior developer should be able to follow an
   ownership example without knowing what a borrow checker is.

## Key Decisions

### No semicolons

Newlines terminate statements. This removes visual noise without introducing
ambiguity — the parser handles multi-line expressions naturally through
context (operators, open brackets, etc.).

```
let x = 42
let y = x + 1
```

### Implicit returns

The last expression in a block is its value. No `return` keyword needed for
the common case.

```
fn double(x: i32) -> i32 {
    x * 2
}
```

### String interpolation

Built into the language, not a macro. The lexer handles it by breaking
interpolated strings into fragments and expressions.

```
let name = "world"
print("Hello, {name}!")
print("2 + 2 = {2 + 2}")
```

### `comptime` instead of macros

Inspired by Zig. Compile-time execution uses the same language — no second
syntax to learn, no macro_rules! pattern matching, no preprocessor.

```
fn make_table(comptime n: usize) -> [f64; n] {
    comptime {
        let table: [f64; n] = undefined
        for i in 0..n {
            table[i] = math.sin(2.0 * math.PI * f64(i) / f64(n))
        }
        table
    }
}
```

### Traits as the only abstraction

No inheritance, no virtual dispatch unless you opt in. Traits with default
methods cover everything from interfaces to mixins.

### `Type.method()` not `Type::method()`

A small syntactic choice that makes Forge feel cleaner than Rust. The double
colon is reserved for turbofish syntax (`parse::<u16>()`).

## What We Stole and From Where

| Feature                    | Origin      |
| -------------------------- | ----------- |
| Ownership + borrow checker | Rust        |
| Result/Option + `?`        | Rust        |
| Pattern matching           | Rust        |
| Traits                     | Rust        |
| Zero-cost generics         | C++/Rust    |
| Operator overloading       | C++         |
| RAII                       | C++         |
| `comptime`                 | Zig         |
| Optional ergonomics        | Swift       |
| Simplicity as a value      | Go          |
| Cleaner syntax             | Carbon      |

## Primitive Types

```
i8  i16  i32  i64  i128    // signed integers
u8  u16  u32  u64  u128    // unsigned integers
f32  f64                    // floating point
bool                        // boolean
str                         // string (UTF-8)
usize  isize                // pointer-sized integers
```

## Reference Programs

The language is defined by 10 sample programs that serve as both specification
and test suite. The compiler is "done" when all 10 run correctly:

1. **hello.fg** — basics, string interpolation, mutability
2. **vec2.fg** — structs, methods, operator overloading
3. **buffer.fg** — ownership, borrowing, move semantics
4. **errors.fg** — Result, `?` propagation, match on errors
5. **concurrency.fg** — Atomic, spawn, compile-time race detection
6. **traits.fg** — trait definitions, default methods
7. **generics.fg** — Stack\<T\>, trait bounds, where clauses
8. **comptime.fg** — compile-time execution and validation
9. **iterators.fg** — Iterator trait, zero-cost combinator chains
10. **http_server.fg** — full app with JSON, database, routing
