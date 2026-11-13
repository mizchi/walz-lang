# Browser WAT reference fixture

This standalone, unpublished benchmark crate exports `compile_source` through
wasm-bindgen. It runs the pinned WAT parser and Core Wasm validator, matching the
narrower native reference in the selfhost benchmark. It provides no Walz
extensions, library catalogue, analyses or playground UI protocol.

The crate is outside the production workspace and has its own lockfile. Release
wasm-pack builds use `--target web` with wasm-opt disabled, matching the full
playground build policy. The browser-size harness uses it only as a reference;
it does not replace either compiler.

See [browser deployment measurements](../../docs/playground-size.md). Run native
fixture checks and asset-accounting tests with `just test-playground-size`.
