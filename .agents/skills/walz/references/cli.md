# Walz CLI examples

## Create an application

```sh
walz new app
cd app
walz check src/main.walz
walz run                         # 42
walz build                       # target/main.wasm
walz test
walz bench --list
walz bench --samples 10
```

`new` creates files from the installed compiler without downloading dependencies or initializing Git. It refuses an existing destination, including an empty directory. To use a dash-prefixed name: `walz new -- -app`.

The application uses `src/main.walz`, a library in `src/math.walz`, `tests/math.test.walz`, and `app_bench.walz`. Library names are exposed by `@export`; only the root entry exports a Wasm function. Tests select the library API rather than importing an entry module with Wasm exports.

Core execution/tests need a Wasm GC-capable Wasmtime on PATH. Benchmarks and Core＋JSPI execution need Node.js 24+. Builds and checks without laws/race-free proofs do not need either runtime. Proof-bearing source requires the configured Lean toolchain.

## Inspect before executing

```sh
walz check --json src/main.walz
walz types src/main.walz
walz type-at src/main.walz 5 7
walz analyze src/main.walz
walz wat src/main.walz
walz macro-expand --text src/main.walz
walz fmt-check src/main.walz
```

Type-at uses one-based line and Unicode-scalar column. It can report no match; do not invent a type. Check JSON carries stable error codes and original locations. `fmt-check` checks formatting without changing the input. Use `walz fmt src/main.walz` to format that file in place.
An unused imported generic may be absent from `types`; that command reports the checked program rather than every available library signature. Consult the core API or check a small typed call when inspecting a generic operation.

## Change the execution boundary

```sh
walz run --target core-js
walz build --target core-js -o dist/app
walz build --target component -o dist/app.component.wasm
walz build src/main.walz --target component --wit api.wit --world guest
walz run src/main.walz --target core-js --bindings policy.json --host host.mjs
walz test tests --target core-js --bindings policy.json --host host.mjs
```

Core execution shows raw Wasm values. Use Core＋JSPI or a Component adapter for JS strings, lists, Option/Result, and external async values. JS i64 values are bigint. Typed resource/async native worlds and the SDK have different supported capabilities; choose the boundary before inventing host imports. Vite loads `.walz` named exports or an explicit `?init` factory; a Content Mapper-capable TS7 can check those imports without declaration files.

## Minimal separate test

```wat
(module
  (func $double (param $n i64) (result i64) (@effect pure) (* $n 2))
  (@test "double returns 42" (= (call $double 21) 42)))
```

Save as `tests/double.test.walz`. Test conditions are i32: zero fails, nonzero succeeds. Benchmarks use `*_bench.walz`; return the measured result and vary inputs so constant folding does not erase the work. Each benchmark run reports its own environment and measurement conditions.
