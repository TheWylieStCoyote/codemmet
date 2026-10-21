# Source collections — functions, types, and methods as data (design)

Status: **milestones 1–4 built** (2026-10-21, branch `feat/source-collections`).
Decisions recorded here came out of an exploration with the owner; the open
items at the end are deliberately deferred, not forgotten. Where the build
refined a decision, the text below says so.

## The problem

Writing tests for a file is repetitive in exactly the way codemmet already
solves for enums: one block per thing, named after the thing. Today the engine
can repeat over enum variants, struct fields, and in-scope list literals
(`match:Msg>arm*`), because `codemmet-tsctx/src/extract.rs` reads them out of
the tree-sitter tree and hands them to the renderer as named
[`Collection`](crates/codemmet-core/src/context.rs)s. It cannot repeat over
**the functions in a file**, **the types**, or **the methods of one type** —
the three lists a test file is built from.

This plan adds those collections, a way to point them at a *different* file
than the one being edited (tests usually live apart from the code under test),
a filter syntax so a template can say "public functions only", and builtin
test triggers per language that use all of it.

```
# in tests/geometry.rs, with src/geometry.rs as the source:
tfn*@src.fns            # one #[test] per public function in src/geometry.rs
tmeth*@src.Point.methods?!static
```

## What exists that this builds on

Read the code, not the docs, when extending any of this:

- **Collections** — `ExpansionContext.collections: BTreeMap<String, Collection>`;
  an `Item` is a `value` plus string `members`. Bodies read `${value}`,
  `${value.key}`, `${value.key:default}`, `${value|snake}`; `${outer}` is the
  enclosing data-driven item (nested repeats).
- **Extraction** — `extract::collections(language, root, text, offset)` is
  called from `TreeSitterProvider::context_at`. File-wide definitions
  (enums, structs) overwrite; scoped list bindings use `entry` so the nearest
  wins.
- **Display-ready members** — the extractor already emits `vis = "pub "` and
  `optional = "?"`: fragments that carry their own spacing so a body can join
  them without conditionals and an absent member vanishes. This is the
  mechanism that lets the first cut ship **without** per-item conditional
  bodies (see "Filters first, conditionals later").
- **Seeding** — the CLI merges `--repeat`/`--member` collections into the
  context (`expand.rs`), and `render.rs` layers `buffer < --repeat < #repeat`.
  The same merge point takes source-file collections.
- **Variant selection** — `active_variant` picks a body by scope or raw node
  kind. Item-keyed variants (deferred) would extend it.
- **Multi-file snippets** — `[snippets.x.files.<label>]` with `path`, `target`,
  `on_exists = "inject"`, `into`; resolved by `codemmet scaffold`.
- **Repeat parsing** — `Repeat::{Count, Source(name), Subject, Inline}` in
  `abbr/mod.rs`; `repeat_spec` in `abbr/parser.rs` reads `@name` with `word()`
  (identifier characters only — it must learn `.` for `src.Point.methods`).

## Decisions

### 1. Terminology

Lists to repeat over are **collections** (the existing term). Single values
derived from the source file (its module path, stem) are **variables**,
written `${src.<key>}` in bodies. Both are needed for tests: the collection
gives the functions, the variable gives the import line.

### 2. Collections, namespaced under `src`

Source-derived collections live under a reserved `src.` prefix so they never
collide with a user's own type names (an enum called `fns` stays `@fns`).

| Collection | Items | Members |
| --- | --- | --- |
| `src.fns` | free functions (top-level, and inside module-like containers) | `name`, `params`, `args`, `ret`, `vis`, `async`, `test` |
| `src.types` | classes, structs, enums, traits, interfaces, unions, type aliases | `name`, `kind`, `vis` |
| `src.<Type>.methods` | the methods of one type, across every `impl` block / class body | as `src.fns`, plus `owner`, `receiver`, `static`, `trait` |

Rules that hold for every language:

