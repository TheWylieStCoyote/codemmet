Good idea, and it fits codemmet's grain — you already have reserved context-resolved markers (${i} for the repeat index, ${value} for data-driven items), so "variables" is the same mechanism extended to editor/environment context. Let me lay out what it'd mean and the design fork, which matters most for Neovim specifically.

What "variables" would mean

Snippet bodies that reference context, not just params/tabstops:

- Document-derived: ${TM_FILENAME}, basename, directory, path-relative-to-project, ${WORKSPACE_NAME}.
- Time: ${CURRENT_YEAR}, date — for license/file headers.
- Editor state: ${TM_SELECTED_TEXT} (wrap), ${CLIPBOARD}, ${TM_LINE_NUMBER}.
- User/project-defined: ${author}, ${license}, ${crate} — set once in config, reused everywhere. This is the most codemmet-shaped and highest-value slice (file headers, boilerplate), and fully deterministic.

The fork — and why it matters for Neovim

Approach A: pass LSP variable syntax through to the editor. Just emit $TM_FILENAME/${CURRENT_YEAR} verbatim in keep-mode output and let the client's snippet engine resolve it. VS Code resolves the full TextMate variable set. But Neovim's vim.snippet only resolves a small subset — unknown variables tend to come out empty or literal, so this is unreliable exactly where you're asking about. It also does nothing for strip mode / the CLI.

Approach B: codemmet resolves them, via the context seam. The host fills a variables map on ExpansionContext; the core substitutes recognized ${VAR} markers. This is on-brand: deterministic, identical across every editor, works in the CLI and strip mode, and testable (the trainer/golden suite can assert them). The pure core can't read the filesystem/clock/clipboard itself (same reason it can't in wasm), so — exactly like ContextProvider — the host supplies the values:

- the LSP server derives filename/dir/relative-path/workspace from the document URI;
- the Neovim plugin passes selection, clipboard, cursor position in the command args (it already passes formatting options this way);
- time comes in as a host-provided timestamp;
- user/project variables load from config (a [variables] table in a pack or .codemmet/).

For Neovim, Approach B is the right call — it sidesteps vim.snippet's spotty variable support entirely.

How it slots into the existing engine

- ExpansionContext gains variables: BTreeMap<String,String> (next to collections).
- Resolution precedence for ${X}: declared param → known variable → today's "undeclared parameter" error. So variables live in a reserved (uppercase-ish / known-name) namespace that can't collide with lowercase params — same pattern as i/value being reserved.
- codemmet check learns the reserved variable set (so ${TM_FILENAME} validates instead of erroring "undeclared param").
- CLI: --var NAME=value (and auto-derive filename from --from), so it's usable and testable headless.

My recommended MVP slice

Ship user/project-defined variables + document filename + current year first (Approach B). That covers the killer use case — file/license headers and project boilerplate — is fully deterministic, works everywhere including the CLI, and needs only: the variables map, the LSP server populating filename/date from the document, and a [variables] config table. Selection/clipboard/cursor variables can follow once the mechanism exists.

Want me to design this out — the ExpansionContext.variables plumbing, the reserved-name resolution + check validation, the LSP/plugin wiring, and the [variables] config — as a plan on a fresh branch?
