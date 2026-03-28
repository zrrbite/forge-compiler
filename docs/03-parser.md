# The Parser

The parser is the second phase of the compiler pipeline. It takes the token
stream from the lexer and builds an Abstract Syntax Tree (AST) — a structured
representation of the program.

**Source files:** `src/parser/mod.rs`, `src/parser/tests.rs`, `src/ast/mod.rs`

## Architecture

The parser uses two complementary techniques:

### Recursive Descent for Structure

Top-level items (functions, structs, enums, impl blocks, traits), statements,
types, and patterns are parsed by hand-written recursive descent functions.
Each grammar rule maps to a method:

```
parse_item()          → fn, struct, enum, impl, trait
parse_function()      → fn name(params) -> Type { body }
parse_struct_item()   → struct Name { fields }
parse_block()         → { stmts }
parse_stmt()          → let, return, break, continue, expr
parse_type()          → Named, Generic<T>, &T, [T], impl Trait, fn(T) -> U
parse_pattern()       → _, name, literal, Variant(fields)
```

### Pratt Parsing for Expressions

Expressions use a Pratt parser (precedence climbing), which handles operator
precedence elegantly without needing a grammar for each precedence level.

Each operator has a left and right "binding power." Higher binding power means
tighter binding:

| Precedence | Operators            | Binding Power |
| ---------- | -------------------- | ------------- |
| Lowest     | `\|\|`               | 3, 4          |
|            | `&&`                 | 5, 6          |
|            | `== !=`              | 7, 8          |
|            | `< > <= >=`          | 9, 10         |
|            | `+ -`                | 11, 12        |
| Highest    | `* / %`              | 13, 14        |
| Prefix     | `- ! & *`            | 17            |

Postfix operators (`.field`, `(args)`, `[index]`, `?`, `::< >`) are handled
in a loop before the infix precedence climbing begins.

## The AST

The AST is defined in `src/ast/mod.rs`. Key types:

### Items (top-level declarations)

- `Function` — name, params, return type, body
- `StructDef` — name, generic params, fields
- `EnumDef` — name, variants (each with optional fields)
- `ImplBlock` — optional trait name, target type, methods
- `TraitDef` — name, generic params, methods (with optional default bodies)

### Statements

- `Let` — mutable flag, name, optional type, optional initializer
- `Expr` — expression statement (last one in a block is the return value)
- `Return`, `Break`, `Continue`

### Expressions (30+ variants)

Literals, identifiers, binary/unary ops, calls, field access, indexing,
blocks, if/else, match (with patterns), closures, assignments (plain and
compound), ranges, references, dereferences, struct literals, try (`?`),
turbofish, arrays, for loops, while loops.

### Types

- `Named` — `i32`, `str`
- `Generic` — `Result<T>`, `Stack<T>`
- `Reference` — `&T`, `&mut T`
- `Array` — `[T]`, `[T; n]`
- `ImplTrait` — `impl Area`
- `Function` — `fn(T) -> U`

## Interesting Design Choices

### Struct Literals vs. Blocks

`Name { ... }` could be either a struct literal or a block preceded by an
identifier. The parser uses a heuristic: if the identifier starts with an
uppercase letter and is followed by `{`, it's treated as a struct literal.
This matches the convention that type names are capitalized.

### Newlines as Statement Terminators

The parser calls `skip_newlines()` at strategic points — between items,
after opening braces, before closing braces. Within an expression,
newlines are not consumed, so they naturally terminate the expression.

### Error Recovery

When the parser encounters an unexpected token, it records an error and
skips to the next "item boundary" (a keyword like `fn`, `struct`, etc.).
This lets it report multiple errors in a single pass.

## Testing

40 tests cover every AST node type, plus integration tests that parse
complete Forge programs (hello.fg, vec2.fg, error handling, traits).

```bash
cargo test parser
```
