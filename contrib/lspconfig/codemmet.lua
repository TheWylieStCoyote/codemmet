---@brief
---
--- https://github.com/TheWylieStCoyote/codemmet
---
--- Emmet-style abbreviation expansion for code: abbreviations like
--- `fn:read>for@lines` expand into idiomatic Rust, Python, C, C++,
--- JavaScript, TypeScript, or Go, offered as snippet-kind completion items
--- as you type. Explicit expansion is available through the
--- `workspace/executeCommand` commands `codemmet.expand` and `codemmet.wrap`.
---
--- Install the `codemmet` binary with `cargo install codemmet`, via
--- `:MasonInstall codemmet`, or from the GitHub releases; the server is the
--- same binary run as `codemmet lsp`.
---
--- The codemmet repository also ships a full Neovim plugin (keymaps, live
--- preview, `.codemmet` file mode) on top of this server; see
--- https://github.com/TheWylieStCoyote/codemmet/blob/master/docs/editors.md
---@type vim.lsp.Config
return {
  cmd = { "codemmet", "lsp" },
  filetypes = {
    "rust",
    "python",
    "c",
    "cpp",
    "javascript",
    "javascriptreact",
    "typescript",
    "typescriptreact",
    "go",
  },
  root_markers = { ".codemmet", ".git" },
}
