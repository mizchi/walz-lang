# Experimental synchronous JS ABI

This experiment compares two direct GC boundaries with the stable Core＋JSPI and
Component paths. It is an opt-in research backend, separate from the stable CLI
build targets. It does not change the Walz source language or Component/WASI ABI.

```sh
just build-js-native examples/flatbuffers/main.walz target/fb-direct utf8
just build-js-native examples/flatbuffers/main.walz target/fb-js-strings js-strings
just test-js-native
just test-js-native-e2e
```

Import `instantiate` from the generated `index.js`. Its `api` has the same shared
JS values as the codec's portable API, including typed arrays, record objects,
Option and tagged Result. It intentionally has no `start`, cancellation API, host
capabilities or owned endpoints. Source modules requiring those facilities are
rejected, rather than silently losing their behavior.

## What changes

Both modes generate private GC constructors/accessors and call the checked guest
directly. Numeric scalars and GC references need no linear-memory record packing.
The host never exposes these GC references as public handles. Bytes/numeric lists
use one reusable transfer memory (64 KiB initially, at most 16 MiB), then copy into
GC or into an independent JS result. There is no per-call linear allocator or
persistent handle registry. Transfer memory retains its high-water capacity;
that retained capacity is intentional and is reported by `__memory()`.

The `utf8` mode keeps the existing GC UTF-8 string representation. TextEncoder and
TextDecoder perform host conversions. The decoder is strict and preserves BOM.

The `js-strings` mode stores a JS string reference with a lazy UTF-8 GC cache.
The JS String Builtins `cast` operation validates construction, and imported string
constants supply literals without a JS table. Compilation requests
`builtins: ['js-string']` and `importedStringConstants: 'walz:strings'`.
JS input uses String.toWellFormed, matching the portable TextEncoder treatment of
unpaired surrogates. Byte length and string views retain UTF-8 semantics; they are
not replaced by UTF-16 length/indexing. Existing core string algorithms still use
UTF-8; this experiment does not rewrite them to JS concat/substring builtins.

UTF-8 conversion uses ordinary TextEncoder/TextDecoder imports plus a GC cache,
not the separate experimental JS Text Encoding Builtins proposal. Invalid source
bytes still return none or the schema's Result error. Validated byte ranges retain
their bytes, including BOM. `js-strings` currently requires native JS String
Builtins, imported string constants and String.toWellFormed; there is no bundled
fallback. A custom getCoreModule loader receives the required compile options.

## Contracts and limits

The compiler reuses the full type/effect/ownership/law checks and portable shape
validation. The supported boundary subset is i32/i64/f32/f64, bool, immutable ABI
records/tuples, bytes, strings, lists, Option and Result. Other nominal scalar and
variant interfaces are rejected explicitly. Used source memories, host imports,
Future/Stream/task/owned-resource types and suspension are unsupported.

Inputs and outputs preserve snapshot semantics. A mutable Uint8Array is not kept
as an immutable `$bytes` value. The host guards reentry while converting arguments,
returns validation errors without poisoning a guest that was not entered, and
poisons the instance after an uncaught guest or intrinsic-host failure. A guest
catch_all cannot turn a failed encoding intrinsic into success. Physical UTF-8
cache writes are private and do not change the source value's meaning.

The transfer limit is per bytes/list transfer. A JS string used only as a string
need not enter the transfer memory; trying to encode a larger string fails. This
is a separate JS-specific resource limit, not a new language-level string limit.

## Measurements

```sh
just js-native-capture target/fb-native-capture
just bench-js-native target/fb-native-capture target/fb-native-run
just size-js-native target/fb-native-capture target/fb-native-size /path/to/binaryen-133/bin/wasm-opt
```

Each output must be a fresh directory. Capture freezes inputs, official JS,
source and generated artifacts. The driver compares the complete codec against
Core, Component and fresh/reused official JS builders, then measures string echo
and UTF-8 byte length independently. All fields and output ownership are checked
outside clocks. Sizes include actual bundled JS and deployed Wasm; optimized
variants are instantiated and checked too.

See the [recorded experiment](../benchmarks/flatbuffers/20261111-js-native-v1/README.md)
for results and the distinction between smaller generic glue and native strings.
