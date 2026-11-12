# FlatBuffers speed comparison

v24.22.2 / V8 13.6.233.17-node.63 / Apple M5 / darwin arm64. 9 samples; 30 ms calibration target.

Reference: official FlatBuffers JavaScript 25.9.23, with a handwritten adapter for message.fbs. Fresh builders start at 64 bytes on both implementations; jsReused clears and reuses its builder. Every encoder returns independently owned bytes. Decode inputs are the same saved official-JS byte buffers. Materialized vectors are copied on both implementations; this is not the official zero-copy accessor path.

Walz decode includes bounds, alignment, length, UTF-8 and terminator validation. The official JS adapter checks the identifier and otherwise trusts valid inputs. These rows compare the current APIs, not identical validation work. Walz resident keeps input values in Wasm GC and returns an opaque GC reference, with one JS/Wasm call per operation. Core/Component include full value ABI transfer; decode also constructs a tagged Result. Resident encode includes the library's finish copy.

Builds, instantiation, I/O, input construction, calibration, warmup and output equality/handle checks are outside sample clocks. Warmup runs at least 4096 calls and 250ms per variant. The event loop drains fulfilled Task promises between batches, outside the clock. All outputs are retained and checked against every source field. Each round rotates variant order; explicit GC occurs before each batch, and automatic GC within the batch counts. Raw samples, calibrated iteration counts, environment, source and artifact hashes are preserved. Each variant has its own calibrated iteration count; no samples are removed. Source capture may be dirty and its exact contents are saved.

## encode

Median µs/op. All variants produce owned output bytes.

| input | wire bytes | jsFresh | jsReused | walzResident | walzCore | walzComponent |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| small | 104 | 0.73 | 0.51 | 0.13 | 1.22 | 4.29 |
| unicode-4k | 4712 | 6.04 | 1.92 | 0.67 | 5.84 | 9.79 |
| bytes-64k | 65672 | 30.27 | 8.84 | 19.52 | 68.57 | 75.61 |
| values-16k | 65880 | 70.42 | 49.56 | 33.31 | 68.92 | 76.00 |

## decode

Median µs/op. All variants materialize every field and copy vectors.

| input | wire bytes | jsMaterialized | walzResident | walzCore | walzComponent |
| --- | ---: | ---: | ---: | ---: | ---: |
| small | 104 | 0.25 | 0.11 | 1.14 | 3.91 |
| unicode-4k | 4712 | 1.46 | 0.61 | 5.72 | 9.34 |
| bytes-64k | 65672 | 5.43 | 1.94 | 49.86 | 54.36 |
| values-16k | 65880 | 5.83 | 22.22 | 58.84 | 66.49 |

Output lengths may differ because table layout and vtable deduplication are not canonical. See report.json for lengths, throughput, paired sample ratios and raw samples. This is a local V8 measurement on four fixed inputs, not a native FlatBuffers or browser benchmark.
