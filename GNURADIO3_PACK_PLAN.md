# GNU Radio 3 snippet pack — plan

**Status: awaiting owner approval before implementation.**

A codemmet contrib pack for classic GNU Radio (3.10 stable conventions) —
the C++ `sync_block`/`general_block` world with its header/`_impl` split,
Python blocks and flowgraphs, `gr_unittest` QA, and a `gr_modtool add`-style
multi-file scaffold. Companion to the GR4 pack (`GNURADIO4_PACK_PLAN.md`),
loadable side-by-side thanks to the `gr3` prefix.

## Decisions (made with the owner)

- **Scope**: all four surfaces — C++ block dev, Python blocks/flowgraphs, QA
  (both languages), per-block OOT scaffold.
- **Audience**: OOT module author — `gr_modtool` layout, module/namespace as
  parameters (`mod`, default `howto`), `#include <gnuradio/...>` externals.
  No FSF copyright boilerplate; SPDX line only.
- **Prefix**: `gr3*` — coexists with the GR4 pack's `gr*` if both are
  path-mapped into one tree.
- **Version**: 3.10 stable (pybind11 era, `make_block_sptr`, YAML GRC).
- **Delivery**: `contrib/packs/gnuradio3/{cpp.toml,python.toml,README.md}`,
  installed via `paths.toml` (`dir = …`) exactly like the GR4 pack.
- **Deliberately omitted**: a whole-module scaffold (`gr_modtool newmod`
  already does that job well); GRC YAML as standalone triggers (codemmet has
  no YAML language — YAML rides in the scaffold, as CMake does in the GR4
  pack).

## The snippet set

### C++ block development (`cpp.toml`)

| Trigger | Expands to | Usage |
| --- | --- | --- |
| `gr3sync` | `_impl` class for a `sync_block`: constructor with `sync_block("${name}", io_signature::make(...), io_signature::make(...))`, `d_`-prefixed members, `work()` skeleton | `gr3sync:decimating_fir` in `lib/*_impl.cc` |
| `gr3gen` | `general_block` `_impl`: `general_work()` with `consume_each()`, `forecast()` | Rate-changing / bursty blocks |
| `gr3decim` / `gr3interp` | `sync_decimator` / `sync_interpolator` `_impl` variants with the ratio constructor arg | Fixed-rate blocks |
| `gr3hdr` | The **public header**: doxygen block, `class ${name} : virtual public gr::sync_block`, `typedef std::shared_ptr<${name}> sptr;`, `static sptr make(...);` | `include/${mod}/${name}.h` |
| `gr3make` | The factory definition: `${name}::sptr ${name}::make(...) { return gnuradio::make_block_sptr<${name}_impl>(...); }` | Top of the `_impl.cc` |
| `gr3sig` | `io_signature::make(${min}, ${max}, sizeof(${type}) * ${vlen})` | Constructor args; repeatable |
| `gr3work` | `int work(int noutput_items, gr_vector_const_void_star& input_items, gr_vector_void_star& output_items)` with `in`/`out` pointer casts and `return noutput_items;` | Inside an `_impl` |
| `gr3gwork` | `general_work(...)` + `consume_each(noutput_items);` | Inside a general block |
| `gr3tag` | `add_item_tag(0, nitems_written(0) + ${offset}, pmt::intern("${key}"), pmt::from_${type}(${val}));` and the `get_tags_in_window` read loop as `gr3tagr` | Stream-tag idioms |
| `gr3msg` | `message_port_register_in(pmt::mp("${port}"));` + `set_msg_handler(...[this](pmt::pmt_t msg){...})` pair | Message-passing blocks |
| `gr3volk` | The volk alignment constructor stanza (`volk_get_alignment` / `set_alignment`) | From multiply_const_impl.cc |

### Python blocks & flowgraphs (`python.toml`)

