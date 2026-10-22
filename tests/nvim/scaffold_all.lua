-- Headless end-to-end test for `scaffold = { interactive = "all" }`: every
-- generated file gets a live tabstop session, one at a time. Run from the
-- repo root:
--   cargo build -p codemmet && nvim --headless --clean -l tests/nvim/scaffold_all.lua

local repo = vim.uv.cwd()
vim.opt.runtimepath:prepend(repo)
vim.cmd("runtime! plugin/codemmet.lua")

local binary = repo .. "/target/debug/codemmet"

local function fail(msg, bufnr)
  io.write("SCAFFOLD-ALL FAIL: " .. msg .. "\n")
  if bufnr and vim.api.nvim_buf_is_valid(bufnr) then
    io.write("--- buffer contents ---\n")
    io.write(table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n") .. "\n")
    io.write("-----------------------\n")
  end
  os.exit(1)
end

if vim.fn.executable(binary) ~= 1 then
  fail(binary .. " is not built; run `cargo build -p codemmet` first")
end

-- A project with a two-file snippet whose supporting file has a tabstop too.
local project = vim.fn.tempname() .. "-proj"
vim.fn.mkdir(project .. "/.codemmet/snippets", "p")
vim.fn.writefile({
  "[meta]",
  'language = "rust"',
  "",
  "[snippets.mpair]",
  'description = "module plus declaration, both with tabstops (multi-file)"',
  'params = [{ name = "name", kind = "ident", default = "thing" }]',
  "",
  "[snippets.mpair.files.decl]",
  'target = "self"',
  'on_exists = "append"',
  "body = '''",
  "mod ${name};",
  "pub use ${name}::${1:Item};",
  "'''",
  "",
  "[snippets.mpair.files.module]",
  'path = "${name}.rs"',
  'on_exists = "create"',
  "primary = true",
  "body = '''",
  "pub struct ${1:Item};",
  "$0",
  "'''",
}, project .. "/.codemmet/snippets/rust.toml")
local lib = project .. "/lib.rs"
vim.fn.writefile({ "fn main() {}" }, lib)

require("codemmet").setup({
  cmd = { binary, "lsp" },
  init_options = { snippets_dir = "/tmp/codemmet-scaffold-all-nonexistent" },
  preview = false,
  scaffold = { interactive = "all" },
})

vim.cmd("edit " .. vim.fn.fnameescape(lib))
local lib_buf = vim.api.nvim_get_current_buf()
vim.bo[lib_buf].filetype = "rust"
local attached = vim.wait(5000, function()
  local client = vim.lsp.get_clients({ name = "codemmet", bufnr = lib_buf })[1]
  return client ~= nil and client.initialized == true
end, 50)
if not attached then
  fail("codemmet LSP client did not attach", lib_buf)
end

require("codemmet").scaffold({ abbr = "mpair:widget" })

-- The primary (widget.rs) opens with its live snippet.
local primary_open = vim.wait(5000, function()
  return vim.api.nvim_buf_get_name(0):match("widget%.rs$") ~= nil
    and vim.api.nvim_buf_get_lines(0, 0, 1, false)[1] == "pub struct Item;"
end, 50)
if not primary_open then
  fail("primary widget.rs did not open with its snippet", vim.api.nvim_get_current_buf())
end
if not vim.snippet.active() then
  fail("primary: no active snippet session")
end
-- The supporting file is on disk, plain, and queued.
local on_disk = table.concat(vim.fn.readfile(lib), "\n")
if on_disk ~= "fn main() {}\n\nmod widget;\npub use widget::Item;" then
  fail("lib.rs on disk: " .. vim.inspect(on_disk))
end
if require("codemmet.scaffold").pending_count() ~= 1 then
  fail("expected one pending file, got " .. require("codemmet.scaffold").pending_count())
end
io.write("scaffold-all: primary + plain supporting file scenario passed\n")
vim.snippet.stop()
vim.cmd("stopinsert")

-- :CodemmetScaffoldNext opens lib.rs and replaces the written block with the
-- live snippet: same text, tabstop active on `Item`.
vim.cmd("CodemmetScaffoldNext")
local live = vim.wait(5000, function()
  return vim.api.nvim_buf_get_name(0) == lib and vim.snippet.active()
end, 50)
if not live then
  fail("CodemmetScaffoldNext did not start a session in lib.rs", vim.api.nvim_get_current_buf())
end
local lines = vim.api.nvim_buf_get_lines(0, 0, -1, false)
if not vim.deep_equal(lines, { "fn main() {}", "", "mod widget;", "pub use widget::Item;" }) then
  fail("lib.rs live buffer differs from the written text", vim.api.nvim_get_current_buf())
end
if require("codemmet.scaffold").pending_count() ~= 0 then
  fail("queue should be empty")
end
io.write("scaffold-all: CodemmetScaffoldNext scenario passed\n")
vim.snippet.stop()
vim.cmd("stopinsert")

-- Entering a pending file starts its session automatically — and a file
-- edited meanwhile is left alone.
vim.fn.writefile({ "fn main() {}" }, lib)
vim.cmd("edit! " .. vim.fn.fnameescape(lib))
os.remove(project .. "/widget.rs")
require("codemmet").scaffold({ abbr = "mpair:gadget" })
vim.wait(5000, function()
  return vim.api.nvim_buf_get_name(0):match("gadget%.rs$") ~= nil and vim.snippet.active()
end, 50)
vim.snippet.stop()
vim.cmd("stopinsert")
vim.cmd("edit " .. vim.fn.fnameescape(lib))
local auto = vim.wait(5000, function()
  return vim.api.nvim_buf_get_name(0) == lib and vim.snippet.active()
end, 50)
if not auto then
  fail("entering the pending file did not start its session", vim.api.nvim_get_current_buf())
end
io.write("scaffold-all: BufEnter auto-start scenario passed\n")

io.write("scaffold-all: all scenarios passed\n")
os.exit(0)
