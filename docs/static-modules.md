# File Modules, @use/@export, and DCE

`@use` selects source definitions without instantiating them. Binary `.wasm`
dependencies require `(component $Constructor)` in a Component entry, followed by
explicit `instantiate` and `@connect`. See [binary constructors](wasm-use.md).


`@include` inserts declarations through textual expansion. It creates neither a scope nor a loaded-file cache.
To select functions, types, tags, or memories, use `@use` with an explicit relative path.

`math.walz`:

```wat
(module
  (@export (func $answer) (func $unused))
  (func $helper (result i32) (@effect pure) (i32.const 42))
  (func $answer (result i32) (@effect pure) (call $helper))
  (func $unused (result i32) (@effect pure) (i32.const 99)))
```

`main.walz`:

```wat
(module
  (@use "./math.walz" (func $answer $chosen))
  (export "main" (func $chosen)))
```

This binds static export `$answer` to `$chosen`, then exposes it as the Wasm `main`.
The artifact includes answer and its helper dependency, but excludes unused. The caller cannot use `$helper` or `$unused`.
The [executable example](../examples/modules.walz) returns 42 with `just run examples/modules.walz`.

## Boundary Rules

| Declaration | Meaning |
| --- | --- |
| `(@export (func $f) (type $T) (tag $Error))` | Statically public names; `@public` is accepted for compatibility |
| `(@export (opaque $State))` | Exposes the type while keeping its representation within its declaration scope |
| `(@use "./file.walz" (func $f $local) (type $T) (tag $Error))` | Selects and aliases a file API; omitting an alias keeps the identifier |
| `(@use "walz:core" (func $string.u64 $unsigned))` | Selects a public identifier from the built-in core |
| `(export "name" (func $local))` | Exposes a Wasm/Component API from the root; statically imported functions are allowed |
| `(export "name")` inside a function | Existing Wasm ABI export |
| `(import "host" "member" (func ...))` | External instantiation requirement; supplied by a host or a Component |

Each file has an independent static scope. Declarations from a loaded file do not flow into the caller.
Names imported by dependencies also remain private unless exported.
The same canonical path shares one definition set; aliases, `./`, and multiple routes do not recreate types.
Opaque types retain their owning scopes. Absolute paths and import cycles are rejected; imports and includes share limits of depth 64 and 16 MiB after expansion.
`source::load` handles filename resolution and mapping diagnostics to original files; string-based check/compile APIs perform no file I/O.

Re-export by combining `(@use "./source.walz" (func $original $renamed))` and `(@export (func $renamed))`.
`@namespace` has been removed. One file is one module. `@use` targets registered packages such as `walz:core` or file paths beginning with `./` or `../`.
Dots are part of identifiers, so `$stock.get` also needs an explicit binding. There is no lookup rule for namespaces declared inside files.
Static file modules add no variable reassignment, top-level execution, start, or memory initialization.
Wildcards, default imports/exports, dynamic imports, and cyclic modules are unsupported.
`@test` inside imported libraries is also rejected. Tests import APIs from a separate entry file, preventing accidental additions to artifact roots.

## Elimination During Emission

First, every loaded source undergoes name resolution and type, effect, ownership, opaque, and required law checks.
Invalid types/effects and unconsumed owned values inside unused functions are still rejected.
Then, starting from ABI exports, tests, and laws, reachability follows direct calls, closures, type fields, value-type payloads, exception tags, and generated-helper dependencies.
Unreachable declarations are removed and IDs reassigned; the same projection serves Core Wasm, Core+JSPI, Component, and profiler ABIs.
Unreachable host functions also disappear from manifests, codecs, and generated imports, avoiding unnecessary capabilities.
Component trusted-binding checks also run before elimination, so this does not expand acceptance of libraries containing unsupported host bindings.
`check`, `ir`, and `analyze` retain all source before elimination. Importing alone does not make a declaration an artifact root.
This reachability analysis and ID reassignment belong to the compiler TCB. No semantics-preservation proof by the Lean contract kernel is added.

Standalone standard WAT remains unchanged. This elimination does not apply to arbitrary WAT with start, memories, tables, globals, or external import resolution.
The existing path is also retained when `@include` produces only standard WAT. Named file imports use typed static linking.

Wasm imports are external definitions required at instantiation, which also runs start and similar initialization. [Wasm Core specification](https://www.w3.org/TR/wasm-core/#modules)
ESM imports refer to bindings in another module rather than declaring value copies. [ECMAScript FAQ](https://github.com/tc39/ecma262/blob/main/FAQ.md#why-are-imported-module-bindings-aliased-instead-of-copied)
Walz adopts explicit name binding and statically links an initialization-free subset of functions/types/tags into a single Wasm module. It does not implement ESM's full execution model.

`just test-modules` checks file sharing, visibility, re-exports, unused-import elimination, and execution on both JS backends.
`just test-e2e` also includes browser execution without host capabilities.

## Standard-Library Artifact Sizes

The following records the introduction of named imports for old input. These are not measurements of current sources/syntax.
Saved artifacts after JSON-builder consolidation were compared with the artifacts from this change using identical inputs and expectations.
[Input/source/artifact hashes and comparison results](stdlib/20261105-imports-dce.json) are saved.
Every call checked JSON output, exactly one input close, and zero outstanding ABI allocations.

| Core Wasm (including name section) | Before, bytes | After named imports + DCE, bytes |
| --- | ---: | ---: |
| Log aggregation | 13,942 | 11,711 |
| Inventory ledger | 15,192 | 13,065 |
| workflow | 22,966 | 20,939 |

Speed was not an acceptance criterion for this change. It addresses reachability of functions, types, and host imports; expression simplification and fully minimizing shared generated helpers remain separate optimizations.

Regression checks passed `just check`, 212 Rust tests, 41 language tests, 45 Node tests, 26 Chromium tests, and 11 evaluation-harness tests.
For Future/Stream registries no longer generated in Components after unused-import elimination, test metrics treat an absent registry as zero.

Memories can also be selected with `(@export (memory $mem))` and `(@use "./file.walz" (memory $mem $local))`. Memory instructions inside libraries explicitly name their target. See [shared memory](threads.md).

Select fixed atomic cells with `(cell $name $alias)` and readonly regions with `(region $name $alias)`. Importing a region also links its memory. These operations do not expose integer pointers. See [atomic cells](atomic-cells.md) and [readonly regions](readonly-regions.md).
