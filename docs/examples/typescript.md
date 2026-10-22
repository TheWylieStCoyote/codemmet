# TypeScript examples

The `typescript` language handles files with the `ts`, `mts`, `cts`, and `tsx` extensions, and matches the LSP ids `typescript` and `typescriptreact`. Pass `--lang typescript`, or let codemmet match on any of those ids or extensions. Its default indent is 2 spaces, and it inherits the JavaScript pack, so every [JavaScript](javascript.md) trigger is available here too. See the [language reference](../languages.md) for the full list of triggers.

> Examples use the default `strip` mode, which renders plain code and leaves an empty spot wherever a tabstop will be — those are the places you fill in. Add `--tabstops keep` to see them as `$1` / `${1:placeholder}` / `$0` markers. See the [examples overview](README.md) for more.

## An interface

```sh
codemmet expand --lang typescript "int:User"
```

```typescript
interface User {
  
}
```

The `int` trigger takes `User` as a glued argument for the name and opens an interface body. The tabstop inside is where you list the members.

## A class with a typed async method

```sh
codemmet expand --lang typescript "class:Service>ctor+afn:fetch>try"
```

```typescript
class Service {
  constructor() {
    
  }
  async fetch(): Promise<void> {
    try {
      
    } catch (err) {
      
    }
  }
}
```

The `afn` here produces a typed `: Promise<void>` signature: TypeScript's pack overrides the JavaScript `afn` it inherits. The method sits beside the `ctor` as a sibling, with `try` nesting a try/catch in its body.

## A typed class method

```sh
codemmet expand --lang typescript "class:Repo>fn:find>ret"
```

```typescript
class Repo {
  find(): void {
    return ;
  }
}
```

Composed inside a class, `fn` picks the method form and gains a `: void` return annotation from the TypeScript pack. The nested `ret` adds a `return` statement with a tabstop for the value.

## A type alias and a generic function

```sh
codemmet expand --lang typescript "typ+fng:identity>ret"
```

```typescript
type Name = ;
function identity<T>(): T {
  return ;
}
```

The `typ` trigger starts a type alias, and `fng` expands to a generic function with a `<T>` parameter and a `: T` return type. The `identity` glued argument names the function; fill the alias value and the returned expression at the tabstops.

## An enum

```sh
codemmet expand --lang typescript "enum:Direction"
```

```typescript
enum Direction {
  
}
```

The `enum` trigger takes `Direction` as a glued argument and opens an enum body. Add your members at the tabstop inside.

## Trigger reference

Every TypeScript trigger, one entry each, with a runnable command and its verbatim output. TypeScript inherits the JavaScript pack, so these include the shared triggers plus the TypeScript-specific overrides (notably `fn` and `afn`, which emit typed signatures here). Examples use the default `strip` mode.

### `aeq`

An equality expectation.

```sh
codemmet expand --lang typescript "aeq"
```

```typescript
expect(actual).toBe();
```

Expands to a Jest/Vitest style `expect(...).toBe(...)` assertion. Fill the actual value and the expected value at the tabstops.

### `afn`

A typed async function declaration (overrides the JavaScript pack).

```sh
codemmet expand --lang typescript "afn:fetch"
```

```typescript
async function fetch(): Promise<void> {
  
}
```

```sh
codemmet expand --lang typescript "class:Service>ctor+afn:fetch"
```

```typescript
class Service {
  constructor() {
    
  }
  async fetch(): Promise<void> {
    
  }
}
```

TypeScript overrides the inherited JavaScript `afn` to add a `: Promise<void>` return annotation. Composed inside a class it becomes an async method sitting beside the `ctor`.

### `arrow`

An arrow function bound to a const.

```sh
codemmet expand --lang typescript "arrow:handler"
```

```typescript
const handler = () => {
  
};
```

```sh
codemmet expand --lang typescript "arrow:add>ret"
```

```typescript
const add = () => {
  return ;
};
```

The glued argument names the const. Nesting `ret` drops a `return` statement into the arrow body.

### `asrt`

An as-type assertion.

```sh
codemmet expand --lang typescript "asrt"
```

