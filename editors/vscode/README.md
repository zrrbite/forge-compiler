# Forge Language Support for VS Code

Syntax highlighting for the [Forge](https://github.com/zrrbite/forge-compiler) programming language.

## Features

- Syntax highlighting for all Forge keywords, types, and operators
- String interpolation highlighting (`"Hello, {name}!"`)
- Comment highlighting
- Auto-closing brackets, quotes, and closure pipes
- Indentation support

## Install

### VS Code (standard)

Symlink the extension into your extensions folder:

```bash
ln -s /path/to/forge-compiler/editors/vscode ~/.vscode/extensions/forge-lang
```

Reload VS Code (Ctrl+Shift+P > "Reload Window").

### VS Code OSS / Code - OSS

The OSS build doesn't auto-discover symlinked extensions. Package and install
as a `.vsix` instead:

```bash
cd editors/vscode
npx @vscode/vsce package --allow-missing-repository
code --install-extension forge-lang-0.1.0.vsix
```

Reload VS Code (Ctrl+Shift+P > "Reload Window").

## Supported syntax

- Keywords: `fn`, `let`, `mut`, `struct`, `impl`, `trait`, `enum`, `if`, `else`, `while`, `for`, `match`, `return`, `use`, `comptime`, `spawn`
- Types: `i32`, `i64`, `f64`, `bool`, `str`, `usize`, and user-defined types (PascalCase)
- Operators: `->`, `=>`, `..`, `?`, `@`, comparison, logical, assignment
- String interpolation: `{expr}` inside double-quoted strings
- Comments: `// line comments`
- Constants: `true`, `false`, `None`, `PI`, `E`
