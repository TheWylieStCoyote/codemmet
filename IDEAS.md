# New ideas — exploration

A brainstorm of candidate directions, grounded in the current state of the
tool (post data-driven repeats, `${outer}`, composites, trainer level 9).
Grouped by theme; starred items are the strongest candidates.

## Making expansions smarter

**★ 1. Case transforms on values** — `${value|snake}`, `${value|pascal}`,
`${value|upper}`. The data-driven work exposed this gap: `fn*@Color` over
`Red, Green` gives `fn Red()` — wrong case for every language's conventions.
Getter/setter generation is impossible today (field `maxSize` → `set_max_size`
needs a snake transform). The classic snippet-engine feature (TextMate/VSCode
have `/upcase` etc.); pure engine work (no tree-sitter), testable on the CLI,
and it makes *existing* features better — arguably higher value than
conditionals.

**2. Zipped collections** — iterate two collections in parallel
(`*@names@types`) instead of nesting for a cross product. Real use: generating
a function signature from paired name/type lists. Narrower than #1.

**3. Import requirements** — a snippet declares
`requires = ["std::collections::HashMap"]`; expanding `hmap` also inserts the
missing `use` at the top of the file. Plain-text edit, so it works in any LSP
client. High practical value; needs per-language import placement logic
(tree-sitter helps).

## Beyond insertion — abbreviations as *edits*

**★ 4. Transform operations** — today codemmet only inserts (plus wrap). A
new verb: apply an abbreviation *to* existing code. `el+` on an existing `if`
adds the else branch; `match!` converts an if-else chain; change a `for` to a
`while` keeping the body. A genuinely new domain (vim-surround for
constructs); tree-sitter makes it tractable — but a big lift, and it overlaps
with rust-analyzer-style assists.

**5. Fill-remaining code actions, generalized** — "Fill match arms" is
Rust-only and one shape. Generalize: fill struct-literal fields, implement
interface stubs, complete a partial enum. Discoverable (no syntax to learn),
plain-text edits. Incremental, well-trodden path.

## Discovery & learning — the trainer's instincts, in the editor

**★ 6. The passive coach** — the inverse of the trainer: after you hand-type
code that a trigger would have produced, a subtle hint says "`matchres` would
have done that." Tree-sitter shape-matching against the snippet vocabulary.
Nobody else has this; it directly attacks the real adoption problem
(vocabulary discovery). Risk: annoyance tuning.

**7. Reverse lookup command** — select code, ask "how could I have typed
this?" — a search over the vocabulary for the abbreviation(s) that produce
it. `capture`'s machinery half-exists for this. The simpler, on-demand
version of #6.

## Ecosystem

**8. Pack registry / `codemmet add <url>`** — install shareable packs from
git. Straightforward; value depends on there being an audience.

**9. VS Code extension** — deferred since the original plan; the LSP already
does the work, this is packaging plus snippet-expansion glue. A reach
multiplier, not a new capability.

**10. Revisit: multi-file scaffold tabstops** — from the deferred-ideas
list: interactive tabstops across all generated files (not just the primary)
for header/impl-split style scaffolds. Deferred pending experience with the
primary-only default.

## File-mode follow-ons (added after file mode / PR #39–#40 shipped)

**11. Live render preview split** — a scratch split showing the rendered
output of the current buffer/`.codemmet` file, re-rendered on debounced
`TextChanged`. Render is fast, atomic, and side-effect-free, so this is safe
by construction. With #12 it turns file mode into an authoring experience.

**12. `.codemmet` filetype support** — filetype detection, a small syntax
file (directives, comments, operators), and starting the LSP with the *inner*
language so trigger completion works while editing `main.rs.codemmet`. Pairs
with an opt-in render-on-save writing the sidecar target file.

**13. Snippet-mode re-entry after buffer render** — after `<A-E>`, drop into
`vim.snippet` at the first construct's tabstops. Blocked on keep-mode
whole-file tabstop renumbering (per-line snippets restart at `$1`), which the
file-mode plan deliberately deferred — core work first, plugin second.

**14. Seed-collection prompt** — when a data-driven repeat (`arm*@X`) finds
no buffer data in the editor, prompt for values (code action or `vim.ui.input`)
instead of the silent single-copy fallback — the editor equivalent of the
CLI's `--repeat` hatch.

**15. Editor ergonomics batch** — `:CodemmetTrain` in a terminal split with
the buffer's filetype; dot-repeat for expand; `wrap` as an operator
(`opfunc`) so `.` repeats the last wrap. Pure Vim polish, no server changes.

## Recommendation

- **#1 case transforms** — the smallest idea that pays off immediately across
  data-driven repeats, composites, and capture; removes a wart anyone using
  `fn*@` hits on day one.
- **#6 the passive coach** — the most original; the one that would make
  codemmet feel unlike anything else.