- **Complete, never pre-filtered.** Private functions, test functions, dunder
  methods — all present. Narrowing is the filter's job (decision 5). The
  discovery command and user templates see everything.
- **Item value is the bare name**, so `${value}` is what you call. Qualified
  forms are members (`owner`).
- **Members are strings; absent means "no".** A filter tests presence
  (`?static`) or value (`?vis=public`). Display-fragment members carry their
  own spacing (`async = "async "`).
- **Keyed by bare type name.** Generics are stripped (`impl<T> Foo<T>` →
  `Foo`). A type's methods merge across all its `impl` blocks and nested class
  bodies; trait-impl methods carry `trait = "<TraitName>"` so `?!trait`
  excludes them.

Member vocabulary (all optional unless noted):

| Member | Meaning | Example |
| --- | --- | --- |
| `name` | bare name (always; equals the value) | `area` |
| `params` | declared parameter list as written, receiver removed | `w: f64, h: f64` |
| `args` | parameter **names** only, comma-joined — for a call | `w, h` |
| `ret` | return type, arrow/colon stripped; absent for none/unit | `f64` |
| `vis` | normalized visibility: `public`, `private`, `protected` (always) | `public` |
| `async` | present on async functions, display-ready | `async ` |
| `static` | present on methods without a receiver / marked static | `static` |
| `receiver` | the receiver as written, when there is one | `&mut self`, `self`, `cls` |
| `owner` | the type a method belongs to | `Point` |
| `trait` | Rust only: the trait an `impl Trait for Type` method implements | `Display` |
| `kind` | `src.types` only: `struct`, `enum`, `class`, `trait`, `interface`, `union`, `type` | `struct` |
| `test` | present on test functions (`#[test]`/`#[*::test]` attribute, `test_` prefix in Python, `Test` prefix in Go) | `test` |

Per-language visibility mapping (`vis`):

| Language | public | private | protected |
| --- | --- | --- | --- |
| Rust | any `pub`/`pub(…)` modifier, or a method of a trait impl | no modifier | — |
| Python | name without leading `_`, and dunders (`__init__`) | leading `_` | — |
| C | no `static` storage class | `static` | — |
| C++ | free fn: not `static`; member: `public:` section (or `struct` default) | `static` / `private:` section (or `class` default) | `protected:` |
| JavaScript | top-level `export`; class member not `#private` | not exported; `#name` | — |
| TypeScript | as JS, plus `public` accessibility modifier | `private`, `#name`, not exported | `protected` |
| Go | uppercase first letter | lowercase | — |

### 3. Variables: `${src.<key>}`

A new template span, `Span::Variable(key)`, with the same `:default` fallback
as `${value}`: a missing variable renders as a tabstop carrying the default (or
the key) so the user gets a hole, never silent empty text. Resolved from a new
`ExpansionContext.vars: BTreeMap<String, String>`.

| Key | Value | Example (`src/shapes/point.rs`) |
| --- | --- | --- |
| `src.path` | the source path as given | `src/shapes/point.rs` |
| `src.file` | file name | `point.rs` |
| `src.stem` | file name without extension | `point` |
| `src.dir` | parent directory | `src/shapes` |
| `src.module` | the language's import spelling of the file | Rust `crate::shapes::point`; Python `shapes.point`; Go the `package` name; C/C++ `point.h`; JS/TS `./point` |

`src.module` is path-derived (plus a `package` line scan for Go) and lives in
core — no tree-sitter needed — so the minimal build gets it too.

### 4. Where the source comes from

Resolution, highest priority first:

1. **Explicit.** CLI `--source <path>` on `expand`, `render`, and `scaffold`;
   `#source <path>` directive in a `.codemmet` file; a `source` argument on
   the LSP `codemmet.expand`/`preview`/`list`/`context` commands.
2. **The buffer itself.** `context_at` always includes the current document's
   `src.*` collections, so Rust's inline `mod tests` needs no flag.