```typescript
value as 
```

Produces a `value as Type` assertion; fill the expression and the target type at the tabstops.

### `aw`

An await statement.

```sh
codemmet expand --lang typescript "aw"
```

```typescript
await ;
```

Awaits an expression on its own statement. The tabstop is the awaited value.

### `case`

A case clause with break.

```sh
codemmet expand --lang typescript "case"
```

```typescript
case value:
  
  break;
```

```sh
codemmet expand --lang typescript "switch>case"
```

```typescript
switch (value) {
  case value:
    
    break;
}
```

Emits a `case` label, a body tabstop, and a trailing `break`. Nested inside a `switch` it indents as a clause of the switch body. Note `case` takes no glued argument; the label value is a tabstop.

### `class`

A class definition.

```sh
codemmet expand --lang typescript "class:Service"
```

```typescript
class Service {
  
}
```

```sh
codemmet expand --lang typescript "class:Service>ctor+afn:fetch"
```

```typescript
class Service {
  constructor() {
    
  }
  async fetch(): Promise<void> {
    
  }
}
```

The glued argument names the class. Child triggers such as `ctor` and `afn` become members of the class body.

### `ctor`

A constructor method.

```sh
codemmet expand --lang typescript "ctor"
```

```typescript
constructor() {
  
}
```

```sh
codemmet expand --lang typescript "class:Service>ctor"
```

```typescript
class Service {
  constructor() {
    
  }
}
```

Expands to a `constructor()` method. It is most useful nested inside a `class`, where it indents as the first member.

### `default`

A default clause.

```sh
codemmet expand --lang typescript "default"
```

```typescript
default:
  
```

```sh
codemmet expand --lang typescript "switch>default"
```

```typescript
switch (value) {
  default:
    
}
```

Emits a `default:` label with a body tabstop. Nested inside a `switch` it becomes the fallthrough clause.

### `doc`

A JSDoc comment line.

```sh
codemmet expand --lang typescript "doc"
```

```typescript
/**  */
```

Produces an inline `/** ... */` JSDoc comment. The tabstop sits between the delimiters.

### `el`

An else block.

```sh
codemmet expand --lang typescript "el"
```

```typescript
else {
  
}
```

```sh
codemmet expand --lang typescript "if+el"
```

```typescript
if (condition) {
  
}
else {
  
}
```

Expands to an `else { }` block. Chained after `if` with `+` it becomes a sibling, forming a complete conditional.

### `elif`

An else-if block.

```sh
codemmet expand --lang typescript "elif"
```

```typescript
else if (condition) {
  
}
```

```sh
codemmet expand --lang typescript "if+elif+el"
```

```typescript
if (condition) {
  
}
else if (condition) {
  
}
else {
  
}
```

Expands to an `else if (condition) { }` block. Chained between `if` and `el` it forms a full if/else-if/else ladder.

### `enum`

An enum definition.

```sh
codemmet expand --lang typescript "enum"
```

```typescript
enum Name {
  
}
```

```sh
codemmet expand --lang typescript "enum:Direction"
```

```typescript
enum Direction {
  
}
```

Opens an enum body. Without an argument the name is a tabstop placeholder (`Name`); a glued argument such as `Direction` names it directly. List the members at the inner tabstop.

### `exp`

Export a const binding.

```sh
codemmet expand --lang typescript "exp"
```

```typescript
export const name = ;
```

Expands to an `export const name = ...;` statement, with tabstops for the binding name and its value.

### `expd`

Export default.

```sh
codemmet expand --lang typescript "expd"
```

```typescript
export default ;
```

Produces an `export default ...;` statement. The tabstop is the exported expression.

### `fn`

A typed function declaration (overrides the JavaScript pack).

```sh
codemmet expand --lang typescript "fn:parse"
```

```typescript
function parse(): void {
  
}
```

```sh
codemmet expand --lang typescript "fn:parse>ret"
```

```typescript
function parse(): void {
  return ;
}
```

TypeScript overrides the inherited JavaScript `fn` to add a `: void` return annotation. Nesting `ret` inserts a `return` statement in the body.

