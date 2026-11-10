# Generating Bindings from WIT

`walz bind host.wit policy.json generated [--world name]` creates `bindings.walz`, `bindings.json`, shared `types.walz`, and implementation template `exports.walz`.
For multiple WIT worlds, select with `--world` or the world argument of Rust API `wit_bindgen::generate`.
Use named imports to select needed functions from the generated module, then specify
`--bindings generated/bindings.json` during compilation. In the JS build API, pass
`{bindings: "generated/bindings.json"}` as the third argument.

Example policy:

```json
{"imports":[{"module":"example:host/service@1.0.0","name":"echo","suspends":false,"capability":"echo"}]}
```

WIT specifies types; policy specifies capabilities and suspension. Generation rejects mismatched WIT/policy async declarations
and duplicate, missing, or extra policy entries. Fixed built-in contracts cannot be overridden.
Importing generated source alone does not establish trust. The host must also explicitly supply functions through `imports`.

Supported types include interface freestanding functions, s32/s64/f32/f64, u8/u16/u32/u64, string, list<u8>,
option/result, tuples of one or more elements, list<T> with shareable elements, and future/stream. Result/future/stream with empty payloads are supported too.
Records, enums, payload variants, bool/char/s8/s16, and 1–32 flags are supported, with aliases resolved. Imported resource own/borrow and constructors/methods are also supported; resources use native WIT Component boundaries. Lists of owned elements and direct world-level import functions are explicitly rejected. Recursive types and mutable/empty records cannot cross the boundary either.

Generated `@abi.type` specifies explicit value ABIs for GC structs/enums and nominal unsigned/flags types. It does not implicitly copy ordinary GC refs. Unsigned source storage uses i32/i64 bit patterns and requires `@newtype.wrap`/unwrap. Core+JSPI codecs validate JS u8/u16/u32/u64 ranges and enum/flags values. Component JS numeric conversions follow the pinned jco Canonical ABI.

`exports.walz` is an implementation template using the same shared types as the signatures. Replace `unreachable` with implementations. Interface functions flatten to `interface-function`. Build the corresponding world with `walz build main.walz --target component --wit api.wit --world guest`, preserving original interface names, type exports, and parameter names. It checks implementation signatures and nominal-type correspondence and internalizes private delivery imports. Native world adapters connect asynchronous endpoints with shareable payloads and imported resource own/borrow to the Canonical ABI. They use typed bindings and reject legacy numeric resource factories. See [native worlds](native-worlds.md) for details and limitations. Full WASI resource-world bindings remain subsequent work; files connect through [version-pinned value adapters](wasi-files.md).

Core+JSPI uses type-specific record codecs; Component uses Canonical ABI flattening/indirection.
External import instance types require no extra type exports and preserve WIT parameter names.
Rust shape checks and Node round trips for both backends are in `tests/wit_bindings.rs` and
`runtime/wit-bindings.test.mjs`.

Cancellation proceeds cooperatively on both JS backends using the Walz SDK. When an asynchronous host Promise has `Symbol.asyncDispose`/`Symbol.dispose`, cancellation calls it.
Release and cancellation completion also wait until the host operation finishes.

Additional ABI declarations are `(@abi.type $Bool (bool))`, `(char)`, `(s8)`, `(s16)`, `(tuple)` for immutable structs, and `(variant "empty" "value")` for cases with at most one payload each. Char validates Unicode scalar values, bool validates 0/1, and small integers validate ranges. Empty tuples and recursive types are rejected to match the pinned Component validator.

<a id="abi-型の実装上限"></a>

## ABI Type Implementation Limits

Type-graph layouts, expansion counts, and boundary type representations are memoized during checking. DAGs repeating the same child type are not rechecked as recursive trees. Per-type limits are 16 MiB inline size, 1,024 flattened slots, 64 KiB expanded type representation, and depth below 64; all checked type representations together are limited to 8 MiB. Exceeding a limit produces `E_ABI_LIMIT`. Size calculations use checked arithmetic. Adapter function parameters collectively also have limits of 1,024 slots and 16 MiB; these parameter limits do not apply to ordinary Core WAT functions. Ordinary private GC types are not implicitly converted to value ABIs. Pass large data through `list`/`bytes`/`stream` rather than repeated inline records.
