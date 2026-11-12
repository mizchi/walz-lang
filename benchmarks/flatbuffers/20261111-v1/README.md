# FlatBuffers baseline, suite v1

Later profiling note (2026-11-11): this v1 timer did not drain fulfilled Task
Promise jobs between batches. The [tuning investigation](../20261111-tuning-v1/README.md#measurement-correction)
records that limitation and a separately versioned measurement method. This
capture and all raw v1 results remain unchanged; use the paired-v3 comparisons
for the retained optimization's before/after effect.

Measured on Apple M5 / macOS arm64 / Node v24.22.2 / V8 13.6.233.17-node.63.
The reference is official FlatBuffers JavaScript 25.9.23 with a handwritten
adapter for the same `message.fbs` schema. This is a V8 comparison; native C++/Rust
and browsers were not measured.

- [Full tables and method](run-1/report.md)
- [All raw samples, calibrated iterations, ratios, and environment](run-1/report.json)
- [Frozen input/source/artifact hashes and compiler provenance](capture/capture.json)
- [Frozen workload values](capture/corpus.json)
- [Frozen measurement code](capture/runner.mjs) and [reference adapter](capture/reference.mjs)

Each variant has nine measured batches after calibration and warmup. Order
rotates per round. Compilation, instantiation, I/O, input setup, equality checks,
and explicit pre-sample GC are excluded; automatic GC during a batch is included.
Every result is retained, all source fields are checked, and Core/Component must
leave no live ABI buffers or host handles. No outliers were removed. The capture
was taken from a dirty working tree; its exact library/compiler sources and
generated artifacts are included rather than inferred from the HEAD commit.

## Results

Median µs/operation. Core includes complete JS value ABI input/output transfer.
Resident keeps both input and output as Wasm GC values, with one JS/Wasm call per
operation. See the full report for Component and reused-builder results.

| Input | Wire bytes | JS encode, fresh | Walz encode, resident | Walz encode, Core | JS decode | Walz decode, resident | Walz decode, Core |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| small | 104 | 0.78 | 0.28 | 2.35 | 0.25 | 0.11 | 1.90 |
| Unicode + 4 KiB payload | 4,712 | 8.30 | 8.33 | 14.26 | 3.30 | 0.69 | 6.76 |
| 64 KiB payload | 65,672 | 34.83 | 188.79 | 225.70 | 9.43 | 1.81 | 50.19 |
| 16,384 i32 values | 65,880 | 73.85 | 187.48 | 215.65 | 7.52 | 37.85 | 75.36 |

Both encoders return owned bytes. Both decoders read every field, produce the
same record values, and copy vector data; the JS baseline uses typed-array views
followed by an owning copy, without temporary JS number arrays. JS fresh and Walz
start builders at 64 bytes. A separate JS reused-builder baseline retains its
capacity and still copies each output.

Walz validates bounds, alignment, lengths, UTF-8, and terminators during decode.
The official JS adapter checks the identifier and trusts the valid fields. This
compares the current APIs, with their different validation work, and does not
compare zero-copy accessor-only reads. All four output lengths happened to match;
equality of byte layout is not required or used as the correctness criterion.

## Interpretation

Resident Walz wins on the small input (about 2.8× encode and 2.3× decode compared
with fresh JS). It also decodes the 64 KiB byte payload about 5.2× faster while
retaining the result inside Wasm GC. These advantages disappear when the current
JS value ABI is included: Core and Component are slower than JS on every input.

Large encoding and numeric-vector decoding need work. Resident 64 KiB encoding
takes 188.79 µs versus 34.83 µs for fresh JS and 8.91 µs for reused JS. Numeric-vector
decode takes 37.85 µs versus 7.52 µs for JS. The current implementation therefore
does not establish a general JS FlatBuffers replacement.

Implementation inspection suggests these next experiments; the baseline alone
does not attribute time to individual functions:

1. Builder reuse and bulk copying. The Walz builder grows an i32-per-byte work
   array, copies live bytes when growing, writes individual bytes, then copies
   through another i32 array into packed immutable bytes at finish.
2. Specialize i32 vector reads. The generic element path repeats range/alignment
   checks and byte-by-byte integer assembly after validating the whole vector.
3. Keep processing inside Wasm and batch ABI operations. Resident decoding is
   useful only if the caller can avoid transferring the full result after every
   operation; compare that application-level pipeline separately.

## Reproduce

Run the captured binaries, JS package, and measurement code without rebuilding:

```sh
just bench-flatbuffers benchmarks/flatbuffers/20261111-v1/capture target/flatbuffers-repeat-1 9
```

The output directory must not exist. The command verifies every captured hash.
It requires Node.js 24+ with Wasm GC and JSPI; explicit GC is enabled by the task.
The rerun records its own environment and measurements.

To measure changed implementation code, create a new capture and result directory:

```sh
just test-flatbuffers
just flatbuffers-capture target/flatbuffers-new/capture
just bench-flatbuffers target/flatbuffers-new/capture target/flatbuffers-new/run-1 9
```

Preserve this v1 capture and its raw results. Change the suite version when inputs,
the schema, reference behavior, or measurement semantics change.
