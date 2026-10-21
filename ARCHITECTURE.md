# codemmet — Architecture

How the software is actually structured, as built. `PLAN.md` records the
original intent; this documents the shape the code has today.

The one-sentence version: **a pure expansion engine (`codemmet-core`) with
everything else — CLI, LSP, trainer, WASM, Neovim — as thin frontends over
it**, connected by two deliberate seams (the `ContextProvider` trait and the
TOML snippet schema).

## The dependency graph

```
                 ┌───────────────────────────────┐
                 │ codemmet-core (pure library)   │  no tree-sitter, no tokio,
                 │ abbr → snippet IR → render     │  no clap; serde/toml only
                 └───────┬───────────┬───────────┘
       ┌─────────────────┤           ├──────────────────┐
       │                 │           │                  │
┌──────▼───────┐  ┌──────▼─────┐  ┌──▼──────────┐  ┌────▼─────────┐
│codemmet-tsctx│  │ codemmet-  │  │codemmet-wasm│  │  codemmet    │
│ tree-sitter  │  │ train      │  │ C-ABI wasm  │  │ (the binary) │
│ Context-     │  │ lessons /  │  │ playground  │  │ cli/ lsp/    │
│ Provider impl│  │ grading    │  │ + trainer   │  │ train/       │
└──────────────┘  └─────┬──────┘  └─────────────┘  └────┬─────────┘
                        └───────────────┬───────────────┘
                                        │  bin pulls tsctx behind the
                                        │  default-on `tree-sitter` feature,
                                        │  train for `codemmet train`
                       ┌────────────────▼──────────────────┐
                       │ Editor clients over the binary:   │
                       │  lua/codemmet + plugin/ (Neovim), │
                       │  any LSP editor via `codemmet lsp`│
                       └───────────────────────────────────┘
```

Two hard rules keep this shape:

