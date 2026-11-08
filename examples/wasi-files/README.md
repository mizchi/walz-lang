# WASI 0.3 file value adapter

See [the adapter guide](../../docs/wasi-files.md) for generation, explicit file grants, ownership and limits. The guest reads bytes as `stream<u8>` with a completion Future; descriptors remain in the host.

Pinned upstream: WebAssembly/WASI `v0.3.0`, commit `3ee2a590c766594ae44a54730fc74fc27da5c609`. The local `filesystem.wit` describes the smaller WLT value interface, not the upstream resource world.

`sum-batch(size)` reads up to `size` bytes into a GC list per suspension and uses
`list.fold` to sum them. `main` retains the one-byte stream example.

After generating `generated/`, compare the two transfer sizes through the CLI:

```sh
WLT_BENCH_INPUT=/absolute/path/input.bin wlt bench examples/wasi-files/io_bench.wlt \
  --target core-js --bindings examples/wasi-files/generated/bindings.json \
  --host examples/wasi-files/bench-host.mjs --samples 5 --iterations 1 --warmup-ms 0
```

Each iteration opens and closes the explicitly granted file. These end-to-end
measurements include file I/O, conversion, JSPI, and guest work.
