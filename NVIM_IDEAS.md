# Neovim plugin — feature ideas (backlog)

Candidate features for `codemmet.nvim`, captured 2026-07-08 after file mode
(PR #39) and the `<A-E>` buffer render (PR #40, stacked). Ranked by
value/cost; each notes what it builds on. Not a commitment — pick, promote to
a plan doc (the `FILE_MODE_PLAN.md` format), then build.

**Already shipped — don't re-propose:** expand (`<A-e>`), wrap
(`<leader>cw`), capture (`:CodemmetCapture`), live ghost-line preview,
fill-match-arms code actions, dogfooding notes, and buffer render (`<A-E>`
/ `:CodemmetRenderBuffer`, whole-buffer or visual selection).

## Quick wins (roughly a day each, plugin-side)

1. **`:checkhealth codemmet`** — a `lua/codemmet/health.lua`: binary found
   and executable; plugin↔binary version match (`codemmet -V`); tree-sitter
   feature compiled in (progressive context and scope variants silently
   degrade without it — worth surfacing); packs valid (shell
   `codemmet check`); server attach status. Nvim-idiomatic hygiene that
   catches the "which binary is my config actually using" class of bug.
2. **`:CodemmetTrain` in a split** — the trainer is already a full ratatui
   TUI; a ~15-line wrapper opening `codemmet train` in a `:terminal` split,
   passing the buffer's filetype as the language. Makes the trainer
   discoverable where people already are.
3. **Trigger picker** — `:CodemmetList` over `codemmet list --json` through
   `vim.ui.select` (Telescope/fzf-lua pick that up automatically):
   fuzzy-browse triggers with descriptions, preview the expansion (the
   read-only `codemmet.preview` LSP command already returns clean text),
   insert on accept. Solves "I know it exists but forgot the trigger".

## File-mode follow-ons (build on PR #39/#40)

4. **Live render preview split** — a scratch split showing the rendered
   output of the current buffer or `.codemmet` file, re-rendered on
   `TextChanged` (debounced). Render is fast, atomic, and side-effect-free,
   so this is safe by construction. The existing `preview.lua` extmark
   machinery is the in-buffer alternative (ghost-render each abbreviation
   line below itself). The best dogfooding loop for the vocabulary itself.
5. **`.codemmet` filetype support** — filetype detection for `*.codemmet`, a
   small syntax file (directives, comments, the `>` `+` `*` `@` operators),
   and — the interesting part — starting the LSP with the *inner* language
   (`main.rs.codemmet` → rust) so trigger completion works while authoring.
   Pairs with `:CodemmetRenderTo <file>` and/or an opt-in render-on-save
   that writes the sidecar target file.
6. **Snippet-mode re-entry after render** — after `<A-E>`, drop into
   `vim.snippet` at the first construct's tabstops (noted as a refinement in
   `docs/editors.md`). Caveat: needs the keep-mode whole-file tabstop
   renumbering that `FILE_MODE_PLAN.md` deliberately deferred (per-line
   snippets restart at `$1`, and multiple `$0`s collide) — CLI/core work
   first, plugin work second.

## Deeper cuts

7. **Dot-repeat and operator support** — make expand dot-repeatable and wrap
   an operator (`operatorfunc`), so `<leader>cw` + `ip` wraps a paragraph
   and `.` repeats the last wrap on another block. Pure Vim ergonomics; no
   server changes.
8. **Seed-collection prompt** — when a data-driven repeat (`arm*@X`) finds
   no buffer data, the editor currently gets the silent single-copy
   fallback; a code action or `vim.ui.input` prompt ("values for X?")
   feeding a `--repeat`-style collection would close the gap between the
   CLI's explicit hatch and editor use.

## Suggested sequencing

1 → 4 → 5: the health check is pure hygiene, and the preview split plus
filetype support together turn file mode from "a thing you pipe" into an
authoring experience.
