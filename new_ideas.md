Same spirit as the conditionals blind-spot list — here's what I'd flag at this milestone, ordered by how soon each bites:

Publishing consequences (before/at first publish)

1. crates.io is permanent, and it changes what codemmet-core is. Versions can't be deleted (only yanked), and publishing core turns its public API into a semver contract — every refactor of Expansion, ContextProvider, Span, etc. becomes a version event, not an internal cleanup. Right now core's API is shaped by "whatever the bin and LSP need." Worth a deliberate pass: what should be pub(crate) before the API freezes in public memory? This is the least reversible thing on this list.

2. codemmet-wasm name-squatting. It's publish = false, which means you aren't claiming the name — but anyone else can. A minimal placeholder publish (or flipping it publishable) reserves it. Same logic for any future crate names you care about.

3. docs.rs will build your crates automatically — and tsctx compiles seven grammars in a resource-limited sandbox. Add [package.metadata.docs.rs] (features to enable, targets) and check the first build; a red docs.rs badge is many users' first impression.

Publishing consequences (before/at first publish) version event, not an internal cleanup. Right now core's API is shaped by "whatever the bin and LSP need." Worth a deliberate pass: what should be pub(crate) before the API freezes in public memory? This is the least reversible thing on this list. 2. codemmet-wasm name-squatting. It's you aren't claiming the name — but publish (or flipping it publishable) reserves it. Same logic for any future crate names you care about. 3. docs.rs will build your crates autos seven grammars in a resource-limited
rename — so no redirects. The old Pages site still serves a stale playground at the old URL that bookmarks and search engines know; the old README still presents itself as the project. Minimum: archive the repo with a "moved to codemmet" banner README, and diits Pages. Otherwise you'll have two l 6. Seven PRs are stranded on the old r #51, #14) — nobody will merge them

7. My floating session docs are one git clean from oblivion — PR_SUMMARY.md, IDEAS.md, CONDITIONALS_CONSIDERATIONS.md, NVIM_Ithe working tree. The idea backlogsespecially represent real decisions; commit them (a docs/notes/ or the plan-doc convention) or archive them somewhere durable.

8. Close the release loop: add a cargo publish --workspace step to release.yml (gated on a CRATES_IO_TOKEN secret), so tag-push = binaries + crates.io atomically — no more manual publish drift like the v0.4.1 tag mismatch. 9. New-repo settings: branch protectio, Dependabot enabled (the config

9. The open design gates haven't gone away: VARIABLES_PLAN.md still awaits your sign-off, and the conditionals A/B designs are documented but unbuilt — both are the natural post-publish feature train.  
   If you want a suggested order: #5 and #6 this week (drift compounds), #1's API pass before you publish anything beyond 0.4.1, #8 whenever you

❯ What about something more editor editor focused?
Good lens — almost everything editor-sugin features; the blind spots areIs.

Beyond Neovim (the reach question) 4. The portability wall is snippet insor. Your completion path, code actions-import additionalTextEdits already work in any client — that was good design. What doesn't travel is the command path (codemmet.expand returning {range, snippet} needs per-editor glue). The snippetTextEdit capability (rust-analyzer pioneered it, standardized in newer LSP) would let expand/wrap/render work via clients that support it — one servermports work as standard code actions; completion works natively. The extension's real job is just: bootstrap the binary, apply {range, snippet} results, and contribute a settings schema. Marketplace presence is where most potential users live — and the web trainer could embed in a webview almost for free.
The one performance item tree-sitter provider on every keystroke via autotrigger (new default!), plus the ghost preview, in the same debounce window. On large files that's editor-felt latency. Worth profiling before users report it; InputEdit-based incremental parsing is the known fix and the seams (per-URI tree cache) were design

If I had to sequence: 1 + 2 this month (they compound with the crates.io/release visibility), 4 beforemake several unnecessary), 7 before the completion-by-default default meets someone's 10k-line file.

Neovim citizenship (cheap, high-trust wins)

1. There's no :help codemmet. The plugin has ~10 modules, a dozen commands, and keymaps — and zero vimdoc. Neovim users reach for :h codemmet before any README; its absence reads as "not a real plugin." A doc/codemmet.txt (hand-written or generated from docs/editors.md) is probably the single highest-trust-per-hour item on this list.

2. mason.nvim + nvim-lspconfig registry entries. Mason is how Neovim users install LSP binaries; lspconfig is how they wire servers. A mason registry PR (it can fetch your GitHub release binaries — which exist as of v0.4.1) plus an lspconfig server definition makes codemmet installable the way people expect — no cargo install, no dir = spec. This is discoverability infrastructure, not a feature.

3. A Neovim version matrix in CI. The plugin claims 0.10, feature-detects 0.11 APIs, and gets tested against whatever's on the runner (your machine: 0.12). Nothing catches a 0.10 regression today. One CI job per claimed version running the headless suites keeps the floor honest — especially now that render/preview/browser lean on newer APIs.

Beyond Neovim (the reach question)

4. The portability wall is snippet insertion, and LSP 3.18 has a door. Your completion path, code actions (transforms, fill-match-arms), and auto-import additionalTextEdits already work in any client — that was good design. What doesn't travel is the command path (codemmet.expand returning {range, snippet} needs per-editor glue). The snippetTextEdit capability (rust-analyzer pioneered it, standardized in newer LSP) would let expand/wrap/render work via plain workspace/applyEdit in clients that support it — one server change instead of N editor plugins.completion + code actions + auto-import today with a five-line config — but docs/editors.md is 95% Neovim, so nobody knows. A matrix (feature × client: works / needs glue / Neovim-only) plus minimal recipes turns "Neovim plugin with an LSP" into "LSP with a first-class Neovim plugin." Honest marketing, near-zero code.

