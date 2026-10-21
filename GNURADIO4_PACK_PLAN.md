# GNU Radio 4 snippet pack — plan

**Status: awaiting owner approval before implementation.**

A codemmet snippet pack for GNU Radio 4 (the C++23 rewrite at
[gnuradio/gnuradio4](https://github.com/gnuradio/gnuradio4)) covering block
authoring, testing, flowgraph building, the advanced idioms (tags, custom
buffers/schedulers), and multi-file block scaffolds. Every snippet body is
derived from real upstream code — reference files are listed at the end.

## Decisions (made with the owner)

- **Delivery: contrib pack** — `contrib/packs/gnuradio4/cpp.toml` shipped in
  the codemmet repo, loaded per-checkout via `paths.toml` path mapping (the
  feature built for exactly this). The builtin C++ pack stays GR-free.
- **Audience: both** upstream-core and OOT authors — namespace is a `str`
  parameter (`ns`, default `gr::blocks`) wherever a namespace appears.
- **Naming: `gr` prefix family** — discoverable as a group in
  `:CodemmetSnippets`, collision-free with builtin `cpp` triggers (verified:
  no builtin starts with `gr`).
- **CMake constraint**: codemmet has no CMake language, so CMake content is
  delivered through **multi-file scaffold pieces** (which render plain text
  to any path), not standalone triggers.

## Installation (documented in `contrib/packs/gnuradio4/README.md`)

```toml
# ~/.config/codemmet/paths.toml
[[map]]
prefix = "/home/you/src/gnuradio4"        # and any OOT module dirs
snippets = "/path/to/codemmet/contrib/packs/gnuradio4"
```

Snippets then load in any buffer under those prefixes (CLI and LSP), layered
on top of the builtin C++ pack.

## The snippet set

### Block authoring

| Trigger | Expands to | Usage |
| --- | --- | --- |
| `grblk` | Full block skeleton: `template<typename T> struct ${name} : gr::Block<${name}<T>>` with `PortIn<T> in`, `PortOut<T> out`, `GR_MAKE_REFLECTABLE(${name}, in, out)`, and a simple `processOne` | `grblk:Decimator` starts a new stream block; `${child}` sits in the body for `grset`/`grdoc` nesting |
| `grsrc` | Source-block skeleton (PortOut only, `processOne()` generating) | `grsrc:NoiseSource` |
| `grsink` | Sink-block skeleton (PortIn only, `processOne(v)` consuming) | `grsink:UdpSink` |
| `grin` / `grout` | `gr::PortIn<T> ${name}{};` / `gr::PortOut<T> ${name}{};` port members | Inside a block; data-driven `grin*[a,b]` writes several |
| `grset` | `Annotated<${type}, "${name}", Visible, Doc<"${doc}">> ${name} = ${default};` | The settings-member idiom from SignalGenerator.hpp; repeatable with `*[...]` |
| `grrefl` | `GR_MAKE_REFLECTABLE(${name}, $1);` | After ports/settings members |
| `grreg` | `GR_REGISTER_BLOCK("${ns}::${name}", ${ns}::${name}, ([T]), [ float, double ])` | Registration above the struct; the type list is a tabstop for extending to the full 14-type list |
| `grdoc` | `using Description = Doc<R""(@brief $0)"">;` | First member of a block |
| `grone` | SIMD-aware `template<gr::meta::t_or_simd<T> V> [[nodiscard]] constexpr V processOne(const V& a) const noexcept` | The Math.hpp pattern; `gronep` = plain scalar variant |
| `grbulk` | `processBulk(std::span<const T> in, std::span<T> out)` skeleton returning `work::Status` | Bulk-processing blocks |

### Testing

| Trigger | Expands to | Usage |
| --- | --- | --- |
| `grtest` | qa-file skeleton: `boost/ut` + gnuradio includes, `const boost::ut::suite<"${name}"> ${name} = [] { using namespace boost::ut; ${child} };` and `int main() {}` | `grtest:decimator` starts `qa_Decimator.cpp` |
| `grcase` | `"${name}"_test = [] { $0 };` | Cases inside a suite; `grcase*[basic,edge]` writes several |
| `grharness` | The full qa_Math harness: `Graph`, `TagSource`, block-under-test via `emplaceBlock`, `TagSink`, string `connect`s with `expect(...has_value())`, `scheduler::Simple`, `runAndWait`, sample comparison | The complete block-test pattern in one expansion |
| `grexpect` | `expect(${cond}) << std::format("$0");` | boost::ut assertion with context |

### Flowgraph / application

| Trigger | Expands to | Usage |
| --- | --- | --- |
| `grgraph` | App scaffold: `gr::Graph graph;` … `gr::scheduler::Simple sched; sched.runAndWait()` with error handling | `main()`-shaped flowgraph program |
| `gremplace` | `auto& ${name} = graph.emplaceBlock<${type}>({{"$1", $2}});` | Per-block; settings map as tabstops |
| `grconn` | `expect(graph.connect(${a}, "${out}"s, ${b}, "${in}"s).has_value());` — plus `grconnt`, the compile-time `graph.connect<"out">(${a}).to<"in">(${b})` form | Both connect dialects |

### Advanced idioms

| Trigger | Expands to | Usage |
| --- | --- | --- |
| `grtag` | Tag-publish idiom: `this->publishTag(gr::property_map{{"${key}", ${value}}}, ${offset});` (exact API verified against Tag.hpp/TagMonitors.hpp at build time) | Inside processOne/processBulk |
| `grtagblk` | Block skeleton with tag-forwarding members and a tag-aware processOne | Tagged-stream blocks |
| `grhist` | `gr::HistoryBuffer<T, ${n}>` member + access pattern | Blocks needing look-back |
| `grbuf` | Custom-buffer skeleton derived from `BufferSkeleton.hpp`'s required interface | Rarely written by hand — the skeleton is the value |
| `grsched` | Custom-scheduler skeleton derived from `Scheduler.hpp`/`SchedulerModel.hpp` | Same rationale |

*(`grbuf`/`grsched` are the riskiest bodies — the implementing agent must
re-derive them from the current headers and mark them clearly as skeletons.)*

### Multi-file scaffolds (codemmet `scaffold`)

| Trigger | Creates | Usage |
| --- | --- | --- |
| `grnewblk` | In-tree new block: `blocks/${lib}/include/gnuradio-4.0/${lib}/${Name}.hpp` (block skeleton, **primary**) + `blocks/${lib}/test/qa_${Name}.cpp` (test skeleton) | `codemmet scaffold "grnewblk:Decimator,filter"` — the whole upstream new-block workflow |
| `groot` | OOT module skeleton: `CMakeLists.txt` (find_package gnuradio4 + target per the blocks/math pattern) + `include/${ns}/${Name}.hpp` + `test/qa_${Name}.cpp` | Bootstraps a standalone GR4 module — this is where the CMake content lives |

## Work description (for the implementing agent)

1. `contrib/packs/gnuradio4/cpp.toml` with the ~20 snippets above; every
   body cross-checked against the reference files (re-fetch at build time —
   GR4 moves fast). `codemmet check --config contrib/packs/gnuradio4` clean.
2. `contrib/packs/gnuradio4/README.md`: what each trigger does, worked
   examples (real `codemmet expand` output pasted), paths.toml install.
3. A workspace test (`crates/codemmet/tests/`) loading the contrib pack via
   the registry so pack breakage fails CI — the anti-drift pin.
4. Verification: `codemmet expand --config contrib/packs/gnuradio4 --lang cpp`
   for every trigger (golden-style assertions on the shape); scaffold e2e for
   `grnewblk`/`groot` in a scratch dir; every body eyeballed against the
   fetched upstream source.
5. Docs: a pointer from `docs/snippets.md` (domain packs section) and
   CHANGELOG.

## Reference files (fetched 2026-07-11)

- `blocks/math/include/gnuradio-4.0/math/Math.hpp` — block struct, ports,
  GR_MAKE_REFLECTABLE, GR_REGISTER_BLOCK, SIMD processOne, Doc<>
- `blocks/basic/include/gnuradio-4.0/basic/SignalGenerator.hpp` — Annotated
  settings members
- `blocks/math/test/qa_Math.cpp` — boost::ut suite, Graph/emplaceBlock/
  connect/scheduler harness
- `core/include/gnuradio-4.0/Tag.hpp`, `blocks/testing/.../TagMonitors.hpp` —
  property_map, tag idioms
- `blocks/math/CMakeLists.txt` — block-library CMake pattern
- `core/include/gnuradio-4.0/{Buffer,BufferSkeleton,HistoryBuffer,Scheduler,SchedulerModel}.hpp`
  — advanced skeleton sources

## Open items the agent decides (with recommendations)

- `grreg` default type list: short `float, double` (recommended) vs the full
  14-type list from Math.hpp (a tabstop either way).
- Whether `grblk` includes a `grdoc` Description by default (recommended:
  yes, with a placeholder).
- Port templates: `PortIn<T>` fixed vs `${type}` parameter (recommended:
  parameter with default `T`).
