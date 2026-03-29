# Forge vs Other Languages

Honest comparisons. Forge is a young language — we show where it shines and
where it still has work to do.

## Performance: fib(35) Benchmark

Recursive Fibonacci is a simple benchmark that tests function call overhead
and basic arithmetic. All produce the same result: 9,227,465.

| Language | Compile Time | Runtime | Binary Size |
|----------|-------------|---------|-------------|
| **Forge (compiled)** | 93ms | 35ms | 15 KB |
| C (gcc -O2) | 85ms | 24ms | 15 KB |
| Rust (rustc -O) | 61ms | 30ms | 3.9 MB |
| **Forge (interpreted)** | — | 53,050ms | — |

### What this tells us

- **Forge compiled is within 15% of Rust** and **1.5x of C** — impressive
  for a young language.
- **Binary size matches C** — both produce ~15KB binaries. Rust's binary is
  263x larger because it statically links the standard library.
- **Compilation speed is on par with C** — our pipeline (lex → parse → HIR →
  LLVM IR → optimize → link) takes about the same time as gcc.
- **The interpreter is ~1000x slower** — expected for a tree-walk interpreter.
  This is the development/prototyping mode, not for production.

### How we got here

Forge uses LLVM's full `-O3` optimization pipeline:
- **Aggressive optimization level** on the target machine
- **Native CPU targeting** — uses your actual CPU's features (AVX, etc.)
- **`default<O3>` pass pipeline** — inlining, loop unrolling, constant
  propagation, dead code elimination, vectorization, function merging

### Room for further improvement

1. **Tail call optimization** — mark recursive calls as `musttail`.
2. **Constant folding** before emitting IR — evaluate `2 + 3` at compile time.
3. **Better alloca placement** — use SSA values directly for simple bindings.
4. **Forge-level inlining** — inline small functions before LLVM sees them.

## Syntax Comparison

The same program in five languages. Notice how Forge eliminates ceremony
while keeping the safety guarantees.

### Forge

```
struct Vec2 {
    x: f64,
    y: f64,
}

impl Vec2 {
    fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }

    fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

impl Add for Vec2 {
    fn add(self, other: Vec2) -> Vec2 {
        Vec2 { x: self.x + other.x, y: self.y + other.y }
    }
}

fn main() {
    let a = Vec2.new(3.0, 4.0)
    let b = Vec2.new(1.0, 2.0)
    let c = a + b
    print("length: {c.length()}")
}
```

**14 lines of logic.** No semicolons, no `println!` macro, `Type.method()` syntax.

### Rust

```rust
struct Vec2 {
    x: f64,
    y: f64,
}

impl Vec2 {
    fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }

    fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;

    fn add(self, other: Vec2) -> Vec2 {
        Vec2 { x: self.x + other.x, y: self.y + other.y }
    }
}

fn main() {
    let a = Vec2::new(3.0, 4.0);
    let b = Vec2::new(1.0, 2.0);
    let c = a + b;
    println!("length: {}", c.length());
}
```

**19 lines.** Requires `type Output`, `&self` vs `self`, `std::ops::Add`,
`println!` macro, semicolons, `::` path syntax.

### C++

```cpp
#include <cmath>
#include <iostream>

struct Vec2 {
    double x, y;

    Vec2(double x, double y) : x(x), y(y) {}

    double length() const {
        return std::sqrt(x * x + y * y);
    }

    Vec2 operator+(const Vec2& other) const {
        return Vec2(x + other.x, y + other.y);
    }
};

int main() {
    Vec2 a(3.0, 4.0);
    Vec2 b(1.0, 2.0);
    Vec2 c = a + b;
    std::cout << "length: " << c.length() << std::endl;
    return 0;
}
```

**18 lines.** `#include`, `const&`, constructor initializer lists, `std::cout`
operator chaining, `return 0`.

### Zig

