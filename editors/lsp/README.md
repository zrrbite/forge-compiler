# Forge Language Server

The Forge LSP provides real-time diagnostics (error highlighting) in your editor.

## Build

```bash
cargo build --release --bin forge-lsp
```

The binary is at `target/release/forge-lsp`.

## Neovim Setup

Add to your Neovim config (e.g., `~/.config/nvim/init.lua`):

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

Replace `/path/to/forge-compiler` with the actual path.

## VS Code Setup

The VS Code extension currently provides syntax highlighting only.
LSP integration requires a JS activation script — coming in a future update.

For now, you can use the generic LSP client extension and point it at `forge-lsp`.

## Features

### Currently supported
- **Diagnostics**: lex and parse errors shown as you type

### Planned
- Go to definition
- Hover for type info
- Autocomplete
- Rename symbol
