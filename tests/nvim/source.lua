-- Headless end-to-end test for `:CodemmetSource` (SOURCE_COLLECTIONS_PLAN.md):
-- point a test buffer at a source file, then expand `tfn*@src.fns` in it.
-- Run from the repo root:
--   cargo build -p codemmet && nvim --headless --clean -l tests/nvim/source.lua

local repo = vim.uv.cwd()
vim.opt.runtimepath:prepend(repo)

local binary = repo .. "/target/debug/codemmet"

local function fail(msg, bufnr)
  io.write("SOURCE FAIL: " .. msg .. "\n")
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

require("codemmet").setup({
  cmd = { binary, "lsp" },
  init_options = { snippets_dir = "/tmp/codemmet-source-nonexistent" },
  preview = false,
})

-- A throwaway project: src/geometry.rs under test, tests/geometry.rs testing it.
local project = vim.fn.tempname() .. "-proj"
vim.fn.mkdir(project .. "/src", "p")
vim.fn.mkdir(project .. "/tests", "p")
local src = project .. "/src/geometry.rs"
vim.fn.writefile({
  "pub fn area(w: f64, h: f64) -> f64 { w * h }",
  "fn helper() {}",
  "pub fn perimeter(w: f64, h: f64) -> f64 { 2.0 * (w + h) }",
  "pub struct Point { x: f64 }",
  "impl Point {",
  "    pub fn new(x: f64) -> Self { Self { x } }",
  "    pub fn len(&self) -> f64 { self.x }",
  "}",
}, src)
local test_file = project .. "/tests/geometry.rs"
vim.fn.writefile({ "" }, test_file)

-- Scenario 1: the convention-based guess finds src/geometry.rs for tests/geometry.rs.
local guess = require("codemmet.source").guess(test_file)
if guess ~= src then
  fail("guess: expected " .. src .. ", got " .. tostring(guess))
end
io.write("source: guess scenario passed\n")

-- Open the test file and wait for the server.
vim.cmd("edit " .. vim.fn.fnameescape(test_file))
local bufnr = vim.api.nvim_get_current_buf()
vim.bo[bufnr].filetype = "rust"
local attached = vim.wait(5000, function()
  local client = vim.lsp.get_clients({ name = "codemmet", bufnr = bufnr })[1]
  return client ~= nil and client.initialized == true
end, 50)
if not attached then
  fail("codemmet LSP client did not attach", bufnr)
end
local client = vim.lsp.get_clients({ name = "codemmet", bufnr = bufnr })[1]
local lsp = require("codemmet.lsp")

---The server's `codemmet.context` result for this buffer (synchronous).
local function context()
  local result
  lsp.exec(client, bufnr, "codemmet.context", {
    { uri = vim.uri_from_bufnr(bufnr), position = { line = 0, character = 0 } },
  }, function(err, res)
    result = { err = err, res = res }
  end)
  vim.wait(5000, function()
    return result ~= nil
  end, 20)
  if not result or result.err then
    fail("codemmet.context failed: " .. vim.inspect(result and result.err))
  end
  return result.res
end

-- Scenario 2: :CodemmetSource {path} is remembered by the server and reported
-- by codemmet.context, with the src.* collections and ${src.*} vars.
require("codemmet").source({ path = src })
local set = vim.wait(5000, function()
  return context().source == src
end, 50)
if not set then
  fail("setSource: context never reported the source " .. src)
end
if vim.b[bufnr].codemmet_source ~= src then
  fail("setSource: vim.b.codemmet_source was not stored")
end
local ctx = context()
if ctx.vars.module ~= "crate::geometry" then
  fail("vars: expected module crate::geometry, got " .. tostring(ctx.vars.module))
end
local fns
for _, c in ipairs(ctx.collections) do
  if c.name == "src.fns" then
    fns = c
  end
end
if not fns or #fns.items ~= 3 then
  fail("collections: expected src.fns with 3 items, got " .. vim.inspect(fns))
end
io.write("source: setSource + context scenario passed\n")

-- Scenario 3: expanding tfn*@src.fns in the test buffer writes one test per
-- public function of the source (the default filter drops `helper`).
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "tfn*@src.fns" })
vim.api.nvim_win_set_cursor(0, { 1, 11 })
require("codemmet").expand()
local expanded = vim.wait(5000, function()
  local text = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
  return text:find("fn area_works()", 1, true) ~= nil
    and text:find("fn perimeter_works()", 1, true) ~= nil
    and text:find("let result = area(w, h);", 1, true) ~= nil
end, 50)
if not expanded then
  fail("expand: buffer never contained the generated tests", bufnr)
