<a id="wasi-03-のファイル値-adapter"></a>

# WASI 0.3 file value adapters

The referenced specification is pinned to [WASI v0.3.0](https://github.com/WebAssembly/WASI/releases/tag/v0.3.0), commit `3ee2a590c766594ae44a54730fc74fc27da5c609`. The host calls `descriptor.open-at` / `read-via-stream` / `write-via-stream` from `proposals/filesystem/wit/types.wit`.

The API passed to Walz is `walz:filesystem/files@0.3.0`. This is a Walz value adapter and does not claim to implement the `wasi:filesystem` resource ABI. The host owns descriptors; guests receive only selected names, byte streams, completion futures, and result errors.

```sh
walz bind examples/wasi-files/filesystem.wit examples/wasi-files/policy.json examples/wasi-files/generated
walz run examples/wasi-files --target core-js \
  --bindings examples/wasi-files/generated/bindings.json --read-file input=./input.bin
walz run examples/wasi-files --target core-js --invoke write \
  --bindings examples/wasi-files/generated/bindings.json --write-file output=./output.bin
```

The example's `main` sums all bytes; `write` outputs `AB`. bind refuses to overwrite existing destinations.

`read-file(name)` returns `tuple<stream<u8>, future<result<_, error-code>>>`. Read the Stream to completion before awaiting the completion Future. `write-file(name, stream<u8>)` returns a completion Future. Reads do not prefetch; writes follow backpressure. Early return/cancellation closes readers and descriptors; completion and cancellation notifications wait for cleanup.

`createWasiFileSystem` in `runtime/wasi-filesystem.mjs` connects to a host-supplied map of `{directory: descriptor, path: selected relative path}`. `runtime/node-filesystem.mjs` provides the same protocol using Node FileHandle. The Node version handles only explicitly specified individual files and does not search arbitrary directories using guest paths.

Returned error-code values are limited to bad-descriptor / interrupted / invalid / io / no-entry / not-permitted / read-only. Unknown host errors become io. Unauthorized names return not-permitted without opening files.

This WASI file value adapter does not expose native descriptor resources to guests. Full error-code coverage, guest exposure of preopens, networking, and native connections for original worlds across the WASI suite are unsupported. For generic adapters handling typed own/borrow of imported resources, see [native worlds](native-worlds.md). Unknown host Promises cannot be forcibly stopped; cancellation also assumes providers finish cleanup.

Readers also support `readMany(max)`; the Node version copies byte chunks up to the limit from a 16 KiB internal buffer.
Writes likewise receive batches from supporting readers and complete the remainder even after short writes.
sum-batch and io_bench in `examples/wasi-files` use [explicit batch transfers](stream-batches.md).
