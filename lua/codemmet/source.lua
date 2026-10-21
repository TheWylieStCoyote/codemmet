---`:CodemmetSource` — the explicit source file for a buffer.
---
---Tests usually live apart from the code under test. The server reads the
---buffer's own definitions by default; this points it at *another* file
---(`codemmet.setSource`), whose functions, types, and methods then back the
---`src.*` collections (`tfn*@src.fns`) and whose path sets the `${src.*}`
---variables for every expansion, preview, and completion in the buffer.
---The choice is kept in `vim.b[bufnr].codemmet_source` so it survives a
---server restart (`resend` on attach).
local lsp = require("codemmet.lsp")

local M = {}

---@class codemmet.SourceOpts
---@field path? string the source file; prompts when absent
---@field clear? boolean forget the source instead

---Convention-based guess of the source for a test file, or nil when no
---candidate exists on disk. Checked candidates, by test-file shape:
---`test_X.py` / `X_test.py` → `X.py`; `X_test.go` → `X.go`;
---`X.test.ts` / `X.spec.ts` (and `.js`, `.tsx`, `.jsx`) → `X.ts` (…);
---`X_test.cpp` / `test_X.c` (…) → `X.cpp` (…); `tests/X.rs` → `src/X.rs`,
---`src/X_test.rs` → `src/X.rs`.
---@param bufname string
---@return string|nil
function M.guess(bufname)
  if bufname == "" then
    return nil
  end
  local dir = vim.fs.dirname(bufname)
  local file = vim.fs.basename(bufname)
  local stem, ext = file:match("^(.*)%.([%w]+)$")
  if not stem then
    return nil
  end
  local candidates = {}
  local function add(d, name)
    candidates[#candidates + 1] = vim.fs.joinpath(d, name)
  end
  local base = stem:match("^test_(.+)$")
    or stem:match("^(.+)_test$")
    or stem:match("^(.+)%.test$")
    or stem:match("^(.+)%.spec$")
  if base then
    add(dir, base .. "." .. ext)
    -- `foo_test.cpp` next to `foo.hpp` / `foo.h`
    if ext == "cpp" or ext == "cc" or ext == "cxx" then
      add(dir, base .. ".hpp")
      add(dir, base .. ".h")
    end
  end
  -- Rust integration tests: tests/foo.rs → src/foo.rs (or src/lib.rs).
  if ext == "rs" and vim.fs.basename(dir) == "tests" then
    local root = vim.fs.dirname(dir)
    add(vim.fs.joinpath(root, "src"), stem .. ".rs")
    add(vim.fs.joinpath(root, "src"), "lib.rs")
  end
  for _, candidate in ipairs(candidates) do
    if vim.fn.filereadable(candidate) == 1 then
      return candidate
    end
  end
  return nil
end

---Send the buffer's source (or `nil` to clear) to the server.
---@param bufnr integer
---@param client vim.lsp.Client
---@param path string|nil
---@param notify boolean report the outcome
local function send(bufnr, client, path, notify)
  lsp.exec(client, bufnr, "codemmet.setSource", {
    { uri = vim.uri_from_bufnr(bufnr), source = path },
  }, function(err, result)
    if err then
      vim.notify("codemmet: " .. (err.message or vim.inspect(err)), vim.log.levels.WARN)
      return
    end
    if not notify then
      return
    end
    if result and result.source then
      local n = result.collections and #result.collections or 0
      vim.notify(
        string.format(
          "codemmet: source %s (%d collection%s)",
          result.source,
          n,
          n == 1 and "" or "s"
        ),
        vim.log.levels.INFO
      )
    else
      vim.notify("codemmet: source cleared", vim.log.levels.INFO)
    end
  end)
end

---Set (or clear) the source for the current buffer.
---@param opts? codemmet.SourceOpts
function M.set(opts)
  opts = opts or {}
  local bufnr = vim.api.nvim_get_current_buf()
  local client = lsp.get_client(bufnr)
  if not client then
    vim.notify("codemmet: no codemmet client attached to this buffer", vim.log.levels.WARN)
    return
  end
  if opts.clear then
    vim.b[bufnr].codemmet_source = nil
    send(bufnr, client, nil, true)
    return
  end
  local function apply(path)
    if not path or path == "" then
      return
    end
    path = vim.fn.fnamemodify(path, ":p")
    vim.b[bufnr].codemmet_source = path
    send(bufnr, client, path, true)
  end
  if opts.path then
    apply(opts.path)
    return
  end
  vim.ui.input({
    prompt = "codemmet source: ",
    default = M.guess(vim.api.nvim_buf_get_name(bufnr)),
    completion = "file",
  }, apply)
end

---Re-send a previously chosen source after the server (re)attaches.
---@param bufnr integer
---@param client vim.lsp.Client
function M.resend(bufnr, client)
  local path = vim.b[bufnr].codemmet_source
  if type(path) == "string" and path ~= "" then
    send(bufnr, client, path, false)
  end
end

return M
