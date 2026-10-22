---Expand a multi-file snippet across files.
---
---Like capture, this touches the filesystem, so it runs the `codemmet scaffold`
---CLI (the single source of truth for path resolution, rendering, and the
---create/append/inject actions) rather than the read-only LSP server. We ask
---for a JSON manifest with tabstops kept, write every non-primary file to disk,
---and drop the primary file's content in as a live `vim.snippet` session so its
---tabstops are interactive.
local lsp = require("codemmet.lsp")

local M = {}

---@class codemmet.ScaffoldOpts
---@field abbr? string the invocation (trigger + args), e.g. "djfeature:blog";
---  when omitted a `vim.ui.input` prompt is shown

---@class codemmet.ScaffoldConfig
---@field interactive? "primary"|"all" which generated files get a live tabstop
---  session: only the primary (default), or every file in turn — the others
---  are written plain and upgraded to a live session when you open them or
---  run `:CodemmetScaffoldNext`

---@type codemmet.ScaffoldConfig
local config = { interactive = "primary" }

---Configure the scaffold behavior (called from `setup`).
---@param opts? codemmet.ScaffoldConfig
function M.configure(opts)
  config = vim.tbl_extend("force", { interactive = "primary" }, opts or {})
end

---Files written plain whose live tabstop session is still pending
---(`interactive = "all"`): what was written, where, and the tabstop twin.
---@type { path: string, action: string, plain: string, snippet: string, offset?: integer }[]
local pending = {}

---The project root for `path` targets: the nearest ancestor with `.codemmet`
---or `.git`, else the file's directory, else the editor cwd.
---@param bufnr integer
---@return string
local function project_root(bufnr)
  local name = vim.api.nvim_buf_get_name(bufnr)
  if name == "" then
    return vim.uv.cwd()
  end
  return vim.fs.root(bufnr, { ".codemmet", ".git" }) or vim.fs.dirname(name)
end

---@param path string
---@return string?
local function read_file(path)
  local fd = io.open(path, "r")
  if not fd then
    return nil
  end
  local content = fd:read("*a")
  fd:close()
  return content
end

---@param path string
---@param content string
local function write_file(path, content)
  vim.fn.mkdir(vim.fs.dirname(path), "p")
  local fd = assert(io.open(path, "w"))
  fd:write(content)
  fd:close()
end

