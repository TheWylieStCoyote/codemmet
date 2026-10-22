# Generating tests from code

codemmet can read a source file's **functions, types, and methods** and write
one test per item — from the command line, from any LSP client, or from
Neovim. This guide walks the whole flow on one example, shows what each step
prints, and ends with how to shape the generated tests to your own
conventions.

The pieces it builds on, each with its own reference page:

- [source collections](context-codegen.md#source-collections-functions-types-and-methods)
  — `src.fns`, `src.types`, `src.methods`, `src.<Type>.methods`, and the
  members each item carries;
- [repeat filters](syntax.md#repeat-filters-predicate) — `?vis=public`,
  `?!async`, `?value~get_*`, `?all`;
- the builtin test triggers `tfn`, `tmeth`, and `tfile` in the
  [language reference](languages.md);
- the LSP commands `codemmet.setSource` and `codemmet.context` and the
  code action, in [editor setup](editors.md).

## The example

Everything below uses this `src/geometry.py`:

```python
"""Plane geometry helpers."""


def area(w: float, h: float) -> float:
    return w * h


def perimeter(w: float, h: float) -> float:
    return 2 * (w + h)


def _clamp(x: float) -> float:
    return max(0.0, x)


async def fetch_shape(name: str) -> dict:
    ...


class Point:
    def __init__(self, x: float, y: float):
        self.x = x
        self.y = y

    def length(self) -> float:
        return (self.x ** 2 + self.y ** 2) ** 0.5

    def scale(self, k: float) -> "Point":
        return Point(self.x * k, self.y * k)

    @staticmethod
    def origin() -> "Point":
        return Point(0.0, 0.0)

    @classmethod
    def from_tuple(cls, t: tuple) -> "Point":
        return cls(*t)
```

Python is the running example; every other language works the same way with
its own test shapes (Rust `#[test]`, GoogleTest, Jest, Go `testing`, C
`assert`) — see [Other languages](#other-languages).

## 1. See what the file yields

Start with `vars`. It prints every collection the file provides, with each
item's members — exactly what a template can reference:

```sh
codemmet vars src/geometry.py
```

```text
src.Point.methods:
  __init__    args="x, y" ctor="ctor" owner="Point" params="x: float, y: float" receiver="self" vis="public"
                .params: x (type="float"), y (type="float")
  length      owner="Point" receiver="self" ret="float" vis="public"
  scale       args="k" owner="Point" params="k: float" receiver="self" ret="\"Point\"" vis="public"
                .params: k (type="float")
  origin      owner="Point" ret="\"Point\"" static="static" vis="public"
  from_tuple  args="t" owner="Point" params="t: tuple" receiver="cls" ret="\"Point\"" vis="public"
                .params: t (type="tuple")

src.fns:
  area         args="w, h" params="w: float, h: float" ret="float" vis="public"
                 .params: w (type="float"), h (type="float")
  perimeter    args="w, h" params="w: float, h: float" ret="float" vis="public"
                 .params: w (type="float"), h (type="float")
  _clamp       args="x" params="x: float" ret="float" vis="private"
                 .params: x (type="float")
  fetch_shape  args="name" async="async " params="name: str" ret="dict" vis="public"
                 .params: name (type="str")

src.methods:
  … (the same five methods, flat, each with its owner)

src.types:
  Point  kind="class" vis="public"
```

Read it like this: every function is an item whose **value** is its name;
the `key="…"` pairs are **members** a body reads as `${value.key}` and a
filter tests with `?key=…`; the indented `.params:` line is a
**sub-collection**, one item per parameter. The listing is complete — the
private `_clamp` and the `async` function are there — because narrowing is
the template's job, not the extractor's.

## 2. Generate tests on the command line

`--source` points an expansion at the file; `tfn*@src.fns` repeats the
builtin `tfn` trigger over its functions:

```sh
codemmet expand --lang python --source src/geometry.py "tfn*@src.fns"
```

```python
def test_area():
    result = area(w, h)
    assert result == expected
def test_perimeter():
    result = perimeter(w, h)
    assert result == expected
```

Two tests, not four. `tfn` carries a **default filter**, `vis=public?!test?!async`,
so the private helper and the `async` function are skipped — an `await`-less
body would be wrong for `fetch_shape`. The call's arguments come from the
`args` member; `expected` is a tabstop (see `--tabstops keep` below).

Change the selection by writing your own filter, which **replaces** the
default:

```sh
codemmet expand --lang python --source src/geometry.py "tfn*@src.fns?all"           # all four
codemmet expand --lang python --source src/geometry.py "tfn*@src.fns?!async"        # area, perimeter, _clamp
codemmet expand --lang python --source src/geometry.py 'tfn*@src.fns?value~*meter'  # perimeter only
```

In the editor there is one more member to filter on: `tested`, set on every
function the test buffer already mentions. `tfn*@src.fns?!tested` adds tests
only for the functions that have none, which is the abbreviation form of the
"add missing tests" code action below (the CLI has no test buffer, so there
`?!tested` keeps everything).

Methods come from `src.Point.methods` (or `src.methods` for every type at
once) through `tmeth`, which picks a body **per item**: `__init__` gets a
construction test, a `staticmethod` or `classmethod` is called on the class,
and an instance method on a constructed `subject`:

```sh
codemmet expand --lang python --source src/geometry.py "tmeth*@src.Point.methods"
```

```python
def test_point_init():
    subject = Point(x, y)
    assert subject is not None
def test_point_length():
    subject = Point()
    result = subject.length()
    assert result == expected
def test_point_scale():
    subject = Point()
    result = subject.scale(k)
    assert result == expected
def test_point_origin():
    result = Point.origin()
    assert result == expected
def test_point_from_tuple():
    result = Point.from_tuple(t)
    assert result == expected
```

For an editor, keep the tabstops; the `expected` placeholders become stops
you tab through:

```sh
codemmet expand --lang python --tabstops keep --source src/geometry.py 'tfn*@src.fns?value=area'
```

```text
def test_area():
    result = area(w, h)
    assert result == ${1:expected}$0
```

## 3. Generate the whole test file

`tfile` is a [multi-file snippet](snippets.md#multi-file-snippets) that
writes the conventional test file for the source — its import line and the
`tfn` and `tmeth` tests — in one go. Without `--write` it prints the manifest:

```sh
codemmet scaffold --lang python --source src/geometry.py tfile
```

```text
# create ./src/test_geometry.py (primary)
from geometry import *

def test_area():
    result = area(w, h)
    assert result == expected
def test_perimeter():
    …
def test_point_from_tuple():
    result = Point.from_tuple(t)
    assert result == expected
```

Run it again later, after adding a function to the source, and it appends
just that function's test: an existing target's tests count, and the items
they mention are left out (`--include-tested` renders them all). A target
that already tests everything gets nothing appended.

Add `--write` to create the file. The path and the import come from the
**source variables** — `${src.dir}/test_${src.stem}.py` and `from
${src.module} import *` — so the same trigger works for any file. Rust's
`tfile` adds the tests to the source file itself instead of creating a
sibling: inside the existing `#[cfg(test)] mod tests` when there is one, or in
a new one appended after the code:

```sh
codemmet scaffold --lang rust --source src/geometry.rs tfile
```

```text
# append src/geometry.rs (primary)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_works() {
        let result = area(w, h);
        assert_eq!(result, expected);
    }
    #[test]
    fn point_new_works() {
        let result = Point::new(x, y);
        assert_eq!(result, expected);
    }
    #[test]
    fn point_len_works() {
        let subject = Point::new();
        let result = subject.len();
        assert_eq!(result, expected);
    }
}
```

## 4. From Neovim

The plugin wraps all of this in two commands. In the test file:

```vim
:edit tests/test_geometry.py
:CodemmetSource              " prompts, prefilled with src/geometry.py when it exists
```

The prompt's default comes from naming conventions (`test_x.py` → `x.py`,
`x_test.go` → `x.go`, `x.test.ts` → `x.ts`, `tests/x.rs` → `src/x.rs`,
`x_test.cpp` → `x.cpp`/`x.hpp`); pass a path to skip it, `none` to clear it.
The notification confirms the file and the number of collections it yielded.
From then on, for this buffer:

- type `tfn*@src.fns` and press `<A-e>` (or `:CodemmetExpand`): the tests are
  inserted as a snippet with the cursor on the first `expected`;
- the live preview and the completion menu see the source too, so a
  half-typed `tfn*@src.` previews against the right file;
- `:CodemmetContext` shows the active source, its `${src.*}` variables, and
  every collection;
- `:CodemmetScaffold tfile` writes the whole test file and opens it as a live
  snippet;
- `:lua vim.lsp.buf.code_action()` offers **Add N missing function/method
  tests from geometry.py** (next section).

**Tests in the same file (Rust).** When the tests belong in the file you are
editing, there is nothing to set: a buffer is its own source, so `src.fns` and
`src.methods` are already the functions and methods around you. At the bottom
of `src/geometry.rs`, type

```text
tmod>tfn*@src.fns+tmeth*@src.methods
```

and press `<A-e>`: a `#[cfg(test)] mod tests { use super::*; … }` with one
test per public function and method appears as a live snippet, cursor on the
first `expected`. Later, inside that module, `tfn*@src.fns?vis=public?!tested`
adds tests for the public functions that still have none: `tested` is set on
every item a name in the test region (from `#[cfg(test)]` on) mentions.

`:CodemmetScaffold tfile` does the same without typing the abbreviation,
and merges: in a file that already has a `mod tests` it inserts the tests
inside it, and only a file without one gets a new module (with no source set,
the scaffold treats the current file as its own). The code-action menu offers
**Add N missing tests to geometry.rs** with the same placement, but only for
the functions no test mentions yet: names used inside the test region count
as tested there, so a function that the rest of the code calls is still
"missing" until a test mentions it. `tfile` applies the same rule when its
target exists (pass `--include-tested` to the CLI for everything); only the
typed abbreviation without `?!tested` generates from the source alone.

The choice is kept per buffer (`vim.b.codemmet_source`) and re-sent if the
server restarts. The plugin launches `codemmet lsp` from your `PATH`, so the
binary there must know these commands — `codemmet vars <file>` is a quick
check; an older build rejects it.

## 5. From any LSP client

Everything the plugin does goes through three standard LSP requests, so any
editor that can send `workspace/executeCommand` and apply a code action can
do the same. A session against the example (paths shortened):

**Set the source.** The path may be absolute or relative to the document's
directory; the reply confirms the resolved path and the collection names.

```json
--> workspace/executeCommand
{"command": "codemmet.setSource",
 "arguments": [{"uri": "file:///proj/tests/test_geometry.py", "source": "../src/geometry.py"}]}

<-- {"source": "/proj/src/geometry.py",
     "collections": ["src.Point.methods", "src.fns", "src.methods", "src.types"]}
```

A bad path is a JSON-RPC error (`source … is not a readable file`); `"source":
null` clears it. The source is **server state for that document**, which is
why completion — a request with no custom arguments — sees it too, and it is
forgotten on `textDocument/didClose`.

**Inspect.** `codemmet.context` reports the scope stack, the source, its
variables, and every collection with its items and members (abridged here):

```json
--> {"command": "codemmet.context",
     "arguments": [{"uri": "file:///proj/tests/test_geometry.py", "position": {"line": 7, "character": 0}}]}

<-- {"language": "python", "scopes": ["root"], "inStringOrComment": false,
     "source": "/proj/src/geometry.py",
     "vars": {"module": "geometry", "stem": "geometry", "file": "geometry.py",
              "dir": "/proj/src", "path": "/proj/src/geometry.py"},
     "collections": [
       {"name": "src.fns", "items": [{"value": "area", "members": {"args": "w, h", "vis": "public", …},
                                      "collections": {"params": [{"value": "w", "members": {"type": "float"}}, …]}},
                                     …]},
       …]}
```

**Expand.** With the abbreviation typed in the buffer, `codemmet.expand` at
the cursor returns the range to replace and the snippet to insert — the same
command the plugin uses for every expansion:

```json
--> {"command": "codemmet.expand",
     "arguments": [{"uri": "file:///proj/tests/test_geometry.py", "position": {"line": 7, "character": 12}}]}

<-- {"range": {"start": {"line": 7, "character": 0}, "end": {"line": 7, "character": 12}},
     "snippet": "def test_area():\n    result = area(w, h)\n    assert result == ${1:expected}\ndef test_perimeter():\n    result = perimeter(w, h)\n    assert result == ${2:expected}$0",
     "isSnippet": true}
```

**Add what is missing.** `textDocument/codeAction` anywhere in the buffer
returns one action per test trigger that still has untested items. Given a
`test_geometry.py` that already contains a `test_area`, the server offers:

```json
<-- [{"title": "Add 1 missing function test from geometry.py",
      "kind": "refactor.rewrite",
      "edit": {"changes": {"file:///proj/tests/test_geometry.py": [
        {"range": {"start": {"line": 7, "character": 12}, "end": {"line": 7, "character": 12}},
         "newText": "\ndef test_perimeter():\n    result = perimeter(w, h)\n    assert result == expected\n"}]}}},
     {"title": "Add 5 missing method tests from geometry.py", "kind": "refactor.rewrite", "edit": {…}}]
```

The edit is plain text appended at the end of the document, so applying it
needs no snippet support. "Missing" means: passes the trigger's default
filter and carries no `tested` mark — the same member `tfn*@src.fns?!tested`
filters on, set when the buffer mentions the name as a whole identifier — so
the action and the abbreviation always agree. Add a function to the source,
request the action again, and you get just its test.

A document with no source set gets the action too when its language keeps
tests in the source file (Rust, whose `tfile` targets the source) or it
already holds tests (Python, from a `def test_…` or `class Test…`): the title
is then **Add N missing tests to `<file>`**, one action for functions and
methods together. In Rust it is wrapped in a `tmod` module when the file has
none and inserted before the closing brace of a trailing `mod tests`
otherwise; in Python the tests are appended bare. Only mentions from the
test region on count as tests: Rust from the first `#[cfg(test)]`, Python
from the first `def test_…` / `async def test_…` / `class Test…`.

**Tests at the bottom of a Python module.** The same applies to a `.py` file
that keeps its tests after the code: `tfn*@src.fns?!tested` below the first
`def test_…` or `class Test…` adds tests only for the functions none of them
mention, and the code action offers the same set. A module without any test
yet is not its own test file — nothing is `tested`, so `tfn*@src.fns` writes
one test per public function, as usual.

## 6. Shaping the tests to your conventions

The builtin triggers are starting points. Three levers, from cheapest to most
expressive:

**Filters** change *which* items render without touching a template:
`tfn*@src.fns?value~parse_*`, `tmeth*@src.methods?!ctor`, `?all`.

**Pick a test framework.** The builtin bodies are pytest, `#[test]`,
GoogleTest, bare `assert`, `testing.T`, and Jest. For unittest,
pytest-asyncio, tokio, Unity, Catch2, doctest, testify, or Vitest, select the
builtin [framework pack](snippets.md#test-framework-packs) instead
of rewriting the triggers — in the project:

```toml
# .codemmet/config.toml
[frameworks]
python = "asyncio"
rust   = "tokio"
```

or per command with `--framework python=asyncio`, or as an editor default with
`setup({ frameworks = { python = "asyncio" } })`. `codemmet frameworks` lists
them. This is also how **async** functions get tests: the default `tfn` skips
them because the body depends on the runtime; with `asyncio` / `tokio` they
render awaiting:

```sh
codemmet --framework rust=tokio expand --lang rust --source src/geometry.rs "tfn*@src.fns"
```

```rust
#[test]
fn area_works() {
    let result = area(w, h);
    assert_eq!(result, expected);
}
#[tokio::test]
async fn fetch_works() {
    let result = fetch(url).await;
    assert_eq!(result, expected);
}
```

Everything above — filters, `tfile`, `:CodemmetSource`, the code action,
`?!tested` — is unchanged under a framework pack; only the bodies differ.

**Override the trigger.** A `tfn` in your own pack (`~/.config/codemmet/snippets/python.toml`,
or a project `.codemmet/snippets/`) replaces the builtin everywhere,
including `tfile` and the code action. Keep `filter` to keep the default
selection, or set your own:

```toml
[meta]
language = "python"

[snippets.tfn]
description = "pytest test, parametrize-ready"
filter = "vis=public?!async?!test"
scope = "fn-body"
body = '''
@pytest.mark.parametrize("${value.args}, expected", [
	(${1:…}),
])
def test_${value|snake}(${value.args}, expected):
	assert ${value}(${value.args}) == expected
'''
```

**Nest over the parameters.** Each function item carries a `params`
sub-collection; a node inside the test repeats over it with
`*@outer.params`, where `${value}` is the parameter, `${value.type}` its
declared type, and `${outer}` the function. A template that binds every
argument on its own line:

```toml
[snippets.tcase]
description = "test with one line per argument (tcase*@src.fns>arg*@outer.params)"
filter = "vis=public?!async?!test"
scope = "fn-body"
body = '''
def test_${value|snake}():
	${child}
	assert ${value}(${value.args}) == ${1:expected}
'''

[snippets.arg]
description = "an argument binding (data-driven over ${outer}'s params)"
body = '''
${value} = ${1:...}  # ${value.type:untyped}
'''
```

```sh
codemmet expand --lang python --config packs --source src/geometry.py 'tcase*@src.fns>arg*@outer.params'
```

```python
def test_area():
    w = ...  # float
    h = ...  # float
    assert area(w, h) == expected
def test_perimeter():
    w = ...  # float
    h = ...  # float
    assert perimeter(w, h) == expected
```

A function with no parameters simply gets no `arg` lines. `codemmet check`
validates the pack, including the `filter` spec.

**Vary the body by item** with a `when` variant when the shape has to change
— the builtin `tmeth` does this for constructors and statics; see
[item-conditional variants](snippets.md#item-conditional-variants-when).

## Other languages

Every language ships `tfn`, `tmeth` (except C, which has no methods), and
`tfile`; the member names and filters are the same everywhere, only the
bodies differ:

| Language | `tfn` body | `tfile` writes | Default filter notes |
| --- | --- | --- | --- |
| Rust | `#[test] fn x_works()` with `assert_eq!` | appends `mod tests` to the source | skips `async`, `#[test]` fns, trait impls |
| Python | pytest function | `test_<stem>.py` beside the source | skips `_private`, `async` |
| C | `static void test_x(void)` with `assert` | `test_<stem>.c` | keeps non-`static`, value-returning fns |
| C++ | GoogleTest `TEST(XTest, Works)` | `<stem>_test.cpp` | `public:` members |
| JavaScript / TypeScript | Jest `test("x works", …)` | `<stem>.test.js` / `.ts` | `export`ed, non-async |
| Go | `func TestX(t *testing.T)` | `<stem>_test.go` with `package` | exported (capitalized) |

Visibility is normalized to `public` / `private` / `protected` per language
(Rust `pub`, Python `_`, C `static`, C++ access sections, JS/TS `export` and
`#private`, Go capitalization), so `?vis=public` means the same thing in all
of them. See the per-language [examples](examples/README.md) for the exact
output of each trigger.

## Troubleshooting

- **`unknown trigger `tfn``, or `vars` is "unexpected argument".** The
  `codemmet` on your `PATH` predates source collections. Check `codemmet -v`
  and rebuild/reinstall.
- **`tfn*@src.fns` renders one blank test with tabstops.** No source, and
  the buffer is not a language with source collections: the collection is
  missing, so the repeat degrades to a scaffold. Pass `--source` (CLI) or set
  `:CodemmetSource` (editor); `codemmet vars` on the source shows what it
  yields. (A source that simply has no functions renders *nothing*: its
  `src.fns` exists and is empty.)
- **Fewer tests than functions.** The default filter — check with
  `codemmet vars` which items are `vis="private"`, `async`, or `test`, and
  widen with `?all` or your own chain.
- **Zero tests.** A written filter matched nothing; that renders nothing
  (deliberately, unlike a missing collection).
- **`${src.module}` is a tabstop / `module` placeholder.** No source was
  given, so the variable is unknown and rendered as a hole to fill.
- **The code action does not appear.** It needs a source set for *that*
  document (`codemmet.context` reports `"source": null` otherwise), and at
  least one untested item passing the trigger's filter.
- **`--no-default-features` build.** Without tree-sitter there is no
  extraction: `vars` prints `(no collections)` and `src.*` repeats degrade.

## See also

- [tutorial chapter 11](tutorials.md#11-generating-tests-from-a-file)
- [context-aware & data-driven expansion](context-codegen.md)
- [abbreviation syntax: repeat filters](syntax.md#repeat-filters-predicate)
- [custom snippets](snippets.md)
- [CLI reference](cli.md): `vars`, `expand --source`, `scaffold --source`
- [editor setup](editors.md): `:CodemmetSource`, `codemmet.setSource`, code actions
