# FlatBuffers in Walz

English | [日本語](README.ja.md)

This library implements a FlatBuffers encoder and checked decoder in Walz. It
uses Wasm GC arrays, immutable bytes, views, Wasm exceptions, and checked bulk
copies, with no host imports. The official JavaScript `flatbuffers`
package is a development dependency used only as an independent interoperability
oracle.

## Run

From the repository root:

```sh
just test-flatbuffers
walz run examples/flatbuffers/main.walz
walz bench examples/flatbuffers --list
walz bench examples/flatbuffers
```

The demo returns `42`. `just test-flatbuffers` runs native Wasm tests and the
official-JavaScript interoperability tests through both Core＋JSPI and Component
bindings. Install development dependencies with `pnpm install` first.

## Compare performance

```sh
just flatbuffers-capture target/flatbuffers-bench/capture
just bench-flatbuffers target/flatbuffers-bench/capture target/flatbuffers-bench/run-1 9
```

Both destinations must be new directories. Capture freezes source, inputs,
generated Wasm/glue, the official JS implementation, and SHA-256 hashes. Run
verifies those hashes and measures the captured artifacts; rebuilding is outside
the timing. Repeat a capture with a different output directory after changes.

The v1 workload has four inputs: small, Unicode with 4 KiB of bytes,
64 KiB of bytes, and 16,384 i32 values. It measures encode and fully materialized
decode with the official JavaScript library, Walz resident GC values, and Walz
Core/Component APIs. JS encode has fresh and reused builder variants, both
returning owned output bytes. Resident calls retain Wasm GC inputs and results
without the value ABI; each operation still includes one JS/Wasm call.
Core/Component include input/output ABI transfer. Walz decode validates bounds,
alignment and UTF-8; the JS adapter trusts fields in these valid inputs. The
suite does not time zero-copy accessor-only decode.

The current v2 runner drains fulfilled Task Promise jobs between batches outside
the timer and warms up at least 4,096 calls and 250 ms per variant before calibration.
The runner rotates measurement order, saves
all samples, and checks every output field and ABI cleanup outside the timer.
Explicit GC runs before each sample, while automatic collection inside the
timed batch remains included. See the [recorded v1 comparison](../../benchmarks/flatbuffers/20261111-v1/README.md)
for the original results. The [profile and tuning record](../../benchmarks/flatbuffers/20261111-tuning-v1/README.md)
contains the retained optimizations, paired-v3 comparisons, bundle sizes and rejected SIMD experiment.
The [packed-storage and batching record](../../benchmarks/flatbuffers/20261111-three-improvements-v1/README.md)
adds synchronous-call allocation checks, query/transform APIs, and an optimized JS comparison.

```sh
just profile-flatbuffers target/flatbuffers-bench/capture target/flatbuffers-profile
just size-flatbuffers target/flatbuffers-bench/capture target/flatbuffers-size /path/to/binaryen-133/bin/wasm-opt
just optimize-flatbuffers target/flatbuffers-bench/capture target/flatbuffers-O3 /path/to/binaryen-133/bin/wasm-opt
just compare-flatbuffers target/flatbuffers-bench/capture target/flatbuffers-O3 target/flatbuffers-comparison
```

Profiling and speed comparisons are separate runs. O3/Oz are optional; ordinary
builds do not invoke Binaryen. Optimized captures can be checked with
`WALZ_FLATBUFFERS_CAPTURE=target/flatbuffers-O3 node --expose-gc --test runtime/flatbuffers.test.mjs`.

## Layout

| File | Responsibility |
| --- | --- |
| [reader.walz](reader.walz) | Checked table, scalar, offset, string, and vector access |
| [builder.walz](builder.walz) | Growing backward builder, alignment, fields, and vtables |
| [Example schema](../../examples/flatbuffers/message.fbs) | Message, nested Child, and `WLZF` identifier |
| [Example adapter](../../examples/flatbuffers/message.walz) | Typed records and the schema-specific encode/decode adapter |
| [Example entry point](../../examples/flatbuffers/main.walz) | Runnable demo and portable JS exports |
| [Example benchmark](../../examples/flatbuffers/codec_bench.walz) | In-Wasm encode/decode benchmark, including materialization |

The reusable source modules live in `walz/flatbuffers/`, separate from
`walz:core`. Import them with a relative path from your source file. For example,
the adapter under `examples/flatbuffers/` uses:

```text
(@use "../../walz/flatbuffers/builder.walz"
  (type $fb.builder) (func $fb.builder.new) (func $fb.builder.start)
  (func $fb.builder.bits-field) (func $fb.builder.end) (func $fb.builder.finish))
(@use "../../walz/flatbuffers/reader.walz"
  (type $fb.table) (tag $fb.invalid) (func $fb.root) (func $fb.bits))
```

The [Message example](../../examples/flatbuffers/README.md) keeps schema-specific
records, the runnable entry point, and the benchmark under `examples/flatbuffers/`.
`$message.encode` takes `(ref $Message)` and returns `$bytes`.
`$message.decode` takes `$bytes` and returns
`(@inst $result (ref $Message) i32)`. The Message record contains an i64 ID,
string name, i32 values, byte payload, bool enabled flag, f64 weight, and optional
Child record. ABI declarations map records to JavaScript objects and WIT records.
Absent strings/vectors become empty values; absent scalars use the schema's
defaults, including `enabled = true`. Child absence is preserved as Option.

Build the same adapter for JavaScript:

```sh
walz build examples/flatbuffers/main.walz --target core-js -o target/flatbuffers
```

