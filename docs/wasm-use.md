# @use of Component Wasm with WIT

In addition to `.walz`, `@use` can read executable Component `.wasm` files with WIT types. It extracts export parameter/result types from the binary and converts them to Walz types. It neither searches for a separate WIT file at runtime nor guesses an ABI from integer pointers in Core Wasm.

```wat
(module
  (@use "./math.wasm" (func $math.add $add))
  (func $main (export "main") (result i32) (@effect host)
    (call $add 20 22)))
```

When a WIT world exports a `math` interface, the function name is `$math.add`; direct world functions use `$add`. Aliasing and re-exporting through `.walz` follow ordinary file-module rules. Name collisions are rejected.

## Building a Library

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
walz build main.walz --target component -o main.component.wasm
# Node／ブラウザ用の生成JSと一緒に配布する場合
node runtime/build.mjs main.walz target/main
```

In JS, await calls as in `const {api} = await instantiate(); const value = await api.main();`. Even for a WIT synchronous function, the JSPI adapter handling calls between Components may return a Promise. The host awaits the actual asynchronous function and does not start the next job before completion.

Use `--wit-input` to create a Component matching the original world. Ordinary Walz Component output contains internal delivery imports, so reusable libraries use this form. Core Wasm with only WIT metadata and WIT packages containing only type definitions are not executable Components. Convert Core Wasm to a Component with Canonical ABI wrappers first.

## Types, Effects, and Ownership

The initial version supports **synchronous functions using shareable values in Components without host imports**.

- Types must be representable by existing WIT value adapters, including s32/s64/f32/f64, strings, bytes, shareable-element lists, records, enums, flags, tuples, and option/result.
- Unsigned, bool, char, and similar types use the same nominal types as existing WIT bindings. Import required records or unsigned types explicitly, for example `(type $math.point $Point)` or `(type $wit-u32 $Count)`.
- Existing type, effect, opaque, and ownership checks apply. WIT provides no purity proof, so external calls conservatively have `host` effects and are not treated as `pure`.
- Selecting async functions or parameters/results containing Future, Stream, or resource produces `E_WASM_USE`, including nested types. Supported synchronous exports remain usable even if the same binary contains unsupported exports.

In the current JS execution system, cancellation of Futures across Component boundaries can turn Walz cancellation exceptions into ordinary traps. WIT type information alone cannot specify cancellation, join, or endpoint lifetimes, so these remain rejected until boundary contracts are established. Existing support for Futures/Streams inside Walz and explicit host bindings is unchanged.

## Linking and Execution

The binary is snapshotted when loaded. Repeated imports of the same canonical path produce one reachable Component dependency instance, sharing internal state. Different files are distinct dependencies.

Generated Components embed the original binary unchanged and pass values through the Canonical ABI. Wasm GC references and raw memory do not pass to other instances. Distributed artifacts still run after the original `.walz`, `.wasm`, and `.wit` files are deleted. Internal dependencies do not appear in the host-import list.

DCE removes unused function bindings; when an entire dependency is unreachable, its embedding and instantiation are also removed. This does not perform function-level DCE inside used dependency binaries. Initialization/start of reachable dependencies follows ordinary Component execution and never runs during type checking.

Current limitations:

- Output requires `--target component`. Core/Core+JSPI output is rejected with `E_COMPONENT_USE_BACKEND`; Core output is possible if DCE removes the dependency.
- Components with imports must first be closed by explicitly composing required capabilities. Unknown host functionality is never allowed automatically.
- Use `@use`, not `@include`, for `.wasm`. Limits are 4 MiB per binary and 16 MiB for all expanded source.
- WIT types also have expansion/depth limits, checked before expanding shared type graphs into enormous strings.
- Obtaining types from WIT does not prove external implementation purity, termination, or internal resource release. `@race-free` continues to reject external functions.

`just test-wasm-use` checks top-level/interface exports, aliases/re-exports/shared instances, type/effect rejection, DCE, execution without original files, ABI-region release, cancellation of unstarted jobs, unsupported-boundary rejection, and Chromium. See the [execution fixture](../tests/component-fixtures/wasm-use).
