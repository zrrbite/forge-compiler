# Forge Syntax Highlighting for Neovim / Vim

Syntax highlighting for the [Forge](https://github.com/zrrbite/forge-compiler) programming language.

## Install

### Manual

Copy or symlink into your Vim/Neovim runtime path:

```bash
# Neovim
mkdir -p ~/.config/nvim
ln -s /path/to/forge-compiler/editors/nvim/ftdetect ~/.config/nvim/ftdetect
ln -s /path/to/forge-compiler/editors/nvim/syntax ~/.config/nvim/syntax

# Vim
ln -s /path/to/forge-compiler/editors/nvim/ftdetect ~/.vim/ftdetect
ln -s /path/to/forge-compiler/editors/nvim/syntax ~/.vim/syntax
```

### lazy.nvim

```lua
{
  dir = "/path/to/forge-compiler/editors/nvim",
  ft = "forge",
}
```

### vim-plug

```vim
Plug '/path/to/forge-compiler/editors/nvim'
```

## Features

- Keywords, control flow, declarations
- Primitive and user-defined types (PascalCase)
- Function definitions and calls
- String interpolation (`{expr}` inside `"..."`)
- Escape sequences
- Comments
- Numbers (integer and float)
- Operators (`->`, `=>`, `..`, `?`, `@`, `::`)
- Boolean and constant highlighting
