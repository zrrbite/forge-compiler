# Standard Library

Forge's standard library provides built-in functions and constants that are
always available without imports.

**Source files:** `src/stdlib/mod.rs`, `src/stdlib/tests.rs`

## Built-in Functions

### I/O

| Function | Signature | Description |
|----------|-----------|-------------|
| `print(value)` | `any -> ()` | Print a value + newline to stdout |
| `println(value)` | `any -> ()` | Alias for print |
| `eprint(value)` | `any -> ()` | Print to stderr |

### Type Conversion

| Function | Signature | Description |
|----------|-----------|-------------|
| `to_str(value)` | `any -> str` | Convert any value to string |
| `to_int(s)` | `str -> i64` | Parse string as integer |
| `to_float(s)` | `str -> f64` | Parse string as float |

### Math

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs(x)` | `num -> num` | Absolute value |
| `min(a, b)` | `num, num -> num` | Minimum of two values |
| `max(a, b)` | `num, num -> num` | Maximum of two values |

### Assertions

| Function | Signature | Description |
|----------|-----------|-------------|
| `assert(cond)` | `bool -> ()` | Panic if false |
| `assert_eq(a, b)` | `any, any -> ()` | Panic if a != b |

## Constants

| Name | Value | Description |
|------|-------|-------------|
| `PI` | 3.14159... | Circle constant |
| `E` | 2.71828... | Euler's number |

## Built-in Methods

These are methods on primitive types, available without any import:

### Arrays
`len()`, `push(val)`, `pop()`, `last()`, `map(f)`, `filter(f)`,
`fold(init, f)`, `each(f)`

### Strings
`len()`, `trim()`, `contains(sub)`

### Floats
`sqrt()`, `abs()`, `sin()`, `cos()`

## Example

```
fn main() {
    let x = -42
    print(abs(x))            // 42
    print(min(10, 20))       // 10
    print(max(10, 20))       // 20
    print(PI)                // 3.141592653589793
    print(to_str(42))        // "42"
    print(to_int("123"))     // 123

    assert(true)
    assert_eq(2 + 2, 4)
}
```

## Testing

13 stdlib tests covering all built-in functions.

```bash
cargo test stdlib
```