### `fng`

A generic function declaration.

```sh
codemmet expand --lang typescript "fng:identity"
```

```typescript
function identity<T>(): T {
  
}
```

```sh
codemmet expand --lang typescript "fng:map>ret"
```

```typescript
function map<T>(): T {
  return ;
}
```

Expands to a function with a `<T>` type parameter and a `: T` return type. The glued argument names the function.

### `fore`

A for...of loop.

```sh
codemmet expand --lang typescript "fore"
```

```typescript
for (const item of items) {
  
}
```

```sh
codemmet expand --lang typescript "fore>pr"
```

```typescript
for (const item of items) {
  console.log();
}
```

Iterates over the elements of an iterable with `for (const item of items)`. Nested triggers land in the loop body.

### `fori`

A for loop over an integer range.

```sh
codemmet expand --lang typescript "fori10"
```

```typescript
for (let i = 0; i < 10; i++) {
  
}
```

```sh
codemmet expand --lang typescript "fori10>pr"
```

```typescript
for (let i = 0; i < 10; i++) {
  console.log();
}
```

The glued number becomes the loop bound (`i < 10`). Nested triggers such as `pr` fill the loop body.

### `forin`

A for...in loop over object keys.

```sh
codemmet expand --lang typescript "forin"
```

```typescript
for (const key in object) {
  
}
```

```sh
codemmet expand --lang typescript "forin>pr"
```

```typescript
for (const key in object) {
  console.log();
}
```

Iterates over the enumerable keys of an object with `for (const key in object)`. Nested triggers land in the body.

### `get`

A getter backed by a private-by-convention field.

```sh
codemmet expand --lang typescript "get:name"
```

```typescript
get name() {
  return this._name;
}
```

```sh
codemmet expand --lang typescript "class:User>get:name"
```

```typescript
class User {
  get name() {
    return this._name;
  }
}
```

The glued argument names the property; the body returns the matching `this._name` field. Nested inside a class it becomes an accessor member.

### `if`

An if block.

```sh
codemmet expand --lang typescript "if"
```

```typescript
if (condition) {
  
}
```

```sh
codemmet expand --lang typescript "if>pr"
```

```typescript
if (condition) {
  console.log();
}
```

Opens an `if (condition) { }` block. Nested triggers fill the body; pair it with `elif`/`el` siblings for full branches.

### `ifchain`

If / else if chain, one branch per item.

```sh
codemmet expand --lang typescript "ifchain"
```

```typescript
if (value === variant) {
  
}
```

```sh
codemmet expand --lang typescript "ifchain:c*@Color" --repeat Color=Color.Red,Color.Green
```

```typescript
if (c === Color.Red) {
  
} else if (c === Color.Green) {
  
}
```

Every copy after the first joins its `else if` onto the previous `}`, so the chain is one statement. In an editor the collection comes from the buffer's enum (`ifchain:c*@Color` with no `--repeat`), each value spelled as the language uses it.

### `imp`

A default import.

```sh
codemmet expand --lang typescript "imp"
```

```typescript
import name from "module";
```

Expands to a default import statement, with tabstops for the binding name and the module specifier.

### `impd`

A destructured (named) import.

```sh
codemmet expand --lang typescript "impd"
```

```typescript
import {  } from "module";
```

Produces a named import with braces. Fill the imported names inside the braces and the module specifier.

### `impt`

A type-only import.

```sh
codemmet expand --lang typescript "impt"
```

```typescript
import type {  } from "module";
```

Expands to an `import type { } from "..."` statement for importing types without emitting runtime code.

### `int`

An interface definition.

```sh
codemmet expand --lang typescript "int"
```

```typescript
interface Name {
  
}
```

```sh
codemmet expand --lang typescript "int:User"
```

```typescript
interface User {
  
}
```

Opens an interface body. Without an argument the name is a placeholder tabstop (`Name`); a glued argument such as `User` names it directly. List the members at the inner tabstop.

### `jsonp`

A JSON.parse expression.

```sh
codemmet expand --lang typescript "jsonp"
```

