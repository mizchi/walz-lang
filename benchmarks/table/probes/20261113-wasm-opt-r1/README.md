# Failed pre-timing probe

No timing samples were collected. The first, original-Wasm variant failed its
test oracle at 129 rows: `y = i % 127` wraps, but the expected sort/slice result
used unsorted input. The corrected oracle sorts the independent JS rows before
slicing. This failure is not evidence of an optimizer defect.

`probe-failed.mjs`, `failure.log` and the generated optimized Wasm are retained.
The completed experiment is [the second run](../20261113-wasm-opt-r2/probe.json).
