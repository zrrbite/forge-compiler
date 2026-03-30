# Forge Language Server

LSP server for the Forge programming language. Provides IDE features in Neovim.

## Features

- **Diagnostics** — lex and parse errors shown as you type
- **Go to definition** (`gd`) — jump to function, struct, method, variable definitions
- **Hover** (`K`) — show function signatures, struct fields, builtin docs

## Build

```bash
cargo build --release --bin forge-lsp
```

## Neovim Setup

Add to `~/.config/nvim/init.lua`:

```lua
vim.api.nvim_create_autocmd("FileType", {
  pattern = "forge",
  callback = function()
    vim.lsp.start({
      name = "forge-lsp",
      cmd = { "/path/to/forge-compiler/target/release/forge-lsp" },
      root_dir = vim.fn.getcwd(),
    })
  end,
})
```

## Usage

| Keybinding | Action |
|------------|--------|
| `gd` | Go to definition |
| `K` | Hover (show type info) |
| (automatic) | Error diagnostics as you type |
| `<space>e` | Show diagnostic at cursor (add keymap: `vim.keymap.set('n', '<leader>e', vim.diagnostic.open_float)`) |

## What Hover Shows

- **Functions**: `fn greet(name: str) -> str`
- **Structs**: field listing with types
- **Methods**: `fn Vec2.length(self) -> f64`
- **Variables**: `let mut count: i64`
- **Builtins**: `fn print(value) — print to stdout`

## Planned

- Autocomplete
- Rename symbol
- Find references