```typescript
JSON.parse()
```

Expands to a `JSON.parse()` call. The tabstop is the string argument to parse.

### `jsons`

A JSON.stringify expression.

```sh
codemmet expand --lang typescript "jsons"
```

```typescript
JSON.stringify()
```

Expands to a `JSON.stringify()` call. The tabstop is the value to serialize.

### `pr`

A console.log call.

```sh
codemmet expand --lang typescript "pr"
```

```typescript
console.log();
```

Expands to a `console.log()` statement. The tabstop holds the arguments to log.

### `prd`

A debug-log with an object-shorthand label.

```sh
codemmet expand --lang typescript "prd"
```

```typescript
console.log({  });
```

Logs a value wrapped in an object literal so the variable name is printed alongside its value via ES2015 shorthand.

### `prom`

A new Promise with an executor.

```sh
codemmet expand --lang typescript "prom"
```

```typescript
new Promise((resolve, reject) => {
  
})
```

```sh
codemmet expand --lang typescript "prom>ret"
```

```typescript
new Promise((resolve, reject) => {
  return ;
})
```

Constructs a `new Promise` with a `(resolve, reject)` executor arrow. Nested triggers fill the executor body.

### `ret`

A return statement.

```sh
codemmet expand --lang typescript "ret"
```

```typescript
return ;
```

Expands to a `return ...;` statement. The tabstop is the returned expression.

### `set`

A setter backed by a private-by-convention field.

```sh
codemmet expand --lang typescript "set:name"
```

```typescript
set name(value) {
  this._name = value;
}
```

```sh
codemmet expand --lang typescript "class:User>set:name"
```

```typescript
class User {
  set name(value) {
    this._name = value;
  }
}
```

The glued argument names the property; the body assigns to the matching `this._name` field. Nested inside a class it becomes an accessor member.

### `stub`

Method stub from an interface or abstract declaration.

```sh
codemmet expand --lang typescript "stub"
```

```typescript
name() {
  
}
```

```sh
codemmet expand --lang typescript "stub*@Shape?all" --repeat Shape=area,scale --member "Shape.decl=area(): number,scale(f: number): void"
```

```typescript
area(): number {
  
}
scale(f: number): void {
  
}
```

In a file (or `--source`) with `interface Shape` or an `abstract class`, `stub*@src.Shape.methods` writes a stub for every required member (optional `name?()` members are not); `?all` includes them.

### `switch`

A switch statement.

```sh
codemmet expand --lang typescript "switch"
```

```typescript
switch (value) {
  
}
```

```sh
codemmet expand --lang typescript "switch>case"
```

```typescript
switch (value) {
  case value:
    
    break;
}
```

Opens a `switch (value) { }` block. Nested `case` and `default` triggers become its clauses.

### `test`

A test case (Jest/Vitest style).

```sh
codemmet expand --lang typescript "test"
```

```typescript
test("works", () => {
  
});
```

```sh
codemmet expand --lang typescript "test>aeq"
```

```typescript
test("works", () => {
  expect(actual).toBe();
});
```

Expands to a `test("...", () => { })` case. Nested triggers such as `aeq` fill the test body with assertions.

### `tfile`

Create <source>.test.ts beside the --source file with a Jest test per exported fn/method (multi-file).

```sh
codemmet scaffold --lang typescript --source geometry.ts tfile --write
```