---A Rust/tree-sitter byte `offset` into `text` as a 1-based row and 0-based
---byte column, for `nvim_win_set_cursor`.
---@param text string
---@param offset integer
---@return integer row, integer col
local function offset_to_pos(text, offset)
  offset = math.min(offset, #text)
  local prefix = text:sub(1, offset)
  local row, nl = 1, 0
  local i = prefix:find("\n", 1, true)
  while i do
    row, nl = row + 1, i
    i = prefix:find("\n", i + 1, true)
  end
  return row, offset - nl
end

---Write a non-primary manifest entry to disk (plain text — non-primary content
---is rendered without tabstops).
---@param entry table
local function apply_to_disk(entry)
  if entry.action == "append" then
    local existing = read_file(entry.path) or ""
    local block = "\n" .. entry.content
    if not block:match("\n$") then
      block = block .. "\n"
    end
    write_file(entry.path, existing .. block)
  elseif entry.action == "inject" then
    local existing = read_file(entry.path) or ""
    local off = math.min(entry.offset or #existing, #existing)
    write_file(entry.path, existing:sub(1, off) .. entry.content .. existing:sub(off + 1))
  else -- create
    local content = entry.content
    if not content:match("\n$") then
      content = content .. "\n"
    end
    write_file(entry.path, content)
  end
end

---Open the primary file and insert its content as a live snippet, so the
---tabstops (`$1`, `$0`) are interactive. Left unsaved for the user to fill.
---@param entry table
local function open_primary(entry)
  vim.fn.mkdir(vim.fs.dirname(entry.path), "p")
  vim.cmd.edit(vim.fn.fnameescape(entry.path))
  local bufnr = vim.api.nvim_get_current_buf()
  if entry.action == "append" then
    -- Normal mode clamps the cursor onto the last character, so expanding at
    -- "end of the last line" would land *before* its final brace. Add the
    -- separating blank line and an empty line to expand on instead (the same
    -- blank-line separator the CLI writes).
    local last = vim.api.nvim_buf_line_count(bufnr)
    local tail = vim.api.nvim_buf_get_lines(bufnr, last - 1, last, false)[1] or ""
    local fresh = tail == "" and { "" } or { "", "" }
    vim.api.nvim_buf_set_lines(bufnr, last, last, false, fresh)
    vim.api.nvim_win_set_cursor(0, { last + #fresh, 0 })
    vim.snippet.expand(entry.content)
  elseif entry.action == "inject" then
    local existing = read_file(entry.path) or ""
    local row, col = offset_to_pos(existing, entry.offset or #existing)
    vim.api.nvim_win_set_cursor(0, { row, col })
    vim.snippet.expand(entry.content)
  else -- create: fresh buffer, expand at the top
    vim.api.nvim_win_set_cursor(0, { 1, 0 })
    vim.snippet.expand(entry.content)
  end
end

---The lines of `text` as a buffer holds them (no trailing empty element for a
---final newline).
---@param text string
---@return string[]
local function lines_of(text)
  local lines = vim.split(text, "\n", { plain = true })
  if lines[#lines] == "" then
    table.remove(lines)
  end
  return lines
end

---Upgrade a file written plain to a live tabstop session: open it, remove the
---block that was written (only if it is still exactly there), and expand the
---tabstop twin in its place. Returns false when the file changed meanwhile.
---@param entry { path: string, action: string, plain: string, snippet: string, offset?: integer }
---@return boolean
local function go_live(entry)
  vim.cmd.edit(vim.fn.fnameescape(entry.path))
  local bufnr = vim.api.nvim_get_current_buf()
  local plain = lines_of(entry.plain)
  if entry.action == "create" then
    local have = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
    if not vim.deep_equal(have, plain) then
      return false
    end
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "" })
    vim.api.nvim_win_set_cursor(0, { 1, 0 })
    vim.snippet.expand(entry.snippet)
  elseif entry.action == "append" then
    -- Written as: blank separator line, then the block.
    local last = vim.api.nvim_buf_line_count(bufnr)
    local from = last - #plain
    local have = vim.api.nvim_buf_get_lines(bufnr, from, last, false)
    if from < 1 or not vim.deep_equal(have, plain) then
      return false
    end
    vim.api.nvim_buf_set_lines(bufnr, from, last, false, { "" })
    vim.api.nvim_win_set_cursor(0, { from + 1, 0 })
    vim.snippet.expand(entry.snippet)
  else -- inject: the block sits at a byte offset
    local text = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n") .. "\n"
    local off = entry.offset or 0
    if text:sub(off + 1, off + #entry.plain) ~= entry.plain then
      return false
    end
    local srow, scol = offset_to_pos(text, off)
    local erow, ecol = offset_to_pos(text, off + #entry.plain)
    vim.api.nvim_buf_set_text(bufnr, srow - 1, scol, erow - 1, ecol, { "" })
    vim.api.nvim_win_set_cursor(0, { srow, scol })
    vim.snippet.expand(entry.snippet)
  end
  return true
end

---Start the live tabstop session of the next pending scaffold file (or of
---`path`, when it is pending). No-op when nothing is pending.
---@param path? string
function M.next(path)
  local index = 1
  if path then
    index = nil
    for i, entry in ipairs(pending) do
      if entry.path == path then
        index = i
        break
      end
    end
    if not index then
      return
    end
  end
  local entry = table.remove(pending, index)
  if not entry then
    vim.notify("codemmet: no scaffold file is waiting for tabstops", vim.log.levels.INFO)
    return
  end
  if not go_live(entry) then
    vim.notify(
      "codemmet: " .. vim.fs.basename(entry.path) .. " changed since it was written; left as is",
      vim.log.levels.WARN
    )
  end
  if #pending > 0 then
    vim.notify(
      ("codemmet: %d more scaffold file(s) with tabstops — :CodemmetScaffoldNext"):format(
        #pending
      ),
      vim.log.levels.INFO
    )
  end
end

---How many scaffold files are still waiting for their tabstop session.
---@return integer
function M.pending_count()
  return #pending
end

---Entering a pending file (by any means) starts its session, unless another
---snippet session is active.
vim.api.nvim_create_autocmd("BufEnter", {
  group = vim.api.nvim_create_augroup("codemmet_scaffold", { clear = true }),
  desc = "Start the pending codemmet scaffold tabstop session for this file",
  callback = function(ev)
    if #pending == 0 or vim.snippet.active() then
      return
    end
    local name = vim.api.nvim_buf_get_name(ev.buf)
    for _, entry in ipairs(pending) do
      if entry.path == name then
        vim.schedule(function()
          if vim.api.nvim_get_current_buf() == ev.buf and not vim.snippet.active() then
            M.next(name)
          end
        end)
        return
      end
    end
  end,
})

---Apply a decoded manifest: non-primary files to disk, the primary opened
---interactively (last, so it ends up focused). Under `interactive = "all"`,
---non-primary entries that carry a tabstop twin (`snippet`) are queued for a
---live session of their own.
---@param manifest table[]
local function apply(manifest)
  if #manifest == 0 then
    vim.notify(
      "codemmet: nothing to add — the target already has every test",
      vim.log.levels.INFO
    )
    return
  end
  local primary
  local names = {}
  local queued = 0
  for _, entry in ipairs(manifest) do
    if entry.primary then
      primary = entry
    else
      apply_to_disk(entry)
      if entry.snippet and entry.snippet ~= entry.content then
        table.insert(pending, {
          path = entry.path,
          action = entry.action,
          plain = entry.content,
          snippet = entry.snippet,
          offset = entry.offset,
        })
        queued = queued + 1
      end
    end
    table.insert(names, entry.action .. " " .. vim.fs.basename(entry.path))
  end
  if primary then
    open_primary(primary)
  end
  local msg = "codemmet: " .. table.concat(names, ", ")
  if queued > 0 then
    msg = msg
      .. (" — %d file(s) with tabstops: open them or :CodemmetScaffoldNext"):format(queued)
  end
  vim.notify(msg, vim.log.levels.INFO)
end

---Run `codemmet scaffold --json` for `invocation` in `bufnr` and apply it.
---@param binary string
---@param invocation string
---@param bufnr integer
local function run(binary, invocation, bufnr)
  local root = project_root(bufnr)
  local cmd = { binary, "scaffold", "--lang", vim.bo[bufnr].filetype, "--root", root }
  local file = vim.api.nvim_buf_get_name(bufnr)
  if file ~= "" then
    vim.list_extend(cmd, { "--from", file }) -- for `self`/sibling targets
  end
  -- The buffer's `:CodemmetSource` file feeds `src.*` collections, `${src.*}`
  -- paths, and `target = "source"` pieces (the `tfile` test scaffolds).
  local source = vim.b[bufnr].codemmet_source
  if type(source) == "string" and source ~= "" then
    vim.list_extend(cmd, { "--source", source })
  end
  if config.interactive == "all" then
    vim.list_extend(cmd, { "--all-tabstops" })
  end
  vim.list_extend(cmd, { "--tabstops", "keep", "--json", invocation })
  vim.system(cmd, { cwd = root, text = true }, function(obj)
    vim.schedule(function()
      if obj.code ~= 0 then
        local msg = (obj.stderr ~= "" and obj.stderr) or ("scaffold exited " .. obj.code)
        vim.notify("codemmet: " .. vim.trim(msg), vim.log.levels.WARN)
        return
      end
      local ok, manifest = pcall(vim.json.decode, obj.stdout)
      if not ok or type(manifest) ~= "table" then
        vim.notify("codemmet: could not parse the scaffold manifest", vim.log.levels.WARN)
        return
      end
      apply(manifest)
    end)
  end)
end

---Expand a multi-file snippet: write its supporting files and open the primary
---file with a live snippet. Prompts for the invocation unless `opts.abbr` is
---given.
---@param opts? codemmet.ScaffoldOpts
function M.scaffold(opts)
  opts = opts or {}
  local bufnr = vim.api.nvim_get_current_buf()
  local client = lsp.get_client(bufnr)
  if not client then
    vim.notify("codemmet: no codemmet client attached to this buffer", vim.log.levels.WARN)
    return
  end
  local binary = client.config.cmd and client.config.cmd[1]
  if type(binary) ~= "string" then
    vim.notify("codemmet: cannot locate the codemmet binary to scaffold", vim.log.levels.WARN)
    return
  end

  if opts.abbr and opts.abbr ~= "" then
    run(binary, opts.abbr, bufnr)
    return
  end
  vim.ui.input({ prompt = "codemmet scaffold: " }, function(input)
    if input == nil or input == "" then
      return -- cancelled
    end
    run(binary, input, bufnr)
  end)
end

return M
