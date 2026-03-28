# The Borrow Checker

The borrow checker enforces Forge's ownership rules at compile time. This is
the feature that distinguishes Forge from languages like C++ — memory safety
without a garbage collector.

**Source files:** `src/borrowck/mod.rs`, `src/borrowck/tests.rs`

## Ownership Rules

Forge follows Rust's ownership model:

1. **Each value has exactly one owner** — the variable that holds it.
2. **When the owner goes out of scope, the value is dropped.**
3. **Values are moved on assignment** — the old variable becomes invalid.
4. **Primitive types are Copy** — integers, floats, bools, and strings are
   implicitly copied instead of moved.

```
let x = Buffer { data: 42 }
let y = x           // x is moved to y
print(x)            // ERROR: use of moved value 'x'
```

But primitives work differently:

```
let x = 42
let y = x           // x is copied (i64 is Copy)
print(x)            // OK — x is still valid
```

## Borrowing Rules

References allow temporary access to a value without taking ownership:

1. **Multiple immutable borrows (`&T`) are allowed** — many readers, no writers.
2. **A mutable borrow (`&mut T`) is exclusive** — one writer, no readers.
3. **Cannot move a value while it's borrowed.**
4. **Cannot mutably borrow an immutable variable.**

```
let x = Buffer { data: 42 }
let a = &x          // OK: immutable borrow
let b = &x          // OK: multiple immutable borrows
let c = &mut x      // ERROR: cannot mutably borrow while immutably borrowed
```

```
let mut x = Buffer { data: 42 }
let a = &mut x       // OK: mutable borrow
let b = &mut x       // ERROR: cannot have two mutable borrows
```

## How It Works

The borrow checker walks the HIR (High-level IR) and tracks the state of
each variable:

| State | Meaning |
|-------|---------|
| **Owned** | Variable holds its value, ready to use |
| **Moved** | Value was moved elsewhere — using it is an error |
| **Borrowed** | An immutable reference exists (count tracked) |
| **MutBorrowed** | A mutable reference exists (exclusive) |

### Copy vs Move

The checker determines if a type is Copy (primitives) or Move (structs, enums)
based on the type annotation or initializer expression:

- `let x = 42` → Copy (integer literal)
- `let x = Buffer { data: 1 }` → Move (struct literal)
- `let x = make_buffer()` → Move (function call, might return struct)

### What It Catches

- Use after move
- Double move
- Move of borrowed value
- Mutable borrow while immutably borrowed
- Multiple mutable borrows
- Mutable borrow of immutable variable
- Assignment to immutable variable

### What It Doesn't Do (Yet)

- **Lifetime analysis** — doesn't track how long references live. A full
  implementation would use Non-Lexical Lifetimes (NLL) like Rust's Polonius.
- **Control flow sensitivity** — doesn't understand that a value might be
  moved in one branch but not another.
- **Partial moves** — can't move a single field out of a struct.

These would require building a control flow graph (CFG) and doing dataflow
analysis. That's future work.

## Testing

15 tests covering move semantics, Copy types, borrow conflicts,
mutability, and integration with real Forge programs.

```bash
cargo test borrowck
```