An explicit source **replaces** the buffer's `src.*` collections and sets the
`src.*` variables (the buffer alone sets no variables: there is no path in a
`DocSnapshot`).

The provider gains one method, `source_collections(&mut self, doc)`, that
parses a document and returns only the `src.*` map; the caller merges it like
`--repeat` seeds. The null provider returns nothing. Variables are computed
by core from the path.

Editor (Neovim): `:CodemmetSource [path]` sets a buffer-local source
(`vim.b.codemmet_source`), sent with every command for that buffer. With no
argument it prompts, prefilling a convention-based guess (`test_foo.py` →
`foo.py`, `foo_test.go` → `foo.go`, `foo.test.ts` → `foo.ts`,
`tests/foo.rs` → `src/foo.rs`, `foo_test.cpp` → `foo.cpp`). Inference is a
prompt default, not silent behaviour.

### 5. Filters: `?predicate`

A repeat may carry predicates after its source. Whitespace-free by design: the
LSP abbreviation extractor splits candidates at whitespace (the first blind
spot in `CONDITIONALS_CONSIDERATIONS.md`).

```
element := atom ( '*' repeat-spec filter* )?
filter  := '?' '!'? KEY ( ( '=' | '~' ) VALUE )?
KEY     := identifier               ; a member name, or `value` for the item text
VALUE   := [^>+^(),?\s]+            ; `*` is allowed here (glob wildcard)
```

| Form | Meaning |
| --- | --- |
| `?key` | member present and non-empty |
| `?!key` | member absent or empty |
| `?key=text` | member equals `text` |
| `?key~glob` | member matches a glob (`*` any run, `?` not available — it opens the next predicate) |
| `?value~test_*` | the item's own text matches |
| `?all` | clear the snippet's default filter (decision 6), keep every item |

Predicates chain with AND: `?vis=public?!async`. A filter applies to `*@name`
and bare `*` repeats; on `*[…]` inline literals and `*n` counts it is a parse
error. Filtering a collection to zero items renders **zero copies** — that is
real information ("nothing public here"), unlike a *missing* collection,
which keeps degrading to one blank tabstop copy.

Why glob and not regex: a regex needs characters the abbreviation grammar
already owns (`(`, `)`, `+`, `^`), and the typed-in-an-editor use case is
"starts with" / "ends with".

### 6. Default filters on snippets, overridable from the abbreviation

Collections are complete; the *template* is where "public only" belongs.

```toml
[snippets.tfn]
filter = "vis=public"   # same predicate grammar, `?` separators optional
```

The abbreviation's own filter **replaces** the default (it does not stack):
`tfn*@src.fns` → public; `tfn*@src.fns?!async` → every non-async function
including private ones; `tfn*@src.fns?all` → everything. This keeps filters
monotonic (they only ever narrow) while the user gets public-by-default.

### 7. Filters first, conditionals later

Per-item conditional bodies (a different template for async functions, or for
methods that need an instance) are **not** in the first cut. Three reasons:

- Display-ready members cover most of it: `${value.async}` renders `async `
  or nothing; a Rust pack can emit the right attribute through a member.
- Repeat filters plus two triggers (`tfn`, `tmeth`, `?static`) cover the rest
  of the test-shape differences without new engine concepts.
- Item-keyed variants need a precedence rule against scope-keyed variants in
  `active_variant`. Better decided with real templates in hand.

The first template that cannot be written with members and filters promotes
this to a milestone (expected: methods that need a constructed instance versus
associated functions — handled meanwhile by `?static` / `?!static`).

### 8. Builtin test triggers

Per language, two data-driven triggers in the builtin packs, using the
framework defaults agreed with the owner. Each body uses `${value…}` so the
bare trigger still renders a tabstop scaffold.

