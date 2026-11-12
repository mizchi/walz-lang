# FlatBuffers Message example

English | [日本語](README.ja.md)

This example uses the [FlatBuffers library](../../walz/flatbuffers/README.md)
under `walz/flatbuffers/`. It keeps the schema-specific code and runnable entry
points separate from the reusable reader and builder.

| File | Responsibility |
| --- | --- |
| [message.fbs](message.fbs) | Wire schema: Message, nested Child, and `WLZF` identifier |
| [message.walz](message.walz) | Typed records and the schema-specific encode/decode adapter |
| [main.walz](main.walz) | Runnable demo returning `42` and portable JS exports |
| [codec_bench.walz](codec_bench.walz) | In-Wasm encode/decode benchmark, including materialization |
| [resident.walz](resident.walz) | Raw GC-reference setup/access for the comparison harness, outside the portable JS value ABI |

From the repository root, after `pnpm install`:

```sh
just test-flatbuffers
walz run examples/flatbuffers/main.walz
walz bench examples/flatbuffers --list
walz bench examples/flatbuffers
walz build examples/flatbuffers/main.walz --target core-js -o target/flatbuffers
```

Import the adapter from another file in this directory:

```text
(@use "./message.walz"
  (type $Message) (func $message.encode) (func $message.decode))
```

`$message.encode` takes `(ref $Message)` and returns `$bytes`.
`$message.decode` takes `$bytes` and returns
`(@inst $result (ref $Message) i32)`. Missing strings/vectors become empty values,
scalars use the schema's defaults, and Child absence is preserved as Option.

JavaScript bindings:

```js
const { instantiate } = await import("./target/flatbuffers/index.js");
const { api } = await instantiate();
const value = {
  id: 42n, name: "日本語🦀",
  values: Int32Array.of(20, 22), payload: Uint8Array.of(0, 128, 255),
  enabled: true, weight: 1.25, child: { count: 42, label: "child" },
};
const bytes = api.encode(value);
const decoded = api.decode(bytes); // { tag: "ok", val: value } or { tag: "err", val: code }
```

Use `undefined` for no Child. The calls are synchronous and require no host
imports. See the [library documentation](../../walz/flatbuffers/README.md#binary-contracts)
for validation errors, view/copy behavior, builder contracts, and format limits.

`just test-flatbuffers` exercises native Wasm and official-JavaScript
interoperability through both Core＋JSPI and Component bindings, including
malformed input and ABI buffer/handle cleanup.

For official-JavaScript comparisons with separate encode/decode measurements and
captured inputs, use [the library performance tasks](../../walz/flatbuffers/README.md#compare-performance).
