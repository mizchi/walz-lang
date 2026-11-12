# FlatBuffers speed comparison

v24.22.2 / V8 13.6.233.17-node.63 / Apple M5 / darwin arm64. 9 samples; 30 ms calibration target.

Reference: official FlatBuffers JavaScript 25.9.23, with a handwritten adapter for message.fbs. Fresh builders start at 64 bytes on both implementations; jsReused clears and reuses its builder. Every encoder returns independently owned bytes. Decode inputs are the same saved official-JS byte buffers. Materialized vectors are copied on both implementations; this is not the official zero-copy accessor path.

Walz decode includes bounds, alignment, length, UTF-8 and terminator validation. The official JS adapter checks the identifier and otherwise trusts valid inputs. These rows compare the current APIs, not identical validation work. Walz resident keeps input values in Wasm GC and returns an opaque GC reference, with one JS/Wasm call per operation. Core/Component include full value ABI transfer; decode also constructs a tagged Result. Resident encode includes the library's finish copy.

Builds, instantiation, I/O, input construction, calibration, warmup and output equality/handle checks are outside sample clocks. Warmup runs at least 4096 calls and 250ms per variant. The event loop drains fulfilled Task promises between batches, outside the clock. All outputs are retained and checked against every source field. Each round rotates variant order; explicit GC occurs before each batch, and automatic GC within the batch counts. Raw samples, calibrated iteration counts, environment, source and artifact hashes are preserved. Each variant has its own calibrated iteration count; no samples are removed. Source capture may be dirty and its exact contents are saved.

## encode

Median µs/op. All variants produce owned output bytes.

| input | wire bytes | jsFresh | jsReused | walzResident | walzCore | walzComponent |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| small | 104 | 0.66 | 0.44 | 0.22 | 1.76 | 5.40 |
| unicode-4k | 4712 | 5.83 | 1.52 | 6.19 | 12.10 | 16.70 |
| bytes-64k | 65672 | 27.67 | 7.90 | 127.13 | 175.17 | 182.00 |
| values-16k | 65880 | 68.51 | 47.99 | 121.79 | 160.12 | 167.76 |

## decode

Median µs/op. All variants materialize every field and copy vectors.

| input | wire bytes | jsMaterialized | walzResident | walzCore | walzComponent |
| --- | ---: | ---: | ---: | ---: | ---: |
| small | 104 | 0.25 | 0.10 | 1.72 | 5.00 |
| unicode-4k | 4712 | 1.37 | 0.59 | 6.35 | 9.85 |
| bytes-64k | 65672 | 5.09 | 1.82 | 49.44 | 53.73 |
| values-16k | 65880 | 5.23 | 21.22 | 57.99 | 64.01 |

Output lengths may differ because table layout and vtable deduplication are not canonical. See report.json for lengths, throughput, paired sample ratios and raw samples. This is a local V8 measurement on four fixed inputs, not a native FlatBuffers or browser benchmark.