```js
const { instantiate } = await import("./target/flatbuffers/index.js");
const { api } = await instantiate();
const value = {
  id: 42n, name: "日本語🦀",
  values: Int32Array.of(20, 22), payload: Uint8Array.of(0, 128, 255),
  enabled: true, weight: 1.25, child: { count: 42, label: "child" },
};
const bytes = api.encode(value); // Uint8Array, standard FlatBuffers wire format
const decoded = api.decode(bytes); // { tag: "ok", val: value } or { tag: "err", val: code }
```

Omit `child` or use `undefined` for no Child. Component output exposes the same
values using the repository's tagged Result binding policy. Encoding and decoding
are synchronous and need no event loop host.

## Binary contracts

The implementation follows the [official FlatBuffers format](https://flatbuffers.dev/internals/):
little-endian scalars, aligned fields, forward u32 object offsets, signed vtable
distances, and u16 vtable entries. Strings have UTF-8 bytes and a NUL terminator;
their length excludes the terminator. The decoder accepts shared vtables on
either side of a table. Tests compare values because valid encoders can choose
different layouts.

The generic reader accepts scalar widths 1, 2, 4, and 8. `$fb.bits` and
`$fb.vector.get` return bit patterns in i64; use explicit Wasm instructions for
sign extension and floating-point reinterpretation. `$fb.i32`, `$fb.i64`, and
`$fb.f64` provide convenient table getters. Missing fields use the supplied
default. `$fb.vector` verifies the entire vector range before iteration.
`$fb.vector.bytes` returns a bytes-view retaining the original immutable source
without copying payload bytes. `$fb.vector.sum-i32` sums a checked width-4 vector as signed i32 values into i64, without materializing a list; other widths raise error 2. `$fb.i32s` and `$fb.string` materialize values.
The Message adapter also copies the payload at its value/ABI boundary; it is
not a zero-copy end-to-end interface.

Untrusted offsets and lengths are widened to i64 before arithmetic. Bounds,
alignment, table/vtable sizes, known field widths, vector lengths, UTF-8, and
string terminators are checked before access or length-based allocation.
Reader errors throw the declared `$fb.invalid` tag, which the Message adapter
catches and converts to Result:

| Code | Meaning |
| --- | --- |
| 1 | Offset, length, or index outside the input |
| 2 | Invalid layout or alignment |
| 3 | Missing/wrong file identifier or invalid identifier argument |
| 4 | Invalid UTF-8 |

`$fb.root` checks the root and table header. Subsequent getters validate each
known field; it is not a schema-independent recursive verifier. The Message
adapter visits every field in its schema and its optional Child. Unknown fields
are ignored for forward compatibility. Limit incoming buffer sizes according
to the application's memory budget before decoding.

Builder objects and offsets are trusted application state. Construct strings,
vectors, and children before starting their parent table. `start` opens one
table; scalar/offset field calls record zero-based field slots; `end` closes it
and returns an offset counted from the end. Pass offsets from the same builder
to `offset-field` or `finish`. Misuse traps, including nested construction while
a table is open, duplicate written fields, invalid slots/widths, and use after
finish. `finish` accepts an empty or four-byte identifier and seals the builder.

## Current limits and tests

- Encoder: scalar bit patterns at widths 1/2/4/8, tables and nested tables,
  UTF-8 strings, byte/i32 vectors, default omission, and optional identifier.
- Decoder: the corresponding primitives, generic scalar vectors, and shared or
  negative-distance vtables. No vtable deduplication in the encoder yet.
- No `.fbs` parser/code generator, unions, inline structs, vectors of offsets,
  size-prefixed buffers, or 64-bit-offset format. Schema adapters are handwritten.
- Builder storage is capped at 64 MiB of wire bytes. Its temporary GC array uses
  one packed i8 per byte. Growth and byte/string writes use bulk copies;
  `finish` takes one immutable range snapshot without an intermediate i32 array.

Tests cover empty/default values, unknown fields, forced scalar defaults,
integer limits, IEEE infinities/NaN/signed zero, Unicode and embedded NUL,
capacity growth, nested/shared vtables, invalid offsets/lengths/alignment,
malformed UTF-8/terminators, every truncated prefix of a fixture, and deterministic
mutations/random inputs. Repeated successes and errors must leave no live ABI
buffers or host handles. These checks do not prove absence of all GC leaks or
malformed-input bugs.

## Query and batch APIs

The [pipeline example](../../examples/flatbuffers/pipeline.walz) exports `inspect`,
`inspect-batch`, `transform`, and `transform-batch`. Queries return a small Summary
record, validating all known Message/Child fields while retaining vector data in
Wasm. Transform performs decode → wrapping i32 addition → encode in one call.
Batch calls accept `list<bytes>` and return one Result per item, in input order;
an invalid item does not skip later items. Empty batches are supported. These
are schema-specific examples, not new language forms or a persistent handle pool.

```sh
walz build examples/flatbuffers/pipeline.walz --target core-js -o target/fb-pipeline
just bench-flatbuffers-pipeline target/flatbuffers-bench/capture target/fb-pipeline-run
```

Build a fresh capture first; older captures have no pipeline artifacts. The
`walz-flatbuffers-pipeline-v2` suite compares materialized, query/fused and
batch-of-eight paths using the same records, checking complete results and ABI
cleanup outside timing. Bytes still copy across the portable ABI. JS query uses
trusted-input ByteBuffer accessors with numeric array views. JS transformation
uses a manual loop and fresh/reused builders; Walz additionally checks layout and UTF-8.
Synchronous runtime calls retain cleanup state without allocating Task handles
or result/completion Promises; queued `start()` calls keep their existing protocol.