1. **Core depends on nothing heavy.** `codemmet-core` has no tree-sitter, no
   async runtime, no CLI dependencies — serde, toml, and error types only.
   Editor context arrives through a trait (see [`context.rs` — the seam](#contextrs--the-seam)).
2. **Arrows only point inward.** `codemmet-tsctx` depends on core, never the
   reverse; the binary composes everything. Tree-sitter sits behind the
   default-on `tree-sitter` cargo feature, so a minimal
   `--no-default-features` build always exists (CI keeps it honest).

These rules are why the identical engine runs unchanged in the browser, and
why a grammar-crate version bump can never ripple into the expansion logic.

## codemmet-core in depth

The core crate is the whole product, really — ~11,300 lines where every
other crate is a delivery mechanism. It owns four things: the abbreviation
grammar, the snippet IR, the renderer, and the snippet-pack loader — and
deliberately owns nothing else (`lib.rs` states the no-tree-sitter /
no-async / no-CLI contract). Almost everything is **pure**: `scaffold.rs`
produces a manifest of "what to write where" and never touches disk;
`filefmt.rs` is pure line classification with no IO or state. That purity is
why the same code runs in the WASM playground and why nearly everything is
unit-testable without fixtures.

Everything is one transformation chain — abbreviation text → AST → composed
IR → rendered expansion — and the module names map onto it directly.

### Dataflow at a glance

```
            abbreviation text                     snippet packs (TOML)
          "fn:main>fori10>pr"          builtin · user dir · paths.toml · --config
                  │                                        │
                  ▼                                        ▼
     ┌────────────────────────┐          ┌───────────────────────────────────┐
     │ abbr/                  │          │ config/ + lang/                   │
     │  lexer + recursive-    │          │  schema → loader (validate)       │
     │  descent parser        │          │  registry: merge layers,          │
     └────────────┬───────────┘          │    user wins per-trigger          │
                  │                      │  profile · imports (`requires`)   │
          Abbreviation AST               └────────────────┬──────────────────┘
   AbbrNode{word, args, repeat,               SnippetSet: trigger →
            children}                         compiled Template
                  │                           (template.rs: body →
                  │                            IrLine{depth, spans})
                  │                                        │
                  │     ┌─ editor / buffer ────────────┐   │
                  │     │ context.rs  ContextProvider  │   │
                  │     │ (NullContextProvider │       │   │
                  │     │  codemmet-tsctx impl)        │   │
                  │     └──────────────┬───────────────┘   │
                  │           ExpansionContext             │
                  │      scopes · indent · collections     │
                  ▼                    ▼                   ▼
  engine.rs  ┌──────────────────────────────────────────────────────────┐
  (façade)   │ render.rs — composition + rendering                      │
             │   resolve_word: longest-prefix trigger, glued args       │
             │   select context variant · bind args → params            │
             │   compose depth-first: children into ${child} at +1,     │
             │     siblings concatenated, repeats fanned out            │
             │   apply case transforms (transform.rs)                   │
             │   renumber tabstops globally · demote all but last $0    │
             │   materialize: indent from depth · keep|strip tabstops   │
             └──────────────────────────────┬───────────────────────────┘
                                            ▼
                             Expansion { text, cursor, tabstops }
```

Built on the same pipeline:

```
 scaffold.rs   trigger with files.* pieces ─► one render per piece
               ─► FileOutput manifest (params forced concrete; only the
                  primary piece keeps tabstops; the caller owns the disk)

 filefmt.rs    classify .codemmet lines ─► one render per line
               ─► merge.rs renumbers the results into one valid document

 capture.rs    real code ─► generalize repeated identifiers ─► Template
               (the inverse; round-trips) ─► config/writer.rs emits TOML
```

### `engine.rs` — the façade

`Engine` (82 lines) is a façade so thin it's almost honest documentation: it
holds a `Registry`, looks up the language, calls `abbr::parse`, and hands
the AST to `render`. Every frontend — CLI, LSP, WASM, trainer grading —
goes through this one entry point. Its second method, `expand_wrap()`,
splices a selection into the composed IR where `$0` would land, so escaping,
mirrors, and tabstop renumbering behave identically to normal expansion
instead of being a separate code path.

### `abbr/` — parsing the abbreviation

Lexer plus hand-written recursive-descent parser for the mini-grammar:
`fn:main>fori10>pr`, operators `>` (child), `+` (sibling), `*` (repeat),
`^` (climb-up), `()` (grouping), `:`-args. Output is
`Abbreviation(Vec<AbbrNode>)` where each node is
`{word, args, repeat, children}`. Two design points:

- **The parser is language-agnostic.** It does *not* split `fori10` into
  trigger + glued arg — that requires knowing the active language's trigger
  set, so resolution happens later in the renderer via
  `SnippetSet::resolve_word` (longest registered prefix wins). Parsing stays
  independent of which snippets are loaded.
- **`Repeat` is an enum, not a number**: `Count(n)`, `Source("@name")`,
  `Subject` (bare `*`), `Inline("[a,b,c]")`. It carries a `budget_weight()`
  used by a size budget that rejects pathological abbreviations before
  rendering — a small but deliberate DoS guard.

`^` climb-up has carefully specified semantics documented in the module:
carets clamp at group boundaries, and groups are *inlined* into their level
— `AbbrNode` has no group concept at all, which keeps the downstream
composition logic simple.

### `snippet/ir.rs` — the load-bearing data structure

A `Template` is lines of `IrLine { depth, spans }` — **indent is a relative
number, not text**. Braces are just characters and the renderer owns
indentation; this single decision is why brace languages (Rust, C) and
indentation languages (Python) share one pipeline. The `Span` enum is the
complete vocabulary of what can appear in a body: `Text`, `Tabstop` (with
placeholder), `Param` (named, optional case transform), `RepeatIndex`
(`${i}`), `ItemValue` (`${value…}`/`${outer…}` for data-driven repeats,
token order `source → .member → |transform → :default`), `ChildSlot`
(`${child}`, at most one), `FinalCursor` (`$0`).

### `snippet/template.rs` — compiling bodies

Parses TOML `body` strings (LSP snippet syntax plus the `${child}` and
`${name}` extensions) into the IR. Its indentation rules are strict on
purpose: tabs are depth levels, exactly-four-space runs normalize to one
level, and mixing is an error. (This rule is what forced the GNU Radio 3
pack's doxygen comments to `//!` line style — ` * ` continuations don't
survive it.)

### `snippet/render.rs` — composition and rendering

The giant (~3,000 lines), deliberately so — composition and rendering
concentrate here rather than smearing across the crate. It resolves each
AST node's word against the trigger set, selects context variants, binds
args to params, composes depth-first (children into `${child}` at +1 depth,
siblings concatenated, repeats fanned out with independent tabstops, a
leaf's `${child}` degrading to a cursor, a parent with children dropping its
own `$0`), renumbers tabstops globally in document order, demotes every `$0`
but the last, and materializes text in `keep` or `strip` tabstop mode —
stripping renders from the IR rather than regexing snippet syntax, so it
cannot be fooled by escapes.

### `snippet/transform.rs` — case transforms

`|snake`/`|pascal`/… with a carefully specified tokenizer (acronym runs:
`HTTPServer` → `HTTP`,`Server`; leading underscores preserved; digits never
split a token). Two invariants: all transforms are **total** (any input
yields something reasonable, so no caller needs a fallback), and transforms
apply **only to supplied text** — a param default always renders verbatim,
because re-casing a placeholder the user is about to overwrite would be
noise.

### `snippet/merge.rs` — file-mode renumbering

Exists for file mode: each `.codemmet` line renders independently with its
own `$1…$0`, so plain concatenation is not a valid LSP snippet. Merge
re-applies the single-snippet renumbering rules across the whole document,
treating each line like a sibling group (the `if+el` precedent). It parses
its own renderer's output **leniently** — unrecognized text passes through
byte-for-byte — so escaping survives the round trip.

### `snippet/capture.rs` — expansion's inverse

Real code in, parameterized template out. The generalization heuristic is
deliberately dumb and predictable: an identifier that occurs more than once
and isn't a keyword becomes a param (repeated identifiers are almost always
the subject of a pattern); everything else stays literal;
`force_params`/`force_literals` override. It guarantees a round trip:
capture then expand with no args reproduces the original dedented text.

### `config/` + `lang/` — the data side

- `config/schema.rs` — the serde mirror of the TOML format
  (`deny_unknown_fields`, so typos fail loudly). `loader.rs` (~1,800 lines,
  the second-largest module) compiles packs into validated trigger sets and
  owns `resolve_word`. `paths.rs` implements the
  `~/.config/codemmet/paths.toml` prefix→dir mapping that scopes contrib
  packs (e.g. GNU Radio) to directory trees. `writer.rs` writes TOML back
  (capture's output path).
- `lang/registry.rs` — merges the layers in precedence order: builtin packs
  (compiled in from `crates/codemmet-core/snippets/*.toml` via
  `include_str!`) → user overrides from `~/.config/codemmet/snippets/` →
  path-mapped packs → explicit `--config`. User definitions win per-trigger;
  `disabled = true` removes a builtin.
- `lang/profile.rs` — per-language facts: LSP language IDs, extensions,
  default indent, inheritance (TypeScript ← JavaScript, C++ ← C).
- `lang/imports.rs` — parses `requires` specs per language
  (`std::collections::HashMap`, `dataclasses:dataclass`, `<stdio.h>`, …)
  into an `ImportSpec` that renders the import line and decides whether an
  existing import satisfies it. Dedup is conservative in the stated safe
  direction: a false negative inserts a visible duplicate import; a false
  positive would silently skip a needed one, so `satisfied_by` only claims
  "already present" on a confident token match.

### `context.rs` — the seam

The trait boundary that keeps tree-sitter out. Anything needing buffer
knowledge — enclosing scopes, indentation, wrap boundaries, existing
imports — goes through the `ContextProvider` trait. Core ships only
`NullContextProvider` (root scope, indent guessed from the line prefix).
`Engine::expand` has taken `&ExpansionContext` since day one, so
context-awareness arrived later without an engine rewrite.

**`codemmet-tsctx`** is the real implementation: compiled-in tree-sitter
grammars behind per-language cargo features, ancestor walks mapped through
per-language kind→scope tables, plus the context-aware pieces that need real
syntax — `codegen.rs`, `extract.rs`, and `imports.rs` (scanning existing
imports so `requires` edits don't duplicate). Snippets opt into context via
per-trigger variants — `fn` inside an `impl` block grows `&self` — with the
top-level body as fallback.

### Beyond one expansion: `scaffold.rs` and `filefmt.rs`

- `scaffold.rs` — multi-file snippets: a trigger with
  `[snippets.X.files.LABEL]` pieces renders each file piece through the
  ordinary render pipeline and pairs it with a resolved target path and an
  on-exists intent, producing a **manifest** (`FileOutput` list) — the
  caller (CLI `--write`, or the editor) owns the filesystem decisions, so
  the module stays pure and testable. Params resolve to concrete text
  before rendering (they appear in file *paths*, and tabstops can't sync
  across files); only the designated **primary** piece keeps interactive
  tabstops. Path templates support `${param|transform}`; `requires` is
  collected per piece. The GNU Radio packs' `grnewblk`/`gr3newmod` are the
  largest users.
- `filefmt.rs` — the `.codemmet` source-file format: pure line
  classification (directives like `#lang`/`#repeat`, comments, blanks,
  abbreviation lines with their leading whitespace as base indent). The
  CLI's file-mode driver owns the semantic rules; keeping the parser here
  makes it unit-testable and reusable.

### The recurring design theme

**Push decisions to data and keep the engine's rules few and mechanical.**
The parser doesn't know triggers; the IR doesn't know indentation text;
scaffold doesn't know the filesystem; imports don't guess. Where behavior
could surprise, the module doc states the invariant outright (total
transforms, verbatim defaults, conservative dedup, one surviving `$0`).
This is what lets whole domains — composites, file mode, ~50 GNU Radio
triggers — land as *data and frontends* rather than engine surgery.

## The frontends

### `crates/codemmet` — the binary

One installed binary, subcommand per concern:

- `cli/` — one module per subcommand: `expand`, `render`, `scaffold`,
  `capture`, `list`, `check`, `wrap`. `build.rs` embeds git/build metadata
  for `-v` (one-liner) and `-V` (full provenance).
- `lsp/` — a synchronous `lsp-server` loop (rust-analyzer stack; no tokio —
  the work is pure CPU). The parts that carry the design weight:
  - `extract.rs` — finds the abbreviation to the left of the cursor
    (quote-aware, whitespace-terminated, capped).
  - `completion.rs` — offers the expansion as a completion item with a
    **server-computed `textEdit`** replacing the abbreviation range; client
    word boundaries are never trusted (the emmet-ls lesson).
  - `commands.rs` — `codemmet.expand` / `codemmet.wrap` *return*
    `{range, snippet}` for the client to expand, because
    `workspace/applyEdit` cannot carry tabstops.
  - `requires.rs` — auto-import weaving as `additionalTextEdits`/extraEdits.
  - `line_index.rs` — UTF-16/byte offset mapping.
- `train/` — the terminal trainer UI over `codemmet-train`.
- `tests/` — CLI integration tests, including the **contrib-pack CI pins**
  that load `contrib/packs/*` through the real registry so pack breakage
  fails the workspace build.

### `codemmet-train`

Pure lesson/exercise/grading logic (levels 1–9: triggers through composites),
deliberately UI-free so the terminal trainer and the web trainer share it
verbatim (`exercise`, `grade`, `hint`, `lessons`, `review`, deterministic
`rng`, `trainer` state machine).

### `codemmet-wasm` + `playground/`

A hand-rolled C-ABI — no wasm-bindgen toolchain: JavaScript allocates with
`cm_alloc`, writes UTF-8 into exported memory, calls `cm_expand` /
trainer entry points, reads results back, frees with `cm_free`. The crate is
the workspace's only `unsafe_code` opt-out (it is a raw FFI boundary).
`playground/index.html` (expander) and `train.html` (trainer) are the JS
side; exports like `cm_levels()` exist specifically so the pages cannot
drift from the crates (anti-drift tests pin them).

### `lua/codemmet` + `plugin/` — the Neovim client

`plugin/codemmet.lua` bootstraps; `lsp.lua` starts the server via
`vim.lsp.start`; then one module per feature — `completion`, `preview`,
`render_preview`, `render`/`filemode`, `scaffold`, `train`, `wrap`,
`browse`, `capture`, `notes`, `health` — each a thin caller of LSP commands
applying results through `vim.snippet.expand` and buffer edits. Vimdoc lives
in `doc/codemmet.txt`; `lsp/codemmet.lua` and `contrib/{mason,lspconfig}/`
carry the registry-facing definitions.

## Extension surfaces

Where new capability lands, in decreasing order of frequency:

1. **Snippet TOML** — new triggers, packs, scaffolds. Domain packs live in
   `contrib/packs/<name>/` and load via `paths.toml`; the GNU Radio 3/4
   packs added ~50 triggers and three multi-file scaffolds with zero engine
   changes. `codemmet check` validates; CI pins in
   `crates/codemmet/tests/cli.rs` guard shipped packs.
2. **`ContextProvider` / tsctx** — new context smarts (scopes, codegen,
   import scanning) land in `codemmet-tsctx` without core knowing.
3. **Engine features** — new body syntax (transforms, `${outer}`,
   data-driven repeats all landed this way) goes into `snippet/`, guarded by
   the snapshot/integration suites.
4. **Frontend features** — new subcommands in `cli/`, new LSP commands in
   `lsp/commands.rs`, new Lua modules mirroring them.

## Quality mechanics

- Per-commit gate: `cargo fmt`, clippy `-D warnings` with `--all-features`
  **and** `--no-default-features`, full workspace tests, `codemmet check`,
  `stylua` for Lua.
- Workspace lints: `unsafe_code = "deny"` (wasm crate excepted),
  clippy pedantic/nursery.
- Anti-drift pattern: anything duplicated outside the crates (web trainer
  levels, contrib packs, doc examples) gets a test pinning it to the source
  of truth.
