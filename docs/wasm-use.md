# Binary Component constructors

`@use` selects a definition from a relative file. For a `.wasm` dependency, select
`(component $Constructor)`, then use standard Component `instantiate` and `alias`
to create state and select ports. Loading a definition never runs its initializer.
Function/type selection directly from `.wasm` is rejected, including unused uses.

```wat
(component
  (@use "./math.wasm" (component $Math))
  (instance $math (instantiate $Math))
  (alias export $math "example:math/math" (instance $api))
  (@lib
    (import "math" "add" (func $add (param $a i32) (param $b i32) (result i32)))
    (func $main (result i32) (@effect host) (call $add 20 22)))
  (@connect "math" (instance $api))
  (@expose "main" $main))
```

Here `import` declares an external requirement. `@connect` supplies that requirement
from an explicitly selected Component port. The same requirement can instead be
provided by the host at instantiation. `@use` grants no host capability.

## Building the dependency

`math.wit`:

```wit
package example:math;
interface math {
  add: func(a: s32, b: s32) -> s32;
}
world library { export math; }
```

`math.walz`:

```wat
(module
  (func $add (export "math-add") (param $a i32) (param $b i32)
    (result i32) (@effect pure) (+ $a $b)))
```

```sh
walz build math.walz --target component --wit-input math.wit --world library -o math.wasm
walz check main.walz
walz build main.walz --wit -o main.component.wasm
# Generate JavaScript and binary artifacts for Node or browsers.
node runtime/build.mjs main.walz target/main
```

The resulting JS uses the native composition adapter:
`const {api} = await instantiate(); const value = await api.main();`.
Native composition has no SDK `start`/queued-task interface. Async ports and owned
Future/Stream/resource values follow the existing native Component contracts;
traps and cancellation follow instance/store lifetime. Native JS mappings unwrap
`result` successes and throw a `ComponentError` with `.payload` for failures. Do not free canonical
buffers while native work may still refer to them.

## State, initialization, and types

Two `instantiate` declarations create independent defined memories, globals, and
resource identities. Aliases of one instance share its state. Repeated `@use`
selections never imply shared state. An unused explicit `instantiate` still runs
its initializer, can trap, and is retained in source order. Selecting an unused
constructor alone causes no initialization. There is no DCE inside the binary.

The source loader snapshots the validated binary, preserving its custom sections.
The output embeds those bytes; distributing it needs no original source files.
A dependency may have imports: supply them explicitly using standard `with`
arguments. Component validation checks the actual port types, including nominal
resource identities. Raw Core Wasm and WIT-only packages are not Component
constructors; wrap Core Wasm with Canonical ABI adapters first.

`@component.type` reads WIT-compatible types from an explicit `@connect` and adds
no capability. Use type aliases for named values and preserve public type ports
when exporting records/resources. External functions retain `host` effects even
when their ABI is known. Type/effect/ownership/proof checks still apply; a WIT
signature does not prove purity or release external resources.

Limits are 4 MiB per binary and 16 MiB expanded source. WIT projections also bound
expanded type graphs before constructing shape strings. `@include` does not load
binary dependencies. Executable Component composition uses Component output;
static `.walz` libraries retain ordinary selective linking and DCE for Core output.

`just test-wasm-use` checks definition snapshots, explicit imports, type/effect
errors, separate/shared state, scalar and structured value copies, async/Future
ports, retained start traps, normal ABI-buffer retirement, and Chromium. The
[fixture](../tests/component-fixtures/wasm-use) demonstrates a static facade
connected to a binary instance. See [Component composition](component-composition.md)
for constructors, resources, type queries, and memory identity.
