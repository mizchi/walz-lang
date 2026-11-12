# FlatBuffers speed comparison

v24.22.2 / V8 13.6.233.17-node.63 / Apple M5 / darwin arm64. 9 samples; 30 ms calibration target.

Reference: official FlatBuffers JavaScript 25.9.23, with a handwritten adapter for message.fbs. Fresh builders start at 64 bytes on both implementations; jsReused clears and reuses its builder. Every encoder returns independently owned bytes. Decode inputs are the same saved official-JS byte buffers. Materialized vectors are copied on both implementations; this is not the official zero-copy accessor path.

Walz decode includes bounds, alignment, length, UTF-8 and terminator validation. The official JS adapter checks the identifier and otherwise trusts valid inputs. These rows compare the current APIs, not identical validation work. Walz resident keeps input values in Wasm GC and returns an opaque GC reference, with one JS/Wasm call per operation. Core/Component include full value ABI transfer; decode also constructs a tagged Result. Resident encode includes the library's finish copy.

Builds, instantiation, I/O, input construction, calibration, warmup and output equality/handle checks are outside sample clocks. All outputs are retained and checked against every source field. Each round rotates variant order; explicit GC occurs before each batch, and automatic GC within the batch counts. Raw samples, calibrated iteration counts, environment, source and artifact hashes are preserved. Each variant has its own calibrated iteration count; no samples are removed. Source capture may be dirty and its exact contents are saved.

## encode

Median µs/op. All variants produce owned output bytes.

| input | wire bytes | jsFresh | jsReused | walzResident | walzCore | walzComponent |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| small | 104 | 0.78 | 0.53 | 0.28 | 2.35 | 6.43 |
| unicode-4k | 4712 | 8.30 | 2.54 | 8.33 | 14.26 | 18.99 |
| bytes-64k | 65672 | 34.83 | 8.91 | 188.79 | 225.70 | 232.96 |
| values-16k | 65880 | 73.85 | 49.10 | 187.48 | 215.65 | 227.94 |

## decode

Median µs/op. All variants materialize every field and copy vectors.

| input | wire bytes | jsMaterialized | walzResident | walzCore | walzComponent |
| --- | ---: | ---: | ---: | ---: | ---: |
| small | 104 | 0.25 | 0.11 | 1.90 | 6.04 |
| unicode-4k | 4712 | 3.30 | 0.69 | 6.76 | 11.81 |
| bytes-64k | 65672 | 9.43 | 1.81 | 50.19 | 56.78 |
| values-16k | 65880 | 7.52 | 37.85 | 75.36 | 81.24 |

Output lengths may differ because table layout and vtable deduplication are not canonical. See report.json for lengths, throughput, paired sample ratios and raw samples. This is a local V8 measurement on four fixed inputs, not a native FlatBuffers or browser benchmark.
