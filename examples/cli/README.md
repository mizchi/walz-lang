# CLI example

Install from the repository root with `cargo install --path . --locked --bin wlt`.
Then run in this directory:

```sh
wlt build                  # target/main.wasm
wlt test                   # 1 passed
wlt bench                  # measures the answer function
```

Tests need Wasmtime with Wasm GC support; benchmarks need Node.js 24+.
This example shares a static file module and imports `option` from `wlt:core`.
The benchmark uses a constant input to exercise the CLI; its timing is not a useful performance comparison.