With the `geometry.ts` from [`tfn`](#tfn), this creates `geometry.test.ts` next to the source, starting with a named import of its exported functions and types:

```typescript
import {
  area,
  Point,
} from "./geometry";
```

followed by the `tfn` tests for its public functions and the `tmeth` tests
for its methods — what
`(timport>tname*@src.fns?vis=public+tname*@src.types?vis=public)+tfn*@src.fns+tmeth*@src.methods`
expands to. A file without classes gets no method tests (its `src.methods`
is empty). Run without `--write` to see the manifest first; in Neovim,
`:CodemmetScaffold tfile` uses the buffer's `:CodemmetSource`. Requires
`--source` (or `--from`): the piece's `path` uses `${src.dir}` and
`${src.stem}`, the import `${src.module}`.

### `timport`

Named import from the --source module (data-driven: `timport>tname*@src.fns?vis=public`).

```sh
codemmet expand --lang typescript --source geometry.ts "timport>tname*@src.fns?vis=public+tname*@src.types?vis=public"
```

```typescript
import {
  area,
  Point,
} from "./geometry";
```

The `tfile` scaffold's header. `${src.module}` is the source's import path
(`./geometry`); the `tname` children fill the list, one exported name per
line, so the import stays valid whatever the count — including zero.

### `tname`

One imported name, for `timport`'s import list (data-driven: `tname*@src.fns`).

```sh
codemmet expand --lang typescript --source geometry.ts "tname*@src.fns?vis=public"
```

```typescript
area,
```

`${value}` is the item's name; without a collection it falls back to a
`name` tabstop. The trailing comma is valid in an import list, which is what
lets `timport>tname*…` render any number of names.

### `tfn`

Jest test calling a function (data-driven).

Given a source file `geometry.ts`:

```typescript
export function area(w, h) { return w * h; }
export class Point {
  constructor(x) { this.x = x; }
  length() { return this.x; }
}
```

```sh
codemmet expand --lang typescript --source geometry.ts "tfn*@src.fns"
```

```typescript
test("area works", () => {
  expect(area(w, h)).toBe(expected);
});
```

One test per function in `src.fns` — the file's free functions — with the
call's arguments filled from `${value.args}`. `tfn` carries a **default
filter** (public, non-test, non-async), which is why the constructor got no test; write your own
chain to change that (`tfn*@src.fns?all` for everything, `?!async` to include
private functions too). Bare `tfn` is the same body with tabstops. See
[source collections](../context-codegen.md#source-collections-functions-types-and-methods)
and [repeat filters](../syntax.md#repeat-filters-predicate); `:CodemmetSource`
sets the source in Neovim.

### `tmeth`

Jest test calling a method on a constructed instance (data-driven).

With the same `geometry.ts` as under [`tfn`](#tfn):

```sh
codemmet expand --lang typescript --source geometry.ts "tmeth*@src.Point.methods"
```

```typescript
test("Point.length works", () => {
  const subject = new Point();
  expect(subject.length()).toBe(expected);
});
```

One test per method of `Point` (`src.Point.methods`; `src.methods` is every
type's methods with an `owner` member). The default filter keeps public,
non-async methods. `constructor` takes the `when = "ctor"` **variant**
(construct and expect the instance), a `static` method the `when = "static"`
one (`Point.origin(..)` on the class); instance methods take the default
body. `?all` lifts the filter.

### `todo`

A TODO comment.

```sh
codemmet expand --lang typescript "todo"
```

```typescript
// TODO: 
```

Expands to a `// TODO:` line comment. The tabstop is the note text.

### `try`

A try/catch block.

```sh
codemmet expand --lang typescript "try"
```

```typescript
try {
  
} catch (err) {
  
}
```

```sh
codemmet expand --lang typescript "try>pr"
```

```typescript
try {
  console.log();
} catch (err) {

}
```

Expands to a `try { } catch (err) { }` block. Nested triggers fill the `try` body; the `catch` body is left as a tabstop.

### `typ`

A type alias.

```sh
codemmet expand --lang typescript "typ"
```

```typescript
type Name = ;
```

```sh
codemmet expand --lang typescript "typ:Id"
```

```typescript
type Id = ;
```

Starts a `type X = ...;` alias. Without an argument the name is a placeholder (`Name`); a glued argument such as `Id` names it directly. The tabstop is the aliased type.

### `while`

A while loop.

```sh
codemmet expand --lang typescript "while"
```

```typescript
while (condition) {
  
}
```

```sh
codemmet expand --lang typescript "while>pr"
```

```typescript
while (condition) {
  console.log();
}
```

Opens a `while (condition) { }` loop. Nested triggers fill the loop body.

## See also

- [abbreviation syntax](../syntax.md)
- [language reference](../languages.md)
- [examples overview](README.md)
