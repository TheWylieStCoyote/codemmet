---Native `vim.lsp.config` definition for the codemmet language server
---(Neovim 0.11+ loads `lsp/*.lua` from the runtimepath). Enable it with:
---
---  vim.lsp.enable("codemmet")
---
---This is the minimal-LSP alternative: the server attaches and completion
---works through your LSP completion UI, nothing more. The full-featured
---path remains `require("codemmet").setup()`, which also installs the
---expand/wrap/render keymaps, wires up built-in completion with
---autotrigger, and powers the live preview, `.codemmet` file mode, and the
---`:Codemmet*` commands — see `:help codemmet-setup`.
---
---Double-attach guard: `setup()` starts its own client (via `vim.lsp.start`,
---also named "codemmet"). If you run both mechanisms, the `root_dir`
---callback below detects a codemmet client already attached to the buffer
---and skips starting a second one. Prefer picking one mechanism anyway;
---see `:help codemmet-vim.lsp.enable`.
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
  -- Equivalent to `root_markers = { ".codemmet", ".git" }`, plus the guard
  -- described above. Never calling `on_dir` aborts this (re)attach only.
  root_dir = function(bufnr, on_dir)
    if #vim.lsp.get_clients({ name = "codemmet", bufnr = bufnr }) > 0 then
      return
    end
    on_dir(vim.fs.root(bufnr, { ".codemmet", ".git" }) or vim.uv.cwd())
  end,
}
