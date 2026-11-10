# CLI example

English | [日本語](README.ja.md)

Install from the repository root with `cargo install --path . --locked --bin walz`.
Then run in this directory:

```sh
walz build                  # target/main.wasm
walz test                   # 1 passed
walz bench                  # measures the answer function
```

Tests need Wasmtime with Wasm GC support; benchmarks need Node.js 24+.
This example shares a static file module and imports `option` from `walz:core`.
The benchmark uses a constant input to exercise the CLI; its timing is not a useful performance comparison.
