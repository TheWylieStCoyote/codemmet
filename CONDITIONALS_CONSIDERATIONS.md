# Conditionals — additional considerations

Companion to the conditionals design exploration (options were: **A** —
`when` predicates on variants for per-item bodies; **B** — `?pred` filters on
data-driven repeats; **C** — inline conditional body spans, recommended
against). These are the blind spots surfaced after the first draft: the first
three arguably change the designs; the rest are risks to carry into
implementation.

1. **Predicate literals with spaces break the editor path.** The Rust
   struct-variant glob `value~*{ .. }` — used in the exploration's own
   examples — contains spaces. The LSP extractor splits the abbreviation
   candidate at whitespace, and its quote scanner only opens a quoted region
   right after `:` or `,` (`lsp/extract.rs`). The most Rust-relevant
   predicates are therefore unrepresentable in an editor unless the quoting
   grammar learns the `?` position. This constrains predicate syntax more
   than anything else.

2. **`${child}` divergence across per-copy templates (A).** Today all copies
   of a node share one template, so child-slot presence cannot vary per copy.
   With `when`-variants, copy 1's template may have `${child}` while copy 2's
   does not — and the node has children. Dropped? Appended? Rejected at
   `check` time ("all variants of a snippet must agree on `${child}`
   presence")? Needs an answer before A is implementable.

3. **The item model is the bottleneck, not the predicate grammar.**
   Predicates can only test what items carry, and items today carry exactly
   one member (`type`). The extractors leave data on the floor: TS
   `x?: number` (an `optional` member — literally the presence use case),
   Rust field visibility, default values. And `--repeat`'s `value:type`
   syntax cannot express two members at all. Enriching the member vocabulary
   may belong *before* conditionals — at minimum, design the vocabulary and
   the predicate grammar as one decision, so `value.optional` exists on day
   one instead of `value.type` being forever the only testable thing.

4. **Code actions compete with B.** The headline filter example (skip
   already-covered match arms) is exactly what the "Fill match arms" code
   action already does — discoverably, with zero syntax. Ask "is this better
   as a code action?" per filter use case; the honest remainder for B may be
   smaller than the exploration stated.

5. **Unreachable-variant analysis.** First-match-wins over `context` ×
   `when` is a decision list, and decision lists rot: a broad early variant
   silently shadows a later one. `codemmet check` validates shape, not
   reachability, today. Cheap to check for presence/equality predicates;
   hard once globs overlap. Becomes worth building the moment `when` lands.

6. **Debuggability has no surface.** "Why did copy 3 render differently?"
   has no answer anywhere — no `--explain`, no trace. Scope variants already
   made expansion context-dependent; `when` makes it data-dependent per copy.
   An `--explain` mode (which variant matched, which predicate, which item)
   may be the difference between conditionals feeling magical and feeling
   haunted.

7. **Spending `?` forecloses futures.** The operator budget is small and
   each character is spent once. `?` after a repeat is the natural filter —
   but also the natural spelling for *optional nodes* (`el?` — conditional
   *composition*, a direction the exploration otherwise never names) and for
   ternaries. Decide what `?` means in codemmet permanently, not just for
   filters.

8. **Pack portability.** A shared pack using a new `when` key hard-fails on
   older binaries (`deny_unknown_fields`; there is no schema-version field).
   Probably acceptable — but make it a decision, not an accident.

9. **The "pointless" combo is the testing vehicle.** Filters on inline
   literals were dismissed (you typed the list; just omit the item) — but
   `*[a,b]?pred` is the only *self-contained* filter form, i.e. the only way
   the trainer, golden tests, and playground can exercise filters at all.