5. VS Code extension — now materially cheaper than when it was deferred. The binary is on GitHub releases and (soon) crates.io; transforms/fill/imports work as standard code actions; completion works natively. The extension's real job is just: bootstrap the binary, apply {range, snippet} results, and contribute a settings schema. Marketplace presence is where most potential users live — and the web trainer could embed in a webview almost for free.

The one performance item

7. Incremental reparsing never landed. The M3 plan said "full reparse first, incremental later" — later never came, and now completion fires the tree-sitter provider on every keystroke via autotrigger (new default!), plus the ghost preview, in the same debounce window. On large files that's editor-felt latency. Worth profiling before users report it; InputEdit-based incremental parsing is the known fix and the seams (per-URI tree cache) were designed for it.

If I had to sequence: 1 + 2 this month (they compound with the crates.io/release visibility), 4 before writing any second-editor plugin (it may make several unnecessary), 7 before the completion-by-default default meets someone's 10k-line file.

✻ Crunched for 59s · 1 local agent still running

❯ Please write the vimdoc and add mason/lspconfig registry entries. Please do this on another branch using worktrees using a agent

● Agent(Vimdoc + editor registry entries)
Backgrounded agent (↓ to manage · ctrl+o to expand)

The agent is running in its own worktree on docs/vimdoc-and-registries, based on upstream master (the new repo — its PRs live there now). Three deliverables:

1. doc/codemmet.txt — full conventional vimdoc, with the documented surface derived from the code in its checkout (not from docs, which could drift), covering setup options, every :Codemmet\* command, keymaps, the Lua API, the .codemmet filetype support, and health. Verified loadable via headless helptags + :help codemmet.
2. contrib/ registry artifacts — a mason-registry package.yaml (schema fetched fresh from a real current package, asset names matched against what your release.yml actually produces post-Intel-macOS-removal) and an nvim-lspconfig server definition in their current lsp/\*.lua style — prepared in-repo with submission instructions, not submitted to the external repos (that's an outward action for you to take or delegate explicitly).
3. A native lsp/codemmet.lua in the plugin — Neovim 0.11+ loads it from runtimepath, so vim.lsp.enable("codemmet") works even without nvim-lspconfig. I flagged the one design hazard for it to resolve deliberately: the double-attach risk if both setup() and vim.lsp.enable run.

Full gate + stylua + helptags verification before each commit, push to upstream, PR against the new repo's master. Report lands here when done.

✻ Waiting for 1 background agent to finish

❯ What would be involved with adding vscode support?

Less than you'd fear — the architecture was accidentally designed for it, because the server already speaks standard LSP and VS Code is the reference LSP client. Here's the honest breakdown:

What works with zero extension code

Once anything launches codemmet lsp for the right languages:

- Completion — the server offers snippet-kind items with server-computed textEdits; VS Code's snippet engine is the dialect the server emits (LSP snippet syntax is VS Code's own TextMate syntax, so tabstops/placeholders/mirrors just work — arguably better than in vim.snippet).
- Auto-import — additionalTextEdits on completion is a VS Code-native mechanism.
- Code actions — fill-match-arms today; the whole transform catalog (el+, ~match) the moment #34/#30 land, with no extension changes.

What the extension actually has to do

Tier 1 — the MVP (~300–500 lines of TypeScript, a weekend of work):

1. Server lifecycle via vscode-languageclient: launch codemmet lsp for the 7+react language ids, pass initializationOptions.
2. Binary bootstrap — the real design decision. Options: require PATH (cheap, hostile), a setting for the path (fine), or rust-analyzer-style auto-download of the matching GitHub release asset per platform (best UX; your release asset naming is already per-target, and the mason work gives you the URL scheme).
3. The command glue — the one thing LSP can't do alone: keybindings for expand/wrap that call workspace/executeCommand, then apply the returned {range, snippet} as delete + insertSnippet(new SnippetString(...)). This is ~40 lines and it's the same shape as lua/codemmet/lsp.lua.
4. Settings schema — contributes.configuration mirroring init_options (snippets dir, completion gate, project packs), so settings UI and IntelliSense work.

Tier 2 — parity features, each independently optional:

- Snippet browser — a QuickPick over codemmet.list is more natural in VS Code than vim.ui.select; maybe 60 lines.
- Ghost preview — inline decorations or the inline-suggestion API; the codemmet.preview command already returns clean text.
- File mode / <A-E> equivalent — buffer text → codemmet render → replace; with keep-mode + insertSnippet, the snippet re-entry feature comes free (VS Code handles whole-document snippets fine).
- .codemmet filetype — contributes.languages + a TextMate grammar (this is real new work; the vim syntax file doesn't translate).
- Capture, trainer — CLI + terminal wiring, thin.

Tier 3 — shipping: editors/vscode/ in-repo (the rust-analyzer monorepo pattern), vsce packaging, Marketplace + OpenVSX publisher accounts, a CI job attaching the .vsix to releases, @vscode/test-electron integration tests, README with GIFs (you have the demo tooling).

The strategic notes

- Do the snippetTextEdit server capability first (from the editor-considerations list) and Tier 1's item 3 shrinks — and every other editor benefits simultaneously.
- The extension is where the Marketplace discoverability lives — realistically worth more users than everything in Tier 2 combined. MVP-first is the right slice.
- One repo-level question: extension in-tree (editors/vscode/, versioned with the server — recommended) vs separate repo.

Want the house treatment — a VSCODE_PLAN.md design doc from an agent for your review, then build on approval? Given the shape is well-understood, this one could honestly go straight to build with the plan doc as its first commit.