| Language | Framework | Triggers |
| --- | --- | --- |
| Rust | std `#[test]` | `tfn` (free fn), `tmeth` (method) |
| Python | pytest | `tfn`, `tmeth` |
| C | plain `assert` | `tfn` |
| C++ | GoogleTest | `tfn`, `tmeth` |
| JavaScript | Jest | `tfn`, `tmeth` |
| TypeScript | Jest | `tfn`, `tmeth` |
| Go | `testing` | `tfn`, `tmeth` |

Framework choice as a snippet option or config setting is deferred; users
override the triggers in their own packs today.

### 9. Discovery: `codemmet vars`

```
codemmet vars --lang rust src/geometry.rs
codemmet vars src/geometry.rs --json
```

Prints every collection the file yields (the `src.*` ones **and** the existing
enum/struct ones, since they come from the same walk) with each item's
members, plus the `src.*` variables. It is the test oracle for the extractor
and the way to learn what a template can reference. The `--lang` flag is
optional when the extension is known.

`codemmet.context` (the LSP command behind `:CodemmetContext`) reports the
same data for the buffer, so the editor's context viewer shows `src.*` too.

## Milestones

Built in this order on one branch, one commit per milestone; later ones
depend on earlier ones.

1. **Extraction + discovery.** *(done)* `src.fns`, `src.types`, `src.<T>.methods` for
   all seven languages in `extract.rs`; `codemmet vars`; `src.*` collections
   in `context_at` and `codemmet.context`. No engine change.
2. **Source + variables.** *(done)* `--source`/`#source`, `ExpansionContext.vars`,
   `${src.key}` spans, `src.module` derivation per language, the LSP `source`
   argument, `:CodemmetSource`.
3. **Filters.** *(done)* Parser (`?` predicates, dotted source names), AST, renderer
   (`resolve_items`), snippet-level `filter` with `?all`, docs.
4. **Test packs.** *(done)* `tfn`/`tmeth` per language with default filters; examples
   in `docs/examples/*`.
5. **Per-item variants** — only when a template forces it (decision 7).
6. **"Add missing tests" code action** — a `fill_targets` shape that parses
   the test file too and offers one edit per untested function. Mirrors the
   match-arm fill. Optional.

## Refinements made while building

- **`src.methods`** (every method, flat, with `owner`) exists beside the
  per-type collections; it fell out of the accumulator for free and makes
  "one test per method in the file" a single repeat.
- **Members added:** `ctor` (constructors: Rust `new`, Python `__init__`,
  C++ `Type()`, JS/TS `constructor`) and `module` (enclosing module /
  namespace path), both useful filter keys.
- **Trait-impl methods are `public`** (as visible as the trait) and Python
  dunders are public (protocol methods); Python's `?receiver=self` is the
  cleanest "instance method" test, so the Python `tmeth` default uses it.
- **Per-document source in the LSP** is state (`codemmet.setSource`) rather
  than an argument on every command, so completion — which has no custom
  arguments — sees it too.
- **`src.module` anchors on the last `src` path component** (and, for
  Python, on `__init__.py` files found on disk), so absolute editor paths
  never leak the filesystem root into the module path.
- **Glob values may contain `*`**: a predicate runs to the next `?`,
  whitespace, or structural operator, and `*` is not one there.
- **Default filters exclude async** in Rust/Python/JS (the bodies have no
  runtime / `await`); C's `tfn` keeps value-returning functions only (`?ret`).

## Deferred / open

- **Parameters as a nested collection** (one line per parameter inside each
  test). Needs an item to carry a sub-collection. `args`/`params` strings
  cover the first cut.
- **Overloads** (C++) and same-named functions in different modules: keyed by
  bare name today; a later `signature` member may disambiguate.
- **Nested functions / closures**: excluded from `src.fns` (only functions
  whose enclosing containers are module-like count).
- **Test framework selection** per project (decision 8).
- **Multi-file test scaffolds** (`tests*@src.fns` that *creates* the test file
  with the import line): needs `${src.*}` in multi-file `path` templates.
  Follows naturally once milestones 2 and 4 are in.