```zig
const std = @import("std");
const math = std.math;

const Vec2 = struct {
    x: f64,
    y: f64,

    pub fn length(self: Vec2) f64 {
        return math.sqrt(self.x * self.x + self.y * self.y);
    }

    pub fn add(self: Vec2, other: Vec2) Vec2 {
        return .{ .x = self.x + other.x, .y = self.y + other.y };
    }
};

pub fn main() !void {
    const a = Vec2{ .x = 3.0, .y = 4.0 };
    const b = Vec2{ .x = 1.0, .y = 2.0 };
    const c = a.add(b);
    const stdout = std.io.getStdOut().writer();
    try stdout.print("length: {d}\n", .{c.length()});
}
```

**19 lines.** `.{ .field = val }` syntax, `!void` error union returns,
`@import`, no operator overloading.

### Go

```go
package main

import (
    "fmt"
    "math"
)

type Vec2 struct {
    X, Y float64
}

func NewVec2(x, y float64) Vec2 {
    return Vec2{x, y}
}

func (v Vec2) Length() float64 {
    return math.Sqrt(v.X*v.X + v.Y*v.Y)
}

func (v Vec2) Add(other Vec2) Vec2 {
    return Vec2{v.X + other.X, v.Y + other.Y}
}

func main() {
    a := NewVec2(3.0, 4.0)
    b := NewVec2(1.0, 2.0)
    c := a.Add(b) // No operator overloading
    fmt.Printf("length: %f\n", c.Length())
}
```

**23 lines.** Exported names must be capitalized, no operator overloading,
`func (v Vec2)` receiver syntax, `fmt.Printf` with format verbs.

## Feature Comparison

| Feature | Forge | Rust | C++ | Zig | Go |
|---------|-------|------|-----|-----|-----|
| Memory safety | ✅ Borrow checker | ✅ Borrow checker | ❌ Manual | ✅ Compile-time | ✅ GC |
| No GC | ✅ | ✅ | ✅ | ✅ | ❌ |
| Generics | ✅ | ✅ | ✅ Templates | ✅ comptime | ✅ |
| Operator overloading | ✅ | ✅ | ✅ | ❌ | ❌ |
| String interpolation | ✅ Built-in | ❌ Macro | ❌ | ❌ | ❌ |
| comptime | ✅ | ❌ | Partial (constexpr) | ✅ | ❌ |
| No semicolons | ✅ | ❌ | ❌ | ❌ | ✅ |
| Pattern matching | ✅ | ✅ | Partial (C++23) | ✅ | ❌ |
| Closures | ✅ | ✅ | ✅ Lambdas | ❌ | ✅ |
| LLVM backend | ✅ | ✅ | Via Clang | ✅ Own backend | Own backend |
| Package manager | ❌ Not yet | ✅ Cargo | ❌ (third party) | ✅ Built-in | ✅ go mod |
| Ecosystem maturity | 🆕 New | ✅ Mature | ✅ Decades | 🔄 Growing | ✅ Mature |

## What Forge Does Better

1. **Syntax clarity** — no semicolons, implicit returns, `Type.method()` instead
   of `Type::method()`, string interpolation without macros.
2. **comptime over macros** — same language for compile-time and runtime code.
   No `macro_rules!`, no preprocessor, no template metaprogramming.
3. **Lower barrier to entry** — Rust's borrow checker is notoriously hard to
   learn. Forge aims for the same guarantees with 95% fewer lifetime annotations.
4. **Tiny binaries** — 15KB vs Rust's 3.9MB for the same program.
5. **Competitive speed** — within 15% of Rust, 1.5x of C, with room to improve.

## What Forge Doesn't Do Yet

1. **Async/await** — no async runtime.
2. **Package manager** — no `cargo`-equivalent yet.
3. **Ecosystem** — no crates.io equivalent, no libraries.
4. **Production readiness** — the compiler is young and hasn't been battle-tested.
5. **IDE support** — no LSP, no syntax highlighting plugins yet.

Forge is an honest project: it's not trying to replace Rust or C++ today. It's
exploring whether you can have Rust's safety with less ceremony, and the early
results are promising.
