# Forge Language Support for VS Code

Syntax highlighting for the [Forge](https://github.com/zrrbite/forge-compiler) programming language.

## Features

- Syntax highlighting for all Forge keywords, types, and operators
- String interpolation highlighting (`"Hello, {name}!"`)
- Comment highlighting
- Auto-closing brackets, quotes, and closure pipes
- Indentation support

## Install from source

1. Copy or symlink this directory into your VS Code extensions folder:

```bash
# Linux
ln -s /path/to/forge-compiler/editors/vscode ~/.vscode/extensions/forge-lang

# macOS
ln -s /path/to/forge-compiler/editors/vscode ~/.vscode/extensions/forge-lang
```

2. Reload VS Code (Ctrl+Shift+P > "Reload Window")

3. Open any `.fg` file — it should have syntax highlighting.

## Supported syntax

- Keywords: `fn`, `let`, `mut`, `struct`, `impl`, `trait`, `enum`, `if`, `else`, `while`, `for`, `match`, `return`, `use`, `comptime`, `spawn`
- Types: `i32`, `i64`, `f64`, `bool`, `str`, `usize`, and user-defined types (PascalCase)
- Operators: `->`, `=>`, `..`, `?`, `@`, comparison, logical, assignment
- String interpolation: `{expr}` inside double-quoted strings
- Comments: `// line comments`
- Constants: `true`, `false`, `None`, `PI`, `E`
