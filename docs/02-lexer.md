# The Lexer

The lexer (tokenizer) is the first phase of the compiler pipeline. It takes
raw source text and breaks it into a stream of tokens — the smallest meaningful
units of the language.

**Source files:** `src/lexer/mod.rs`, `src/lexer/token.rs`, `src/lexer/tests.rs`

## Token Types

Forge has the following token categories:

### Keywords (26)

```
fn let mut struct impl trait enum match if else while for in
return break continue use comptime spawn where pub mod self Self
true false
```

`true` and `false` are lexed as `BoolLiteral` tokens rather than identifiers.
Identifiers that happen to start with a keyword prefix (like `fns`, `letters`,
`selfish`) are correctly recognized as identifiers, not keywords.

### Literals

- **Integers:** decimal (`42`), hex (`0xFF`), binary (`0b1010`), octal (`0o77`),
  all with optional `_` separators (`1_000_000`)
- **Floats:** decimal point (`3.14`), exponents (`1e10`, `2.5e-3`), with `_`
  separators
- **Booleans:** `true`, `false`
- **Strings:** `"hello world"` with escape sequences (`\n`, `\t`, `\\`, `\"`,
  `\{`, `\}`, `\0`)

### Operators

Single-character: `+ - * / % & | ! . : ? @`

Multi-character: `== != <= >= && || += -= *= /= %= -> => .. ..= ::`

### Delimiters and Punctuation

`( ) { } [ ] , ;`

The semicolon exists for array type syntax (`[T; n]`), not as a statement
terminator.

### Newlines

Newlines are significant — they serve as statement terminators. The lexer:
- Collapses multiple consecutive newlines into one
- Strips leading and trailing newlines
- Preserves newlines between statements

Spaces, tabs, and carriage returns are insignificant whitespace.

### Comments

Line comments with `//`. Everything from `//` to the end of the line is
ignored. The newline after a comment is preserved (it may be a statement
terminator).

## String Interpolation

This is the most complex part of the lexer. Forge strings support inline
expressions:

```
"Hello, {name}!"
"sum = {a + b}"
```

The lexer handles this by switching modes. When it encounters `{` inside a
string, it:

1. Emits a `StringFragment` token for the text before the `{`
2. Pushes the current brace depth onto an interpolation stack
3. Returns to normal lexing mode

Normal tokens are emitted for the expression inside the braces. When a `}`
is encountered at the interpolation depth, the lexer switches back to string
mode and continues scanning.

The token sequence for `"Hello, {name}!"` is:

```
StringFragment("Hello, ")
Identifier("name")
StringFragment("!")
StringEnd
```

Plain strings with no interpolation are emitted as a single `StringLiteral`
token. Escaped braces (`{{` and `}}`) produce literal brace characters.

## Error Recovery

The lexer doesn't stop at the first error. When it encounters an unexpected
character, it:

1. Records a `LexError` with the position and message
2. Skips the character (handling multi-byte UTF-8 correctly)
3. Continues lexing

This means downstream phases still get a token stream to work with, and the
user sees all lexing errors at once.

## Testing

33 tests cover every token category, edge cases (underscores in numbers,
escaped braces, multi-byte characters), and integration tests that lex
complete Forge programs from the sample files.

```bash
cargo test lexer
```