| Trigger | Expands to | Usage |
| --- | --- | --- |
| `gr3pyblk` | `class ${name}(gr.sync_block):` with `__init__(self, ...)` calling `gr.sync_block.__init__(self, name=..., in_sig=[np.float32], out_sig=[np.float32])` and `def work(self, input_items, output_items)` | Embedded/OOT Python blocks |
| `gr3hier` | `class ${name}(gr.hier_block2):` with the `io_signature` ctor and `self.connect(...)` body | Hierarchical blocks |
| `gr3top` | `class ${name}(gr.top_block):` + `__init__` + `main()` runner (`tb.start/wait` with SIGINT handling) | Hand-written flowgraph apps |
| `gr3conn` | `self.connect((${a}, ${ap}), (${b}, ${bp}))` | Repeatable connect lines |
| `gr3msgpy` | `self.message_port_register_in(pmt.intern("${port}"))` + `set_msg_handler` | Python message handling |

### QA (`python.toml`, plus a C++ note)

| Trigger | Expands to | Usage |
| --- | --- | --- |
| `gr3qa` | `class qa_${name}(gr_unittest.TestCase):` with `setUp` (`self.tb = gr.top_block()`), `tearDown`, one `test_000`, and the `gr_unittest.run(qa_${name})` footer | `python/${mod}/qa_${name}.py` |
| `gr3qatest` | One test method: `vector_source_${suffix}` → block → `vector_sink_${suffix}`, `self.tb.run()`, `assertFloatTuplesAlmostEqual` | Repeatable inside `gr3qa` (`${child}` nesting) |

### Multi-file scaffold (codemmet `scaffold`)

| Trigger | Creates | Usage |
| --- | --- | --- |
| `gr3newblk` | The `gr_modtool add` equivalent for a sync block: `include/${mod}/${name}.h` (public header), `lib/${name}_impl.h`, `lib/${name}_impl.cc` (**primary**), `grc/${mod}_${name}.block.yml` (GRC YAML piece), `python/${mod}/qa_${name}.py` | `codemmet scaffold "gr3newblk:decimator,howto" --lang cpp --root <oot-dir> --write` |

Scaffold notes: the pybind11 binding stub is **not** generated (modtool
derives it by parsing the header — `gr_modtool bind ${name}` remains the
right tool; the README says so). CMakeLists additions are reported as
appends where the layout is predictable, otherwise documented as a manual
step in the piece's `on_create` header comment.

## Work description (for the implementing agent)

1. `contrib/packs/gnuradio3/cpp.toml` + `python.toml` per the tables;
   **re-fetch references at build time** from `gnuradio/gnuradio` (branch
   `maint-3.10` — verify it exists, else `main` and note the drift risk).
2. `contrib/packs/gnuradio3/README.md` — per-trigger docs with real pasted
   expansion output, `paths.toml` install (`dir =` key), scaffold walkthrough,
   and a "GR3 vs GR4 pack" note (gr3 vs gr prefixes, both mappable).
3. Extend the contrib-pack CI pin in `crates/codemmet/tests/cli.rs` to cover
   this pack (check exit 0 + one cpp and one python expansion spot-check).
4. Verification: expand every trigger in both languages; scaffold e2e with
   `--write` in a scratch tree; both packs' `codemmet check` clean; bodies
   eyeballed against fetched sources.
5. Docs pointer (`docs/snippets.md` domain-packs section mentions both packs)
   and CHANGELOG.

## Reference files (to re-fetch at build time)

- `gr-blocks/lib/multiply_const_impl.{cc,h}` + `gr-blocks/include/gnuradio/blocks/multiply_const.h`
  — sync_block ctor, io_signature, work, make factory, header/impl split, volk
- `gr-blocks/python/blocks/qa_multiply_conjugate.py` — gr_unittest pattern
- `gr-utils/modtool/templates/` — the authoritative OOT per-block file shapes
  (header/impl/qa/grc yaml templates modtool itself uses)
- An embedded Python block + a `.block.yml` from `grc/blocks/` for the YAML piece
- A `general_work`/`forecast` block (e.g. something in gr-blocks with rate change)
- A message-port block for `gr3msg` (e.g. `gr-blocks/lib/message_debug_impl.cc`)

## Open items the agent decides (with recommendations)

- `gr3sync` type handling: `sizeof(float)` literals with tabstops
  (recommended — GR3 blocks are not templated in the common OOT case) vs a
  `type` param threaded through the signatures.
- Whether `gr3work` casts use `static_cast<const ${type}*>` (recommended,
  modern) vs C-style casts (what much in-tree code still uses).
- `gr3newblk`'s YAML piece: minimal valid block.yml (recommended) vs
  fully-annotated with parameters/asserts.
