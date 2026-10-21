# Open PR summary — TheWylieStCoyote/codemmit

Snapshot updated 2026-07-10 (twelfth revision).
**6 open PRs — 3 mergeable, 3 need a rebase pass.** Master tips at
`9674e1b`: **#26 (multi-file snippets) is merged**, including its
post-review fixes — scaffolded files now carry `requires` imports
(create/append weaving, Go/inject manual-report), case transforms work in
`[files.*]` bodies and paths, the multi-file test scaffold was renamed
`itest` (collision with the grouped `mtest`), and — long time coming — the
branch carried the **capture smoke fix**, so the full nvim smoke suite now
passes on master for the first time. `check`: 297 triggers.
Cumulative since the `44f04be` baseline: preview config, trainer split,
completion wired by default, unified `:checkhealth codemmet`, snippet browser
+ context viewer, unified picker behavior, wrap-as-operator + dot-repeat,
`.codemmet` file mode (render, directives, progressive context, atomic
output), `<A-E>` buffer/selection render, the flag-first CLI fix,
`.codemmet` filetype (syntax, inner-language LSP, `:CodemmetRenderTo`,
render-on-save), the live render-preview split, keep-mode whole-file
renumbering + snippet re-entry after `<A-E>`, and `VARIABLES_PLAN.md`.

## Ready to merge now

| PR | Branch | Base | Size | Summary |
| --- | --- | --- | --- | --- |
| #32 | `fix/item-member-parity` | master | +44/−5 | Now docs-only: the member+default combination table rows and "Partially-populated items" section (the engine fix itself reached master inside #38's merge). Rebased down 2026-07-10 |
| #33 | `feat/conditionals` | master | +349/−0 | Conditionals design exploration (doc only): when-variants, repeat filters, shared predicate grammar, A-then-B recommendation |
| #30 | `feat/transform-nvim` | feat/transform-ops | +409/−3 | `:CodemmetTransform` + `codemmet.transform` command with server-computed cursor (the deferred T3). One-commit stacked review; auto-retargets to master when #34 merges |

## Conflicting vs master — need a rebase pass

The predicted mechanical overlaps (CHANGELOG, `docs/editors.md`,
`lua/codemmet/init.lua`) plus, for CLI-surface PRs, file mode's reshaping of
`crates/codemmet/src/cli/mod.rs`.

| PR | Branch | Size | Summary | Rebase notes |
| --- | --- | --- | --- | --- |
| #34 | `feat/transform-ops` | +2110/−91 | Abbreviations as edits: `codemmet transform` CLI, `el+` ×7 languages, `~match` (Rust) | Flipped back to CONFLICTING when #36/#38 moved master (CHANGELOG-class); quick re-rebase |
| #44 | `feat/nvim-seed-collections` | +559/−34 | Prompt for missing collections; `collections` param + `missingCollections` on `codemmet.expand` — the editor twin of `--repeat` | `lsp/commands.rs` collides with the browser commands (#50-era) + plugin files |
| #26 | `feat/mutli-file-snippets` | +2531/−31 | Multi-file snippets: `[snippets.<t>.files.<label>]` schema + `codemmet scaffold` CLI | Older branch; file mode + scaffold now share the CLI surface — worth a composition look during rebase |
| #14 | `feat/invertable` | +986/−8 | Code → abbreviation: per-language recognizers, `codemmet abbreviate` CLI, `:CodemmetAbbreviate` | Oldest open branch; plugin files moved a lot since |

## Suggested next actions

1. Merge #32 (docs-only) and #33 — both green.
2. Re-rebase #34 (now conflicts with #35's engine/CLI landings too), then
   merge #34 → #30.
3. Rebase #44 — its `codemmet.expand` changes now meet #35's `extraEdits`
   reshaping of the same command; expect a real semantic union. Then #14
   (the last ancient branch — recognizers will need awareness of everything
   that landed since).

## Resolved to date (this effort)

| PR | Outcome |
| --- | --- |
| #29 `feat/nvim-preview-config` | Merged — preview debounce/cancellation |
| #42 `feat/nvim-train` | Merged — trainer terminal split |
| #46 `feat/nvim-dot-repeat` | Merged — wrap operator + dot-repeat (rebased first) |
| #49 `feat/nvim-completion-health` | Merged — union of closed #31 + #45 |
| #39 `feat/file-mode` | Merged — `.codemmet` file mode |
| #43 `feat/codemmet-filetype` | Merged — `.codemmet` filetype + render-to (rebased first) |
| #47 `feat/nvim-render-preview` | Merged — live render-preview split (rebased first) |
| #48 `feat/render-snippet-reentry` | Merged — keep-mode renumbering + snippet re-entry (rebased first) |
| #36 `feat/case-transforms` | Merged — `${value\|snake}` case transforms (rebased twice as master moved) |
| #38 `feat/zip-collections` | Merged — `--member` zip + richer extraction; carried the parity engine fix to master |
| #35 `feat/import-requires` | Merged — auto-import via `requires` (un-qualified builtins included) |
| #26 `feat/mutli-file-snippets` | Merged — `codemmet scaffold` + `itest` rename + review fixes + the capture smoke fix |
| #40 `feat/nvim-render` | Closed-as-merged — `<A-E>` render + flag-first CLI fix on master |
| #50 `feat/nvim-snippet-picker` | Closed-as-merged — unified picker (union of closed #37 + #41) |
| #31, #45 | Closed — superseded by #49 |
| #37, #41 | Closed — superseded by #50 |

Pre-session context: #28 (trainer level 9), #27 (value-in-args/`${outer}`),
#25 (composites), #23 (project snippets) merged earlier; **#24
(`feat/header-impl-split`) was closed unmerged** — its branch still exists if
that work is ever revived; #22 (ratatui bump) closed unmerged.

## Open decision gates

- **`VARIABLES_PLAN.md`** (on master) — snippet-variables implementation
  blocked on owner sign-off of the design. (The #35 un-qualify question was
  settled by merging #35.)

## Repository naming caution

Two GitHub repos still exist: `TheWylieStCoyote/codemmit` (git remote; all
PRs) and `TheWylieStCoyote/codemmet` (new, default branch `main`, no PRs).
Bare `gh` resolves to the new one and returns empty — use
`-R TheWylieStCoyote/codemmit`.
