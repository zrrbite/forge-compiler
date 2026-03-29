# Forge

A systems programming language that combines Rust's memory safety with C++'s
zero-cost abstractions — wrapped in syntax designed to be enjoyable to write.

```
fn greet(name: str) -> str {
    "Hello, {name}!"
}

fn main() {
    let name = "World"
    print(greet(name))

    let numbers = [1, 2, 3, 4, 5]
    let sum = numbers.fold(0, |acc, x| acc + x)
    print("Sum: {sum}")
}
```

No semicolons. Implicit returns. String interpolation. Ownership without the ceremony.

## Features

- **Memory safe** — ownership and borrow checking at compile time, no garbage collector
- **Zero-cost abstractions** — structs, traits, generics, operator overloading
- **String interpolation** — `"Hello, {name}!"` just works
- **No semicolons** — newlines terminate statements
- **Implicit returns** — last expression is the return value
- **comptime** — compile-time evaluation using the same language (no macros)
- **Result/Option** — `Ok`, `Err`, `Some`, `None` with `?` propagation
- **Closures** — `|x| x * 2` with environment capture
- **Built-in methods** — arrays have `map`, `filter`, `fold`, `push`, `pop`
- **Module system** — `use math` imports from `math.fg`
- **Compiles to native** — LLVM backend produces native binaries

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (for building the compiler)
- LLVM 21 (for native compilation — interpretation works without it)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# On Arch Linux, LLVM is available via:
sudo pacman -S llvm

# On Ubuntu/Debian:
sudo apt install llvm-dev

# On macOS:
brew install llvm
```

### Install Forge

```bash
git clone https://github.com/zrrbite/forge-compiler.git
cd forge-compiler
cargo build --release
```

The binary is at `target/release/forge`. You can copy it to your PATH:

```bash
sudo cp target/release/forge /usr/local/bin/
```

### Write Your First Program

Create `hello.fg`:

```
fn main() {
    print("Hello from Forge!")
}
```

Run it:

```bash
forge hello.fg
```

### Compile to Native Binary

```bash
forge --compile hello.fg -o hello
./hello
```

## Language Tour

### Variables

```
let x = 42                  // immutable, type inferred
let mut count = 0           // mutable
let name: str = "Forge"     // explicit type annotation
count += 1                  // compound assignment
```

### Functions

```
fn add(a: i32, b: i32) -> i32 {
    a + b                   // implicit return
}

fn greet(name: str) -> str {
    "Hello, {name}!"        // string interpolation
}
```

### Structs and Methods

```
struct Vec2 {
    x: f64,
    y: f64,
}

impl Vec2 {
    fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }       // field shorthand
    }

    fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

let v = Vec2.new(3.0, 4.0)  // Type.method() syntax
print(v.length())            // 5
```

### Operator Overloading

```
impl Add for Vec2 {
    fn add(self, other: Vec2) -> Vec2 {
        Vec2 { x: self.x + other.x, y: self.y + other.y }
    }
}

let c = a + b   // dispatches to add()
```

### Error Handling

```
fn safe_div(a: i64, b: i64) -> i64 {
    if b == 0 {
        Err("division by zero")
    } else {
        Ok(a / b)
    }
}

fn main() {
    let result = safe_div(10, 2)
    match result {
        Ok(v)  => print(v),
        Err(e) => print(e),
    }

    // Or use ? to propagate errors:
    let value = safe_div(10, 2)?
}
```

### Collections

```
// Arrays with built-in methods
let items = [1, 2, 3, 4, 5]
let doubled = items.map(|x| x * 2)
let evens = items.filter(|x| x % 2 == 0)
let sum = items.fold(0, |acc, x| acc + x)

// Mutable arrays
let mut list = []
list.push(1)
list.push(2)
list.push(3)

// HashMap
let mut config = HashMap()
config.insert("port", 8080)
let port = config.get("port").unwrap()
```

### Control Flow

```
// If/else (expressions — they return values)
let max = if a > b { a } else { b }

// Match
match status {
    200 => print("OK"),
    404 => print("Not Found"),
    _   => print("Unknown"),
}

// Loops
for item in items {
    print(item)
}

while condition {
    step()
}
```

### Compile-Time Evaluation

```
let factorial = comptime {
    let mut result = 1
    for i in 1..11 {
        result = result * i
    }
    result
}
// factorial is 3628800 — computed at compile time, zero runtime cost.
```

### Modules

```
// math.fg
fn double(x: i32) -> i32 { x * 2 }

// main.fg
use math
fn main() { print(double(21)) }
```

### File I/O

```
let content = File.read("data.txt").unwrap()
File.write("output.txt", "Hello!")
```

## CLI Reference

```bash
forge <file.fg>              # Interpret (run directly)
forge --compile <file.fg>    # Compile to native binary
forge --compile -o name      # Specify output binary name
forge --ir <file.fg>         # Dump LLVM IR
forge --ast <file.fg>        # Dump AST
forge --tokens <file.fg>     # Dump token stream
```

## Project Status

Forge is a working language with an interpreter and LLVM compilation backend.

| Component | Status |
|-----------|--------|
| Lexer | Complete — 26 keywords, string interpolation, all operators |
| Parser | Complete — recursive descent + Pratt expression parsing |
| HIR | Complete — desugared intermediate representation |
| Type Checker | Complete — inference, unification, generics |
| Borrow Checker | Basic — use-after-move, borrow conflicts |
| Interpreter | Complete — full language support |
| LLVM Codegen | Partial — functions, structs, methods, loops, if/else |
| Standard Library | Growing — print, math, file I/O, HashMap |
| Module System | Complete — use declarations, multi-file programs |
| Test Suite | 307 tests across all phases |

## Documentation

Detailed documentation is in the `docs/` directory:

- [Language Design](docs/01-language-design.md) — design decisions and feature origins
- [Lexer](docs/02-lexer.md) — tokenization and string interpolation
- [Parser](docs/03-parser.md) — recursive descent + Pratt parsing
- [Interpreter](docs/04-interpreter.md) — tree-walk evaluation
- [Roadmap](docs/05-whats-next.md) — what's done, what's next
- [Codegen](docs/06-codegen.md) — LLVM IR generation
- [Borrow Checker](docs/07-borrowck.md) — ownership and borrowing
- [Comptime](docs/08-comptime.md) — compile-time evaluation
- [Standard Library](docs/09-stdlib.md) — built-in functions

## Building from Source

```bash
git clone https://github.com/zrrbite/forge-compiler.git
cd forge-compiler
cargo build --release       # build
cargo test                  # run 307 tests
cargo run -- hello.fg       # run a program
```

## License

MIT — see [LICENSE](LICENSE).