end
local text = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
if text:find("helper", 1, true) then
  fail("expand: private helper should be filtered out by tfn's default", bufnr)
end
io.write("source: expand tfn*@src.fns scenario passed\n")
if vim.snippet.active() then
  vim.snippet.stop()
end
vim.cmd("stopinsert")

-- Scenario 4: a per-type method repeat with an explicit filter chain.
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "tmeth*@src.Point.methods?all" })
vim.api.nvim_win_set_cursor(0, { 1, 27 })
require("codemmet").expand()
local methods = vim.wait(5000, function()
  local t = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
  return t:find("fn point_new_works()", 1, true) ~= nil
    and t:find("fn point_len_works()", 1, true) ~= nil
end, 50)
if not methods then
  fail("expand: tmeth over Point's methods never rendered", bufnr)
end
io.write("source: expand tmeth with ?all scenario passed\n")
if vim.snippet.active() then
  vim.snippet.stop()
end
vim.cmd("stopinsert")

-- Scenario 5: clearing forgets the source; the buffer's own (empty) definitions apply.
require("codemmet").source({ clear = true })
local cleared = vim.wait(5000, function()
  return context().source == vim.NIL or context().source == nil
end, 50)
if not cleared then
  fail("clear: context still reports a source")
end
if vim.b[bufnr].codemmet_source ~= nil then
  fail("clear: vim.b.codemmet_source still set")
end
io.write("source: clear scenario passed\n")

-- Scenario 6: a Rust file is its own source — no :CodemmetSource needed.
-- `:CodemmetScaffold tfile` appends the test module to the open buffer (after
-- the final brace, not inside it), and the code-action menu offers the
-- missing tests.
vim.cmd("edit! " .. vim.fn.fnameescape(src))
local src_buf = vim.api.nvim_get_current_buf()
vim.bo[src_buf].filetype = "rust"
local src_attached = vim.wait(5000, function()
  local c = vim.lsp.get_clients({ name = "codemmet", bufnr = src_buf })[1]
  return c ~= nil and c.initialized == true
end, 50)
if not src_attached then
  fail("codemmet LSP client did not attach to the source buffer", src_buf)
end
require("codemmet").scaffold({ abbr = "tfile" })
local scaffolded = vim.wait(5000, function()
  return table
    .concat(vim.api.nvim_buf_get_lines(src_buf, 0, -1, false), "\n")
    :find("mod tests", 1, true) ~= nil
end, 50)
if not scaffolded then
  fail("scaffold: tfile never appended a test module to the source buffer", src_buf)
end
local lines = vim.api.nvim_buf_get_lines(src_buf, 0, -1, false)
local cfg
for i, line in ipairs(lines) do
  if line == "#[cfg(test)]" then
    cfg = i
  end
end
if not cfg or lines[cfg - 1] ~= "" or lines[cfg - 2] ~= "}" or lines[#lines] ~= "}" then
  fail("scaffold: the module must follow the impl's closing brace after one blank line", src_buf)
end
if vim.snippet.active() then
  vim.snippet.stop()
end
vim.cmd("stopinsert")
io.write("source: same-file tfile scaffold scenario passed\n")

vim.api.nvim_buf_set_lines(src_buf, 0, -1, false, vim.fn.readfile(src))
local src_client = vim.lsp.get_clients({ name = "codemmet", bufnr = src_buf })[1]
local actions
src_client:request("textDocument/codeAction", {
  textDocument = { uri = vim.uri_from_bufnr(src_buf) },
  range = { start = { line = 0, character = 0 }, ["end"] = { line = 0, character = 0 } },
  context = { diagnostics = {} },
}, function(_, res)
  actions = res or {}
end, src_buf)
vim.wait(5000, function()
  return actions ~= nil
end, 20)
if not actions or #actions ~= 1 or actions[1].title ~= "Add 4 missing tests to geometry.rs" then
  fail(
    "code action: expected 'Add 4 missing tests to geometry.rs', got " .. vim.inspect(actions),
    src_buf
  )
end
io.write("source: same-file code action scenario passed\n")

io.write("source: all scenarios passed\n")
os.exit(0)
